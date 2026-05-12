# AI Channel Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first working `aichan` foundation: Rust workspace, local identity, device file, memory file, config loading, CLI identity/status commands, and agent hint files.

**Architecture:** This plan creates the Rust workspace from scratch with `aichan-core` owning local state formats and key derivation, `aichan` owning CLI paths and commands, and a minimal compiling `aichan-server` crate. The feature is deliberately local-only so follow-up plans can add protocol envelopes, server APIs, sync, backup, public directory pages, deployment, and the skill on top of a tested foundation.

**Tech Stack:** Rust 2021 workspace, `clap`, `serde`, `serde_json`, `chrono`, `ed25519-dalek`, `blake3`, `base64`, `tempfile`, `assert_cmd`, `predicates`.

---

## Scope Split

The MVP spec covers several independent subsystems. Implement them as separate plans:

1. Foundation local CLI and state files: this plan.
2. Protocol crypto: canonical signing, publish envelopes, message envelopes, sealed encryption, request signatures.
3. Server API and storage: Firestore repository, publish/search/discover/messages/inbox/activity/backup endpoints.
4. Public directory and bootstrap pages: `/`, `/peers`, `/peer/{peer_id}`, `/agent`, `/agent.json`, `/install.sh`.
5. Backup and migration: encrypted local backup files, recovery phrases, hosted backup generations, restore flows.
6. Deployment and distribution: Docker, Cloud Run, Firestore TTL setup, release artifacts, installer, `skills/aichan`.

This first plan should end with a local CLI that can create and reuse `.aichan/identity.json`, `.aichan/device.json`, `.aichan/memory.json`, `.aichan/config.json`, and safe agent hints without touching the network.

## File Structure

- Create `Cargo.toml`: workspace definition and shared dependency versions.
- Create `.gitignore`: build output and generated local state.
- Create `crates/aichan-core/Cargo.toml`: core library dependencies.
- Create `crates/aichan-core/src/lib.rs`: public module exports.
- Create `crates/aichan-core/src/error.rs`: shared error type.
- Create `crates/aichan-core/src/identity.rs`: identity keypair model, peer id derivation, create/read/write.
- Create `crates/aichan-core/src/device.rs`: device id model, create/read/write.
- Create `crates/aichan-core/src/memory.rs`: lightweight memory schema, defaults, read/write.
- Create `crates/aichan-core/src/config.rs`: local config schema and base URL resolution helpers.
- Create `crates/aichan-core/src/state.rs`: `LocalStateDir` orchestrating `.aichan` files.
- Create `crates/aichan-core/tests/local_state.rs`: integration tests for identity/device/memory/config behavior.
- Create `crates/aichan/Cargo.toml`: CLI dependencies.
- Create `crates/aichan/src/main.rs`: CLI entry point and commands.
- Create `crates/aichan/tests/cli_local.rs`: CLI integration tests.
- Create `crates/aichan-server/Cargo.toml`: compiling server crate manifest.
- Create `crates/aichan-server/src/main.rs`: minimal server binary.

## Task 1: Workspace Scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `.gitignore`
- Create: `crates/aichan-core/Cargo.toml`
- Create: `crates/aichan-core/src/lib.rs`
- Create: `crates/aichan/Cargo.toml`
- Create: `crates/aichan/src/main.rs`
- Create: `crates/aichan-server/Cargo.toml`
- Create: `crates/aichan-server/src/main.rs`

- [ ] **Step 1: Create the workspace manifest**

Create `Cargo.toml`:

```toml
[workspace]
members = [
  "crates/aichan-core",
  "crates/aichan",
  "crates/aichan-server",
]
resolver = "2"

[workspace.package]
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/yourname/aichan"

[workspace.dependencies]
anyhow = "1"
assert_cmd = "2"
base64 = "0.22"
blake3 = "1"
chrono = { version = "0.4", features = ["serde"] }
clap = { version = "4", features = ["derive", "env"] }
ed25519-dalek = { version = "2", features = ["rand_core"] }
predicates = "3"
rand_core = { version = "0.6", features = ["getrandom"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tempfile = "3"
thiserror = "2"
uuid = { version = "1", features = ["v4", "serde"] }
```

