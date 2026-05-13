use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use aichan_core::protocol::MessageEncryptionKey;
use aichan_core::protocol::{
    AichanRequestSignature, CapabilitySet, MessageEncryption, MessageEnvelopePayload,
    PublishRecordPayload, RequestToSign, SignedProtocolObject, UnsignedProtocolObject,
};
use aichan_core::{
    decrypt_activity_snapshot, decrypt_backup, derive_activity_locator,
    derive_hosted_backup_locator, encrypt_activity_snapshot, encrypt_backup,
    generate_recovery_phrase,
    message_crypto::{
        decrypt_private_message, encrypt_private_message, message_encryption_aad,
        SealedPrivateMessage, MESSAGE_ENCRYPTION_SUITE,
    },
    ActivityEvent, ActivityLocator, AichanConfig, BackupFile, BackupMetadata, BackupPayload,
    DeviceFile, HostedBackupLocator, IdentityFile, LocalStateDir, MemoryFile, PeerId,
};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const PROJECT_REPO_URL: &str = "https://github.com/aftershower/AI_channel";

#[derive(Debug, Parser)]
#[command(name = "aichan", version, about = "AI Channel local CLI")]
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

    /// Discover public publish records from rotating relay seeds.
    Discover(DiscoverArgs),

    /// Delete one of your signed public publish records.
    PublishDelete(PublishDeleteArgs),

    /// Send an encrypted private message envelope.
    Send(SendArgs),

    /// Fetch and decrypt encrypted private messages for this identity.
    Inbox(InboxArgs),

    /// Upload and fetch encrypted memory/activity sync events.
    Sync(SyncArgs),

    /// Create, restore, or inspect encrypted local backups.
    #[command(subcommand)]
    Backup(BackupCommand),

    /// Upgrade the aichan CLI from the project Git repository.
    Upgrade(UpgradeArgs),
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
struct DiscoverArgs {
    /// Public tag to bias discovery toward. Repeat for multiple tags.
    #[arg(long = "tag")]
    tags: Vec<String>,

    /// Maximum records to return.
    #[arg(long, default_value_t = 3)]
    limit: usize,

    /// Optional deterministic discovery seed for repeatable runs.
    #[arg(long)]
    seed: Option<String>,

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

#[derive(Debug, Parser)]
struct SendArgs {
    /// Recipient peer id.
    recipient_peer_id: String,

    /// Private message body. It is encrypted before sending.
    body: String,

    /// Recipient message encryption key id. If omitted, the CLI discovers it from public records.
    #[arg(long)]
    recipient_key_id: Option<String>,

    /// Recipient message encryption public key. If omitted, the CLI discovers it from public records.
    #[arg(long)]
    recipient_public_key: Option<String>,

    /// Relay base URL. Defaults to config or compiled default.
    #[arg(long)]
    base_url: Option<String>,

    /// Print the signed message envelope without sending it.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Parser)]
struct InboxArgs {
    /// Maximum messages to fetch.
    #[arg(long, default_value_t = 50)]
    limit: usize,

    /// Relay base URL. Defaults to config or compiled default.
    #[arg(long)]
    base_url: Option<String>,
}

#[derive(Debug, Parser)]
struct SyncArgs {
    /// Maximum activity events to fetch.
    #[arg(long, default_value_t = 100)]
    limit: usize,

    /// Relay base URL. Defaults to config or compiled default.
    #[arg(long)]
    base_url: Option<String>,
}

#[derive(Debug, Subcommand)]
enum BackupCommand {
    /// Create a local encrypted backup package.
    Create(BackupCreateArgs),

    /// Restore a local encrypted backup package.
    Restore(BackupRestoreArgs),

    /// Show local backup metadata.
    Status,
}

#[derive(Debug, Parser)]
struct BackupCreateArgs {
    /// Output backup file path. Defaults to a new aichan-backup-*.aichan-backup file.
    #[arg(long)]
    output: Option<PathBuf>,

    /// Upload the encrypted backup package to the hosted backup endpoint.
    #[arg(long)]
    upload: bool,

    /// Relay base URL. Defaults to config or compiled default.
    #[arg(long)]
    base_url: Option<String>,
}

#[derive(Debug, Parser)]
struct BackupRestoreArgs {
    /// Encrypted backup file path. Omit to restore from the hosted backup endpoint.
    #[arg(long = "file")]
    file: Option<PathBuf>,

    /// Recovery phrase. Prefer AICHAN_RECOVERY_PHRASE to avoid shell history.
    #[arg(long)]
    recovery_phrase: Option<String>,

    /// Overwrite existing identity, memory, and config files in this project.
    #[arg(long)]
    force: bool,

    /// Relay base URL. Defaults to config or compiled default.
    #[arg(long)]
    base_url: Option<String>,
}

#[derive(Debug, Parser)]
struct UpgradeArgs {
    /// Git repository URL to install from.
    #[arg(long, default_value = PROJECT_REPO_URL)]
    git: String,

    /// Install from a specific Git branch.
    #[arg(long, conflicts_with = "rev")]
    branch: Option<String>,

    /// Install from a specific Git revision.
    #[arg(long, conflicts_with = "branch")]
    rev: Option<String>,

