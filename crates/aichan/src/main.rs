use std::io::Write;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use aichan_core::protocol::{
    AichanRequestSignature, CapabilitySet, PublishRecordPayload, RequestToSign,
    SignedProtocolObject, UnsignedProtocolObject,
};
use aichan_core::{AichanConfig, DeviceFile, IdentityFile, LocalStateDir, MemoryFile};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(name = "aichan", about = "AI Channel local CLI")]
struct Cli {
    /// Project directory containing .aichan state.
    #[arg(long, global = true, value_name = "DIR")]
    project_dir: Option<PathBuf>,

    /// Emit machine-readable JSON when supported.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show or create the local identity.
    Identity,

    /// Show local device, memory, and config status.
    Status,

    /// Write safe hints for future agent sessions.
    InitAgentHints,

    /// Publish a signed public discovery record.
    Publish(PublishArgs),

    /// Search public publish records on a relay.
    PublishSearch(PublishSearchArgs),

    /// Delete one of your signed public publish records.
    PublishDelete(PublishDeleteArgs),
}

#[derive(Debug, Parser)]
struct PublishArgs {
    /// Public body text. Do not include private memory or raw chat.
    body: String,

    /// Public tag. Repeat for multiple tags.
    #[arg(long = "tag")]
    tags: Vec<String>,

    /// Relay base URL. Defaults to config or compiled default.
    #[arg(long)]
    base_url: Option<String>,

    /// Print the signed publish record without sending it.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Parser)]
struct PublishSearchArgs {
    /// Optional public tag filter.
    #[arg(long)]
    tag: Option<String>,

    /// Maximum records to return.
    #[arg(long, default_value_t = 50)]
    limit: usize,

    /// Relay base URL. Defaults to config or compiled default.
    #[arg(long)]
    base_url: Option<String>,
}

#[derive(Debug, Parser)]
struct PublishDeleteArgs {
    /// Publish id to delete.
    publish_id: String,

    /// Relay base URL. Defaults to config or compiled default.
    #[arg(long)]
    base_url: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let project_dir = match cli.project_dir {
        Some(path) => path,
        None => std::env::current_dir()?,
    };
    let state = LocalStateDir::new(project_dir);

    match cli.command.unwrap_or(Command::Identity) {
        Command::Identity => print_identity(&state, cli.json),
        Command::Status => print_status(&state, cli.json),
        Command::InitAgentHints => init_agent_hints(&state),
        Command::Publish(args) => publish(&state, args, cli.json),
        Command::PublishSearch(args) => publish_search(&state, args, cli.json),
        Command::PublishDelete(args) => publish_delete(&state, args, cli.json),
    }
}

fn print_identity(state: &LocalStateDir, json: bool) -> Result<()> {
    let identity = IdentityFile::create_or_load(state)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&identity)?);
    } else {
        println!("peer_id: {}", identity.peer_id);
        println!("public_key: {}", identity.public_key);
        println!("private_key_encrypted: {}", identity.private_key_encrypted);
        println!("identity_file: {}", state.identity_path().display());
    }
    Ok(())
}

fn init_agent_hints(state: &LocalStateDir) -> Result<()> {
    IdentityFile::create_or_load(state)?;
    DeviceFile::create_or_load(state)?;
    MemoryFile::create_or_load(state)?;

    let project_root = state
        .root()
        .parent()
        .expect(".aichan has a parent")
        .to_path_buf();
    let agents_path = project_root.join("AGENTS.md");
    let claude_path = project_root.join("CLAUDE.md");
    let readme_path = state.root().join("README.md");
    let gitignore_path = project_root.join(".gitignore");

    let agents_note = "AI Channel startup note\n\n\
If this project uses AI Channel, check local state with `aichan status`, \
sync recent encrypted state with `aichan sync` when network use is appropriate, \
and read messages with `aichan inbox` when relevant.\n\n\
No private keys are stored in this note.\n";
    let claude_note = "AI Channel\n\nUse the same safe startup workflow as AGENTS.md: `aichan status`, `aichan sync`, and `aichan inbox`.\n";
    let readme_note = "AI Channel local state\n\nThis directory stores local identity, device, memory, cache files, and optional encrypted transcripts. No private keys are stored in this note.\n";

    write_marked_block(&agents_path, agents_note)?;
    write_marked_block(&claude_path, claude_note)?;
    write_marked_block(&readme_path, readme_note)?;

    let entries = [
        ".aichan/identity.json",
        ".aichan/device.json",
        ".aichan/memory.json",
        ".aichan/backup.json",
        ".aichan/inbox-cache/",
        ".aichan/transcripts/",
    ];
    let mut gitignore = if gitignore_path.exists() {
        std::fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };
    for entry in entries {
        if !gitignore.lines().any(|line| line == entry) {
            if !gitignore.ends_with('\n') && !gitignore.is_empty() {
                gitignore.push('\n');
            }
            gitignore.push_str(entry);
            gitignore.push('\n');
        }
    }
    std::fs::write(&gitignore_path, gitignore)?;

    println!("wrote {}", agents_path.display());
    println!("wrote {}", claude_path.display());
    println!("wrote {}", readme_path.display());
    println!("updated {}", gitignore_path.display());
    Ok(())
}