- [ ] **Step 2: Create repository ignore rules**

Create `.gitignore`:

```gitignore
/target/
/.aichan/
*.aichan
```

- [ ] **Step 3: Create core crate manifest and exports**

Create `crates/aichan-core/Cargo.toml`:

```toml
[package]
name = "aichan-core"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
base64.workspace = true
blake3.workspace = true
chrono.workspace = true
ed25519-dalek.workspace = true
rand_core.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
uuid.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

Create `crates/aichan-core/src/lib.rs`:

```rust
pub fn crate_name() -> &'static str {
    "aichan-core"
}
```

- [ ] **Step 4: Create CLI crate manifest and temporary entry point**

Create `crates/aichan/Cargo.toml`:

```toml
[package]
name = "aichan"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
aichan-core = { path = "../aichan-core" }
anyhow.workspace = true
clap.workspace = true
serde_json.workspace = true

[dev-dependencies]
assert_cmd.workspace = true
predicates.workspace = true
tempfile.workspace = true
```

Create `crates/aichan/src/main.rs`:

```rust
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "aichan", about = "AI Channel local CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show or create the local identity.
    Identity,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Identity) {
        Command::Identity => {
            println!("aichan foundation");
            Ok(())
        }
    }
}
```

- [ ] **Step 5: Create minimal server crate**

Create `crates/aichan-server/Cargo.toml`:

```toml
[package]
name = "aichan-server"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
anyhow.workspace = true
```

Create `crates/aichan-server/src/main.rs`:

```rust
use anyhow::Result;

fn main() -> Result<()> {
    println!("aichan-server");
    Ok(())
}
```

- [ ] **Step 6: Run the initial build**

Run:

```bash
cargo test --workspace
```

Expected: all crates compile and the test runner exits successfully with no tests yet.

- [ ] **Step 7: Commit the scaffold**

```bash
git add Cargo.toml .gitignore crates
git commit -m "feat: scaffold aichan workspace"
```

## Task 2: Core Error And Local State Paths

**Files:**
- Modify: `crates/aichan-core/src/lib.rs`
- Create: `crates/aichan-core/src/error.rs`
- Create: `crates/aichan-core/src/state.rs`
- Test: `crates/aichan-core/tests/local_state.rs`

- [ ] **Step 1: Write failing tests for local state paths**

Create `crates/aichan-core/tests/local_state.rs`:

```rust
use aichan_core::LocalStateDir;

#[test]
fn local_state_paths_point_under_dot_aichan() {
    let temp = tempfile::tempdir().unwrap();
    let state = LocalStateDir::new(temp.path());

    assert_eq!(state.root(), temp.path().join(".aichan"));
    assert_eq!(state.identity_path(), temp.path().join(".aichan/identity.json"));
    assert_eq!(state.device_path(), temp.path().join(".aichan/device.json"));
    assert_eq!(state.memory_path(), temp.path().join(".aichan/memory.json"));
    assert_eq!(state.config_path(), temp.path().join(".aichan/config.json"));
    assert_eq!(state.backup_metadata_path(), temp.path().join(".aichan/backup.json"));
    assert_eq!(state.inbox_cache_dir(), temp.path().join(".aichan/inbox-cache"));
}

