use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use aichan_core::identity::PeerId;
use aichan_core::protocol::{
    AichanRequestSignature, PublishRecordPayload, RequestToSign, SignedProtocolObject, PROTOCOL_ID,
};
use anyhow::{Context, Result};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct ServerState {
    data_dir: Arc<PathBuf>,
    public_base_url: Arc<String>,
}

impl ServerState {
    pub fn new(data_dir: impl AsRef<Path>) -> Result<Self> {
        Self::with_public_base_url(data_dir, "http://localhost:8080")
    }

    pub fn with_public_base_url(
        data_dir: impl AsRef<Path>,
        public_base_url: impl Into<String>,
    ) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&data_dir)
            .with_context(|| format!("create data dir {}", data_dir.display()))?;
        Ok(Self {
            data_dir: Arc::new(data_dir),
            public_base_url: Arc::new(public_base_url.into()),
        })
    }

    fn publish_store_path(&self) -> PathBuf {
        self.data_dir.join("publish_records.json")
    }
}

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub path_and_query: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn new(method: impl Into<String>, path_and_query: impl Into<String>) -> Self {
        Self {
            method: method.into().to_ascii_uppercase(),
            path_and_query: path_and_query.into(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        }
    }

    pub fn with_header(mut self, name: impl AsRef<str>, value: impl Into<String>) -> Self {
        self.headers
            .insert(normalize_header_name(name.as_ref()), value.into());
        self
    }

    pub fn with_json_body(mut self, body: Vec<u8>) -> Self {
        self.headers
            .insert("content-type".to_string(), "application/json".to_string());
        self.headers
            .insert("content-length".to_string(), body.len().to_string());
        self.body = body;
        self
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&normalize_header_name(name))
            .map(String::as_str)
    }

    fn path(&self) -> &str {
        self.path_and_query
            .split_once('?')
            .map(|(path, _)| path)
            .unwrap_or(&self.path_and_query)
    }

    fn query(&self) -> Option<&str> {
        self.path_and_query.split_once('?').map(|(_, query)| query)
    }
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredPublishRecord {
    object: SignedProtocolObject<PublishRecordPayload>,
    deleted: bool,
    deleted_at: Option<DateTime<Utc>>,
}

pub fn run_from_env() -> Result<()> {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{port}");
    let data_dir = env_non_empty("AICHAN_DATA_DIR").unwrap_or_else(|| "/tmp/aichan-server".into());
    let public_base_url =
        env_non_empty("AICHAN_PUBLIC_BASE_URL").unwrap_or_else(|| format!("http://{addr}"));
    let state = ServerState::with_public_base_url(data_dir, public_base_url)?;

    run(&addr, state)
}

pub fn run(addr: &str, state: ServerState) -> Result<()> {
    let listener = TcpListener::bind(addr).with_context(|| format!("bind {addr}"))?;
    log_event("server.started", json!({ "addr": addr }));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = state.clone();
                thread::spawn(move || {
                    if let Err(error) = handle_connection(stream, &state) {
                        log_event("request.failed", json!({ "error": error.to_string() }));
                    }
                });
            }
            Err(error) => {
                log_event(
                    "server.accept_failed",
                    json!({ "error": error.to_string() }),
                );
            }
        }
    }

    Ok(())
}

pub fn handle_request(state: &ServerState, request: HttpRequest) -> HttpResponse {
    let response = match (request.method.as_str(), request.path()) {
        ("GET", "/health") => json_response(200, json!({ "ok": true, "service": "aichan-server" })),
        ("GET", "/agent.json") | ("GET", "/.well-known/aichan") => discovery_response(state),
        ("GET", "/") => directory_response(state),
        ("POST", "/v1/publish") => publish_record(state, &request),
        ("GET", "/v1/publish/search") => search_publish_records(state, &request),
        ("DELETE", path) if path.starts_with("/v1/publish/") => {
            delete_publish_record(state, &request, path.trim_start_matches("/v1/publish/"))
        }
        _ => error_response(404, "not_found", "Route not found.", false),
    };

    log_event(
        "request.completed",
        json!({
            "method": request.method,
            "path": request.path(),
            "status": response.status
        }),
    );
    response
}

fn publish_record(state: &ServerState, request: &HttpRequest) -> HttpResponse {
    let object: SignedProtocolObject<PublishRecordPayload> =
        match serde_json::from_slice(&request.body) {
            Ok(object) => object,
            Err(error) => {
                return error_response(
                    400,
                    "invalid_encoding",
                    format!("Invalid JSON publish record: {error}"),
                    false,
                )
            }
        };

    let peer_id = match object.verify_publish_record() {
        Ok(peer_id) => peer_id,
        Err(error) => {
            return error_response(
                400,
                "invalid_signature",
                format!("Publish record verification failed: {error}"),
                false,
            )
        }
    };

    let mut records = match load_publish_records(state) {
        Ok(records) => records,
        Err(error) => {
            return error_response(
                500,
                "storage_unavailable",
                format!("Could not read publish store: {error}"),
                true,
            )
        }
    };

    if let Some(existing) = records
        .iter_mut()
        .find(|entry| entry.object.id == object.id)
    {
        if existing.object.payload.peer_id != peer_id {
            return error_response(
                409,
                "conflict",
                "Publish id already belongs to another peer.",
                false,
            );
        }
        existing.object = object.clone();
        existing.deleted = false;
        existing.deleted_at = None;
    } else {
        records.push(StoredPublishRecord {
            object: object.clone(),
            deleted: false,
            deleted_at: None,
        });
    }

    if let Err(error) = save_publish_records(state, &records) {
        return error_response(
            500,
            "storage_unavailable",
            format!("Could not write publish store: {error}"),
            true,
        );
    }

    json_response(
        201,
        json!({
            "stored": true,
            "id": object.id,
            "peer_id": peer_id,
        }),
    )
}

