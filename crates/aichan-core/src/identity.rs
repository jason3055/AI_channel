use std::path::Path;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Utc};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{io_error, json_error, AichanError, Result};
use crate::message_crypto::MessageKeyPair;
use crate::state::LocalStateDir;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerId(String);

impl PeerId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn parse(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if !value.starts_with("peer_") {
            return Err(AichanError::InvalidIdentity(
                "peer_id must start with peer_".to_string(),
            ));
        }
        Ok(Self(value))
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_public_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_private_key: Option<String>,
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
            let mut identity = Self::read_from(&path)?;
            if identity.ensure_message_key_pair() {
                identity.write_replace(&path)?;
            }
            return Ok(identity);
        }

        state.ensure_dirs()?;
        let mut rng = OsRng;
        let signing_key = SigningKey::generate(&mut rng);
        let verifying_key: VerifyingKey = signing_key.verifying_key();
        let public_bytes = verifying_key.to_bytes();
        let private_bytes = signing_key.to_bytes();

        let message_keys = MessageKeyPair::generate(format!("key_{}", Uuid::new_v4().simple()));
        let identity = Self {
            version: 1,
            peer_id: derive_peer_id(&public_bytes),
            public_key: URL_SAFE_NO_PAD.encode(public_bytes),
            private_key: URL_SAFE_NO_PAD.encode(private_bytes),
            private_key_encrypted: false,
            message_key_id: Some(message_keys.key_id().to_string()),
            message_public_key: Some(message_keys.public_key().to_string()),
            message_private_key: Some(message_keys.private_key().to_string()),
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

    fn write_replace(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let bytes = serde_json::to_vec_pretty(self).map_err(|source| json_error(path, source))?;
        write_identity_file_replace(path, &bytes)
    }

    pub fn write_replace_to(&self, path: impl AsRef<Path>) -> Result<()> {
        self.validate()?;
        self.write_replace(path)
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
        match (
            &self.message_key_id,
            &self.message_public_key,
            &self.message_private_key,
        ) {
            (None, None, None) => {}
            (Some(key_id), Some(public_key), Some(private_key)) => {
                MessageKeyPair::from_parts(key_id, public_key, private_key)?;
            }
            _ => {
                return Err(AichanError::InvalidIdentity(
                    "message key fields must be all present or all absent".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn ensure_message_key_pair(&mut self) -> bool {
        if self.message_key_pair().is_ok() {
            return false;
        }
        let message_keys = MessageKeyPair::generate(format!("key_{}", Uuid::new_v4().simple()));
        self.message_key_id = Some(message_keys.key_id().to_string());
        self.message_public_key = Some(message_keys.public_key().to_string());
        self.message_private_key = Some(message_keys.private_key().to_string());
        true
    }

    pub fn signing_key(&self) -> Result<SigningKey> {
        if self.private_key_encrypted {
            return Err(AichanError::InvalidIdentity(
                "encrypted private keys are not supported by this CLI yet".to_string(),
            ));
        }

        let private_key = decode_key::<32>(&self.private_key, "private_key")?;
        let signing_key = SigningKey::from_bytes(&private_key);
        let public_key = decode_key::<32>(&self.public_key, "public_key")?;
        if signing_key.verifying_key().to_bytes() != public_key {
            return Err(AichanError::InvalidIdentity(
                "private_key does not match public_key".to_string(),
            ));
        }

        Ok(signing_key)
    }

    pub fn message_key_pair(&self) -> Result<MessageKeyPair> {
        MessageKeyPair::from_parts(
            self.message_key_id.as_deref().ok_or_else(|| {
                AichanError::InvalidIdentity("missing message_key_id".to_string())
            })?,
            self.message_public_key.as_deref().ok_or_else(|| {
                AichanError::InvalidIdentity("missing message_public_key".to_string())
            })?,
            self.message_private_key.as_deref().ok_or_else(|| {
                AichanError::InvalidIdentity("missing message_private_key".to_string())
            })?,
        )
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

#[cfg(unix)]
fn write_identity_file_replace(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
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

#[cfg(not(unix))]
fn write_identity_file_replace(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)
        .map_err(|source| io_error(path, source))?;
    file.write_all(bytes)
        .map_err(|source| io_error(path, source))
}