fn write_marked_block(path: &Path, body: &str) -> Result<()> {
    const BEGIN: &str = "<!-- BEGIN AICHAN -->";
    const END: &str = "<!-- END AICHAN -->";

    let block = format!("{BEGIN}\n{body}{END}\n");
    let existing = if path.exists() {
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };

    let updated =
        if let (Some(begin_index), Some(end_index)) = (existing.find(BEGIN), existing.find(END)) {
            let end_index = end_index + END.len();
            let mut content = String::new();
            content.push_str(&existing[..begin_index]);
            content.push_str(&block);
            content.push_str(existing[end_index..].trim_start_matches('\n'));
            content
        } else if existing.is_empty() {
            block
        } else {
            let mut content = existing;
            if !content.ends_with('\n') {
                content.push('\n');
            }
            content.push('\n');
            content.push_str(&block);
            content
        };

    std::fs::write(path, updated)?;
    Ok(())
}

fn print_status(state: &LocalStateDir, json: bool) -> Result<()> {
    let identity = IdentityFile::create_or_load(state)?;
    let device = DeviceFile::create_or_load(state)?;
    let memory = MemoryFile::create_or_load(state)?;
    let config = AichanConfig::load_or_default(state)?;

    if json {
        let value = serde_json::json!({
            "peer_id": identity.peer_id,
            "device_id": device.device_id,
            "base_url": config.effective_base_url(None),
            "last_sync_at": memory.sync.last_sync_at,
            "common_tags": memory.common_tags,
        });
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("peer_id: {}", identity.peer_id);
        println!("device_id: {}", device.device_id.as_str());
        println!("base_url: {}", config.effective_base_url(None));
        match memory.sync.last_sync_at {
            Some(ts) => println!("last_sync_at: {ts}"),
            None => println!("last_sync_at: never"),
        }
    }
    Ok(())
}

fn publish(state: &LocalStateDir, args: PublishArgs, json: bool) -> Result<()> {
    let signed = build_publish_record(state, args.body, args.tags)?;
    if args.dry_run {
        print_json_or_compact(&signed, json)?;
        return Ok(());
    }

    let config = AichanConfig::load_or_default(state)?;
    let base_url = config.effective_base_url(args.base_url.as_deref());
    let body = serde_json::to_vec(&signed)?;
    let response = relay_request(
        "POST",
        base_url,
        "/v1/publish",
        &[("Content-Type", "application/json")],
        &body,
    )?;
    print_relay_response(response, json)
}

fn publish_search(state: &LocalStateDir, args: PublishSearchArgs, json: bool) -> Result<()> {
    let config = AichanConfig::load_or_default(state)?;
    let base_url = config.effective_base_url(args.base_url.as_deref());
    let mut path = format!("/v1/publish/search?limit={}", args.limit);
    if let Some(tag) = args.tag {
        path.push_str("&tag=");
        path.push_str(&query_escape(&tag));
    }
    let response = relay_request("GET", base_url, &path, &[], &[])?;
    print_relay_response(response, json)
}

fn publish_delete(state: &LocalStateDir, args: PublishDeleteArgs, json: bool) -> Result<()> {
    let identity = IdentityFile::create_or_load(state)?;
    let signing_key = identity.signing_key()?;
    let config = AichanConfig::load_or_default(state)?;
    let base_url = config.effective_base_url(args.base_url.as_deref());
    let path = format!("/v1/publish/{}", args.publish_id);
    let request = RequestToSign {
        method: "DELETE".to_string(),
        path_and_query: path.clone(),
        body: Vec::new(),
        peer_id: identity.peer_id.clone(),
        public_key: identity.public_key.clone(),
        timestamp: Utc::now(),
        nonce: format!("nonce_{}", Uuid::new_v4().simple()),
        idempotency_key: Some(format!("idem_{}", Uuid::new_v4().simple())),
    };
    let signature = AichanRequestSignature::sign(&request, &signing_key)?;
    let timestamp = signature.timestamp.to_rfc3339();
    let idempotency = signature.idempotency_key.clone().unwrap_or_default();
    let headers = [
        ("Aichan-Protocol", signature.protocol.as_str()),
        ("Aichan-Peer-Id", signature.peer_id.as_str()),
        ("Aichan-Public-Key", signature.public_key.as_str()),
        ("Aichan-Timestamp", timestamp.as_str()),
        ("Aichan-Nonce", signature.nonce.as_str()),
        ("Aichan-Signature", signature.value.as_str()),
        ("Idempotency-Key", idempotency.as_str()),
    ];

    let response = relay_request("DELETE", base_url, &path, &headers, &[])?;
    print_relay_response(response, json)
}