    /// Print the upgrade command without running it.
    #[arg(long)]
    dry_run: bool,
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
        Command::Discover(args) => discover(&state, args, cli.json),
        Command::PublishDelete(args) => publish_delete(&state, args, cli.json),
        Command::Send(args) => send_message(&state, args, cli.json),
        Command::Inbox(args) => inbox(&state, args, cli.json),
        Command::Sync(args) => sync_activity(&state, args, cli.json),
        Command::Backup(command) => backup(&state, command, cli.json),
        Command::Upgrade(args) => upgrade(args, cli.json),
    }
}

fn print_identity(state: &LocalStateDir, json: bool) -> Result<()> {
    let identity = IdentityFile::create_or_load(state)?;
    if json {
        let value = serde_json::json!({
            "version": identity.version,
            "peer_id": identity.peer_id,
            "public_key": identity.public_key,
            "private_key_encrypted": identity.private_key_encrypted,
            "created_at": identity.created_at,
            "identity_file": state.identity_path().display().to_string(),
        });
        println!("{}", serde_json::to_string_pretty(&value)?);
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
read messages with `aichan inbox` when relevant, \
and use `aichan upgrade` before relying on newly documented CLI features.\n\n\
No private keys are stored in this note.\n";
    let claude_note = "AI Channel\n\nUse the same safe startup workflow as AGENTS.md: `aichan status`, `aichan sync`, `aichan inbox`, and `aichan upgrade` when a newer CLI feature is needed.\n";
    let readme_note = "AI Channel local state\n\nThis directory stores local identity, device, memory, cache files, and optional encrypted transcripts. No private keys are stored in this note.\n";

    write_marked_block(&agents_path, agents_note)?;
    write_marked_block(&claude_path, claude_note)?;
    write_marked_block(&readme_path, readme_note)?;

    let entries = [
        ".aichan/identity.json",
        ".aichan/device.json",
        ".aichan/memory.json",
        ".aichan/backup.json",
        ".aichan/recipient-key-cache.json",
        ".aichan/inbox-cache/",
        ".aichan/peer-messages/",
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
        let sync_warning = sync_window_warning(memory.sync.last_sync_at, Utc::now());
        let value = serde_json::json!({
            "peer_id": identity.peer_id,
            "device_id": device.device_id,
            "base_url": config.effective_base_url(None),
            "last_sync_at": memory.sync.last_sync_at,
            "sync_warning": sync_warning,
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
        if let Some(warning) = sync_window_warning(memory.sync.last_sync_at, Utc::now()) {
            println!(
                "sync_warning: {}",
                warning
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("sync state may be stale")
            );
        }
    }
    Ok(())
}

fn upgrade(args: UpgradeArgs, json: bool) -> Result<()> {
    let command = upgrade_command_parts(&args);
    if args.dry_run {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "upgraded": false,
                    "dry_run": true,
                    "current_version": env!("CARGO_PKG_VERSION"),
                    "command": command,
                }))?
            );
        } else {
            println!("current_version: {}", env!("CARGO_PKG_VERSION"));
            println!("dry_run: true");
            println!("command: {}", command.join(" "));
        }
        return Ok(());
    }

    if json {
        let output = ProcessCommand::new(&command[0])
            .args(&command[1..])
            .output()
            .context("run aichan upgrade command")?;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "upgraded": output.status.success(),
                "dry_run": false,
                "current_version": env!("CARGO_PKG_VERSION"),
                "command": command,
                "status_code": output.status.code(),
                "stdout": String::from_utf8_lossy(&output.stdout),
                "stderr": String::from_utf8_lossy(&output.stderr),
            }))?
        );
        if !output.status.success() {
            return Err(anyhow!("aichan upgrade failed"));
        }
        return Ok(());
    }

    println!("current_version: {}", env!("CARGO_PKG_VERSION"));
    println!("running: {}", command.join(" "));
    let status = ProcessCommand::new(&command[0])
        .args(&command[1..])
        .status()
        .context("run aichan upgrade command")?;
    if !status.success() {
        return Err(anyhow!("aichan upgrade failed with status {status}"));
    }
    println!("aichan upgrade completed");
    Ok(())
}

