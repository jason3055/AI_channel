use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use chrono::{DateTime, Utc};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use crate::device::DeviceId;
use crate::error::{AichanError, Result};
use crate::identity::IdentityFile;
use crate::memory::MemoryFile;

pub const ACTIVITY_ENCRYPTION_SUITE: &str = "aichan.activity.chacha20poly1305.hkdf-sha256.v1";
pub const ACTIVITY_CONTENT_ENCODING: &str = "application/aichan+json; version=1";
const ACTIVITY_KDF: &str = "hkdf-sha256";
const ACTIVITY_DERIVATION_SALT: &[u8] = b"aichan.activity.v1";
const ACTIVITY_BUCKET_INFO: &[u8] = b"aichan.activity.v1.bucket_id";
const ACTIVITY_AUTH_INFO: &[u8] = b"aichan.activity.v1.auth_token";
const ACTIVITY_ENCRYPTION_INFO: &[u8] = b"aichan.activity.v1.encryption_key";
const ACTIVITY_TTL_SECONDS: i64 = 604_800;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityLocator {
    pub bucket_id: String,
    pub auth_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityEvent {
    pub version: u8,
    pub event_id: String,
    pub source_device_id: DeviceId,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub content_encoding: String,
    pub encryption: ActivityEncryption,
    pub ciphertext: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityEncryption {
    pub suite: String,
    pub kdf: String,
    pub salt: String,
    pub nonce: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityPayload {
    pub version: u8,
    pub memory: MemoryFile,
    pub created_at: DateTime<Utc>,
}

pub fn derive_activity_locator(identity: &IdentityFile) -> Result<ActivityLocator> {
    let seed = identity.signing_key()?.to_bytes();
    let hk = Hkdf::<Sha256>::new(Some(ACTIVITY_DERIVATION_SALT), &seed);
    let mut bucket_bytes = [0_u8; 24];
    let mut auth_bytes = [0_u8; 32];
    hk.expand(ACTIVITY_BUCKET_INFO, &mut bucket_bytes)
        .map_err(|_| {
            AichanError::InvalidProtocol("activity bucket derivation failed".to_string())
        })?;
    hk.expand(ACTIVITY_AUTH_INFO, &mut auth_bytes)
        .map_err(|_| AichanError::InvalidProtocol("activity auth derivation failed".to_string()))?;

    Ok(ActivityLocator {
        bucket_id: format!("sync_{}", URL_SAFE_NO_PAD.encode(bucket_bytes)),
        auth_token: format!("auth_{}", URL_SAFE_NO_PAD.encode(auth_bytes)),
    })
}

pub fn encrypt_activity_snapshot(
    identity: &IdentityFile,
    source_device_id: DeviceId,
    memory: &MemoryFile,
) -> Result<ActivityEvent> {
    memory.validate()?;
    let now = Utc::now();
    let event_id = format!("act_{}", Uuid::new_v4().simple());
    let payload = ActivityPayload {
        version: 1,
        memory: memory.clone(),
        created_at: now,
    };
    let plaintext = serde_json::to_vec(&payload).map_err(|source| {
        AichanError::InvalidProtocol(format!("invalid activity payload: {source}"))
    })?;
    let mut salt = [0_u8; 16];
    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);
    let key = derive_activity_key(identity, &salt)?;
    let aad = activity_aad(&event_id, source_device_id.as_str(), now);
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            chacha20poly1305::aead::Payload {
                msg: plaintext.as_ref(),
                aad: aad.as_ref(),
            },
        )
        .map_err(|_| AichanError::InvalidProtocol("activity encryption failed".to_string()))?;

    Ok(ActivityEvent {
        version: 1,
        event_id,
        source_device_id,
        created_at: now,
        expires_at: now + chrono::Duration::seconds(ACTIVITY_TTL_SECONDS),
        content_encoding: ACTIVITY_CONTENT_ENCODING.to_string(),
        encryption: ActivityEncryption {
            suite: ACTIVITY_ENCRYPTION_SUITE.to_string(),
            kdf: ACTIVITY_KDF.to_string(),
            salt: URL_SAFE_NO_PAD.encode(salt),
            nonce: URL_SAFE_NO_PAD.encode(nonce),
        },
        ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
    })
}

pub fn decrypt_activity_snapshot(
    identity: &IdentityFile,
    event: &ActivityEvent,
) -> Result<ActivityPayload> {
    event.validate()?;
    let salt = decode_base64url_array::<16>(&event.encryption.salt, "activity salt")?;
    let nonce = decode_base64url_array::<12>(&event.encryption.nonce, "activity nonce")?;
    let ciphertext = URL_SAFE_NO_PAD
        .decode(&event.ciphertext)
        .map_err(|source| {
            AichanError::InvalidProtocol(format!("invalid activity ciphertext encoding: {source}"))
        })?;
    let key = derive_activity_key(identity, &salt)?;
    let aad = activity_aad(
        &event.event_id,
        event.source_device_id.as_str(),
        event.created_at,
    );
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            chacha20poly1305::aead::Payload {
                msg: ciphertext.as_ref(),
                aad: aad.as_ref(),
            },
        )
        .map_err(|_| AichanError::InvalidProtocol("activity decryption failed".to_string()))?;
    let payload: ActivityPayload = serde_json::from_slice(&plaintext).map_err(|source| {
        AichanError::InvalidProtocol(format!("invalid activity payload: {source}"))
    })?;
    if payload.version != 1 {
        return Err(AichanError::InvalidProtocol(format!(
            "unsupported activity payload version {}",
            payload.version
        )));
    }
    payload.memory.validate()?;
    Ok(payload)
}