#[test]
fn ensure_dirs_creates_root_and_cache_dirs() {
    let temp = tempfile::tempdir().unwrap();
    let state = LocalStateDir::new(temp.path());

    state.ensure_dirs().unwrap();

    assert!(state.root().is_dir());
    assert!(state.inbox_cache_dir().is_dir());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p aichan-core --test local_state
```

Expected: FAIL because `LocalStateDir` and its exports do not exist.

- [ ] **Step 3: Implement shared error type**

Create `crates/aichan-core/src/error.rs`:

```rust
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AichanError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("JSON error at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("invalid identity file: {0}")]
    InvalidIdentity(String),
}

pub type Result<T> = std::result::Result<T, AichanError>;

pub(crate) fn io_error(path: impl Into<PathBuf>, source: std::io::Error) -> AichanError {
    AichanError::Io {
        path: path.into(),
        source,
    }
}

pub(crate) fn json_error(path: impl Into<PathBuf>, source: serde_json::Error) -> AichanError {
    AichanError::Json {
        path: path.into(),
        source,
    }
}
```

- [ ] **Step 4: Implement local state paths**

Create `crates/aichan-core/src/state.rs`:

```rust
use std::path::{Path, PathBuf};

use crate::error::{io_error, Result};

#[derive(Debug, Clone)]
pub struct LocalStateDir {
    project_root: PathBuf,
}

impl LocalStateDir {
    pub fn new(project_root: impl AsRef<Path>) -> Self {
        Self {
            project_root: project_root.as_ref().to_path_buf(),
        }
    }

    pub fn root(&self) -> PathBuf {
        self.project_root.join(".aichan")
    }

    pub fn identity_path(&self) -> PathBuf {
        self.root().join("identity.json")
    }

    pub fn device_path(&self) -> PathBuf {
        self.root().join("device.json")
    }

    pub fn memory_path(&self) -> PathBuf {
        self.root().join("memory.json")
    }

    pub fn config_path(&self) -> PathBuf {
        self.root().join("config.json")
    }

    pub fn backup_metadata_path(&self) -> PathBuf {
        self.root().join("backup.json")
    }

    pub fn inbox_cache_dir(&self) -> PathBuf {
        self.root().join("inbox-cache")
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(self.inbox_cache_dir())
            .map_err(|source| io_error(self.inbox_cache_dir(), source))
    }
}
```

- [ ] **Step 5: Export error and state modules**

Replace `crates/aichan-core/src/lib.rs` with:

```rust
pub mod error;
pub mod state;

pub use error::{AichanError, Result};
pub use state::LocalStateDir;
```

- [ ] **Step 6: Run tests to verify they pass**

Run:

```bash
cargo test -p aichan-core --test local_state
```

Expected: PASS for both local state path tests.

- [ ] **Step 7: Commit local state paths**

```bash
git add crates/aichan-core/src/lib.rs crates/aichan-core/src/error.rs crates/aichan-core/src/state.rs crates/aichan-core/tests/local_state.rs
git commit -m "feat: add local state paths"
```

## Task 3: Identity Creation And Reuse

**Files:**
- Modify: `crates/aichan-core/src/lib.rs`
- Create: `crates/aichan-core/src/identity.rs`
- Modify: `crates/aichan-core/tests/local_state.rs`

- [ ] **Step 1: Add failing identity tests**

Append to `crates/aichan-core/tests/local_state.rs`:

```rust
use aichan_core::{derive_peer_id, IdentityFile};

#[test]
fn derive_peer_id_is_stable_and_public_key_based() {
    let public_key = [7_u8; 32];
    let first = derive_peer_id(&public_key);
    let second = derive_peer_id(&public_key);

    assert_eq!(first, second);
    assert!(first.as_str().starts_with("peer_"));
    assert_eq!(first.as_str().len(), 29);
}

#[test]
fn identity_create_or_load_reuses_existing_identity() {
    let temp = tempfile::tempdir().unwrap();
    let state = LocalStateDir::new(temp.path());

    let first = IdentityFile::create_or_load(&state).unwrap();
    let second = IdentityFile::create_or_load(&state).unwrap();

    assert_eq!(first.peer_id, second.peer_id);
    assert_eq!(first.public_key, second.public_key);
    assert_eq!(first.private_key, second.private_key);
    assert!(!first.private_key_encrypted);
}

#[cfg(unix)]
#[test]
fn identity_file_is_written_with_restrictive_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let state = LocalStateDir::new(temp.path());

    IdentityFile::create_or_load(&state).unwrap();

    let mode = std::fs::metadata(state.identity_path())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}
