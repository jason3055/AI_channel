use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::error::{AichanError, Result};

pub const MESSAGE_ENCRYPTION_SUITE: &str = "aichan.x25519.chacha20poly1305.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageKeyPair {
    key_id: String,
    public_key: String,
    private_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealedPrivateMessage {
    pub suite: String,
    pub recipient_key_id: String,
    pub ephemeral_public_key: String,
    pub nonce: String,
    pub ciphertext: String,
}

impl MessageKeyPair {
    pub fn generate(key_id: impl Into<String>) -> Self {
        let private = StaticSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&private);
        Self {
            key_id: key_id.into(),
            public_key: URL_SAFE_NO_PAD.encode(public.as_bytes()),
            private_key: URL_SAFE_NO_PAD.encode(private.to_bytes()),
        }
    }

    pub fn from_parts(
        key_id: impl Into<String>,
        public_key: impl Into<String>,
        private_key: impl Into<String>,
    ) -> Result<Self> {
        let key_pair = Self {
            key_id: key_id.into(),
            public_key: public_key.into(),
            private_key: private_key.into(),
        };
        key_pair.validate()?;
        Ok(key_pair)
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    pub fn private_key(&self) -> &str {
        &self.private_key
    }

    pub fn validate(&self) -> Result<()> {
        let private = StaticSecret::from(decode_base64url_array::<32>(
            &self.private_key,
            "message_private_key",
        )?);
        let public = decode_base64url_array::<32>(&self.public_key, "message_public_key")?;
        if PublicKey::from(&private).to_bytes() != public {
            return Err(AichanError::InvalidIdentity(
                "message_private_key does not match message_public_key".to_string(),
            ));
        }
        if self.key_id.trim().is_empty() {
            return Err(AichanError::InvalidIdentity(
                "message_key_id must not be empty".to_string(),
            ));
        }
        Ok(())
    }
}

pub fn encrypt_private_message(
    recipient_public_key: &str,
    recipient_key_id: &str,
    plaintext: &[u8],
    aad: &[u8],
) -> Result<SealedPrivateMessage> {
    let recipient_public = PublicKey::from(decode_base64url_array::<32>(
        recipient_public_key,
        "recipient_public_key",
    )?);
    let ephemeral_private = StaticSecret::random_from_rng(OsRng);
    let ephemeral_public = PublicKey::from(&ephemeral_private);
    let shared_secret = ephemeral_private.diffie_hellman(&recipient_public);
    let key = derive_message_key(shared_secret.as_bytes(), aad)?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|_| AichanError::InvalidProtocol("message encryption failed".to_string()))?;

    Ok(SealedPrivateMessage {
        suite: MESSAGE_ENCRYPTION_SUITE.to_string(),
        recipient_key_id: recipient_key_id.to_string(),
        ephemeral_public_key: URL_SAFE_NO_PAD.encode(ephemeral_public.as_bytes()),
        nonce: URL_SAFE_NO_PAD.encode(nonce),
        ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
    })
}

pub fn decrypt_private_message(
    recipient_keys: &MessageKeyPair,
    sealed: &SealedPrivateMessage,
    aad: &[u8],
) -> Result<Vec<u8>> {
    if sealed.suite != MESSAGE_ENCRYPTION_SUITE {
        return Err(AichanError::InvalidProtocol(format!(
            "unsupported message encryption suite {}",
            sealed.suite
        )));
    }
    if sealed.recipient_key_id != recipient_keys.key_id {
        return Err(AichanError::InvalidProtocol(
            "message recipient key id does not match local key".to_string(),
        ));
    }
    let private = StaticSecret::from(decode_base64url_array::<32>(
        &recipient_keys.private_key,
        "message_private_key",
    )?);
    let ephemeral_public = PublicKey::from(decode_base64url_array::<32>(
        &sealed.ephemeral_public_key,
        "ephemeral_public_key",
    )?);
    let nonce = decode_base64url_array::<12>(&sealed.nonce, "message_nonce")?;
    let ciphertext = URL_SAFE_NO_PAD
        .decode(&sealed.ciphertext)
        .map_err(|source| {
            AichanError::InvalidProtocol(format!("invalid message ciphertext encoding: {source}"))
        })?;
    let shared_secret = private.diffie_hellman(&ephemeral_public);
    let key = derive_message_key(shared_secret.as_bytes(), aad)?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| AichanError::InvalidProtocol("message decryption failed".to_string()))
}

pub fn message_encryption_aad(
    message_id: &str,
    sender: &str,
    recipient: &str,
    created_at: &str,
) -> Vec<u8> {
    format!("aichan.message.v1\n{message_id}\n{sender}\n{recipient}\n{created_at}").into_bytes()
}

fn derive_message_key(shared_secret: &[u8; 32], aad: &[u8]) -> Result<[u8; 32]> {
    let hk = Hkdf::<Sha256>::new(Some(b"aichan message encryption v1"), shared_secret);
    let mut key = [0_u8; 32];
    hk.expand(aad, &mut key)
        .map_err(|_| AichanError::InvalidProtocol("message key derivation failed".to_string()))?;
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
