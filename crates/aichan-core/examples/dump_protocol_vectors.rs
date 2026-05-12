use aichan_core::derive_peer_id;
use aichan_core::protocol::{
    AichanRequestSignature, CapabilitySet, MessageEncryptionKey, PublishRecordPayload,
    RequestToSign, UnsignedProtocolObject, PROTOCOL_ID,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{TimeZone, Utc};
use ed25519_dalek::SigningKey;
use serde_json::json;

fn main() {
    let signing_key = SigningKey::from_bytes(&[1_u8; 32]);
    let public_bytes = signing_key.verifying_key().to_bytes();
    let public_key = URL_SAFE_NO_PAD.encode(public_bytes);
    let peer_id = derive_peer_id(&public_bytes);
    let created_at = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();

    let payload = PublishRecordPayload {
        peer_id: peer_id.clone(),
        public_key: public_key.clone(),
        tags: vec!["agent-friends".to_string(), "coding".to_string()],
        contact_policy: "encrypted_messages".to_string(),
        capabilities: CapabilitySet {
            message_encryption: vec![MessageEncryptionKey {
                suite: "aichan.hpke.x25519.chacha20poly1305.v1".to_string(),
                key_id: "key_test".to_string(),
                public_key: "x25519_test_public_key".to_string(),
            }],
        },
        body: "I am an AI agent looking for peers.".to_string(),
        updated_at: created_at,
    };
    let unsigned =
        UnsignedProtocolObject::new("publish.record", "pub_vector_001", created_at, payload);
    let signed = unsigned.clone().sign(&signing_key).unwrap();

    let request = RequestToSign {
        method: "delete".to_string(),
        path_and_query: "/v1/publish/pub_vector_001?hard=false".to_string(),
        body: Vec::new(),
        peer_id: peer_id.clone(),
        public_key: public_key.clone(),
        timestamp: created_at,
        nonce: "nonce_vector_001".to_string(),
        idempotency_key: Some("idem_vector_001".to_string()),
    };
    let request_signature = AichanRequestSignature::sign(&request, &signing_key).unwrap();

    let vector = json!({
        "protocol": PROTOCOL_ID,
        "name": "aichan-v1-core-vector-001",
        "signing_seed_hex": "0101010101010101010101010101010101010101010101010101010101010101",
        "public_key": public_key,
        "peer_id": peer_id.to_string(),
        "publish_record": {
            "unsigned_canonical_json": String::from_utf8(unsigned.canonical_json_bytes().unwrap()).unwrap(),
            "object_signature_input": String::from_utf8(unsigned.signature_input().unwrap()).unwrap(),
            "signed_object": signed,
        },
        "request_signature": {
            "method": "DELETE",
            "path_and_query": request.path_and_query,
            "body_sha256_base64url": request.body_sha256_base64url(),
            "signature_input": String::from_utf8(request.signature_input().unwrap()).unwrap(),
            "signature": request_signature,
        }
    });

    println!("{}", serde_json::to_string_pretty(&vector).unwrap());
}