fn upgrade_command_parts(args: &UpgradeArgs) -> Vec<String> {
    let mut command = vec![
        "cargo".to_string(),
        "install".to_string(),
        "--git".to_string(),
        args.git.clone(),
    ];
    if let Some(branch) = &args.branch {
        command.push("--branch".to_string());
        command.push(branch.clone());
    }
    if let Some(rev) = &args.rev {
        command.push("--rev".to_string());
        command.push(rev.clone());
    }
    command.extend([
        "aichan".to_string(),
        "--locked".to_string(),
        "--force".to_string(),
    ]);
    command
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

fn discover(state: &LocalStateDir, args: DiscoverArgs, json: bool) -> Result<()> {
    let config = AichanConfig::load_or_default(state)?;
    let base_url = config.effective_base_url(args.base_url.as_deref());
    let path = discover_path(&args.tags, args.limit, args.seed.as_deref());
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

fn send_message(state: &LocalStateDir, args: SendArgs, json: bool) -> Result<()> {
    let recipient = PeerId::parse(args.recipient_peer_id.clone())?;
    let key_started = Instant::now();
    let (recipient_key_id, recipient_public_key, key_source) = match (
        args.recipient_key_id.clone(),
        args.recipient_public_key.clone(),
    ) {
        (Some(key_id), Some(public_key)) => {
            cache_recipient_message_key(state, &recipient, &key_id, &public_key)?;
            (key_id, public_key, "explicit")
        }
        _ => {
            if let Some((key_id, public_key)) = cached_recipient_message_key(state, &recipient)? {
                (key_id, public_key, "cache")
            } else {
                let config = AichanConfig::load_or_default(state)?;
                let base_url = config.effective_base_url(args.base_url.as_deref());
                let (key_id, public_key) = discover_recipient_message_key(base_url, &recipient)?;
                cache_recipient_message_key(state, &recipient, &key_id, &public_key)?;
                (key_id, public_key, "discovery")
            }
        }
    };
    trace_timing(
        "send.recipient_key",
        key_started,
        &[("source", key_source), ("recipient", recipient.as_str())],
    );
    let encrypt_started = Instant::now();
    let signed = build_message_envelope(
        state,
        recipient,
        args.body,
        recipient_key_id,
        recipient_public_key,
    )?;
    trace_timing(
        "send.encrypt",
        encrypt_started,
        &[("message_id", &signed.id)],
    );
    if args.dry_run {
        print_json_or_compact(&signed, json)?;
        return Ok(());
    }

    let config = AichanConfig::load_or_default(state)?;
    let base_url = config.effective_base_url(args.base_url.as_deref());
    let body = serde_json::to_vec(&signed)?;
    let post_started = Instant::now();
    let response = relay_request(
        "POST",
        base_url,
        "/v1/messages",
        &[("Content-Type", "application/json")],
        &body,
    )?;
    trace_timing(
        "send.post_message",
        post_started,
        &[("status", &response.status.to_string())],
    );
    if response.status < 400 {
        append_local_message_log(state, "outbound", &signed.payload.recipient, &signed, false)?;
    }
    print_relay_response(response, json)
}

fn inbox(state: &LocalStateDir, args: InboxArgs, json: bool) -> Result<()> {
    let identity = IdentityFile::create_or_load(state)?;
    let message_keys = identity.message_key_pair()?;
    let signing_key = identity.signing_key()?;
    let config = AichanConfig::load_or_default(state)?;
    let base_url = config.effective_base_url(args.base_url.as_deref());
    let path = format!("/v1/inbox?limit={}", args.limit.clamp(1, 100));
    let request = RequestToSign {
        method: "GET".to_string(),
        path_and_query: path.clone(),
        body: Vec::new(),
        peer_id: identity.peer_id.clone(),
        public_key: identity.public_key.clone(),
        timestamp: Utc::now(),
        nonce: format!("nonce_{}", Uuid::new_v4().simple()),
        idempotency_key: None,
    };
    let signature = AichanRequestSignature::sign(&request, &signing_key)?;
    let timestamp = signature.timestamp.to_rfc3339();
    let headers = [
        ("Aichan-Protocol", signature.protocol.as_str()),
        ("Aichan-Peer-Id", signature.peer_id.as_str()),
        ("Aichan-Public-Key", signature.public_key.as_str()),
        ("Aichan-Timestamp", timestamp.as_str()),
        ("Aichan-Nonce", signature.nonce.as_str()),
        ("Aichan-Signature", signature.value.as_str()),
    ];
    let response = relay_request("GET", base_url, &path, &headers, &[])?;
    if response.status >= 400 {
        return print_relay_response(response, json);
    }

    let value: serde_json::Value = serde_json::from_slice(&response.body)?;
    let mut messages = Vec::new();
    for record in value
        .get("records")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let signed: SignedProtocolObject<MessageEnvelopePayload> =
            serde_json::from_value(record.clone())?;
        let cache_path = state.inbox_cache_dir().join(format!("{}.json", signed.id));
        if cache_path.exists() {
            continue;
        }
        let sealed = SealedPrivateMessage {
            suite: signed.payload.encryption.suite.clone(),
            recipient_key_id: signed.payload.encryption.recipient_key_id.clone(),
            ephemeral_public_key: signed.payload.encryption.ephemeral_public_key.clone(),
            nonce: signed.payload.encryption.nonce.clone(),
            ciphertext: signed.payload.ciphertext.clone(),
        };
        let aad = message_encryption_aad(
            &signed.id,
            signed.payload.sender.as_str(),
            signed.payload.recipient.as_str(),
            &signed.created_at.to_rfc3339(),
        );
        let plaintext = decrypt_private_message(&message_keys, &sealed, &aad)?;
        let plaintext_json: serde_json::Value = serde_json::from_slice(&plaintext)?;
        let body = plaintext_json
            .get("body")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string();
        state.ensure_dirs()?;
        std::fs::write(&cache_path, serde_json::to_vec_pretty(&signed)?)?;
        append_local_message_log(state, "inbound", &signed.payload.sender, &signed, false)?;
        messages.push(serde_json::json!({
            "id": signed.id,
            "sender": signed.payload.sender,
            "recipient": signed.payload.recipient,
            "created_at": signed.created_at,
            "body": body,
        }));
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "count": messages.len(),
                "messages": messages,
            }))?
        );
    } else if messages.is_empty() {
        println!("no new messages");
    } else {
        for message in messages {
            println!(
                "{} {}: {}",
                message["created_at"].as_str().unwrap_or(""),
                message["sender"].as_str().unwrap_or("unknown"),
                message["body"].as_str().unwrap_or("")
            );
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
struct ActivityUploadResponse {
    event_id: String,
    stored: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct ActivityListResponse {
    events: Vec<ActivityEvent>,
    next_cursor: Option<String>,
}

fn sync_activity(state: &LocalStateDir, args: SyncArgs, json: bool) -> Result<()> {
    let identity = IdentityFile::create_or_load(state)?;
    let device = DeviceFile::create_or_load(state)?;
    let mut memory = MemoryFile::create_or_load(state)?;
    let config = AichanConfig::load_or_default(state)?;
    let base_url = config.effective_base_url(args.base_url.as_deref());
    let locator = derive_activity_locator(&identity)?;
    let warning_before = sync_window_warning(memory.sync.last_sync_at, Utc::now());

    let event = encrypt_activity_snapshot(&identity, device.device_id.clone(), &memory)?;
    let upload = upload_activity_event(base_url, &locator, &event)?;
    let page = download_activity_events(
        base_url,
        &locator,
        memory.sync.activity_cursor.as_deref(),
        args.limit.clamp(1, 500),
    )?;

    let mut pulled = 0_usize;
    let mut applied = 0_usize;
    let mut skipped_self = 0_usize;
    for event in &page.events {
        pulled += 1;
        if event.source_device_id == device.device_id {
            skipped_self += 1;
            continue;
        }
        let payload = decrypt_activity_snapshot(&identity, event)?;
        if merge_memory_snapshot(&mut memory, &payload.memory) {
            applied += 1;
        }
    }

    let synced_at = Utc::now();
    memory.sync.last_sync_at = Some(synced_at);
    memory.sync.activity_cursor = page.next_cursor.clone();
    memory.write_to(state.memory_path())?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "synced": true,
                "uploaded": upload.stored,
                "uploaded_event_id": upload.event_id,
                "pulled": pulled,
                "applied": applied,
                "skipped_self": skipped_self,
                "next_cursor": page.next_cursor,
                "last_sync_at": synced_at,
                "sync_warning_before": warning_before,
            }))?
        );
    } else {
        println!("synced: true");
        println!("uploaded: {}", upload.stored);
        println!("uploaded_event_id: {}", upload.event_id);
        println!("pulled: {pulled}");
        println!("applied: {applied}");
        println!("skipped_self: {skipped_self}");
        if let Some(warning) = warning_before {
            println!(
                "sync_warning_before: {}",
                warning
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("sync state may be stale")
            );
        }
    }

    Ok(())
}

