use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, SecondsFormat, Utc};
use ed25519_dalek::{Signature as Ed25519Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::error::{AichanError, Result};
use crate::identity::{derive_peer_id, PeerId};

pub const PROTOCOL_ID: &str = "aichan/1";
pub const JSON_MEDIA_TYPE: &str = "application/aichan+json; version=1";
pub const OBJECT_SIGNATURE_DOMAIN: &str = "aichan.object.v1";
pub const REQUEST_SIGNATURE_DOMAIN: &str = "aichan.request.v1";
pub const ED25519_ALG: &str = "ed25519";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extension {
    pub name: String,
    pub version: u32,
    pub critical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageEncryptionKey {
    pub suite: String,
    pub key_id: String,
    pub public_key: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySet {
    #[serde(default)]
    pub message_encryption: Vec<MessageEncryptionKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishRecordPayload {
    pub peer_id: PeerId,
    pub public_key: String,
    pub tags: Vec<String>,
    pub contact_policy: String,
    pub capabilities: CapabilitySet,
    pub body: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageEncryption {
    pub suite: String,
    pub recipient_key_id: String,
    pub ephemeral_public_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageEnvelopePayload {
    pub sender: PeerId,
    pub recipient: PeerId,
    pub content_encoding: String,
    pub encryption: MessageEncryption,
    pub ciphertext: String,
    pub expires_at: DateTime<Utc>,
    pub ttl_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureBlock {
    pub alg: String,
    pub public_key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedProtocolObject<T> {
    pub protocol: String,
    #[serde(rename = "type")]
    pub object_type: String,
    pub id: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub extensions: Vec<Extension>,
    pub payload: T,
}

impl<T> UnsignedProtocolObject<T> {
    pub fn new(
        object_type: impl Into<String>,
        id: impl Into<String>,
        created_at: DateTime<Utc>,
        payload: T,
    ) -> Self {
        Self {
            protocol: PROTOCOL_ID.to_string(),
            object_type: object_type.into(),
            id: id.into(),
            created_at,
            extensions: Vec::new(),
            payload,
        }
    }

    pub fn with_extensions(mut self, extensions: Vec<Extension>) -> Self {
        self.extensions = extensions;
        self
    }
}

impl<T> UnsignedProtocolObject<T>
where
    T: Serialize,
{
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>> {
        canonical_json_bytes(self)
    }

    pub fn signature_input(&self) -> Result<Vec<u8>> {
        object_signature_input(self)
    }

    pub fn sign(self, signing_key: &SigningKey) -> Result<SignedProtocolObject<T>> {
        let signature_input = self.signature_input()?;
        let signature = signing_key.sign(&signature_input);
        let public_key = signing_key.verifying_key().to_bytes();

        Ok(SignedProtocolObject {
            protocol: self.protocol,
            object_type: self.object_type,
            id: self.id,
            created_at: self.created_at,
            extensions: self.extensions,
            payload: self.payload,
            signature: SignatureBlock {
                alg: ED25519_ALG.to_string(),
                public_key: URL_SAFE_NO_PAD.encode(public_key),
                value: URL_SAFE_NO_PAD.encode(signature.to_bytes()),
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedProtocolObject<T> {
    pub protocol: String,
    #[serde(rename = "type")]
    pub object_type: String,
    pub id: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub extensions: Vec<Extension>,
    pub payload: T,
    pub signature: SignatureBlock,
}

impl<T> SignedProtocolObject<T>
where
    T: Clone + Serialize,
{
    pub fn unsigned(&self) -> UnsignedProtocolObject<T> {
        UnsignedProtocolObject {
            protocol: self.protocol.clone(),
            object_type: self.object_type.clone(),
            id: self.id.clone(),
            created_at: self.created_at,
            extensions: self.extensions.clone(),
            payload: self.payload.clone(),
        }
    }

    pub fn unsigned_canonical_json_bytes(&self) -> Result<Vec<u8>> {
        self.unsigned().canonical_json_bytes()
    }

    pub fn verify_signature(&self) -> Result<PeerId> {
        if self.protocol != PROTOCOL_ID {
            return Err(protocol_error(format!(
                "unsupported protocol {}",
                self.protocol
            )));
        }
        if self.signature.alg != ED25519_ALG {
            return Err(protocol_error(format!(
                "unsupported signature algorithm {}",
                self.signature.alg
            )));
        }

        let public_key = decode_base64url_array::<32>(&self.signature.public_key, "public_key")?;
        let verifying_key = VerifyingKey::from_bytes(&public_key)
            .map_err(|source| protocol_error(format!("invalid public_key: {source}")))?;
        let signature_bytes = decode_base64url_array::<64>(&self.signature.value, "signature")?;
        let signature = Ed25519Signature::from_bytes(&signature_bytes);

        verifying_key
            .verify(&self.unsigned().signature_input()?, &signature)
            .map_err(|source| protocol_error(format!("invalid signature: {source}")))?;

        Ok(derive_peer_id(&public_key))
    }
}

impl SignedProtocolObject<PublishRecordPayload> {
    pub fn verify_publish_record(&self) -> Result<PeerId> {
        if self.object_type != "publish.record" {
            return Err(protocol_error(format!(
                "expected publish.record, got {}",
                self.object_type
            )));
        }

        let signer_peer_id = self.verify_signature()?;
        let payload_public_key =
            decode_base64url_array::<32>(&self.payload.public_key, "public_key")?;
        let payload_peer_id = derive_peer_id(&payload_public_key);

        if self.payload.peer_id != payload_peer_id {
            return Err(protocol_error(
                "payload peer_id does not match payload public_key",
            ));
        }
        if self.signature.public_key != self.payload.public_key {
            return Err(protocol_error(
                "signature public_key does not match payload public_key",
            ));
        }
        if signer_peer_id != self.payload.peer_id {
            return Err(protocol_error(
                "signature public_key does not match payload peer_id",
            ));
        }

        Ok(signer_peer_id)
    }
}

impl SignedProtocolObject<MessageEnvelopePayload> {
    pub fn verify_message_envelope(&self) -> Result<PeerId> {
        if self.object_type != "message.envelope" {
            return Err(protocol_error(format!(
                "expected message.envelope, got {}",
                self.object_type
            )));
        }

        let signer_peer_id = self.verify_signature()?;
        if signer_peer_id != self.payload.sender {
            return Err(protocol_error(
                "signature public_key does not match message sender",
            ));
        }

        Ok(signer_peer_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestToSign {
    pub method: String,
    pub path_and_query: String,
    pub body: Vec<u8>,
    pub peer_id: PeerId,
    pub public_key: String,
    pub timestamp: DateTime<Utc>,
    pub nonce: String,
    pub idempotency_key: Option<String>,
}

impl RequestToSign {
    pub fn body_sha256_base64url(&self) -> String {
        let digest = Sha256::digest(&self.body);
        URL_SAFE_NO_PAD.encode(digest)
    }

    pub fn signature_input(&self) -> Result<Vec<u8>> {
        let public_key = decode_base64url_array::<32>(&self.public_key, "public_key")?;
        if self.peer_id != derive_peer_id(&public_key) {
            return Err(protocol_error("peer_id does not match public_key"));
        }

        let input = [
            REQUEST_SIGNATURE_DOMAIN.to_string(),
            self.method.to_ascii_uppercase(),
            self.path_and_query.clone(),
            self.body_sha256_base64url(),
            PROTOCOL_ID.to_string(),
            self.peer_id.to_string(),
            self.public_key.clone(),
            format_timestamp(self.timestamp),
            self.nonce.clone(),
            self.idempotency_key.clone().unwrap_or_default(),
        ]
        .join("\n");

        Ok(input.into_bytes())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AichanRequestSignature {
    pub protocol: String,
    pub alg: String,
    pub peer_id: PeerId,
    pub public_key: String,
    pub timestamp: DateTime<Utc>,
    pub nonce: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    pub value: String,
}

impl AichanRequestSignature {
    pub fn sign(request: &RequestToSign, signing_key: &SigningKey) -> Result<Self> {
        let signing_public_key = URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_bytes());
        if signing_public_key != request.public_key {
            return Err(protocol_error(
                "request public_key does not match signing key",
            ));
        }

        let signature = signing_key.sign(&request.signature_input()?);
        Ok(Self {
            protocol: PROTOCOL_ID.to_string(),
            alg: ED25519_ALG.to_string(),
            peer_id: request.peer_id.clone(),
            public_key: request.public_key.clone(),
            timestamp: request.timestamp,
            nonce: request.nonce.clone(),
            idempotency_key: request.idempotency_key.clone(),
            value: URL_SAFE_NO_PAD.encode(signature.to_bytes()),
        })
    }

    pub fn verify(&self, request: &RequestToSign) -> Result<()> {
        if self.protocol != PROTOCOL_ID {
            return Err(protocol_error(format!(
                "unsupported protocol {}",
                self.protocol
            )));
        }
        if self.alg != ED25519_ALG {
            return Err(protocol_error(format!(
                "unsupported signature algorithm {}",
                self.alg
            )));
        }
        if self.peer_id != request.peer_id
            || self.public_key != request.public_key
            || self.timestamp != request.timestamp
            || self.nonce != request.nonce
            || self.idempotency_key != request.idempotency_key
        {
            return Err(protocol_error(
                "request signature headers do not match request",
            ));
        }

        let public_key = decode_base64url_array::<32>(&self.public_key, "public_key")?;
        if self.peer_id != derive_peer_id(&public_key) {
            return Err(protocol_error("peer_id does not match public_key"));
        }
        let verifying_key = VerifyingKey::from_bytes(&public_key)
            .map_err(|source| protocol_error(format!("invalid public_key: {source}")))?;
        let signature_bytes = decode_base64url_array::<64>(&self.value, "signature")?;
        let signature = Ed25519Signature::from_bytes(&signature_bytes);

        verifying_key
            .verify(&request.signature_input()?, &signature)
            .map_err(|source| protocol_error(format!("invalid request signature: {source}")))?;
        Ok(())
    }
}

pub fn canonical_json_bytes<T>(value: &T) -> Result<Vec<u8>>
where
    T: Serialize,
{
    let value = serde_json::to_value(value)
        .map_err(|source| protocol_error(format!("cannot serialize protocol value: {source}")))?;
    let canonical = canonicalize_json_value(value)?;
    serde_json::to_vec(&canonical)
        .map_err(|source| protocol_error(format!("cannot encode canonical JSON: {source}")))
}

fn object_signature_input<T>(value: &T) -> Result<Vec<u8>>
where
    T: Serialize,
{
    let mut input = Vec::from(OBJECT_SIGNATURE_DOMAIN.as_bytes());
    input.push(b'\n');
    input.extend(canonical_json_bytes(value)?);
    Ok(input)
}

fn canonicalize_json_value(value: Value) -> Result<Value> {
    match value {
        Value::Null | Value::Bool(_) | Value::String(_) => Ok(value),
        Value::Number(number) => {
            if number.is_f64() {
                return Err(protocol_error(
                    "floating point numbers are not allowed in signed protocol JSON",
                ));
            }
            Ok(Value::Number(number))
        }
        Value::Array(values) => values
            .into_iter()
            .map(canonicalize_json_value)
            .collect::<Result<Vec<_>>>()
            .map(Value::Array),
        Value::Object(object) => {
            let mut entries = object.into_iter().collect::<Vec<_>>();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));

            let mut sorted = Map::new();
            for (key, value) in entries {
                sorted.insert(key, canonicalize_json_value(value)?);
            }
            Ok(Value::Object(sorted))
        }
    }
}

fn decode_base64url_array<const N: usize>(encoded: &str, field: &str) -> Result<[u8; N]> {
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|source| protocol_error(format!("invalid {field} encoding: {source}")))?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        protocol_error(format!("{field} must be {N} bytes, got {}", bytes.len()))
    })
}

fn format_timestamp(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn protocol_error(message: impl Into<String>) -> AichanError {
    AichanError::InvalidProtocol(message.into())
}
