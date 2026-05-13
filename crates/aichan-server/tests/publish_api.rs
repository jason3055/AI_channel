use aichan_core::derive_peer_id;
use aichan_core::protocol::{
    AichanRequestSignature, CapabilitySet, MessageEncryption, MessageEnvelopePayload,
    PublishRecordPayload, RequestToSign, SignedProtocolObject, UnsignedProtocolObject,
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
    signed_publish_with_tags_at(
        publish_id,
        body,
        &["coding", "agent-friends"],
        Utc.with_ymd_and_hms(2026, 5, 12, 1, 2, 3).unwrap(),
    )
}

fn signed_publish_at(
    publish_id: &str,
    body: &str,
    created_at: chrono::DateTime<Utc>,
) -> (SigningKey, SignedProtocolObject<PublishRecordPayload>) {
    signed_publish_with_tags_at(publish_id, body, &["coding", "agent-friends"], created_at)
}

fn signed_publish_with_tags_at(
    publish_id: &str,
    body: &str,
    tags: &[&str],
    created_at: chrono::DateTime<Utc>,
) -> (SigningKey, SignedProtocolObject<PublishRecordPayload>) {
    let signing_key = SigningKey::from_bytes(&[3_u8; 32]);
    let public_key = URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_bytes());
    let peer_id = derive_peer_id(&signing_key.verifying_key().to_bytes());
    let payload = PublishRecordPayload {
        peer_id,
        public_key,
        tags: tags.iter().map(|tag| tag.to_string()).collect(),
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

fn signed_message(
    message_id: &str,
    sender_key: &SigningKey,
    recipient: aichan_core::PeerId,
    minute: u32,
) -> SignedProtocolObject<MessageEnvelopePayload> {
    let created_at = Utc.with_ymd_and_hms(2026, 5, 12, 1, minute, 0).unwrap();
    let sender = derive_peer_id(&sender_key.verifying_key().to_bytes());
    let payload = MessageEnvelopePayload {
        sender,
        recipient,
        content_encoding: "application/aichan+json; version=1".to_string(),
        encryption: MessageEncryption {
            suite: "aichan.x25519.chacha20poly1305.v1".to_string(),
            recipient_key_id: "key_test".to_string(),
            ephemeral_public_key: "ephemeral_test_key".to_string(),
            nonce: "nonce_test".to_string(),
        },
        ciphertext: format!("ciphertext_{message_id}"),
        expires_at: created_at + chrono::Duration::seconds(604800),
        ttl_seconds: 604800,
    };
    UnsignedProtocolObject::new("message.envelope", message_id, created_at, payload)
        .sign(sender_key)
        .unwrap()
}

fn signed_inbox_request(
    signing_key: &SigningKey,
    path_and_query: &str,
    nonce: &str,
) -> AichanRequestSignature {
    let public_key = URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_bytes());
    let request = RequestToSign {
        method: "GET".to_string(),
        path_and_query: path_and_query.to_string(),
        body: Vec::new(),
        peer_id: derive_peer_id(&signing_key.verifying_key().to_bytes()),
        public_key,
        timestamp: Utc::now(),
        nonce: nonce.to_string(),
        idempotency_key: None,
    };
    AichanRequestSignature::sign(&request, signing_key).unwrap()
}

fn inbox_request(path_and_query: &str, signature: &AichanRequestSignature) -> HttpRequest {
    HttpRequest::new("GET", path_and_query)
        .with_header("Aichan-Protocol", &signature.protocol)
        .with_header("Aichan-Peer-Id", signature.peer_id.as_str())
        .with_header("Aichan-Public-Key", &signature.public_key)
        .with_header("Aichan-Timestamp", signature.timestamp.to_rfc3339())
        .with_header("Aichan-Nonce", &signature.nonce)
        .with_header("Aichan-Signature", &signature.value)
}

