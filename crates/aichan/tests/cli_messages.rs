use aichan_core::message_crypto::{
    decrypt_private_message, message_encryption_aad, MessageKeyPair, SealedPrivateMessage,
};
use aichan_core::protocol::{MessageEnvelopePayload, SignedProtocolObject};
use aichan_core::{IdentityFile, LocalStateDir};
use assert_cmd::Command;
use chrono::Utc;
use predicates::prelude::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

fn aichan() -> Command {
    Command::cargo_bin("aichan").unwrap()
}

fn start_test_server() -> (tempfile::TempDir, String) {
    let data_dir = tempfile::tempdir().unwrap();
    let addr = reserve_local_addr();
    let base_url = format!("http://{addr}");
    let state = aichan_server::ServerState::with_public_base_url(data_dir.path(), base_url.clone())
        .unwrap();
    let run_addr = addr.clone();
    thread::spawn(move || {
        aichan_server::run(&run_addr, state).unwrap();
    });
    wait_for_server(&addr);
    (data_dir, base_url)
}

fn reserve_local_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    addr.to_string()
}

fn wait_for_server(addr: &str) {
    for _ in 0..100 {
        if let Ok(mut stream) = TcpStream::connect(addr) {
            stream
                .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let mut response = String::new();
            let _ = stream.read_to_string(&mut response);
            if response.contains("200 OK") {
                return;
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("test server did not become ready at {addr}");
}

#[test]
fn send_dry_run_outputs_encrypted_message_envelope() {
    let temp = tempfile::tempdir().unwrap();
    let recipient_keys = MessageKeyPair::generate("key_test");

    let output = aichan()
        .current_dir(temp.path())
        .args([
            "--json",
            "send",
            "peer_recipient_for_test",
            "hello private agent",
            "--recipient-key-id",
            recipient_keys.key_id(),
            "--recipient-public-key",
            recipient_keys.public_key(),
            "--dry-run",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let signed: SignedProtocolObject<MessageEnvelopePayload> =
        serde_json::from_slice(&output).unwrap();

    assert_eq!(signed.object_type, "message.envelope");
    assert_eq!(signed.payload.recipient.as_str(), "peer_recipient_for_test");
    assert_eq!(
        signed.payload.encryption.suite,
        "aichan.x25519.chacha20poly1305.v1"
    );
    assert!(!signed.payload.ciphertext.contains("hello private agent"));
    signed.verify_message_envelope().unwrap();

    let sealed = SealedPrivateMessage {
        suite: signed.payload.encryption.suite.clone(),
        recipient_key_id: signed.payload.encryption.recipient_key_id.clone(),
        ephemeral_public_key: signed.payload.encryption.ephemeral_public_key.clone(),
        nonce: signed.payload.encryption.nonce.clone(),
        ciphertext: signed.payload.ciphertext.clone(),
    };
    let aad = message_encryption_aad(
        &signed.id,
        signed.payload.sender.as_str(),
        signed.payload.recipient.as_str(),
        &signed.created_at.to_rfc3339(),
    );
    let plaintext = decrypt_private_message(&recipient_keys, &sealed, &aad).unwrap();
    let body: serde_json::Value = serde_json::from_slice(&plaintext).unwrap();
    assert_eq!(body["body"], "hello private agent");
}

#[test]
fn send_dry_run_uses_cached_recipient_message_key_without_network_lookup() {
    let temp = tempfile::tempdir().unwrap();
    let recipient_keys = MessageKeyPair::generate("key_cached");
    let recipient = "peer_recipient_cached";
    std::fs::create_dir(temp.path().join(".aichan")).unwrap();
    std::fs::write(
        temp.path().join(".aichan/recipient-key-cache.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "version": 1,
            "peers": [{
                "peer_id": recipient,
                "suite": "aichan.x25519.chacha20poly1305.v1",
                "key_id": recipient_keys.key_id(),
                "public_key": recipient_keys.public_key(),
                "updated_at": "2026-05-13T16:00:00Z"
            }]
        }))
        .unwrap(),
    )
    .unwrap();

    let output = aichan()
        .current_dir(temp.path())
        .args([
            "--json",
            "send",
            recipient,
            "cached hello",
            "--base-url",
            "http://127.0.0.1:9",
            "--dry-run",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let signed: SignedProtocolObject<MessageEnvelopePayload> =
        serde_json::from_slice(&output).unwrap();
    assert_eq!(signed.payload.recipient.as_str(), recipient);
    assert_eq!(
        signed.payload.encryption.recipient_key_id,
        recipient_keys.key_id()
    );
}

#[test]
fn inbox_rejects_relay_records_with_invalid_message_signature() {
    let (server_dir, base_url) = start_test_server();
    let sender = tempfile::tempdir().unwrap();
    let recipient = tempfile::tempdir().unwrap();
    let recipient_state = LocalStateDir::new(recipient.path());
    let recipient_identity = IdentityFile::create_or_load(&recipient_state).unwrap();
    let recipient_keys = recipient_identity.message_key_pair().unwrap();

    let output = aichan()
        .current_dir(sender.path())
        .args([
            "--json",
            "send",
            recipient_identity.peer_id.as_str(),
            "relay tampered hello",
            "--recipient-key-id",
            recipient_keys.key_id(),
            "--recipient-public-key",
            recipient_keys.public_key(),
            "--dry-run",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let mut record: serde_json::Value = serde_json::from_slice(&output).unwrap();
    record["payload"]["content_encoding"] = serde_json::json!("tampered/aichan");
    std::fs::write(
        server_dir.path().join("message_envelopes.json"),
        serde_json::to_vec_pretty(&serde_json::json!([{
            "object": record,
            "stored_at": Utc::now(),
        }]))
        .unwrap(),
    )
    .unwrap();

    aichan()
        .current_dir(recipient.path())
        .args(["inbox", "--base-url", &base_url])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "message envelope verification failed",
        ));
}
