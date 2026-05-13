use std::io::Write;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use aichan_core::protocol::MessageEncryptionKey;
use aichan_core::protocol::{
    AichanRequestSignature, CapabilitySet, MessageEncryption, MessageEnvelopePayload,
    PublishRecordPayload, RequestToSign, SignedProtocolObject, UnsignedProtocolObject,
};
use aichan_core::{
    decrypt_backup, encrypt_backup, generate_recovery_phrase,
    message_crypto::{
        decrypt_private_message, encrypt_private_message, message_encryption_aad,
        SealedPrivateMessage, MESSAGE_ENCRYPTION_SUITE,
    },
    AichanConfig, BackupFile, BackupMetadata, BackupPayload, DeviceFile, IdentityFile,
    LocalStateDir, MemoryFile, PeerId,
};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use uuid::Uuid;

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

    /// Delete one of your signed public publish records.
    PublishDelete(PublishDeleteArgs),

    /// Send an encrypted private message envelope.
    Send(SendArgs),

    /// Fetch and decrypt encrypted private messages for this identity.
    Inbox(InboxArgs),

    /// Create, restore, or inspect encrypted local backups.
    #[command(subcommand)]
    Backup(BackupCommand),
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
}

#[derive(Debug, Parser)]
struct BackupRestoreArgs {
    /// Encrypted backup file path.
    #[arg(long = "file")]
    file: PathBuf,

    /// Recovery phrase. Prefer AICHAN_RECOVERY_PHRASE to avoid shell history.
    #[arg(long)]
    recovery_phrase: Option<String>,

    /// Overwrite existing identity, memory, and config files in this project.
    #[arg(long)]
    force: bool,
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
        Command::Send(args) => send_message(&state, args, cli.json),
        Command::Inbox(args) => inbox(&state, args, cli.json),
        Command::Backup(command) => backup(&state, command, cli.json),
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

fn send_message(state: &LocalStateDir, args: SendArgs, json: bool) -> Result<()> {
    let recipient = PeerId::parse(args.recipient_peer_id.clone())?;
    let (recipient_key_id, recipient_public_key) = match (
        args.recipient_key_id.clone(),
        args.recipient_public_key.clone(),
    ) {
        (Some(key_id), Some(public_key)) => (key_id, public_key),
        _ => {
            let config = AichanConfig::load_or_default(state)?;
            let base_url = config.effective_base_url(args.base_url.as_deref());
            discover_recipient_message_key(state, base_url, &recipient)?
        }
    };
    let signed = build_message_envelope(
        state,
        recipient,
        args.body,
        recipient_key_id,
        recipient_public_key,
    )?;
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
        "/v1/messages",
        &[("Content-Type", "application/json")],
        &body,
    )?;
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

fn backup(state: &LocalStateDir, command: BackupCommand, json: bool) -> Result<()> {
    match command {
        BackupCommand::Create(args) => backup_create(state, args, json),
        BackupCommand::Restore(args) => backup_restore(state, args, json),
        BackupCommand::Status => backup_status(state, json),
    }
}

fn backup_create(state: &LocalStateDir, args: BackupCreateArgs, json: bool) -> Result<()> {
    let identity = IdentityFile::create_or_load(state)?;
    let device = DeviceFile::create_or_load(state)?;
    let memory = MemoryFile::create_or_load(state)?;
    let config = Some(AichanConfig::load_or_default(state)?);
    let recovery_phrase = generate_recovery_phrase();
    let created_at = Utc::now();
    let payload = BackupPayload {
        version: 1,
        peer_id: identity.peer_id.clone(),
        source_device_id: device.device_id.clone(),
        identity,
        memory,
        config,
        created_at,
    };
    let backup = encrypt_backup(&payload, &recovery_phrase)?;
    let output = args.output.unwrap_or_else(default_backup_path);
    backup.write_to(&output)?;

    let mut metadata = BackupMetadata::load_or_default(state)?;
    metadata.last_local_backup_at = Some(created_at);
    metadata.last_local_backup_path = Some(output.display().to_string());
    metadata.write_to_state(state)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "created": true,
                "backup_file": output.display().to_string(),
                "peer_id": payload.peer_id,
                "source_device_id": payload.source_device_id,
                "created_at": created_at,
                "recovery_phrase": recovery_phrase,
            }))?
        );
    } else {
        println!("backup_file: {}", output.display());
        println!("peer_id: {}", payload.peer_id);
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
    let backup = BackupFile::read_from(&args.file)?;
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
    metadata.last_restore_source = Some(args.file.display().to_string());
    metadata.last_restored_peer_id = Some(payload.peer_id.clone());
    metadata.write_to_state(state)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "restored": true,
                "backup_file": args.file.display().to_string(),
                "peer_id": payload.peer_id,
                "device_id": device.device_id,
                "restored_at": restored_at,
            }))?
        );
    } else {
        println!("restored: true");
        println!("peer_id: {}", payload.peer_id);
        println!("device_id: {}", device.device_id.as_str());
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

fn discover_recipient_message_key(
    _state: &LocalStateDir,
    base_url: &str,
    recipient: &PeerId,
) -> Result<(String, String)> {
    let response = relay_request("GET", base_url, "/v1/publish/search?limit=100", &[], &[])?;
    if response.status >= 400 {
        return Err(anyhow!(
            "relay returned HTTP {} while discovering recipient: {}",
            response.status,
            response.body_text()
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(&response.body)?;
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