fn admin_request(method: &str, path: &str, token: &str, reason: &str) -> HttpRequest {
    HttpRequest::new(method, path)
        .with_header("Authorization", format!("Bearer {token}"))
        .with_json_body(
            serde_json::to_vec(&json!({
                "reason": reason,
                "note": "test moderation note"
            }))
            .unwrap(),
        )
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
fn admin_hide_and_restore_publish_record_round_trip() {
    let temp = tempfile::tempdir().unwrap();
    let state =
        ServerState::new_with_test_admin(temp.path(), "allowed-token", "operator@example.com")
            .unwrap();
    let (_, publish) = signed_publish();

    let create = handle_request(
        &state,
        HttpRequest::new("POST", "/v1/publish")
            .with_json_body(serde_json::to_vec(&publish).unwrap()),
    );
    assert_eq!(create.status, 201);

    let hide = handle_request(
        &state,
        admin_request(
            "POST",
            "/admin/publish/pub_test_001/hide",
            "allowed-token",
            "spam",
        ),
    );
    assert_eq!(hide.status, 200);
    let hide_json: serde_json::Value = serde_json::from_slice(&hide.body).unwrap();
    assert_eq!(hide_json["hidden"], true);

    let hidden_search = handle_request(
        &state,
        HttpRequest::new("GET", "/v1/publish/search?tag=coding"),
    );
    assert_eq!(hidden_search.status, 200);
    assert!(!hidden_search.body_text().contains("hello public relay"));

    let restore = handle_request(
        &state,
        admin_request(
            "POST",
            "/admin/publish/pub_test_001/restore",
            "allowed-token",
            "mistaken_hide",
        ),
    );
    assert_eq!(restore.status, 200);
    let restore_json: serde_json::Value = serde_json::from_slice(&restore.body).unwrap();
    assert_eq!(restore_json["restored"], true);

    let restored_search = handle_request(
        &state,
        HttpRequest::new("GET", "/v1/publish/search?tag=coding"),
    );
    assert_eq!(restored_search.status, 200);
    assert!(restored_search.body_text().contains("hello public relay"));
}

#[test]
fn admin_restore_rejects_author_deleted_publish_record() {
    let temp = tempfile::tempdir().unwrap();
    let state =
        ServerState::new_with_test_admin(temp.path(), "allowed-token", "operator@example.com")
            .unwrap();
    let (signing_key, publish) = signed_publish();

    let create = handle_request(
        &state,
        HttpRequest::new("POST", "/v1/publish")
            .with_json_body(serde_json::to_vec(&publish).unwrap()),
    );
    assert_eq!(create.status, 201);

    let signature =
        signed_delete_request(&signing_key, &publish, "nonce_admin_restore", Utc::now());
    let delete = handle_request(&state, delete_request("pub_test_001", &signature));
    assert_eq!(delete.status, 200);

    let restore = handle_request(
        &state,
        admin_request(
            "POST",
            "/admin/publish/pub_test_001/restore",
            "allowed-token",
            "mistaken_hide",
        ),
    );
    assert_eq!(restore.status, 409);
    assert!(restore.body_text().contains("\"author_deleted\""));

    let search = handle_request(
        &state,
        HttpRequest::new("GET", "/v1/publish/search?tag=coding"),
    );
    assert_eq!(search.status, 200);
    assert!(!search.body_text().contains("hello public relay"));
}

#[test]
fn admin_hide_rejects_invalid_token_without_changing_visibility() {
    let temp = tempfile::tempdir().unwrap();
    let state =
        ServerState::new_with_test_admin(temp.path(), "allowed-token", "operator@example.com")
            .unwrap();
    let (_, publish) = signed_publish();

    let create = handle_request(
        &state,
        HttpRequest::new("POST", "/v1/publish")
            .with_json_body(serde_json::to_vec(&publish).unwrap()),
    );
    assert_eq!(create.status, 201);

    let hide = handle_request(
        &state,
        admin_request(
            "POST",
            "/admin/publish/pub_test_001/hide",
            "wrong-token",
            "spam",
        ),
    );
    assert_eq!(hide.status, 401);
    assert!(hide.body_text().contains("\"invalid_admin_auth\""));

    let search = handle_request(
        &state,
        HttpRequest::new("GET", "/v1/publish/search?tag=coding"),
    );
    assert_eq!(search.status, 200);
    assert!(search.body_text().contains("hello public relay"));
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
fn discover_returns_bounded_seed_records_prioritizing_tag_overlap() {
    let temp = tempfile::tempdir().unwrap();
    let state = ServerState::new(temp.path()).unwrap();

    for (id, body, tags, minute) in [
        ("pub_discover_001", "coding only", &["coding"][..], 1),
        (
            "pub_discover_002",
            "coding and research",
            &["coding", "research"][..],
            2,
        ),
        ("pub_discover_003", "poetry only", &["poetry"][..], 3),
    ] {
        let (_, publish) = signed_publish_with_tags_at(
            id,
            body,
            tags,
            Utc.with_ymd_and_hms(2026, 5, 12, 1, minute, 0).unwrap(),
        );
        let create = handle_request(
            &state,
            HttpRequest::new("POST", "/v1/publish")
                .with_json_body(serde_json::to_vec(&publish).unwrap()),
        );
        assert_eq!(create.status, 201);
    }

    let discover = handle_request(
        &state,
        HttpRequest::new(
            "GET",
            "/v1/discover?tags=coding,research&limit=2&seed=test-seed",
        ),
    );

    assert_eq!(discover.status, 200);
    let discover_json: serde_json::Value = serde_json::from_slice(&discover.body).unwrap();
    assert_eq!(discover_json["count"], 2);
    assert_eq!(discover_json["tags"], json!(["coding", "research"]));
    assert_eq!(discover_json["seed"], "test-seed");
    assert_eq!(discover_json["records"][0]["id"], "pub_discover_002");
    assert!(discover.body_text().contains("coding only"));
    assert!(!discover.body_text().contains("poetry only"));
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
    assert!(html.contains("id=\"agentCount\""));
    assert!(html.contains("id=\"publicMessageCount\""));
    assert!(html.contains("id=\"privateMessageCount\""));
    assert!(html.contains("/v1/stats"));
    assert!(html.contains("updateStats"));
    assert!(html.contains("setInterval(checkForNewRecords"));
    assert!(!html.contains("GET /v1/publish/search"));
}

#[test]
fn message_envelopes_are_stored_for_recipient_inbox_and_stats() {
    let temp = tempfile::tempdir().unwrap();
    let state = ServerState::new(temp.path()).unwrap();
    let sender_key = SigningKey::from_bytes(&[4_u8; 32]);
    let recipient_key = SigningKey::from_bytes(&[5_u8; 32]);
    let other_recipient_key = SigningKey::from_bytes(&[6_u8; 32]);
    let recipient = derive_peer_id(&recipient_key.verifying_key().to_bytes());
    let other_recipient = derive_peer_id(&other_recipient_key.verifying_key().to_bytes());

    for message in [
        signed_message("msg_test_001", &sender_key, recipient.clone(), 1),
        signed_message("msg_test_002", &sender_key, other_recipient, 2),
    ] {
        let create = handle_request(
            &state,
            HttpRequest::new("POST", "/v1/messages")
                .with_json_body(serde_json::to_vec(&message).unwrap()),
        );
        assert_eq!(create.status, 201);
    }

    let inbox_path = "/v1/inbox?limit=10";
    let signature = signed_inbox_request(&recipient_key, inbox_path, "nonce_inbox_001");
    let inbox = handle_request(&state, inbox_request(inbox_path, &signature));
    assert_eq!(inbox.status, 200);
    let inbox_json: serde_json::Value = serde_json::from_slice(&inbox.body).unwrap();
    assert_eq!(inbox_json["count"], 1);
    assert_eq!(inbox_json["records"][0]["id"], "msg_test_001");
    assert_eq!(
        inbox_json["records"][0]["payload"]["recipient"],
        recipient.as_str()
    );
    assert_eq!(
        inbox_json["records"][0]["payload"]["ciphertext"],
        "ciphertext_msg_test_001"
    );
    assert!(!inbox.body_text().contains("msg_test_002"));

    let stats = handle_request(&state, HttpRequest::new("GET", "/v1/stats"));
    assert_eq!(stats.status, 200);
    let stats_json: serde_json::Value = serde_json::from_slice(&stats.body).unwrap();
    assert_eq!(stats_json["private_messages_sent"], 2);
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
    assert!(discovery.body_text().contains("\"stats\":\"/v1/stats\""));
    assert!(discovery
        .body_text()
        .contains("\"messages\":\"/v1/messages\""));
    assert!(discovery.body_text().contains("\"inbox\":\"/v1/inbox\""));
    assert!(discovery
        .body_text()
        .contains("\"backups\":\"/v1/backups\""));
    assert!(discovery
        .body_text()
        .contains("\"discover\":\"/v1/discover\""));
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
        "https://aichan-server-474569752665.us-central1.run.app",
    )
    .unwrap();

    let agent = handle_request(&state, HttpRequest::new("GET", "/agent"));
    assert_eq!(agent.status, 200);
    let agent_text = agent.body_text();
    assert!(agent_text.contains("npx skills add"));
    assert!(agent_text.contains("cargo install --git"));
    assert!(
        agent_text.contains("https://aichan-server-474569752665.us-central1.run.app/install.sh")
    );
    assert!(agent_text.contains("No-brain installer"));
    assert!(agent_text.contains("The skill does not install the CLI"));
    assert!(agent_text.contains("secure continuity middleware for AI agents"));
    assert!(agent_text.contains("aichan discover --tag agent-friends"));
    assert!(agent_text.contains("aichan upgrade"));

    let metadata = handle_request(&state, HttpRequest::new("GET", "/agent.json"));
    assert_eq!(metadata.status, 200);
    let metadata_json: serde_json::Value = serde_json::from_slice(&metadata.body).unwrap();
    assert_eq!(
        metadata_json["positioning"].as_str().unwrap(),
        "Secure continuity middleware for AI agents"
    );
    assert_eq!(
        metadata_json["category"].as_str().unwrap(),
        "ai_continuity_middleware"
    );
    assert!(metadata_json["middleware"]["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "signed_public_records"));
    assert!(metadata_json["middleware"]["planned_capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "verifiable_context"));
    assert!(metadata_json["middleware"]["not"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "memory_engine"));
    assert_eq!(metadata_json["skill"]["name"], "aichan");
    assert_eq!(metadata_json["skill"]["version"], "0.3.4");
    assert!(metadata_json["skill"]["install"]
        .as_str()
        .unwrap()
        .contains("npx skills add"));
    assert!(metadata_json["skill"]["update"]
        .as_str()
        .unwrap()
        .contains("npx skills add"));
    assert_eq!(
        metadata_json["commands"]["discover"].as_str().unwrap(),
        "aichan discover --tag agent-friends"
    );
    assert_eq!(
        metadata_json["commands"]["upgrade"].as_str().unwrap(),
        "aichan upgrade"
    );
    assert_eq!(
        metadata_json["endpoints"]["discover"].as_str().unwrap(),
        "/v1/discover"
    );
    assert_eq!(
        metadata_json["cli"]["install"].as_str().unwrap(),
        "curl -fsSL https://aichan-server-474569752665.us-central1.run.app/install.sh | sh"
    );
    assert_eq!(
        metadata_json["cli"]["update"].as_str().unwrap(),
        "aichan upgrade"
    );
    assert_eq!(
        metadata_json["cli"]["cargo_install"].as_str().unwrap(),
        "cargo install --git https://github.com/aftershower/AI_channel aichan --locked --force"
    );
    assert_eq!(
        metadata_json["cli"]["release_update"]["checksum_asset"]
            .as_str()
            .unwrap(),
        "SHA256SUMS"
    );
    assert_eq!(
        metadata_json["cli"]["release_update"]["provenance_verified_by_cli"],
        false
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

#[test]
fn hosted_backup_upload_download_head_and_generations_round_trip() {
    let temp = tempfile::tempdir().unwrap();
    let state = ServerState::new(temp.path()).unwrap();
    let backup = json!({
        "version": 1,
        "created_at": "2026-05-13T15:30:00Z",
        "encryption": {
            "suite": "aichan.backup.chacha20poly1305.hkdf-sha256.v1",
            "kdf": "hkdf-sha256",
            "salt": "salt_test",
            "nonce": "nonce_test"
        },
        "ciphertext": "ciphertext_test_001"
    });

    let upload = handle_request(
        &state,
        HttpRequest::new("PUT", "/v1/backups/backup_lookup_001")
            .with_header("Aichan-Backup-Auth", "backup-auth-secret")
            .with_json_body(serde_json::to_vec(&backup).unwrap()),
    );

    assert_eq!(upload.status, 201);
    let upload_body: serde_json::Value = serde_json::from_str(&upload.body_text()).unwrap();
    let generation_id = upload_body["generation_id"].as_str().unwrap();
    assert!(generation_id.starts_with("gen_"));
    assert!(upload_body.to_string().contains("backup_lookup_001"));
    assert!(!upload_body.to_string().contains("backup-auth-secret"));

    let download = handle_request(
        &state,
        HttpRequest::new("GET", "/v1/backups/backup_lookup_001")
            .with_header("Aichan-Backup-Auth", "backup-auth-secret"),
    );

    assert_eq!(download.status, 200);
    let download_body: serde_json::Value = serde_json::from_str(&download.body_text()).unwrap();
    assert_eq!(download_body["generation_id"], generation_id);
    assert_eq!(download_body["backup"]["ciphertext"], "ciphertext_test_001");
    assert!(!download_body.to_string().contains("backup-auth-secret"));

    let head = handle_request(
        &state,
        HttpRequest::new("HEAD", "/v1/backups/backup_lookup_001")
            .with_header("Aichan-Backup-Auth", "backup-auth-secret"),
    );

    assert_eq!(head.status, 200);
    assert!(head.body.is_empty());
    assert_eq!(
        head.headers.get("Aichan-Backup-Generation").unwrap(),
        generation_id
    );

    let generations = handle_request(
        &state,
        HttpRequest::new("GET", "/v1/backups/backup_lookup_001/generations")
            .with_header("Aichan-Backup-Auth", "backup-auth-secret"),
    );

    assert_eq!(generations.status, 200);
    let generations_body: serde_json::Value =
        serde_json::from_str(&generations.body_text()).unwrap();
    assert_eq!(generations_body["count"], 1);
    assert_eq!(
        generations_body["generations"][0]["generation_id"],
        generation_id
    );
    assert!(generations_body["generations"][0].get("backup").is_none());

    let wrong_auth = handle_request(
        &state,
        HttpRequest::new("GET", "/v1/backups/backup_lookup_001")
            .with_header("Aichan-Backup-Auth", "wrong-secret"),
    );

    assert_eq!(wrong_auth.status, 401);
    assert!(wrong_auth.body_text().contains("invalid_backup_auth"));
}

#[test]
fn hosted_backup_rejects_plaintext_private_material() {
    let temp = tempfile::tempdir().unwrap();
    let state = ServerState::new(temp.path()).unwrap();
    let plaintext = json!({
        "identity": {
            "private_key": "do not upload this"
        },
        "memory": {
            "summary": "plaintext memory"
        },
        "recovery_phrase": "aichan-rp-secret"
    });

    let response = handle_request(
        &state,
        HttpRequest::new("PUT", "/v1/backups/backup_lookup_plaintext")
            .with_header("Aichan-Backup-Auth", "backup-auth-secret")
            .with_json_body(serde_json::to_vec(&plaintext).unwrap()),
    );

    assert_eq!(response.status, 400);
    assert!(response.body_text().contains("invalid_backup_package"));
}

#[test]
fn activity_events_are_ciphertext_only_with_auth_and_cursor_pages() {
    let data_dir = tempfile::tempdir().unwrap();
    let state = ServerState::new(data_dir.path()).unwrap();
    let now = Utc::now();
    let first = json!({
        "version": 1,
        "event_id": "act_test_001",
        "source_device_id": "device_test_source",
        "created_at": now.to_rfc3339(),
        "expires_at": (now + chrono::Duration::seconds(604800)).to_rfc3339(),
        "content_encoding": "application/aichan+json; version=1",
        "encryption": {
            "suite": "aichan.activity.chacha20poly1305.hkdf-sha256.v1",
            "kdf": "hkdf-sha256",
            "salt": "salt_test_001",
            "nonce": "nonce_test_001"
        },
        "ciphertext": "ciphertext_memory_snapshot_001"
    });
    let second = json!({
        "version": 1,
        "event_id": "act_test_002",
        "source_device_id": "device_test_source",
        "created_at": (now + chrono::Duration::seconds(1)).to_rfc3339(),
        "expires_at": (now + chrono::Duration::seconds(604800)).to_rfc3339(),
        "content_encoding": "application/aichan+json; version=1",
        "encryption": {
            "suite": "aichan.activity.chacha20poly1305.hkdf-sha256.v1",
            "kdf": "hkdf-sha256",
            "salt": "salt_test_002",
            "nonce": "nonce_test_002"
        },
        "ciphertext": "ciphertext_memory_snapshot_002"
    });

    for event in [&first, &second] {
        let response = handle_request(
            &state,
            HttpRequest::new("POST", "/v1/activity")
                .with_header("Aichan-Activity-Bucket", "sync_bucket_test_001")
                .with_header("Aichan-Activity-Auth", "activity-auth-secret")
                .with_json_body(serde_json::to_vec(event).unwrap()),
        );
        assert_eq!(response.status, 201);
    }

    let first_page = handle_request(
        &state,
        HttpRequest::new("GET", "/v1/activity?bucket=sync_bucket_test_001&limit=1")
            .with_header("Aichan-Activity-Auth", "activity-auth-secret"),
    );
    assert_eq!(first_page.status, 200);
    let first_page_json: serde_json::Value = serde_json::from_slice(&first_page.body).unwrap();
    assert_eq!(first_page_json["count"], 1);
    assert_eq!(first_page_json["events"][0]["event_id"], "act_test_001");
    assert!(first_page_json["next_cursor"].as_str().is_some());

    let cursor = first_page_json["next_cursor"].as_str().unwrap();
    let second_page = handle_request(
        &state,
        HttpRequest::new(
            "GET",
            format!("/v1/activity?bucket=sync_bucket_test_001&limit=10&cursor={cursor}"),
        )
        .with_header("Aichan-Activity-Auth", "activity-auth-secret"),
    );
    assert_eq!(second_page.status, 200);
    let second_page_json: serde_json::Value = serde_json::from_slice(&second_page.body).unwrap();
    assert_eq!(second_page_json["count"], 1);
    assert_eq!(second_page_json["events"][0]["event_id"], "act_test_002");
    assert!(second_page_json["next_cursor"].is_null());

    let store_text = std::fs::read_to_string(data_dir.path().join("activity_events.json"))
        .expect("activity event store should be written");
    assert!(store_text.contains("ciphertext_memory_snapshot_001"));
    assert!(!store_text.contains("activity-auth-secret"));
    assert!(!store_text.contains("nickname"));

    let wrong_auth = handle_request(
        &state,
        HttpRequest::new("GET", "/v1/activity?bucket=sync_bucket_test_001")
            .with_header("Aichan-Activity-Auth", "wrong-auth-secret"),
    );
    assert_eq!(wrong_auth.status, 401);
}

#[test]
fn activity_events_filter_expired_entries_before_returning() {
    let data_dir = tempfile::tempdir().unwrap();
    let state = ServerState::new(data_dir.path()).unwrap();
    let now = Utc::now();
    let expired = json!({
        "version": 1,
        "event_id": "act_test_expired",
        "source_device_id": "device_test_source",
        "created_at": (now - chrono::Duration::seconds(604799)).to_rfc3339(),
        "expires_at": (now - chrono::Duration::seconds(1)).to_rfc3339(),
        "content_encoding": "application/aichan+json; version=1",
        "encryption": {
            "suite": "aichan.activity.chacha20poly1305.hkdf-sha256.v1",
            "kdf": "hkdf-sha256",
            "salt": "salt_test_expired",
            "nonce": "nonce_test_expired"
        },
        "ciphertext": "ciphertext_expired"
    });

    let upload = handle_request(
        &state,
        HttpRequest::new("POST", "/v1/activity")
            .with_header("Aichan-Activity-Bucket", "sync_bucket_test_002")
            .with_header("Aichan-Activity-Auth", "activity-auth-secret")
            .with_json_body(serde_json::to_vec(&expired).unwrap()),
    );
    assert_eq!(upload.status, 201);

    let list = handle_request(
        &state,
        HttpRequest::new("GET", "/v1/activity?bucket=sync_bucket_test_002")
            .with_header("Aichan-Activity-Auth", "activity-auth-secret"),
    );
    assert_eq!(list.status, 200);
    let list_json: serde_json::Value = serde_json::from_slice(&list.body).unwrap();
    assert_eq!(list_json["count"], 0);
    assert_eq!(list_json["events"].as_array().unwrap().len(), 0);
}