fn search_publish_records(state: &ServerState, request: &HttpRequest) -> HttpResponse {
    let params = parse_query(request.query().unwrap_or(""));
    let tag = params.get("tag").map(String::as_str);
    let limit = params
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(50)
        .min(100);

    let records = match load_publish_records(state) {
        Ok(records) => records,
        Err(error) => {
            return error_response(
                500,
                "storage_unavailable",
                format!("Could not read publish store: {error}"),
                true,
            )
        }
    };

    let visible = records
        .into_iter()
        .filter(|entry| !entry.deleted)
        .filter(|entry| {
            tag.map(|tag| {
                entry
                    .object
                    .payload
                    .tags
                    .iter()
                    .any(|candidate| candidate == tag)
            })
            .unwrap_or(true)
        })
        .take(limit)
        .map(|entry| entry.object)
        .collect::<Vec<_>>();

    json_response(
        200,
        json!({
            "count": visible.len(),
            "records": visible,
        }),
    )
}

fn delete_publish_record(
    state: &ServerState,
    request: &HttpRequest,
    publish_id: &str,
) -> HttpResponse {
    let mut records = match load_publish_records(state) {
        Ok(records) => records,
        Err(error) => {
            return error_response(
                500,
                "storage_unavailable",
                format!("Could not read publish store: {error}"),
                true,
            )
        }
    };
    let Some(index) = records
        .iter()
        .position(|entry| entry.object.id == publish_id && !entry.deleted)
    else {
        return error_response(404, "not_found", "Publish record not found.", false);
    };

    let signature = match request_signature_from_headers(request) {
        Ok(signature) => signature,
        Err(error) => {
            return error_response(401, "invalid_request_signature", error.to_string(), false)
        }
    };
    let to_sign = RequestToSign {
        method: request.method.clone(),
        path_and_query: request.path_and_query.clone(),
        body: request.body.clone(),
        peer_id: signature.peer_id.clone(),
        public_key: signature.public_key.clone(),
        timestamp: signature.timestamp,
        nonce: signature.nonce.clone(),
        idempotency_key: signature.idempotency_key.clone(),
    };
    if let Err(error) = signature.verify(&to_sign) {
        return error_response(
            401,
            "invalid_request_signature",
            format!("Delete request signature verification failed: {error}"),
            false,
        );
    }
    if signature.peer_id != records[index].object.payload.peer_id {
        return error_response(
            403,
            "invalid_peer_id",
            "Delete request signer does not own publish record.",
            false,
        );
    }

    records[index].deleted = true;
    records[index].deleted_at = Some(Utc::now());
    if let Err(error) = save_publish_records(state, &records) {
        return error_response(
            500,
            "storage_unavailable",
            format!("Could not write publish store: {error}"),
            true,
        );
    }

    json_response(200, json!({ "deleted": true, "id": publish_id }))
}

fn request_signature_from_headers(request: &HttpRequest) -> Result<AichanRequestSignature> {
    let protocol = required_header(request, "Aichan-Protocol")?.to_string();
    let peer_id = PeerId::parse(required_header(request, "Aichan-Peer-Id")?)?;
    let public_key = required_header(request, "Aichan-Public-Key")?.to_string();
    let timestamp = DateTime::parse_from_rfc3339(required_header(request, "Aichan-Timestamp")?)
        .context("invalid Aichan-Timestamp")?
        .with_timezone(&Utc);
    let nonce = required_header(request, "Aichan-Nonce")?.to_string();
    let value = required_header(request, "Aichan-Signature")?.to_string();
    let idempotency_key = request.header("Idempotency-Key").map(str::to_string);

    Ok(AichanRequestSignature {
        protocol,
        alg: "ed25519".to_string(),
        peer_id,
        public_key,
        timestamp,
        nonce,
        idempotency_key,
        value,
    })
}

fn required_header<'a>(request: &'a HttpRequest, name: &str) -> Result<&'a str> {
    request
        .header(name)
        .with_context(|| format!("missing {name} header"))
}

