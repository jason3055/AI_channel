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
    signed_publish_with("pub_test_001", "hello public relay")
}

fn signed_publish_with(
    publish_id: &str,
    body: &str,
) -> (SigningKey, SignedProtocolObject<PublishRecordPayload>) {
    signed_publish_at(
        publish_id,
        body,
        Utc.with_ymd_and_hms(2026, 5, 12, 1, 2, 3).unwrap(),
    )
}

fn signed_publish_at(
    publish_id: &str,
    body: &str,
    created_at: chrono::DateTime<Utc>,
) -> (SigningKey, SignedProtocolObject<PublishRecordPayload>) {
    let signing_key = SigningKey::from_bytes(&[3_u8; 32]);
    let public_key = URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_bytes());
    let peer_id = derive_peer_id(&signing_key.verifying_key().to_bytes());
    let payload = PublishRecordPayload {
        peer_id,
        public_key,
        tags: vec!["coding".to_string(), "agent-friends".to_string()],
        contact_policy: "encrypted_messages".to_string(),
        capabilities: CapabilitySet::default(),
        body: body.to_string(),
        updated_at: created_at,
    };
    let object = UnsignedProtocolObject::new("publish.record", publish_id, created_at, payload)
        .sign(&signing_key)
        .unwrap();

    (signing_key, object)
}

fn signed_delete_request(
    signing_key: &SigningKey,
    publish: &SignedProtocolObject<PublishRecordPayload>,
    nonce: &str,
    timestamp: chrono::DateTime<Utc>,
) -> AichanRequestSignature {
    let delete_request = RequestToSign {
        method: "DELETE".to_string(),
        path_and_query: format!("/v1/publish/{}", publish.id),
        body: Vec::new(),
        peer_id: publish.payload.peer_id.clone(),
        public_key: publish.payload.public_key.clone(),
        timestamp,
        nonce: nonce.to_string(),
        idempotency_key: Some(format!("idem_{nonce}")),
    };
    AichanRequestSignature::sign(&delete_request, signing_key).unwrap()
}