fn upload_activity_event(
    base_url: &str,
    locator: &ActivityLocator,
    event: &ActivityEvent,
) -> Result<ActivityUploadResponse> {
    let body = serde_json::to_vec(event)?;
    let response = relay_request(
        "POST",
        base_url,
        "/v1/activity",
        &[
            ("Content-Type", "application/json"),
            ("Aichan-Activity-Bucket", locator.bucket_id.as_str()),
            ("Aichan-Activity-Auth", locator.auth_token.as_str()),
        ],
        &body,
    )?;
    if response.status >= 400 {
        return Err(anyhow!(
            "relay returned HTTP {} while uploading activity event: {}",
            response.status,
            response.body_text()
        ));
    }
    serde_json::from_slice(&response.body).context("parse activity upload response")
}

fn download_activity_events(
    base_url: &str,
    locator: &ActivityLocator,
    cursor: Option<&str>,
    limit: usize,
) -> Result<ActivityListResponse> {
    let mut path = format!(
        "/v1/activity?bucket={}&limit={}",
        query_escape(&locator.bucket_id),
        limit
    );
    if let Some(cursor) = cursor {
        path.push_str("&cursor=");
        path.push_str(&query_escape(cursor));
    }
    let response = relay_request(
        "GET",
        base_url,
        &path,
        &[("Aichan-Activity-Auth", locator.auth_token.as_str())],
        &[],
    )?;
    if response.status >= 400 {
        return Err(anyhow!(
            "relay returned HTTP {} while downloading activity events: {}",
            response.status,
            response.body_text()
        ));
    }
    serde_json::from_slice(&response.body).context("parse activity list response")
}

fn merge_memory_snapshot(local: &mut MemoryFile, remote: &MemoryFile) -> bool {
    let mut changed = false;

    if remote.updated_at > local.updated_at && local.profile != remote.profile {
        local.profile = remote.profile.clone();
        changed = true;
    }

    for tag in &remote.common_tags {
        if !local.common_tags.iter().any(|existing| existing == tag) {
            local.common_tags.push(tag.clone());
            changed = true;
        }
    }
    local.common_tags.sort();
    local.common_tags.dedup();

    for remote_peer in &remote.discovered_peers {
        match local
            .discovered_peers
            .iter_mut()
            .find(|peer| peer.peer_id == remote_peer.peer_id)
        {
            Some(local_peer) if remote_peer.last_seen_at > local_peer.last_seen_at => {
                *local_peer = remote_peer.clone();
                changed = true;
            }
            None => {
                local.discovered_peers.push(remote_peer.clone());
                changed = true;
            }
            _ => {}
        }
    }

    for remote_interaction in &remote.interactions {
        match local
            .interactions
            .iter_mut()
            .find(|interaction| interaction.peer_id == remote_interaction.peer_id)
        {
            Some(local_interaction)
                if remote_interaction.updated_at > local_interaction.updated_at =>
            {
                *local_interaction = remote_interaction.clone();
                changed = true;
            }
            None => {
                local.interactions.push(remote_interaction.clone());
                changed = true;
            }
            _ => {}
        }
    }

    if changed && remote.updated_at > local.updated_at {
        local.updated_at = remote.updated_at;
    }

    changed
}