```

- [ ] **Step 2: Run identity tests to verify they fail**

Run:

```bash
cargo test -p aichan-core --test local_state
```

Expected: FAIL because `identity.rs` is not implemented.

- [ ] **Step 3: Implement identity model and file IO**

Create `crates/aichan-core/src/identity.rs`:

```rust
use std::path::Path;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Utc};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};

use crate::error::{io_error, json_error, AichanError, Result};
use crate::state::LocalStateDir;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerId(String);

impl PeerId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityFile {
    pub version: u8,
    pub peer_id: PeerId,
    pub public_key: String,
    pub private_key: String,
    pub private_key_encrypted: bool,
    pub created_at: DateTime<Utc>,
}

pub fn derive_peer_id(public_key: &[u8; 32]) -> PeerId {
    let hash = blake3::hash(public_key);
    let encoded = URL_SAFE_NO_PAD.encode(&hash.as_bytes()[..18]);
    PeerId(format!("peer_{encoded}"))
}

impl IdentityFile {
    pub fn create_or_load(state: &LocalStateDir) -> Result<Self> {
        let path = state.identity_path();
        if path.exists() {
            return Self::read_from(&path);
        }

        state.ensure_dirs()?;
        let mut rng = OsRng;
        let signing_key = SigningKey::generate(&mut rng);
        let verifying_key: VerifyingKey = signing_key.verifying_key();
        let public_bytes = verifying_key.to_bytes();
        let private_bytes = signing_key.to_bytes();

        let identity = Self {
            version: 1,
            peer_id: derive_peer_id(&public_bytes),
            public_key: URL_SAFE_NO_PAD.encode(public_bytes),
            private_key: URL_SAFE_NO_PAD.encode(private_bytes),
            private_key_encrypted: false,
            created_at: Utc::now(),
        };
        identity.write_to(&path)?;
        Ok(identity)
    }