fn delete_request(publish_id: &str, signature: &AichanRequestSignature) -> HttpRequest {
    HttpRequest::new("DELETE", format!("/v1/publish/{publish_id}"))
        .with_header("Aichan-Protocol", &signature.protocol)
        .with_header("Aichan-Peer-Id", signature.peer_id.as_str())
        .with_header("Aichan-Public-Key", &signature.public_key)
        .with_header("Aichan-Timestamp", signature.timestamp.to_rfc3339())
        .with_header("Aichan-Nonce", &signature.nonce)
        .with_header(
            "Idempotency-Key",
            signature.idempotency_key.as_deref().unwrap(),
        )
        .with_header("Aichan-Signature", &signature.value)
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

    let signature = signed_delete_request(&signing_key, &publish, "nonce_delete_001", Utc::now());
    let delete = handle_request(&state, delete_request("pub_test_001", &signature));

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
fn publish_search_returns_recent_records_with_cursor_pages() {
    let temp = tempfile::tempdir().unwrap();
    let state = ServerState::new(temp.path()).unwrap();

    for (id, body, minute) in [
        ("pub_page_001", "oldest public note", 1),
        ("pub_page_002", "middle public note", 2),
        ("pub_page_003", "newest public note", 3),
    ] {
        let (_, publish) = signed_publish_at(
            id,
            body,
            Utc.with_ymd_and_hms(2026, 5, 12, 1, minute, 0).unwrap(),
        );
        let create = handle_request(
            &state,
            HttpRequest::new("POST", "/v1/publish")
                .with_json_body(serde_json::to_vec(&publish).unwrap()),
        );
        assert_eq!(create.status, 201);
    }

    let first = handle_request(
        &state,
        HttpRequest::new("GET", "/v1/publish/search?tag=coding&limit=2"),
    );
    assert_eq!(first.status, 200);
    let first_json: serde_json::Value = serde_json::from_slice(&first.body).unwrap();
    assert_eq!(first_json["count"], 2);
    assert_eq!(first_json["window_limit"], 10_000);
    assert_eq!(first_json["has_more"], true);
    assert_eq!(first_json["records"][0]["id"], "pub_page_003");
    assert_eq!(first_json["records"][1]["id"], "pub_page_002");
    let cursor = first_json["next_cursor"].as_str().unwrap();
    assert!(!cursor.is_empty());

    let second = handle_request(
        &state,
        HttpRequest::new(
            "GET",
            format!("/v1/publish/search?tag=coding&limit=2&cursor={cursor}"),
        ),
    );
    assert_eq!(second.status, 200);
    let second_json: serde_json::Value = serde_json::from_slice(&second.body).unwrap();
    assert_eq!(second_json["count"], 1);
    assert_eq!(second_json["has_more"], false);
    assert!(second_json["next_cursor"].is_null());
    assert_eq!(second_json["records"][0]["id"], "pub_page_001");
}

#[test]
fn directory_page_loads_publish_api_pages_and_new_record_notice() {
    let temp = tempfile::tempdir().unwrap();
    let state = ServerState::new(temp.path()).unwrap();

    let page = handle_request(&state, HttpRequest::new("GET", "/"));

    assert_eq!(page.status, 200);
    let html = page.body_text();
    assert!(html.contains("/v1/publish/search?limit="));
    assert!(html.contains("id=\"moreLink\""));
    assert!(html.contains("id=\"newNotice\""));
    assert!(html.contains("setInterval(checkForNewRecords"));
    assert!(!html.contains("GET /v1/publish/search"));
}

#[test]
fn author_deleted_publish_id_cannot_be_replayed_back_into_search() {
    let temp = tempfile::tempdir().unwrap();
    let state = ServerState::new(temp.path()).unwrap();
    let (signing_key, publish) = signed_publish();
    let body = serde_json::to_vec(&publish).unwrap();

    let create = handle_request(
        &state,
        HttpRequest::new("POST", "/v1/publish").with_json_body(body.clone()),
    );
    assert_eq!(create.status, 201);

    let signature = signed_delete_request(&signing_key, &publish, "nonce_replay_guard", Utc::now());
    let delete = handle_request(&state, delete_request("pub_test_001", &signature));
    assert_eq!(delete.status, 200);

    let replay = handle_request(
        &state,
        HttpRequest::new("POST", "/v1/publish").with_json_body(body),
    );
    assert_eq!(replay.status, 409);
    assert!(replay.body_text().contains("\"publish_deleted\""));

    let search = handle_request(
        &state,
        HttpRequest::new("GET", "/v1/publish/search?tag=coding"),
    );
    assert_eq!(search.status, 200);
    assert!(!search.body_text().contains("hello public relay"));
}

#[test]
fn delete_rejects_stale_request_signature() {
    let temp = tempfile::tempdir().unwrap();
    let state = ServerState::new(temp.path()).unwrap();
    let (signing_key, publish) = signed_publish();
    let body = serde_json::to_vec(&publish).unwrap();

    let create = handle_request(
        &state,
        HttpRequest::new("POST", "/v1/publish").with_json_body(body),
    );
    assert_eq!(create.status, 201);

    let stale_signature = signed_delete_request(
        &signing_key,
        &publish,
        "nonce_stale_delete",
        Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap(),
    );
    let delete = handle_request(&state, delete_request("pub_test_001", &stale_signature));

    assert_eq!(delete.status, 401);
    assert!(delete.body_text().contains("\"stale_request_signature\""));
}

#[test]
fn delete_rejects_reused_nonce_for_same_peer() {
    let temp = tempfile::tempdir().unwrap();
    let state = ServerState::new(temp.path()).unwrap();
    let (signing_key, first) = signed_publish_with("pub_test_001", "first relay note");
    let (_, second) = signed_publish_with("pub_test_002", "second relay note");

    for publish in [&first, &second] {
        let create = handle_request(
            &state,
            HttpRequest::new("POST", "/v1/publish")
                .with_json_body(serde_json::to_vec(publish).unwrap()),
        );
        assert_eq!(create.status, 201);
    }

    let first_signature =
        signed_delete_request(&signing_key, &first, "nonce_once_only", Utc::now());
    let first_delete = handle_request(&state, delete_request("pub_test_001", &first_signature));
    assert_eq!(first_delete.status, 200);

    let second_signature =
        signed_delete_request(&signing_key, &second, "nonce_once_only", Utc::now());
    let second_delete = handle_request(&state, delete_request("pub_test_002", &second_signature));
    assert_eq!(second_delete.status, 401);
    assert!(second_delete
        .body_text()
        .contains("\"replayed_request_nonce\""));
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
fn favicon_request_does_not_create_warning_404_noise() {
    let temp = tempfile::tempdir().unwrap();
    let state = ServerState::new(temp.path()).unwrap();

    let favicon = handle_request(&state, HttpRequest::new("GET", "/favicon.ico"));

    assert_eq!(favicon.status, 204);
    assert!(favicon.body.is_empty());
}

#[test]
fn agent_bootstrap_explains_skill_cli_and_installer() {
    let temp = tempfile::tempdir().unwrap();
    let state = ServerState::with_public_base_url(
        temp.path(),
        "https://aichan-server-w4rouatrfa-uc.a.run.app",
    )
    .unwrap();

    let agent = handle_request(&state, HttpRequest::new("GET", "/agent"));
    assert_eq!(agent.status, 200);
    let agent_text = agent.body_text();
    assert!(agent_text.contains("npx skills add"));
    assert!(agent_text.contains("cargo install --git"));
    assert!(agent_text.contains("https://aichan-server-w4rouatrfa-uc.a.run.app/install.sh"));
    assert!(agent_text.contains("No-brain installer"));
    assert!(agent_text.contains("The skill does not install the CLI"));

    let metadata = handle_request(&state, HttpRequest::new("GET", "/agent.json"));
    assert_eq!(metadata.status, 200);
    let metadata_json: serde_json::Value = serde_json::from_slice(&metadata.body).unwrap();
    assert_eq!(metadata_json["skill"]["name"], "aichan");
    assert_eq!(metadata_json["skill"]["version"], "0.1.0");
    assert!(metadata_json["skill"]["install"]
        .as_str()
        .unwrap()
        .contains("npx skills add"));
    assert!(metadata_json["skill"]["update"]
        .as_str()
        .unwrap()
        .contains("npx skills add"));
    assert_eq!(
        metadata_json["cli"]["install"].as_str().unwrap(),
        "curl -fsSL https://aichan-server-w4rouatrfa-uc.a.run.app/install.sh | sh"
    );
    assert_eq!(
        metadata_json["cli"]["cargo_install"].as_str().unwrap(),
        "cargo install --git https://github.com/aftershower/AI_channel aichan --locked --force"
    );
    assert!(metadata_json["cli"]["fallback_install"]
        .as_str()
        .unwrap()
        .contains("cargo install --git"));

    let installer = handle_request(&state, HttpRequest::new("GET", "/install.sh"));
    assert_eq!(installer.status, 200);
    assert_eq!(
        installer.headers.get("Content-Type").map(String::as_str),
        Some("text/x-shellscript; charset=utf-8")
    );
    let script = installer.body_text();
    assert!(script.contains("set -eu"));
    assert!(script.contains("cargo not found; installing Rust toolchain with rustup"));
    assert!(script.contains("sh -s -- -y --no-modify-path"));
    assert!(script.contains("curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"));
    assert!(script.contains(
        "cargo install --git https://github.com/aftershower/AI_channel aichan --locked --force"
    ));
    assert!(script.contains("aichan --version"));
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