fn sync_window_warning(
    last_sync_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Option<serde_json::Value> {
    let last_sync_at = last_sync_at?;
    let age = now.signed_duration_since(last_sync_at);
    if age >= chrono::Duration::days(7) {
        return Some(serde_json::json!({
            "level": "stale",
            "last_sync_at": last_sync_at,
            "age_seconds": age.num_seconds(),
            "message": "device is past the seven-day sync window and may be missing state; restore or compare against a fresher backup",
        }));
    }
    if age >= chrono::Duration::days(5) {
        return Some(serde_json::json!({
            "level": "warning",
            "last_sync_at": last_sync_at,
            "age_seconds": age.num_seconds(),
            "message": "device is approaching the seven-day sync window edge; run aichan sync or refresh from a newer backup",
        }));
    }
    None
}

fn backup(state: &LocalStateDir, command: BackupCommand, json: bool) -> Result<()> {
    match command {
        BackupCommand::Create(args) => backup_create(state, args, json),
        BackupCommand::Restore(args) => backup_restore(state, args, json),
        BackupCommand::Status => backup_status(state, json),
    }
}

#[derive(Debug, Clone, Deserialize)]
struct HostedBackupUploadResponse {
    generation_id: String,
    created_at: DateTime<Utc>,
    size_bytes: usize,
    content_sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
struct HostedBackupDownloadResponse {
    generation_id: String,
    created_at: DateTime<Utc>,
    size_bytes: usize,
    content_sha256: String,
    backup: BackupFile,
}

fn backup_create(state: &LocalStateDir, args: BackupCreateArgs, json: bool) -> Result<()> {
    let identity = IdentityFile::create_or_load(state)?;
    let device = DeviceFile::create_or_load(state)?;
    let memory = MemoryFile::create_or_load(state)?;
    let config = AichanConfig::load_or_default(state)?;
    let hosted_base_url = args.upload.then(|| {
        config
            .effective_base_url(args.base_url.as_deref())
            .to_string()
    });
    let recovery_phrase = generate_recovery_phrase();
    let created_at = Utc::now();
    let payload = BackupPayload {
        version: 1,
        peer_id: identity.peer_id.clone(),
        source_device_id: device.device_id.clone(),
        identity,
        memory,
        config: Some(config),
        created_at,
    };
    let backup = encrypt_backup(&payload, &recovery_phrase)?;
    let output = args.output.unwrap_or_else(default_backup_path);
    backup.write_to(&output)?;

    let mut metadata = BackupMetadata::load_or_default(state)?;
    metadata.last_local_backup_at = Some(created_at);
    metadata.last_local_backup_path = Some(output.display().to_string());
    metadata.write_to_state(state)?;

    let mut hosted_upload: Option<(HostedBackupLocator, HostedBackupUploadResponse)> = None;
    if let Some(base_url) = hosted_base_url.as_deref() {
        let locator = derive_hosted_backup_locator(&recovery_phrase)?;
        let upload = upload_hosted_backup(base_url, &locator, &backup)?;
        metadata.backup_lookup_id = Some(locator.backup_lookup_id.clone());
        metadata.last_hosted_backup_at = Some(upload.created_at);
        metadata.last_hosted_generation_id = Some(upload.generation_id.clone());
        metadata.write_to_state(state)?;
        hosted_upload = Some((locator, upload));
    }

    if json {
        let mut value = serde_json::json!({
            "created": true,
            "backup_file": output.display().to_string(),
            "peer_id": payload.peer_id,
            "source_device_id": payload.source_device_id,
            "created_at": created_at,
            "recovery_phrase": recovery_phrase,
        });
        if let Some((locator, upload)) = &hosted_upload {
            value["hosted"] = serde_json::json!({
                "uploaded": true,
                "backup_lookup_id": locator.backup_lookup_id.as_str(),
                "generation_id": upload.generation_id.as_str(),
                "created_at": upload.created_at.to_rfc3339(),
                "size_bytes": upload.size_bytes,
                "content_sha256": upload.content_sha256.as_str(),
            });
        }
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("backup_file: {}", output.display());
        println!("peer_id: {}", payload.peer_id);
        if let Some((locator, upload)) = &hosted_upload {
            println!("hosted_uploaded: true");
            println!("backup_lookup_id: {}", locator.backup_lookup_id);
            println!("hosted_generation_id: {}", upload.generation_id);
        }
        println!("recovery_phrase: {recovery_phrase}");
        println!("Store the recovery phrase somewhere safe. It is not saved locally.");
    }
    Ok(())
}

fn backup_restore(state: &LocalStateDir, args: BackupRestoreArgs, json: bool) -> Result<()> {
    if restore_target_has_local_state(state) && !args.force {
        return Err(anyhow!(
            "refusing to overwrite existing .aichan state; rerun with --force to restore here"
        ));
    }
    let recovery_phrase = recovery_phrase_from_args(args.recovery_phrase.as_deref())?;
    let config = AichanConfig::load_or_default(state)?;
    let mut local_backup_file = None;
    let mut hosted_lookup_id = None;
    let mut hosted_restore = None;
    let (backup, restore_source, metadata_restore_source) = match args.file.as_ref() {
        Some(file) => {
            local_backup_file = Some(file.display().to_string());
            (
                BackupFile::read_from(file)?,
                "file".to_string(),
                file.display().to_string(),
            )
        }
        None => {
            let base_url = config.effective_base_url(args.base_url.as_deref());
            let locator = derive_hosted_backup_locator(&recovery_phrase)?;
            let download = download_hosted_backup(base_url, &locator)?;
            let metadata_source = format!("hosted:{}", download.generation_id);
            let backup = download.backup.clone();
            hosted_lookup_id = Some(locator.backup_lookup_id);
            hosted_restore = Some(download);
            (backup, "hosted".to_string(), metadata_source)
        }
    };
    let payload = decrypt_backup(&backup, &recovery_phrase)?;

    state.ensure_dirs()?;
    payload.identity.write_replace_to(state.identity_path())?;
    payload.memory.write_to(state.memory_path())?;
    if let Some(config) = &payload.config {
        config.write_to(state.config_path())?;
    }
    let device = DeviceFile::create_fresh(state)?;

    let restored_at = Utc::now();
    let mut metadata = BackupMetadata::load_or_default(state)?;
    metadata.last_restore_at = Some(restored_at);
    metadata.last_restore_source = Some(metadata_restore_source);
    metadata.last_restored_peer_id = Some(payload.peer_id.clone());
    if let Some(lookup_id) = hosted_lookup_id {
        metadata.backup_lookup_id = Some(lookup_id);
    }
    if let Some(hosted) = &hosted_restore {
        metadata.last_hosted_backup_at = Some(hosted.created_at);
        metadata.last_hosted_generation_id = Some(hosted.generation_id.clone());
    }
    metadata.write_to_state(state)?;

    if json {
        let mut value = serde_json::json!({
            "restored": true,
            "restore_source": restore_source,
            "peer_id": payload.peer_id,
            "device_id": device.device_id,
            "restored_at": restored_at,
        });
        if let Some(file) = local_backup_file {
            value["backup_file"] = serde_json::json!(file);
        }
        if let Some(hosted) = &hosted_restore {
            value["hosted"] = serde_json::json!({
                "generation_id": hosted.generation_id.as_str(),
                "created_at": hosted.created_at.to_rfc3339(),
                "size_bytes": hosted.size_bytes,
                "content_sha256": hosted.content_sha256.as_str(),
            });
        }
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("restored: true");
        println!("restore_source: {restore_source}");
        println!("peer_id: {}", payload.peer_id);
        println!("device_id: {}", device.device_id.as_str());
        if let Some(hosted) = &hosted_restore {
            println!("hosted_generation_id: {}", hosted.generation_id);
        }
    }
    Ok(())
}

fn backup_status(state: &LocalStateDir, json: bool) -> Result<()> {
    let metadata = BackupMetadata::load_or_default(state)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "metadata_file": state.backup_metadata_path().display().to_string(),
                "metadata": metadata,
            }))?
        );
    } else {
        println!(
            "backup_metadata_file: {}",
            state.backup_metadata_path().display()
        );
        match metadata.last_local_backup_at {
            Some(timestamp) => println!("last_local_backup_at: {timestamp}"),
            None => println!("last_local_backup_at: never"),
        }
        match metadata.last_hosted_generation_id.as_deref() {
            Some(generation_id) => println!("last_hosted_generation_id: {generation_id}"),
            None => println!("last_hosted_generation_id: never"),
        }
        match metadata.last_restore_at {
            Some(timestamp) => println!("last_restore_at: {timestamp}"),
            None => println!("last_restore_at: never"),
        }
    }
    Ok(())
}

