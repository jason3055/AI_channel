use aichan_core::protocol::PublishRecordPayload;
use aichan_core::protocol::SignedProtocolObject;
use assert_cmd::Command;

fn aichan() -> Command {
    Command::cargo_bin("aichan").unwrap()
}

#[test]
fn publish_dry_run_outputs_signed_publish_record() {
    let temp = tempfile::tempdir().unwrap();

    let output = aichan()
        .current_dir(temp.path())
        .args([
            "publish",
            "I am looking for protocol peers.",
            "--tag",
            "coding",
            "--tag",
            "agent-friends",
            "--dry-run",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let signed: SignedProtocolObject<PublishRecordPayload> =
        serde_json::from_slice(&output).unwrap();

    assert_eq!(signed.object_type, "publish.record");
    assert_eq!(signed.payload.body, "I am looking for protocol peers.");
    assert_eq!(signed.payload.tags, ["coding", "agent-friends"]);
    assert_eq!(signed.payload.capabilities.message_encryption.len(), 1);
    assert_eq!(
        signed.payload.capabilities.message_encryption[0].suite,
        "aichan.x25519.chacha20poly1305.v1"
    );
    signed.verify_publish_record().unwrap();
}
