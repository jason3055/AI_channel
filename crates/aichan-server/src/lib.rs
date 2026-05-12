use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use aichan_core::identity::PeerId;
use aichan_core::protocol::{
    AichanRequestSignature, PublishRecordPayload, RequestToSign, SignedProtocolObject, PROTOCOL_ID,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct ServerState {
    data_dir: Arc<PathBuf>,
    public_base_url: Arc<String>,
    rate_limiter: Arc<RateLimiter>,
    connection_limiter: Arc<ConnectionLimiter>,
    publish_store_lock: Arc<Mutex<()>>,
    request_auth: Arc<RequestAuthTracker>,
}

impl ServerState {
    pub fn new(data_dir: impl AsRef<Path>) -> Result<Self> {
        Self::with_public_base_url(data_dir, "http://localhost:8080")
    }

    pub fn with_public_base_url(
        data_dir: impl AsRef<Path>,
        public_base_url: impl Into<String>,
    ) -> Result<Self> {
        Self::with_public_base_url_and_rate_limits(
            data_dir,
            public_base_url,
            RateLimitConfig::default(),
        )
    }

    pub fn with_rate_limits(
        data_dir: impl AsRef<Path>,
        rate_limits: RateLimitConfig,
    ) -> Result<Self> {
        Self::with_public_base_url_and_rate_limits(data_dir, "http://localhost:8080", rate_limits)
    }

    pub fn with_public_base_url_and_rate_limits(
        data_dir: impl AsRef<Path>,
        public_base_url: impl Into<String>,
        rate_limits: RateLimitConfig,
    ) -> Result<Self> {
        Self::with_public_base_url_rate_limits_and_max_connections(
            data_dir,
            public_base_url,
            rate_limits,
            DEFAULT_MAX_CONNECTIONS,
        )
    }

    pub fn with_public_base_url_rate_limits_and_max_connections(
        data_dir: impl AsRef<Path>,
        public_base_url: impl Into<String>,
        rate_limits: RateLimitConfig,
        max_connections: usize,
    ) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&data_dir)
            .with_context(|| format!("create data dir {}", data_dir.display()))?;
        Ok(Self {
            data_dir: Arc::new(data_dir),
            public_base_url: Arc::new(public_base_url.into()),
            rate_limiter: Arc::new(RateLimiter::new(rate_limits)),
            connection_limiter: Arc::new(ConnectionLimiter::new(max_connections.max(1))),
            publish_store_lock: Arc::new(Mutex::new(())),
            request_auth: Arc::new(RequestAuthTracker::default()),
        })
    }

    fn publish_store_path(&self) -> PathBuf {
        self.data_dir.join("publish_records.json")
    }

    fn rate_limits(&self) -> RateLimitConfig {
        self.rate_limiter.config
    }

    fn check_rate_limit(&self, request: &HttpRequest) -> Option<RateLimitExceeded> {
        let class = RateLimitClass::for_request(request)?;
        self.rate_limiter.check(
            RateLimitKey {
                client: request.client_key(),
                class,
            },
            class.limit(self.rate_limiter.config),
        )
    }
}

#[derive(Debug)]
struct ConnectionLimiter {
    max_connections: usize,
    active_connections: Arc<Mutex<usize>>,
}

impl ConnectionLimiter {
    fn new(max_connections: usize) -> Self {
        Self {
            max_connections: max_connections.max(1),
            active_connections: Arc::new(Mutex::new(0)),
        }
    }

    fn try_acquire(&self) -> Option<ConnectionGuard> {
        let mut active = self
            .active_connections
            .lock()
            .expect("connection limiter mutex poisoned");
        if *active >= self.max_connections {
            return None;
        }
        *active += 1;
        Some(ConnectionGuard {
            active_connections: Arc::clone(&self.active_connections),
        })
    }
}

#[derive(Debug)]
struct ConnectionGuard {
    active_connections: Arc<Mutex<usize>>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        let mut active = self
            .active_connections
            .lock()
            .expect("connection limiter mutex poisoned");
        *active = active.saturating_sub(1);
    }
}

#[derive(Debug, Default)]
struct RequestAuthTracker {
    seen_nonces: Mutex<BTreeMap<RequestNonceKey, DateTime<Utc>>>,
}

