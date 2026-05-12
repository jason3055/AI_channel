use aichan_core::derive_peer_id;
use aichan_core::protocol::{
    AichanRequestSignature, CapabilitySet, PublishRecordPayload, RequestToSign,
    SignedProtocolObject, UnsignedProtocolObject,
};
use aichan_server::{handle_request, HttpRequest, RateLimitConfig, ServerState};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{TimeZone, Utc};
use ed25519_dalek::SigningKey;
use serde_json::json;

fn signed_publish() -> (SigningKey, SignedProtocolObject<PublishRecordPayload>) {
    let signing_key = SigningKey::from_bytes(&[3_u8; 32]);
    let public_key = URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_bytes());
    let peer_id = derive_peer_id(&signing_key.verifying_key().to_bytes());
    let created_at = Utc.with_ymd_and_hms(2026, 5, 12, 1, 2, 3).unwrap();
    let payload = PublishRecordPayload {
        peer_id,
        public_key,
        tags: vec!["coding".to_string(), "agent-friends".to_string()],
        contact_policy: "encrypted_messages".to_string(),
        capabilities: CapabilitySet::default(),
        body: "hello public relay".to_string(),
        updated_at: created_at,
    };
    let object = UnsignedProtocolObject::new("publish.record", "pub_test_001", created_at, payload)
        .sign(&signing_key)
        .unwrap();

    (signing_key, object)
}

#[test]
fn publish_search_and_author_delete_round_trip() {
    let temp = tempfile::tempdir().unwrap();
    let state = ServerState::new(temp.path()).unwrap();
    let (signing_key, publish) = signed_publish();
    let body = serde_json::to_vec(&publish).unwrap();

    let create = handle_request(
        &state,
        HttpRequest::new("POST", "/v1/publish").with_json_body(body),
    );

    assert_eq!(create.status, 201);
    assert!(create.body_text().contains("pub_test_001"));

    let search = handle_request(
        &state,
        HttpRequest::new("GET", "/v1/publish/search?tag=coding"),
    );

    assert_eq!(search.status, 200);
    assert!(search.body_text().contains("hello public relay"));

    let delete_request = RequestToSign {
        method: "DELETE".to_string(),
        path_and_query: "/v1/publish/pub_test_001".to_string(),
        body: Vec::new(),
        peer_id: publish.payload.peer_id.clone(),
        public_key: publish.payload.public_key.clone(),
        timestamp: Utc.with_ymd_and_hms(2026, 5, 12, 1, 3, 0).unwrap(),
        nonce: "nonce_delete_001".to_string(),
        idempotency_key: Some("idem_delete_001".to_string()),
    };
    let signature = AichanRequestSignature::sign(&delete_request, &signing_key).unwrap();
    let delete = handle_request(
        &state,
        HttpRequest::new("DELETE", "/v1/publish/pub_test_001")
            .with_header("Aichan-Protocol", &signature.protocol)
            .with_header("Aichan-Peer-Id", signature.peer_id.as_str())
            .with_header("Aichan-Public-Key", &signature.public_key)
            .with_header("Aichan-Timestamp", signature.timestamp.to_rfc3339())
            .with_header("Aichan-Nonce", &signature.nonce)
            .with_header(
                "Idempotency-Key",
                signature.idempotency_key.as_deref().unwrap(),
            )
            .with_header("Aichan-Signature", &signature.value),
    );

    assert_eq!(delete.status, 200);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&delete.body).unwrap(),
        json!({"deleted": true, "id": "pub_test_001"})
    );

    let after_delete = handle_request(
        &state,
        HttpRequest::new("GET", "/v1/publish/search?tag=coding"),
    );

    assert_eq!(after_delete.status, 200);
    assert!(!after_delete.body_text().contains("hello public relay"));
}

#[test]
fn health_and_discovery_are_available_before_storage_setup() {
    let temp = tempfile::tempdir().unwrap();
    let state = ServerState::new(temp.path()).unwrap();

    let health = handle_request(&state, HttpRequest::new("GET", "/health"));
    let discovery = handle_request(&state, HttpRequest::new("GET", "/.well-known/aichan"));

    assert_eq!(health.status, 200);
    assert!(health.body_text().contains("\"ok\":true"));
    assert_eq!(discovery.status, 200);
    assert!(discovery.body_text().contains("\"protocol\":\"aichan/1\""));
}

#[test]
fn publish_writes_are_rate_limited_per_client() {
    let temp = tempfile::tempdir().unwrap();
    let state = ServerState::with_rate_limits(
        temp.path(),
        RateLimitConfig {
            read_per_minute: 100,
            write_per_minute: 1,
            max_body_bytes: 65536,
        },
    )
    .unwrap();
    let (_, publish) = signed_publish();
    let body = serde_json::to_vec(&publish).unwrap();

    let first = handle_request(
        &state,
        HttpRequest::new("POST", "/v1/publish")
            .with_header("X-Forwarded-For", "203.0.113.10")
            .with_json_body(body.clone()),
    );
    let second = handle_request(
        &state,
        HttpRequest::new("POST", "/v1/publish")
            .with_header("X-Forwarded-For", "203.0.113.10")
            .with_json_body(body),
    );
    let other_client = handle_request(
        &state,
        HttpRequest::new("POST", "/v1/publish")
            .with_header("X-Forwarded-For", "203.0.113.11")
            .with_json_body(serde_json::to_vec(&publish).unwrap()),
    );

    assert_eq!(first.status, 201);
    assert_eq!(second.status, 429);
    assert!(second.headers.contains_key("Retry-After"));
    assert!(second.body_text().contains("\"rate_limited\""));
    assert_eq!(other_client.status, 201);
}

#[test]
fn oversized_publish_body_is_rejected_before_json_parse() {
    let temp = tempfile::tempdir().unwrap();
    let state = ServerState::with_rate_limits(
        temp.path(),
        RateLimitConfig {
            read_per_minute: 100,
            write_per_minute: 100,
            max_body_bytes: 16,
        },
    )
    .unwrap();

    let response = handle_request(
        &state,
        HttpRequest::new("POST", "/v1/publish")
            .with_header("X-Forwarded-For", "203.0.113.20")
            .with_json_body(vec![b'x'; 17]),
    );

    assert_eq!(response.status, 413);
    assert!(response.body_text().contains("\"payload_too_large\""));
}