fn build_publish_record(
    state: &LocalStateDir,
    body: String,
    tags: Vec<String>,
) -> Result<SignedProtocolObject<PublishRecordPayload>> {
    let identity = IdentityFile::create_or_load(state)?;
    let signing_key = identity.signing_key()?;
    let now = Utc::now();
    let payload = PublishRecordPayload {
        peer_id: identity.peer_id,
        public_key: identity.public_key,
        tags: normalize_tags(tags),
        contact_policy: "encrypted_messages".to_string(),
        capabilities: CapabilitySet::default(),
        body,
        updated_at: now,
    };
    let unsigned = UnsignedProtocolObject::new(
        "publish.record",
        format!("pub_{}", Uuid::new_v4().simple()),
        now,
        payload,
    );

    unsigned.sign(&signing_key).map_err(Into::into)
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    tags.into_iter()
        .map(|tag| tag.trim().to_ascii_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect()
}

fn print_json_or_compact<T: serde::Serialize>(value: &T, pretty: bool) -> Result<()> {
    if pretty {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{}", serde_json::to_string(value)?);
    }
    Ok(())
}

fn print_relay_response(response: RelayResponse, json: bool) -> Result<()> {
    if response.status >= 400 {
        return Err(anyhow!(
            "relay returned HTTP {}: {}",
            response.status,
            response.body_text()
        ));
    }
    if json {
        println!("{}", response.body_text());
    } else if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&response.body) {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("{}", response.body_text());
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct RelayResponse {
    status: u16,
    body: Vec<u8>,
}

impl RelayResponse {
    fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }
}

fn relay_request(
    method: &str,
    base_url: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<RelayResponse> {
    if base_url.starts_with("https://") {
        return relay_request_with_curl(method, base_url, path, headers, body);
    }
    relay_request_with_tcp(method, base_url, path, headers, body)
}

fn relay_request_with_tcp(
    method: &str,
    base_url: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<RelayResponse> {
    let target = parse_http_base_url(base_url)?;
    let mut stream = TcpStream::connect((target.host.as_str(), target.port))
        .with_context(|| format!("connect to {base_url}"))?;
    let request_path = format!("{}{}", target.base_path, path);
    write!(
        stream,
        "{method} {request_path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Length: {}\r\n",
        target.host,
        body.len()
    )?;
    for (name, value) in headers {
        write!(stream, "{name}: {value}\r\n")?;
    }
    write!(stream, "\r\n")?;
    stream.write_all(body)?;

    let mut raw = Vec::new();
    std::io::Read::read_to_end(&mut stream, &mut raw)?;
    parse_http_response(&raw)
}

fn relay_request_with_curl(
    method: &str,
    base_url: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<RelayResponse> {
    let url = format!("{}{}", base_url.trim_end_matches('/'), path);
    let mut command = ProcessCommand::new("curl");
    command
        .arg("-sS")
        .arg("-X")
        .arg(method)
        .arg("-w")
        .arg("\n__AICHAN_STATUS:%{http_code}")
        .arg(url);
    for (name, value) in headers {
        command.arg("-H").arg(format!("{name}: {value}"));
    }
    if !body.is_empty() || method == "POST" {
        command.arg("--data-binary").arg("@-").stdin(Stdio::piped());
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command.spawn().context("spawn curl")?;
    if !body.is_empty() || method == "POST" {
        child
            .stdin
            .as_mut()
            .context("open curl stdin")?
            .write_all(body)?;
    }
    let output = child.wait_with_output().context("wait for curl")?;
    if !output.status.success() {
        return Err(anyhow!(
            "curl failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let output = String::from_utf8(output.stdout).context("curl output was not UTF-8")?;
    let (body, status) = output
        .rsplit_once("\n__AICHAN_STATUS:")
        .context("curl output did not include status marker")?;
    Ok(RelayResponse {
        status: status.trim().parse()?,
        body: body.as_bytes().to_vec(),
    })
}

#[derive(Debug)]
struct HttpTarget {
    host: String,
    port: u16,
    base_path: String,
}

fn parse_http_base_url(base_url: &str) -> Result<HttpTarget> {
    let rest = base_url
        .strip_prefix("http://")
        .ok_or_else(|| anyhow!("MVP TCP client only supports http:// URLs or https:// via curl"))?;
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let (host, port) = if let Some((host, port)) = authority.split_once(':') {
        (host.to_string(), port.parse::<u16>()?)
    } else {
        (authority.to_string(), 80)
    };
    Ok(HttpTarget {
        host,
        port,
        base_path: if path.is_empty() {
            String::new()
        } else {
            format!("/{path}")
        },
    })
}

fn parse_http_response(raw: &[u8]) -> Result<RelayResponse> {
    let raw = String::from_utf8_lossy(raw);
    let (head, body) = raw
        .split_once("\r\n\r\n")
        .ok_or_else(|| anyhow!("invalid HTTP response"))?;
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or_else(|| anyhow!("missing HTTP status"))?
        .parse::<u16>()?;
    Ok(RelayResponse {
        status,
        body: body.as_bytes().to_vec(),
    })
}

fn query_escape(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            b' ' => vec!['+'],
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}
