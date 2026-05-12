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