impl ActivityEvent {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(AichanError::InvalidProtocol(format!(
                "unsupported activity event version {}",
                self.version
            )));
        }
        if !self.event_id.starts_with("act_") {
            return Err(AichanError::InvalidProtocol(
                "activity event_id must start with act_".to_string(),
            ));
        }
        if self.expires_at <= self.created_at {
            return Err(AichanError::InvalidProtocol(
                "activity expires_at must be after created_at".to_string(),
            ));
        }
        if self.expires_at - self.created_at > chrono::Duration::seconds(ACTIVITY_TTL_SECONDS) {
            return Err(AichanError::InvalidProtocol(
                "activity ttl exceeds seven-day sync window".to_string(),
            ));
        }
        if self.content_encoding != ACTIVITY_CONTENT_ENCODING {
            return Err(AichanError::InvalidProtocol(format!(
                "unsupported activity content encoding {}",
                self.content_encoding
            )));
        }
        if self.encryption.suite != ACTIVITY_ENCRYPTION_SUITE {
            return Err(AichanError::InvalidProtocol(format!(
                "unsupported activity encryption suite {}",
                self.encryption.suite
            )));
        }
        if self.encryption.kdf != ACTIVITY_KDF {
            return Err(AichanError::InvalidProtocol(format!(
                "unsupported activity kdf {}",
                self.encryption.kdf
            )));
        }
        Ok(())
    }
}

fn derive_activity_key(identity: &IdentityFile, salt: &[u8; 16]) -> Result<[u8; 32]> {
    let seed = identity.signing_key()?.to_bytes();
    let hk = Hkdf::<Sha256>::new(Some(salt), &seed);
    let mut key = [0_u8; 32];
    hk.expand(ACTIVITY_ENCRYPTION_INFO, &mut key)
        .map_err(|_| AichanError::InvalidProtocol("activity key derivation failed".to_string()))?;
    Ok(key)
}

fn activity_aad(event_id: &str, source_device_id: &str, created_at: DateTime<Utc>) -> Vec<u8> {
    format!(
        "aichan.activity.v1\n{event_id}\n{source_device_id}\n{}",
        created_at.to_rfc3339()
    )
    .into_bytes()
}

fn decode_base64url_array<const N: usize>(encoded: &str, field: &str) -> Result<[u8; N]> {
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|source| {
        AichanError::InvalidProtocol(format!("invalid {field} encoding: {source}"))
    })?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        AichanError::InvalidProtocol(format!("{field} must be {N} bytes, got {}", bytes.len()))
    })
}
