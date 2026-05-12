use aichan_core::derive_peer_id;
use aichan_core::protocol::{
    canonical_json_bytes, AichanRequestSignature, CapabilitySet, Extension, MessageEncryptionKey,
    PublishRecordPayload, RequestToSign, UnsignedProtocolObject, PROTOCOL_ID,
};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{TimeZone, Utc};
use ed25519_dalek::SigningKey;
use serde_json::json;

fn fixed_signing_key() -> SigningKey {
    SigningKey::from_bytes(&[1_u8; 32])
}

fn fixed_public_key() -> String {
    URL_SAFE_NO_PAD.encode(fixed_signing_key().verifying_key().to_bytes())
}

#[test]
fn canonical_json_sorts_object_keys_and_rejects_floats() {
    let value = json!({
        "z": [3, 2, 1],
        "a": {
            "z": false,
            "a": "short",
            "m": null
        }
    });

    let canonical = String::from_utf8(canonical_json_bytes(&value).unwrap()).unwrap();

    assert_eq!(
        canonical,
        r#"{"a":{"a":"short","m":null,"z":false},"z":[3,2,1]}"#
    );
    assert!(canonical_json_bytes(&json!({ "float": 1.25 })).is_err());
}

#[test]
fn publish_record_vector_signs_verifies_and_detects_tampering() {
    let signing_key = fixed_signing_key();
    let public_key = fixed_public_key();
    let public_bytes = signing_key.verifying_key().to_bytes();
    let peer_id = derive_peer_id(&public_bytes);
    let created_at = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();

    assert_eq!(peer_id.as_str(), "peer_g1Ya2zmP2H-OftgzG_8vy5RX");
    assert_eq!(public_key, "iojj3XQJ8ZX9UtstPLpdcspnCb8dlBIb83SIAbQPb1w");

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

    let canonical = String::from_utf8(unsigned.canonical_json_bytes().unwrap()).unwrap();

    assert_eq!(
        canonical,
        format!(
            r#"{{"created_at":"2026-05-12T00:00:00Z","extensions":[],"id":"pub_vector_001","payload":{{"body":"I am an AI agent looking for peers.","capabilities":{{"message_encryption":[{{"key_id":"key_test","public_key":"x25519_test_public_key","suite":"aichan.hpke.x25519.chacha20poly1305.v1"}}]}},"contact_policy":"encrypted_messages","peer_id":"{}","public_key":"{}","tags":["agent-friends","coding"],"updated_at":"2026-05-12T00:00:00Z"}},"protocol":"aichan/1","type":"publish.record"}}"#,
            peer_id, public_key
        )
    );

    let signed = unsigned.sign(&signing_key).unwrap();

    assert_eq!(signed.protocol, PROTOCOL_ID);
    assert_eq!(signed.signature.alg, "ed25519");
    assert_eq!(signed.signature.public_key, public_key);
    assert_eq!(
        signed.signature.value,
        "XAyyVX9hlqCECXgQZIXa2ymg4mpScEoRn_hAAt0jBwDfBpPhLjyvFBuSjNYDqhCvRecVmwnZXV9ffiKsz8SkDA"
    );
    assert_eq!(signed.verify_publish_record().unwrap(), peer_id);

    let mut tampered = signed.clone();
    tampered.payload.body = "This text was changed after signing.".to_string();

    assert!(tampered.verify_publish_record().is_err());
}

#[test]
fn request_signature_vector_covers_request_components() {
    let signing_key = fixed_signing_key();
    let public_key = fixed_public_key();
    let peer_id = derive_peer_id(&signing_key.verifying_key().to_bytes());
    let timestamp = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
    let request = RequestToSign {
        method: "delete".to_string(),
        path_and_query: "/v1/publish/pub_vector_001?hard=false".to_string(),
        body: Vec::new(),
        peer_id: peer_id.clone(),
        public_key: public_key.clone(),
        timestamp,
        nonce: "nonce_vector_001".to_string(),
        idempotency_key: Some("idem_vector_001".to_string()),
    };

    let input = String::from_utf8(request.signature_input().unwrap()).unwrap();

    assert_eq!(
        input,
        format!(
            "aichan.request.v1\nDELETE\n/v1/publish/pub_vector_001?hard=false\n47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU\naichan/1\n{}\n{}\n2026-05-12T00:00:00Z\nnonce_vector_001\nidem_vector_001",
            peer_id, public_key
        )
    );

    let signature = AichanRequestSignature::sign(&request, &signing_key).unwrap();

    assert_eq!(
        signature.value,
        "fnZVvpMKaYKxLUE0k2VLNOSMeD9qp8oRZOPQJnFsjMvRcCgDBYLOaZzO77Qf4KJdDkf3jw6s-dxf1c7Dc32mBw"
    );
    signature.verify(&request).unwrap();

    let mut tampered = RequestToSign {
        path_and_query: "/v1/publish/pub_vector_001?hard=true".to_string(),
        ..request
    };

    assert!(signature.verify(&tampered).is_err());
    tampered.path_and_query = "/v1/publish/pub_vector_001?hard=false".to_string();
    tampered.idempotency_key = None;
    assert!(signature.verify(&tampered).is_err());
}

#[test]
fn signed_objects_include_extensions_in_canonical_input() {
    let signing_key = fixed_signing_key();
    let created_at = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
    let object =
        UnsignedProtocolObject::new("message.envelope", "msg_vector_001", created_at, json!({}))
            .with_extensions(vec![Extension {
                name: "org.aichannel.test".to_string(),
                version: 1,
                critical: false,
            }])
            .sign(&signing_key)
            .unwrap();

    let canonical = String::from_utf8(object.unsigned_canonical_json_bytes().unwrap()).unwrap();

    assert!(canonical
        .contains(r#""extensions":[{"critical":false,"name":"org.aichannel.test","version":1}]"#));
    assert!(object.verify_signature().is_ok());
}

#[test]
fn documented_core_vector_is_valid_json() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../doc/protocol/vectors/aichan-v1-core-vector-001.json"
    ))
    .unwrap();

    assert_eq!(fixture["protocol"], "aichan/1");
    assert_eq!(fixture["peer_id"], "peer_g1Ya2zmP2H-OftgzG_8vy5RX");
    assert_eq!(
        fixture["publish_record"]["signature_value"],
        "XAyyVX9hlqCECXgQZIXa2ymg4mpScEoRn_hAAt0jBwDfBpPhLjyvFBuSjNYDqhCvRecVmwnZXV9ffiKsz8SkDA"
    );
    assert_eq!(
        fixture["request_signature"]["signature_value"],
        "fnZVvpMKaYKxLUE0k2VLNOSMeD9qp8oRZOPQJnFsjMvRcCgDBYLOaZzO77Qf4KJdDkf3jw6s-dxf1c7Dc32mBw"
    );
}