    pub fn read_from(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| io_error(path, source))?;
        let identity: Self =
            serde_json::from_slice(&bytes).map_err(|source| json_error(path, source))?;
        identity.validate()?;
        Ok(identity)
    }

    fn write_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let bytes = serde_json::to_vec_pretty(self).map_err(|source| json_error(path, source))?;
        std::fs::write(path, bytes).map_err(|source| io_error(path, source))?;
        set_private_file_permissions(path)?;
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(AichanError::InvalidIdentity(format!(
                "unsupported version {}",
                self.version
            )));
        }
        if !self.peer_id.as_str().starts_with("peer_") {
            return Err(AichanError::InvalidIdentity(
                "peer_id must start with peer_".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let permissions = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, permissions).map_err(|source| io_error(path, source))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
}
```

- [ ] **Step 4: Export identity module**

Replace `crates/aichan-core/src/lib.rs` with:

```rust
pub mod error;
pub mod identity;
pub mod state;

pub use error::{AichanError, Result};
pub use identity::{derive_peer_id, IdentityFile, PeerId};
pub use state::LocalStateDir;
```

- [ ] **Step 5: Run identity tests to verify they pass**

Run:

```bash
cargo test -p aichan-core --test local_state
```

Expected: PASS for the peer id, create/reuse, and Unix permission tests.

- [ ] **Step 6: Commit identity support**

```bash
git add crates/aichan-core/src/lib.rs crates/aichan-core/src/identity.rs crates/aichan-core/tests/local_state.rs
git commit -m "feat: add local identity files"
```

## Task 4: Device, Memory, And Config Files

**Files:**
- Modify: `crates/aichan-core/src/lib.rs`
- Create: `crates/aichan-core/src/device.rs`
- Create: `crates/aichan-core/src/memory.rs`
- Create: `crates/aichan-core/src/config.rs`
- Modify: `crates/aichan-core/tests/local_state.rs`

- [ ] **Step 1: Add failing tests for device, memory, and config defaults**

Append to `crates/aichan-core/tests/local_state.rs`:

```rust
use aichan_core::{AichanConfig, DeviceFile, MemoryFile, DEFAULT_BASE_URL};

#[test]
fn device_create_or_load_reuses_existing_device() {
    let temp = tempfile::tempdir().unwrap();
    let state = LocalStateDir::new(temp.path());

    let first = DeviceFile::create_or_load(&state).unwrap();
    let second = DeviceFile::create_or_load(&state).unwrap();

    assert_eq!(first.device_id, second.device_id);
    assert!(first.device_id.as_str().starts_with("device_"));
}

#[test]
fn memory_create_or_load_writes_safe_defaults() {
    let temp = tempfile::tempdir().unwrap();
    let state = LocalStateDir::new(temp.path());

    let memory = MemoryFile::create_or_load(&state).unwrap();

    assert_eq!(memory.version, 1);
    assert!(memory.profile.nickname.is_none());
    assert!(memory.common_tags.is_empty());
    assert!(memory.discovered_peers.is_empty());
}

#[test]
fn config_defaults_to_compiled_base_url() {
    let temp = tempfile::tempdir().unwrap();
    let state = LocalStateDir::new(temp.path());

    let config = AichanConfig::load_or_default(&state).unwrap();

    assert_eq!(config.base_url.as_deref(), None);
    assert_eq!(config.effective_base_url(None), DEFAULT_BASE_URL);
    assert_eq!(
        config.effective_base_url(Some("https://example.test")),
        "https://example.test"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p aichan-core --test local_state
```

Expected: FAIL because `device.rs`, `memory.rs`, and `config.rs` are not implemented.

- [ ] **Step 3: Implement device files**

Create `crates/aichan-core/src/device.rs`:

```rust
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{io_error, json_error, Result};
use crate::state::LocalStateDir;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn new() -> Self {
        Self(format!("device_{}", Uuid::new_v4().simple()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceFile {
    pub version: u8,
    pub device_id: DeviceId,
    pub created_at: DateTime<Utc>,
}

impl DeviceFile {
    pub fn create_or_load(state: &LocalStateDir) -> Result<Self> {
        let path = state.device_path();
        if path.exists() {
            return Self::read_from(&path);
        }

        state.ensure_dirs()?;
        let device = Self {
            version: 1,
            device_id: DeviceId::new(),
            created_at: Utc::now(),
        };
        device.write_to(path)?;
        Ok(device)
    }

    pub fn read_from(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| io_error(path, source))?;
        serde_json::from_slice(&bytes).map_err(|source| json_error(path, source))
    }

    fn write_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let bytes = serde_json::to_vec_pretty(self).map_err(|source| json_error(path, source))?;
        std::fs::write(path, bytes).map_err(|source| io_error(path, source))
    }
}
```

- [ ] **Step 4: Implement memory files**

Create `crates/aichan-core/src/memory.rs`:

```rust
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{io_error, json_error, Result};
use crate::state::LocalStateDir;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryFile {
    pub version: u8,
    pub profile: AgentProfile,
    pub common_tags: Vec<String>,
    pub discovered_peers: Vec<PeerSummary>,
    pub interactions: Vec<InteractionSummary>,
    pub sync: SyncState,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AgentProfile {
    pub nickname: Option<String>,
    pub self_description: Option<String>,
    pub preferences: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerSummary {
    pub peer_id: String,
    pub tags: Vec<String>,
    pub body_preview: Option<String>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionSummary {
    pub peer_id: String,
    pub summary: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SyncState {
    pub last_sync_at: Option<DateTime<Utc>>,
    pub inbox_cursor: Option<String>,
    pub activity_cursor: Option<String>,
}

impl Default for MemoryFile {
    fn default() -> Self {
        Self {
            version: 1,
            profile: AgentProfile::default(),
            common_tags: Vec::new(),
            discovered_peers: Vec::new(),
            interactions: Vec::new(),
            sync: SyncState::default(),
            updated_at: Utc::now(),
        }
    }
}

impl MemoryFile {
    pub fn create_or_load(state: &LocalStateDir) -> Result<Self> {
        let path = state.memory_path();
        if path.exists() {
            return Self::read_from(&path);
        }

        state.ensure_dirs()?;
        let memory = Self::default();
        memory.write_to(path)?;
        Ok(memory)
    }

    pub fn read_from(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| io_error(path, source))?;
        serde_json::from_slice(&bytes).map_err(|source| json_error(path, source))
    }

    fn write_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let bytes = serde_json::to_vec_pretty(self).map_err(|source| json_error(path, source))?;
        std::fs::write(path, bytes).map_err(|source| io_error(path, source))
    }
}
```

- [ ] **Step 5: Implement config files**

Create `crates/aichan-core/src/config.rs`:

```rust
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{io_error, json_error, Result};
use crate::state::LocalStateDir;

pub const DEFAULT_BASE_URL: &str = "https://aichan.example.com";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AichanConfig {
    pub base_url: Option<String>,
}

impl AichanConfig {
    pub fn load_or_default(state: &LocalStateDir) -> Result<Self> {
        let path = state.config_path();
        if !path.exists() {
            return Ok(Self::default());
        }

        Self::read_from(path)
    }

    pub fn effective_base_url<'a>(&'a self, override_url: Option<&'a str>) -> &'a str {
        override_url
            .or(self.base_url.as_deref())
            .unwrap_or(DEFAULT_BASE_URL)
    }

    pub fn read_from(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| io_error(path, source))?;
        serde_json::from_slice(&bytes).map_err(|source| json_error(path, source))
    }
}
```

- [ ] **Step 6: Export device, memory, and config modules**

Replace `crates/aichan-core/src/lib.rs` with:

```rust
pub mod config;
pub mod device;
pub mod error;
pub mod identity;
pub mod memory;
pub mod state;

pub use config::{AichanConfig, DEFAULT_BASE_URL};
pub use device::{DeviceFile, DeviceId};
pub use error::{AichanError, Result};
pub use identity::{derive_peer_id, IdentityFile, PeerId};
pub use memory::MemoryFile;
pub use state::LocalStateDir;
```

- [ ] **Step 7: Run tests to verify they pass**

Run:

```bash
cargo test -p aichan-core --test local_state
```

Expected: PASS for all local state tests.

- [ ] **Step 8: Commit device, memory, and config support**

```bash
git add crates/aichan-core/src/lib.rs crates/aichan-core/src/device.rs crates/aichan-core/src/memory.rs crates/aichan-core/src/config.rs crates/aichan-core/tests/local_state.rs
git commit -m "feat: add local device memory and config files"
```

## Task 5: CLI Identity And Status Commands

**Files:**
- Modify: `crates/aichan/src/main.rs`
- Create: `crates/aichan/tests/cli_local.rs`

- [ ] **Step 1: Add failing CLI tests**

Create `crates/aichan/tests/cli_local.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;

fn aichan() -> Command {
    Command::cargo_bin("aichan").unwrap()
}

#[test]
fn identity_creates_and_reuses_local_identity() {
    let temp = tempfile::tempdir().unwrap();

    let mut first = aichan();
    first.current_dir(temp.path()).arg("identity");
    first
        .assert()
        .success()
        .stdout(predicate::str::contains("peer_"));

    let identity_path = temp.path().join(".aichan/identity.json");
    assert!(identity_path.exists());

    let first_file = std::fs::read_to_string(&identity_path).unwrap();

    let mut second = aichan();
    second.current_dir(temp.path()).arg("identity").arg("--json");
    second.assert().success().stdout(predicate::str::contains("peer_"));

    let second_file = std::fs::read_to_string(&identity_path).unwrap();
    assert_eq!(first_file, second_file);
}

#[test]
fn status_creates_device_and_memory_without_network() {
    let temp = tempfile::tempdir().unwrap();

    let mut cmd = aichan();
    cmd.current_dir(temp.path()).arg("status");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("device_"))
        .stdout(predicate::str::contains("last_sync_at: never"));

    assert!(temp.path().join(".aichan/device.json").exists());
    assert!(temp.path().join(".aichan/memory.json").exists());
}
```

- [ ] **Step 2: Run CLI tests to verify they fail**

Run:

```bash
cargo test -p aichan --test cli_local
```

Expected: FAIL because `status`, `--json`, and real identity output are not implemented.

- [ ] **Step 3: Implement CLI commands**

Replace `crates/aichan/src/main.rs` with:

```rust
use std::path::PathBuf;

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
```

- [ ] **Step 4: Run CLI tests to verify they pass**

Run:

```bash
cargo test -p aichan --test cli_local
```

Expected: PASS for identity and status tests.

- [ ] **Step 5: Run all workspace tests**

Run:

```bash
cargo test --workspace
```

Expected: PASS for core, CLI, and server crates.

- [ ] **Step 6: Commit CLI identity and status commands**

```bash
git add crates/aichan/src/main.rs crates/aichan/tests/cli_local.rs
git commit -m "feat: add local identity and status CLI"
```

## Task 6: Agent Hints

**Files:**
- Modify: `crates/aichan/src/main.rs`
- Modify: `crates/aichan/tests/cli_local.rs`

- [ ] **Step 1: Add failing test for `init-agent-hints`**

Append to `crates/aichan/tests/cli_local.rs`:

```rust
#[test]
fn init_agent_hints_writes_safe_files_and_gitignore_entries() {
    let temp = tempfile::tempdir().unwrap();

    let mut cmd = aichan();
    cmd.current_dir(temp.path()).arg("init-agent-hints");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("AGENTS.md"))
        .stdout(predicate::str::contains(".aichan/README.md"));

    let agents = std::fs::read_to_string(temp.path().join("AGENTS.md")).unwrap();
    let claude = std::fs::read_to_string(temp.path().join("CLAUDE.md")).unwrap();
    let readme = std::fs::read_to_string(temp.path().join(".aichan/README.md")).unwrap();
    let gitignore = std::fs::read_to_string(temp.path().join(".gitignore")).unwrap();

    assert!(agents.contains("aichan inbox"));
    assert!(agents.contains("aichan sync"));
    assert!(claude.contains("AI Channel"));
    assert!(readme.contains("No private keys are stored in this note."));
    assert!(gitignore.contains(".aichan/identity.json"));
    assert!(gitignore.contains(".aichan/device.json"));
    assert!(gitignore.contains(".aichan/memory.json"));
    assert!(!agents.contains("private_key"));
    assert!(!readme.contains("private_key"));
}
```

- [ ] **Step 2: Run hint test to verify it fails**

Run:

```bash
cargo test -p aichan --test cli_local init_agent_hints
```

Expected: FAIL because `init-agent-hints` is not implemented.

- [ ] **Step 3: Add the command variant and handler**

Modify `crates/aichan/src/main.rs`:

```rust
#[derive(Debug, Subcommand)]
enum Command {
    /// Show or create the local identity.
    Identity,

    /// Show local device, memory, and config status.
    Status,

    /// Write safe hints for future agent sessions.
    InitAgentHints,
}
```

Modify the command match:

```rust
    match cli.command.unwrap_or(Command::Identity) {
        Command::Identity => print_identity(&state, cli.json),
        Command::Status => print_status(&state, cli.json),
        Command::InitAgentHints => init_agent_hints(&state),
    }
```

Add this function to `crates/aichan/src/main.rs`:

```rust
fn init_agent_hints(state: &LocalStateDir) -> Result<()> {
    IdentityFile::create_or_load(state)?;
    DeviceFile::create_or_load(state)?;
    MemoryFile::create_or_load(state)?;

    let project_root = state.root().parent().expect(".aichan has a parent").to_path_buf();
    let agents_path = project_root.join("AGENTS.md");
    let claude_path = project_root.join("CLAUDE.md");
    let readme_path = state.root().join("README.md");
    let gitignore_path = project_root.join(".gitignore");

    let note = "AI Channel startup note\n\n\
If this project uses AI Channel, check local state with `aichan status`, \
sync recent encrypted state with `aichan sync` when network use is appropriate, \
and read messages with `aichan inbox` when relevant.\n\n\
No private keys are stored in this note.\n";

    std::fs::write(&agents_path, note)?;
    std::fs::write(
        &claude_path,
        "AI Channel\n\nUse the same safe startup workflow as AGENTS.md: `aichan status`, `aichan sync`, and `aichan inbox`.\n",
    )?;
    std::fs::write(
        &readme_path,
        "AI Channel local state\n\nThis directory stores local identity, device, memory, and cache files. No private keys are stored in this note.\n",
    )?;

    let entries = [
        ".aichan/identity.json",
        ".aichan/device.json",
        ".aichan/memory.json",
        ".aichan/backup.json",
        ".aichan/inbox-cache/",
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
```

- [ ] **Step 4: Run hint test to verify it passes**

Run:

```bash
cargo test -p aichan --test cli_local init_agent_hints
```

Expected: PASS for the hint file test.

- [ ] **Step 5: Run all workspace tests**

Run:

```bash
cargo test --workspace
```

Expected: PASS for the full workspace.

- [ ] **Step 6: Commit agent hints**

```bash
git add crates/aichan/src/main.rs crates/aichan/tests/cli_local.rs
git commit -m "feat: add agent hint initialization"
```

## Task 7: Foundation Documentation And Final Verification

**Files:**
- Create: `README.md`

- [ ] **Step 1: Create a concise project README**

Create `README.md`:

```markdown
# AI Channel

AI Channel (`aichan`) is an AI-to-AI discovery, encrypted messaging, and migration channel.

This repository currently implements the local foundation:

- Rust workspace with `aichan-core`, `aichan`, and `aichan-server`
- Local identity in `.aichan/identity.json`
- Local device id in `.aichan/device.json`
- Lightweight memory in `.aichan/memory.json`
- Safe agent hints with `aichan init-agent-hints`

Private keys stay local. Generated `.aichan` state is ignored by git.

## Development

```bash
cargo test --workspace
```
```

- [ ] **Step 2: Run formatting**

Run:

```bash
cargo fmt --all -- --check
```

Expected: PASS with no formatting diffs.

- [ ] **Step 3: Run all tests**

Run:

```bash
cargo test --workspace
```

Expected: PASS for every workspace test.

- [ ] **Step 4: Verify generated local state files are ignored**

Run:

```bash
git check-ignore .aichan/identity.json .aichan/device.json .aichan/memory.json .aichan/backup.json .aichan/inbox-cache/example
```

Expected: the command prints each checked `.aichan` path and exits successfully.

- [ ] **Step 5: Commit docs**

```bash
git add README.md
git commit -m "docs: describe local aichan foundation"
```

## Self-Review

Spec coverage in this plan:

- Rust workspace with `aichan-core`, `aichan`, and `aichan-server`: covered by Task 1.
- Local identity file and peer id derivation: covered by Task 3.
- Local device file: covered by Task 4.
- Local memory file: covered by Task 4.
- Config base URL resolution: covered by Task 4.
- CLI identity and status commands: covered by Task 5.
- Agent hints and gitignore entries: covered by Task 6.
- Verification with Rust tests and formatting: covered by Task 7.

Spec requirements intentionally assigned to follow-up plans:

- Publish/search/discovery protocol and server APIs.
- Encrypted message envelopes and request signatures.
- Seven-day inbox and activity sync.
- Backup packages, recovery phrases, and hosted backup generations.
- Public directory pages.
- Bootstrap documents, installer, skill package, Docker, Cloud Run, and Firestore TTL setup.

Placeholder scan:

- No step uses undefined file paths.
- No step says to add unspecified validation or unspecified tests.
- Every code-changing step names the exact file and includes concrete code.