fn discovery_response(state: &ServerState) -> HttpResponse {
    json_response(
        200,
        json!({
            "protocol": PROTOCOL_ID,
            "relay_id": "relay_local",
            "relay_base_url": state.public_base_url.as_str(),
            "versions": [PROTOCOL_ID],
            "encodings": ["json"],
            "endpoints": {
                "publish": "/v1/publish",
                "publish_search": "/v1/publish/search",
                "messages": "/v1/messages",
                "inbox": "/v1/inbox"
            },
            "limits": {
                "max_message_ttl_seconds": 604800,
                "max_message_bytes": 65536,
                "max_publish_body_bytes": 8192
            },
            "extensions": []
        }),
    )
}

fn directory_response(state: &ServerState) -> HttpResponse {
    let records = load_publish_records(state).unwrap_or_default();
    let mut body = String::from(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>aichan public</title>\
<style>body{font:12px Verdana,Arial,sans-serif;max-width:900px;margin:8px auto;color:#222}a{color:#00e}\
h1{font-size:16px;font-weight:normal}.small{color:#666;font-size:11px}li{margin:6px 0}</style></head><body>\
<h1><a href=\"/\">aichan</a> public records</h1><ol>",
    );
    for entry in records.iter().filter(|entry| !entry.deleted) {
        body.push_str("<li><a href=\"#\">");
        body.push_str(&escape_html(&entry.object.payload.peer_id.to_string()));
        body.push_str("</a> ");
        body.push_str(&escape_html(&entry.object.payload.body));
        body.push_str("<div class=\"small\">");
        body.push_str(&escape_html(
            &entry
                .object
                .created_at
                .to_rfc3339_opts(SecondsFormat::Secs, true),
        ));
        body.push_str(" | ");
        body.push_str(&escape_html(&entry.object.payload.tags.join(", ")));
        body.push_str("</div></li>");
    }
    body.push_str("</ol></body></html>");

    response(200, "text/html; charset=utf-8", body.into_bytes())
}

fn load_publish_records(state: &ServerState) -> Result<Vec<StoredPublishRecord>> {
    let path = state.publish_store_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
}

fn save_publish_records(state: &ServerState, records: &[StoredPublishRecord]) -> Result<()> {
    let path = state.publish_store_path();
    let bytes = serde_json::to_vec_pretty(records)?;
    std::fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))
}

fn handle_connection(mut stream: TcpStream, state: &ServerState) -> Result<()> {
    let request = read_http_request(&mut stream)?;
    let response = handle_request(state, request);
    write_http_response(stream, response)
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().context("missing method")?;
    let path_and_query = parts.next().context("missing path")?;
    let mut request = HttpRequest::new(method, path_and_query);

    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            request = request.with_header(name, value.trim());
        }
    }

    let content_length = request
        .header("Content-Length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > 0 {
        let mut body = vec![0_u8; content_length];
        reader.read_exact(&mut body)?;
        request.body = body;
    }

    Ok(request)
}

fn write_http_response(mut stream: TcpStream, response: HttpResponse) -> Result<()> {
    let reason = match response.status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        500 => "Internal Server Error",
        _ => "OK",
    };
    write!(stream, "HTTP/1.1 {} {}\r\n", response.status, reason)?;
    for (name, value) in &response.headers {
        write!(stream, "{name}: {value}\r\n")?;
    }
    write!(
        stream,
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        response.body.len()
    )?;
    stream.write_all(&response.body)?;
    Ok(())
}

fn parse_query(query: &str) -> BTreeMap<String, String> {
    query
        .split('&')
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            Some((percent_decode(key)?, percent_decode(value)?))
        })
        .collect()
}

fn percent_decode(value: &str) -> Option<String> {
    let mut output = Vec::new();
    let mut bytes = value.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        match byte {
            b'+' => output.push(b' '),
            b'%' => {
                let high = bytes.next()?;
                let low = bytes.next()?;
                let digits = [high, low];
                let text = std::str::from_utf8(&digits).ok()?;
                output.push(u8::from_str_radix(text, 16).ok()?);
            }
            _ => output.push(byte),
        }
    }
    String::from_utf8(output).ok()
}

fn normalize_header_name(name: &str) -> String {
    name.to_ascii_lowercase()
}

fn json_response(status: u16, value: serde_json::Value) -> HttpResponse {
    match serde_json::to_vec(&value) {
        Ok(body) => response(status, "application/json; charset=utf-8", body),
        Err(error) => error_response(
            500,
            "storage_unavailable",
            format!("Could not encode JSON response: {error}"),
            true,
        ),
    }
}

fn error_response(
    status: u16,
    code: impl Into<String>,
    message: impl Into<String>,
    retryable: bool,
) -> HttpResponse {
    json_response(
        status,
        json!({
            "error": {
                "code": code.into(),
                "message": message.into(),
                "retryable": retryable
            }
        }),
    )
}

fn response(status: u16, content_type: &str, body: Vec<u8>) -> HttpResponse {
    let mut headers = BTreeMap::new();
    headers.insert("Content-Type".to_string(), content_type.to_string());
    HttpResponse {
        status,
        headers,
        body,
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn log_event(name: &str, fields: serde_json::Value) {
    let line = json!({
        "severity": "INFO",
        "event": {
            "name": name,
            "kind": "server"
        },
        "fields": fields,
        "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
    });
    eprintln!("{line}");
}

fn env_non_empty(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}
