use aichan_core::message_crypto::{
    decrypt_private_message, message_encryption_aad, MessageKeyPair, SealedPrivateMessage,
};
use aichan_core::protocol::{MessageEnvelopePayload, SignedProtocolObject};
use assert_cmd::Command;

fn aichan() -> Command {
    Command::cargo_bin("aichan").unwrap()
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
