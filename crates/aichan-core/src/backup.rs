use std::path::Path;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use chrono::{DateTime, Utc};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::config::AichanConfig;
use crate::device::DeviceId;
use crate::error::{io_error, json_error, AichanError, Result};
use crate::identity::{IdentityFile, PeerId};
use crate::memory::MemoryFile;
use crate::state::LocalStateDir;

pub const BACKUP_ENCRYPTION_SUITE: &str = "aichan.backup.chacha20poly1305.hkdf-sha256.v1";
const BACKUP_KDF: &str = "hkdf-sha256";
const RECOVERY_PHRASE_PREFIX: &str = "aichan-rp-";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupFile {
    pub version: u8,
    pub created_at: DateTime<Utc>,
    pub encryption: BackupEncryption,
    pub ciphertext: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupEncryption {
    pub suite: String,
    pub kdf: String,
    pub salt: String,
    pub nonce: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupPayload {
    pub version: u8,
    pub peer_id: PeerId,
    pub source_device_id: DeviceId,
    pub identity: IdentityFile,
    pub memory: MemoryFile,
    pub config: Option<AichanConfig>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_local_backup_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_local_backup_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_restore_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_restore_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_restored_peer_id: Option<PeerId>,
}

impl Default for BackupMetadata {
    fn default() -> Self {
        Self {
            version: 1,
            last_local_backup_at: None,
            last_local_backup_path: None,
            last_restore_at: None,
            last_restore_source: None,
            last_restored_peer_id: None,
        }
    }
}

impl BackupMetadata {
    pub fn load_or_default(state: &LocalStateDir) -> Result<Self> {
        let path = state.backup_metadata_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = std::fs::read(&path).map_err(|source| io_error(&path, source))?;
        let metadata: Self =
            serde_json::from_slice(&bytes).map_err(|source| json_error(&path, source))?;
        metadata.validate()?;
        Ok(metadata)
    }

    pub fn write_to_state(&self, state: &LocalStateDir) -> Result<()> {
        state.ensure_dirs()?;
        let path = state.backup_metadata_path();
        let bytes = serde_json::to_vec_pretty(self).map_err(|source| json_error(&path, source))?;
        std::fs::write(&path, bytes).map_err(|source| io_error(path, source))
    }

    fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(AichanError::InvalidProtocol(format!(
                "unsupported backup metadata version {}",
                self.version
            )));
        }
        Ok(())
    }
}

impl BackupPayload {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(AichanError::InvalidProtocol(format!(
                "unsupported backup payload version {}",
                self.version
            )));
        }
        if self.identity.peer_id != self.peer_id {
            return Err(AichanError::InvalidProtocol(
                "backup peer_id does not match identity peer_id".to_string(),
            ));
        }
        self.identity.signing_key()?;
        self.identity.message_key_pair()?;
        self.memory.validate()?;
        Ok(())
    }
}

pub fn generate_recovery_phrase() -> String {
    let mut bytes = [0_u8; 24];
    OsRng.fill_bytes(&mut bytes);
    format!("{RECOVERY_PHRASE_PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes))
}

pub fn encrypt_backup(payload: &BackupPayload, recovery_phrase: &str) -> Result<BackupFile> {
    payload.validate()?;
    let mut salt = [0_u8; 16];
    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);
    let key = derive_backup_key(recovery_phrase, &salt)?;
    let plaintext = serde_json::to_vec(payload).map_err(|source| {
        AichanError::InvalidProtocol(format!("invalid backup payload: {source}"))
    })?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_ref())
        .map_err(|_| AichanError::InvalidProtocol("backup encryption failed".to_string()))?;

    Ok(BackupFile {
        version: 1,
        created_at: Utc::now(),
        encryption: BackupEncryption {
            suite: BACKUP_ENCRYPTION_SUITE.to_string(),
            kdf: BACKUP_KDF.to_string(),
            salt: URL_SAFE_NO_PAD.encode(salt),
            nonce: URL_SAFE_NO_PAD.encode(nonce),
        },
        ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
    })
}

pub fn decrypt_backup(backup: &BackupFile, recovery_phrase: &str) -> Result<BackupPayload> {
    backup.validate()?;
    let salt = decode_base64url_array::<16>(&backup.encryption.salt, "backup salt")?;
    let nonce = decode_base64url_array::<12>(&backup.encryption.nonce, "backup nonce")?;
    let ciphertext = URL_SAFE_NO_PAD
        .decode(&backup.ciphertext)
        .map_err(|source| {
            AichanError::InvalidProtocol(format!("invalid backup ciphertext encoding: {source}"))
        })?;
    let key = derive_backup_key(recovery_phrase, &salt)?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| AichanError::InvalidProtocol("backup decryption failed".to_string()))?;
    let payload: BackupPayload = serde_json::from_slice(&plaintext).map_err(|source| {
        AichanError::InvalidProtocol(format!("invalid backup payload: {source}"))
    })?;
    payload.validate()?;
    Ok(payload)
}

impl BackupFile {
    pub fn read_from(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| io_error(path, source))?;
        let backup: Self =
            serde_json::from_slice(&bytes).map_err(|source| json_error(path, source))?;
        backup.validate()?;
        Ok(backup)
    }

    pub fn write_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let bytes = serde_json::to_vec_pretty(self).map_err(|source| json_error(path, source))?;
        std::fs::write(path, bytes).map_err(|source| io_error(path, source))
    }

    fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(AichanError::InvalidProtocol(format!(
                "unsupported backup version {}",
                self.version
            )));
        }
        if self.encryption.suite != BACKUP_ENCRYPTION_SUITE {
            return Err(AichanError::InvalidProtocol(format!(
                "unsupported backup encryption suite {}",
                self.encryption.suite
            )));
        }
        if self.encryption.kdf != BACKUP_KDF {
            return Err(AichanError::InvalidProtocol(format!(
                "unsupported backup kdf {}",
                self.encryption.kdf
            )));
        }
        Ok(())
    }
}

fn derive_backup_key(recovery_phrase: &str, salt: &[u8; 16]) -> Result<[u8; 32]> {
    if !recovery_phrase.starts_with(RECOVERY_PHRASE_PREFIX) {
        return Err(AichanError::InvalidProtocol(
            "recovery phrase has invalid format".to_string(),
        ));
    }
    let hk = Hkdf::<Sha256>::new(Some(salt), recovery_phrase.as_bytes());
    let mut key = [0_u8; 32];
    hk.expand(b"aichan backup encryption v1", &mut key)
        .map_err(|_| AichanError::InvalidProtocol("backup key derivation failed".to_string()))?;
    Ok(key)
}

fn decode_base64url_array<const N: usize>(encoded: &str, field: &str) -> Result<[u8; N]> {
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|source| {
        AichanError::InvalidProtocol(format!("invalid {field} encoding: {source}"))
    })?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        AichanError::InvalidProtocol(format!("{field} must be {N} bytes, got {}", bytes.len()))
    })
}