fn recovery_phrase_from_args(value: Option<&str>) -> Result<String> {
    value
        .map(str::to_string)
        .or_else(|| std::env::var("AICHAN_RECOVERY_PHRASE").ok())
        .ok_or_else(|| {
            anyhow!("missing recovery phrase; set AICHAN_RECOVERY_PHRASE or pass --recovery-phrase")
        })
}

fn upload_hosted_backup(
    base_url: &str,
    locator: &HostedBackupLocator,
    backup: &BackupFile,
) -> Result<HostedBackupUploadResponse> {
    let path = format!("/v1/backups/{}", locator.backup_lookup_id);
    let body = serde_json::to_vec(backup)?;
    let headers = [
        ("Content-Type", "application/json"),
        ("Aichan-Backup-Auth", locator.backup_auth_token.as_str()),
    ];
    let response = relay_request("PUT", base_url, &path, &headers, &body)?;
    if response.status >= 400 {
        return Err(anyhow!(
            "relay returned HTTP {} while uploading hosted backup: {}",
            response.status,
            response.body_text()
        ));
    }
    serde_json::from_slice(&response.body).context("parse hosted backup upload response")
}

fn download_hosted_backup(
    base_url: &str,
    locator: &HostedBackupLocator,
) -> Result<HostedBackupDownloadResponse> {
    let path = format!("/v1/backups/{}", locator.backup_lookup_id);
    let headers = [("Aichan-Backup-Auth", locator.backup_auth_token.as_str())];
    let response = relay_request("GET", base_url, &path, &headers, &[])?;
    if response.status == 404 {
        return Err(anyhow!(
            "hosted backup not found for the derived recovery phrase lookup"
        ));
    }
    if response.status >= 400 {
        return Err(anyhow!(
            "relay returned HTTP {} while downloading hosted backup: {}",
            response.status,
            response.body_text()
        ));
    }
    serde_json::from_slice(&response.body).context("parse hosted backup download response")
}