impl RequestAuthTracker {
    fn mark_nonce_once(&self, peer_id: &PeerId, nonce: &str, now: DateTime<Utc>) -> bool {
        let mut seen = self
            .seen_nonces
            .lock()
            .expect("request auth nonce mutex poisoned");
        let oldest_allowed = now - ChronoDuration::seconds(REQUEST_SIGNATURE_MAX_SKEW_SECONDS);
        seen.retain(|_, seen_at| *seen_at >= oldest_allowed);

        let key = RequestNonceKey {
            peer_id: peer_id.to_string(),
            nonce: nonce.to_string(),
        };
        if seen.contains_key(&key) {
            return false;
        }
        seen.insert(key, now);
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RequestNonceKey {
    peer_id: String,
    nonce: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitConfig {
    pub read_per_minute: u32,
    pub write_per_minute: u32,
    pub max_body_bytes: usize,
}

impl RateLimitConfig {
    fn from_env() -> Self {
        Self {
            read_per_minute: env_u32("AICHAN_READ_RATE_PER_MINUTE")
                .unwrap_or_else(|| Self::default().read_per_minute)
                .max(1),
            write_per_minute: env_u32("AICHAN_WRITE_RATE_PER_MINUTE")
                .unwrap_or_else(|| Self::default().write_per_minute)
                .max(1),
            max_body_bytes: env_usize("AICHAN_MAX_BODY_BYTES")
                .unwrap_or_else(|| Self::default().max_body_bytes)
                .max(1024),
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            read_per_minute: 120,
            write_per_minute: 20,
            max_body_bytes: 65_536,
        }
    }
}

#[derive(Debug)]
struct RateLimiter {
    config: RateLimitConfig,
    buckets: Mutex<BTreeMap<RateLimitKey, RateLimitBucket>>,
}

impl RateLimiter {
    fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: Mutex::new(BTreeMap::new()),
        }
    }

    fn check(&self, key: RateLimitKey, limit: u32) -> Option<RateLimitExceeded> {
        let now = Instant::now();
        let mut buckets = self.buckets.lock().expect("rate limiter mutex poisoned");
        let bucket = buckets.entry(key.clone()).or_insert(RateLimitBucket {
            window_start: now,
            count: 0,
        });

        if now.duration_since(bucket.window_start) >= RATE_LIMIT_WINDOW {
            bucket.window_start = now;
            bucket.count = 0;
        }

        if bucket.count >= limit {
            let retry_after = RATE_LIMIT_WINDOW
                .saturating_sub(now.duration_since(bucket.window_start))
                .as_secs()
                .max(1);
            return Some(RateLimitExceeded {
                key,
                retry_after_seconds: retry_after,
            });
        }

        bucket.count += 1;
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RateLimitKey {
    client: String,
    class: RateLimitClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RateLimitClass {
    Read,
    Write,
}

impl RateLimitClass {
    fn for_request(request: &HttpRequest) -> Option<Self> {
        if request.path() == "/health" {
            return None;
        }
        match request.method.as_str() {
            "GET" => Some(Self::Read),
            "POST" | "PUT" | "PATCH" | "DELETE" => Some(Self::Write),
            _ => Some(Self::Read),
        }
    }

    fn limit(self, config: RateLimitConfig) -> u32 {
        match self {
            Self::Read => config.read_per_minute,
            Self::Write => config.write_per_minute,
        }
    }
}

#[derive(Debug, Clone)]
struct RateLimitBucket {
    window_start: Instant,
    count: u32,
}

#[derive(Debug, Clone)]
struct RateLimitExceeded {
    key: RateLimitKey,
    retry_after_seconds: u64,
}

const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_SIGNATURE_MAX_SKEW_SECONDS: i64 = 300;
const DEFAULT_MAX_CONNECTIONS: usize = 64;

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

    fn client_key(&self) -> String {
        self.header("X-Forwarded-For")
            .and_then(|value| value.split(',').next())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| self.header("X-Real-IP"))
            .unwrap_or("unknown")
            .to_string()
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
    let state = ServerState::with_public_base_url_rate_limits_and_max_connections(
        data_dir,
        public_base_url,
        RateLimitConfig::from_env(),
        env_usize("AICHAN_MAX_CONNECTIONS")
            .unwrap_or(DEFAULT_MAX_CONNECTIONS)
            .max(1),
    )?;

    run(&addr, state)
}

pub fn run(addr: &str, state: ServerState) -> Result<()> {
    let listener = TcpListener::bind(addr).with_context(|| format!("bind {addr}"))?;
    log_event("server.started", json!({ "addr": addr }));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let Some(connection_guard) = state.connection_limiter.try_acquire() else {
                    log_event(
                        "server.connection_rejected",
                        json!({ "reason": "max_connections" }),
                    );
                    if let Err(error) = write_http_response(
                        stream,
                        error_response(
                            503,
                            "server_busy",
                            "Server is at the configured connection limit.",
                            true,
                        ),
                    ) {
                        log_event(
                            "server.connection_reject_failed",
                            json!({ "error": error.to_string() }),
                        );
                    }
                    continue;
                };
                let state = state.clone();
                thread::spawn(move || {
                    let _connection_guard = connection_guard;
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
    if request.body.len() > state.rate_limits().max_body_bytes {
        return error_response(
            413,
            "payload_too_large",
            "Request body exceeds the configured maximum size.",
            false,
        );
    }

    if let Some(limited) = state.check_rate_limit(&request) {
        log_event(
            "rate_limit.exceeded",
            json!({
                "class": format!("{:?}", limited.key.class).to_ascii_lowercase(),
                "path": request.path(),
                "retry_after_seconds": limited.retry_after_seconds
            }),
        );
        return rate_limited_response(limited.retry_after_seconds);
    }

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

    let _store_guard = state
        .publish_store_lock
        .lock()
        .expect("publish store mutex poisoned");
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
        if existing.deleted {
            return error_response(
                409,
                "publish_deleted",
                "Publish id was author-deleted and cannot be reused.",
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
    let _store_guard = state
        .publish_store_lock
        .lock()
        .expect("publish store mutex poisoned");
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
    if let Some(response) = validate_request_auth_controls(state, &signature) {
        return response;
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
    if nonce.trim().is_empty() {
        anyhow::bail!("empty Aichan-Nonce header");
    }
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

fn validate_request_auth_controls(
    state: &ServerState,
    signature: &AichanRequestSignature,
) -> Option<HttpResponse> {
    let now = Utc::now();
    let oldest_allowed = now - ChronoDuration::seconds(REQUEST_SIGNATURE_MAX_SKEW_SECONDS);
    let newest_allowed = now + ChronoDuration::seconds(REQUEST_SIGNATURE_MAX_SKEW_SECONDS);
    if signature.timestamp < oldest_allowed || signature.timestamp > newest_allowed {
        return Some(error_response(
            401,
            "stale_request_signature",
            "Request signature timestamp is outside the accepted replay window.",
            false,
        ));
    }

    if !state
        .request_auth
        .mark_nonce_once(&signature.peer_id, &signature.nonce, now)
    {
        return Some(error_response(
            401,
            "replayed_request_nonce",
            "Request signature nonce has already been used in this replay window.",
            false,
        ));
    }

    None
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
                "max_publish_body_bytes": 8192,
                "max_body_bytes": state.rate_limits().max_body_bytes,
                "read_per_minute": state.rate_limits().read_per_minute,
                "write_per_minute": state.rate_limits().write_per_minute
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
    let temp_path = path.with_file_name("publish_records.json.tmp");
    let bytes = serde_json::to_vec_pretty(records)?;
    std::fs::write(&temp_path, bytes).with_context(|| format!("write {}", temp_path.display()))?;
    std::fs::rename(&temp_path, &path)
        .with_context(|| format!("rename {} to {}", temp_path.display(), path.display()))
}

fn handle_connection(mut stream: TcpStream, state: &ServerState) -> Result<()> {
    stream.set_read_timeout(Some(REQUEST_READ_TIMEOUT))?;
    stream.set_write_timeout(Some(REQUEST_READ_TIMEOUT))?;
    let response = match read_http_request(&mut stream, state.rate_limits().max_body_bytes)? {
        ReadHttpRequest::Request(request) => handle_request(state, request),
        ReadHttpRequest::PayloadTooLarge => error_response(
            413,
            "payload_too_large",
            "Request body exceeds the configured maximum size.",
            false,
        ),
    };
    write_http_response(stream, response)
}

enum ReadHttpRequest {
    Request(HttpRequest),
    PayloadTooLarge,
}

fn read_http_request(stream: &mut TcpStream, max_body_bytes: usize) -> Result<ReadHttpRequest> {
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
    if content_length > max_body_bytes {
        return Ok(ReadHttpRequest::PayloadTooLarge);
    }
    if content_length > 0 {
        let mut body = vec![0_u8; content_length];
        reader.read_exact(&mut body)?;
        request.body = body;
    }

    Ok(ReadHttpRequest::Request(request))
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
        503 => "Service Unavailable",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
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

fn rate_limited_response(retry_after_seconds: u64) -> HttpResponse {
    let mut response = error_response(
        429,
        "rate_limited",
        "Too many requests. Please retry after the indicated delay.",
        true,
    );
    response
        .headers
        .insert("Retry-After".to_string(), retry_after_seconds.to_string());
    response
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

fn env_u32(name: &str) -> Option<u32> {
    env_non_empty(name).and_then(|value| value.parse().ok())
}

fn env_usize(name: &str) -> Option<usize> {
    env_non_empty(name).and_then(|value| value.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_limiter_rejects_after_limit_and_releases_on_drop() {
        let limiter = ConnectionLimiter::new(1);
        let first = limiter.try_acquire().expect("first connection allowed");

        assert!(limiter.try_acquire().is_none());

        drop(first);
        assert!(limiter.try_acquire().is_some());
    }
}
