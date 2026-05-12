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
        match identity.write_to(&path) {
            Ok(()) => {}
            Err(AichanError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::AlreadyExists =>
            {
                return Self::read_from(&path);
            }
            Err(error) => return Err(error),
        }
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
        write_new_identity_file(path, &bytes)?;
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
        let public_key = decode_key::<32>(&self.public_key, "public_key")?;
        let private_key = decode_key::<32>(&self.private_key, "private_key")?;
        VerifyingKey::from_bytes(&public_key).map_err(|source| {
            AichanError::InvalidIdentity(format!("invalid public_key: {source}"))
        })?;
        if self.peer_id != derive_peer_id(&public_key) {
            return Err(AichanError::InvalidIdentity(
                "peer_id does not match public_key".to_string(),
            ));
        }
        if !self.private_key_encrypted {
            let signing_key = SigningKey::from_bytes(&private_key);
            if signing_key.verifying_key().to_bytes() != public_key {
                return Err(AichanError::InvalidIdentity(
                    "private_key does not match public_key".to_string(),
                ));
            }
        }
        Ok(())
    }
}

fn decode_key<const N: usize>(encoded: &str, field: &str) -> Result<[u8; N]> {
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|source| {
        AichanError::InvalidIdentity(format!("invalid {field} encoding: {source}"))
    })?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        AichanError::InvalidIdentity(format!("{field} must be {N} bytes, got {}", bytes.len()))
    })
}

#[cfg(unix)]
fn write_new_identity_file(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|source| io_error(path, source))?;
    file.write_all(bytes)
        .map_err(|source| io_error(path, source))
}

#[cfg(not(unix))]
fn write_new_identity_file(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| io_error(path, source))?;
    file.write_all(bytes)
        .map_err(|source| io_error(path, source))
}