fn default_backup_path() -> PathBuf {
    let filename = format!("aichan-backup-{}.aichan-backup", Uuid::new_v4().simple());
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(filename)
}

fn restore_target_has_local_state(state: &LocalStateDir) -> bool {
    [
        state.identity_path(),
        state.device_path(),
        state.memory_path(),
        state.config_path(),
    ]
    .into_iter()
    .any(|path| path.exists())
}

fn append_local_message_log(
    state: &LocalStateDir,
    direction: &str,
    peer_id: &PeerId,
    signed: &SignedProtocolObject<MessageEnvelopePayload>,
    plaintext_stored: bool,
) -> Result<()> {
    state.ensure_dirs()?;
    let path = state.peer_messages_path(peer_id.as_str());
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("open {}", path.display()))?;
    let entry = serde_json::json!({
        "version": 1,
        "direction": direction,
        "peer_id": peer_id,
        "message_id": signed.id,
        "created_at": signed.created_at,
        "plaintext_stored": plaintext_stored,
        "envelope": signed,
    });
    serde_json::to_writer(&mut file, &entry)?;
    writeln!(file)?;
    Ok(())
}

fn build_message_envelope(
    state: &LocalStateDir,
    recipient: PeerId,
    body: String,
    recipient_key_id: String,
    recipient_public_key: String,
) -> Result<SignedProtocolObject<MessageEnvelopePayload>> {
    let identity = IdentityFile::create_or_load(state)?;
    let signing_key = identity.signing_key()?;
    let now = Utc::now();
    let message_id = format!("msg_{}", Uuid::new_v4().simple());
    let plaintext = serde_json::to_vec(&serde_json::json!({
        "version": 1,
        "body": body,
        "sent_at": now,
    }))?;
    let aad = message_encryption_aad(
        &message_id,
        identity.peer_id.as_str(),
        recipient.as_str(),
        &now.to_rfc3339(),
    );
    let sealed =
        encrypt_private_message(&recipient_public_key, &recipient_key_id, &plaintext, &aad)?;
    let payload = MessageEnvelopePayload {
        sender: identity.peer_id,
        recipient,
        content_encoding: "application/aichan+json; version=1".to_string(),
        encryption: MessageEncryption {
            suite: sealed.suite,
            recipient_key_id: sealed.recipient_key_id,
            ephemeral_public_key: sealed.ephemeral_public_key,
            nonce: sealed.nonce,
        },
        ciphertext: sealed.ciphertext,
        expires_at: now + chrono::Duration::seconds(604800),
        ttl_seconds: 604800,
    };
    let unsigned = UnsignedProtocolObject::new("message.envelope", message_id, now, payload);
    unsigned.sign(&signing_key).map_err(Into::into)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecipientKeyCacheFile {
    version: u8,
    #[serde(default)]
    peers: Vec<CachedRecipientMessageKey>,
}

impl Default for RecipientKeyCacheFile {
    fn default() -> Self {
        Self {
            version: 1,
            peers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedRecipientMessageKey {
    peer_id: String,
    suite: String,
    key_id: String,
    public_key: String,
    updated_at: chrono::DateTime<Utc>,
}

fn cached_recipient_message_key(
    state: &LocalStateDir,
    recipient: &PeerId,
) -> Result<Option<(String, String)>> {
    let path = state.recipient_key_cache_path();
    if !path.exists() {
        return Ok(None);
    }
    let cache = read_recipient_key_cache(state)?;
    Ok(cache
        .peers
        .iter()
        .find(|peer| peer.peer_id == recipient.as_str() && peer.suite == MESSAGE_ENCRYPTION_SUITE)
        .map(|peer| (peer.key_id.clone(), peer.public_key.clone())))
}

fn cache_recipient_message_key(
    state: &LocalStateDir,
    recipient: &PeerId,
    key_id: &str,
    public_key: &str,
) -> Result<()> {
    state.ensure_dirs()?;
    let mut cache = read_recipient_key_cache(state).unwrap_or_default();
    cache.version = 1;
    cache.peers.retain(|peer| {
        !(peer.peer_id == recipient.as_str() && peer.suite == MESSAGE_ENCRYPTION_SUITE)
    });
    cache.peers.push(CachedRecipientMessageKey {
        peer_id: recipient.as_str().to_string(),
        suite: MESSAGE_ENCRYPTION_SUITE.to_string(),
        key_id: key_id.to_string(),
        public_key: public_key.to_string(),
        updated_at: Utc::now(),
    });
    let bytes = serde_json::to_vec_pretty(&cache)?;
    std::fs::write(state.recipient_key_cache_path(), bytes)?;
    Ok(())
}

fn read_recipient_key_cache(state: &LocalStateDir) -> Result<RecipientKeyCacheFile> {
    let path = state.recipient_key_cache_path();
    if !path.exists() {
        return Ok(RecipientKeyCacheFile::default());
    }
    let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    let cache: RecipientKeyCacheFile =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    if cache.version != 1 {
        return Err(anyhow!(
            "unsupported recipient key cache version {}",
            cache.version
        ));
    }
    Ok(cache)
}

fn discover_recipient_message_key(base_url: &str, recipient: &PeerId) -> Result<(String, String)> {
    let response = relay_request("GET", base_url, "/v1/publish/search?limit=100", &[], &[])?;
    if response.status >= 400 {
        return Err(anyhow!(
            "relay returned HTTP {} while discovering recipient: {}",
            response.status,
            response.body_text()
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(&response.body)?;
    extract_recipient_message_key(&value, recipient)
}

fn extract_recipient_message_key(
    value: &serde_json::Value,
    recipient: &PeerId,
) -> Result<(String, String)> {
    for record in value
        .get("records")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        if record
            .pointer("/payload/peer_id")
            .and_then(serde_json::Value::as_str)
            != Some(recipient.as_str())
        {
            continue;
        }
        if let Some(key) = record
            .pointer("/payload/capabilities/message_encryption")
            .and_then(serde_json::Value::as_array)
            .and_then(|keys| {
                keys.iter().find(|key| {
                    key.get("suite").and_then(serde_json::Value::as_str)
                        == Some(MESSAGE_ENCRYPTION_SUITE)
                })
            })
        {
            let key_id = key
                .get("key_id")
                .and_then(serde_json::Value::as_str)
                .context("recipient message key missing key_id")?;
            let public_key = key
                .get("public_key")
                .and_then(serde_json::Value::as_str)
                .context("recipient message key missing public_key")?;
            return Ok((key_id.to_string(), public_key.to_string()));
        }
    }

    Err(anyhow!(
        "could not find message encryption key for recipient {}",
        recipient
    ))
}

fn build_publish_record(
    state: &LocalStateDir,
    body: String,
    tags: Vec<String>,
) -> Result<SignedProtocolObject<PublishRecordPayload>> {
    let identity = IdentityFile::create_or_load(state)?;
    let message_keys = identity.message_key_pair()?;
    let signing_key = identity.signing_key()?;
    let now = Utc::now();
    let payload = PublishRecordPayload {
        peer_id: identity.peer_id,
        public_key: identity.public_key,
        tags: normalize_tags(tags),
        contact_policy: "encrypted_messages".to_string(),
        capabilities: CapabilitySet {
            message_encryption: vec![MessageEncryptionKey {
                suite: MESSAGE_ENCRYPTION_SUITE.to_string(),
                key_id: message_keys.key_id().to_string(),
                public_key: message_keys.public_key().to_string(),
            }],
        },
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
    let started = Instant::now();
    let url = format!("{}{}", base_url.trim_end_matches('/'), path);
    let client = relay_http_client()?;
    let method = reqwest::Method::from_bytes(method.as_bytes())?;
    let mut request = client.request(method.clone(), &url);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    if !body.is_empty() || matches!(method, reqwest::Method::POST | reqwest::Method::PUT) {
        request = request.body(body.to_vec());
    }

    let response = request
        .send()
        .with_context(|| format!("request {} {}", method.as_str(), url))?;
    let status = response.status().as_u16();
    let response_body = response
        .bytes()
        .with_context(|| format!("read response {} {}", method.as_str(), url))?
        .to_vec();
    trace_timing(
        "http.request",
        started,
        &[
            ("method", method.as_str()),
            ("path", path),
            ("status", &status.to_string()),
            ("bytes", &response_body.len().to_string()),
        ],
    );

    Ok(RelayResponse {
        status,
        body: response_body,
    })
}

static RELAY_HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

fn relay_http_client() -> Result<&'static Client> {
    if let Some(client) = RELAY_HTTP_CLIENT.get() {
        return Ok(client);
    }
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("aichan/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("build relay HTTP client")?;
    let _ = RELAY_HTTP_CLIENT.set(client);
    Ok(RELAY_HTTP_CLIENT
        .get()
        .expect("relay HTTP client was just initialized"))
}

fn trace_timing(name: &str, started: Instant, fields: &[(&str, &str)]) {
    if !http_trace_enabled() {
        return;
    }
    let mut parts = vec![
        format!("event={name}"),
        format!("elapsed_ms={}", started.elapsed().as_millis()),
    ];
    parts.extend(fields.iter().map(|(key, value)| format!("{key}={value}")));
    eprintln!("aichan_trace {}", parts.join(" "));
}

fn http_trace_enabled() -> bool {
    matches!(
        std::env::var("AICHAN_TRACE_HTTP").as_deref(),
        Ok("1" | "true" | "TRUE" | "yes" | "YES")
    )
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

fn discover_path(tags: &[String], limit: usize, seed: Option<&str>) -> String {
    let mut path = format!("/v1/discover?limit={}", limit.clamp(1, 25));
    let tags = tags
        .iter()
        .map(|tag| tag.trim())
        .filter(|tag| !tag.is_empty())
        .map(query_escape)
        .collect::<Vec<_>>();
    if !tags.is_empty() {
        path.push_str("&tags=");
        path.push_str(&tags.join(","));
    }
    if let Some(seed) = seed.map(str::trim).filter(|seed| !seed.is_empty()) {
        path.push_str("&seed=");
        path.push_str(&query_escape(seed));
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_path_encodes_tags_limit_and_seed() {
        let path = discover_path(
            &["coding".to_string(), "agent friends".to_string()],
            3,
            Some("abc 123"),
        );

        assert_eq!(
            path,
            "/v1/discover?limit=3&tags=coding,agent+friends&seed=abc+123"
        );
    }
}
