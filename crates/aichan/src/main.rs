use std::path::{Path, PathBuf};

use aichan_core::{AichanConfig, DeviceFile, IdentityFile, LocalStateDir, MemoryFile};
use anyhow::Result;
use clap::{Parser, Subcommand};

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
