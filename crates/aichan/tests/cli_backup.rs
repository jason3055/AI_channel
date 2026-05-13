use aichan_core::{DeviceFile, IdentityFile, MemoryFile};
use assert_cmd::Command;
use predicates::prelude::*;

fn aichan() -> Command {
    Command::cargo_bin("aichan").unwrap()
}

#[test]
fn backup_create_and_restore_round_trip_same_peer_with_new_device() {
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let backup_path = source.path().join("agent.aichan-backup");

    aichan()
        .current_dir(source.path())
        .arg("status")
        .assert()
        .success();

    let source_identity = IdentityFile::read_from(source.path().join(".aichan/identity.json"))
        .expect("source identity should exist");
    let source_device = DeviceFile::read_from(source.path().join(".aichan/device.json"))
        .expect("source device should exist");
    let mut memory = MemoryFile::read_from(source.path().join(".aichan/memory.json"))
        .expect("source memory should exist");
    memory.profile.nickname = Some("backup-test-agent".to_string());
    std::fs::write(
        source.path().join(".aichan/memory.json"),
        serde_json::to_vec_pretty(&memory).unwrap(),
    )
    .unwrap();

    let output = aichan()
        .current_dir(source.path())
        .args([
            "--json",
            "backup",
            "create",
            "--output",
            backup_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("recovery_phrase"))
        .get_output()
        .stdout
        .clone();

    let create_json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let recovery_phrase = create_json["recovery_phrase"].as_str().unwrap();
    assert!(backup_path.exists());
    assert_eq!(create_json["peer_id"], source_identity.peer_id.as_str());

    let backup_text = std::fs::read_to_string(&backup_path).unwrap();
    assert!(!backup_text.contains(&source_identity.private_key));
    assert!(!backup_text.contains(&source_identity.public_key));
    assert!(!backup_text.contains("backup-test-agent"));

    aichan()
        .current_dir(target.path())
        .env("AICHAN_RECOVERY_PHRASE", recovery_phrase)
        .args([
            "--json",
            "backup",
            "restore",
            "--file",
            backup_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("restored"));

    let restored_identity = IdentityFile::read_from(target.path().join(".aichan/identity.json"))
        .expect("restored identity should exist");
    let restored_device = DeviceFile::read_from(target.path().join(".aichan/device.json"))
        .expect("restored device should exist");
    let restored_memory = MemoryFile::read_from(target.path().join(".aichan/memory.json"))
        .expect("restored memory should exist");

    assert_eq!(restored_identity.peer_id, source_identity.peer_id);
    assert_ne!(restored_device.device_id, source_device.device_id);
    assert_eq!(
        restored_memory.profile.nickname.as_deref(),
        Some("backup-test-agent")
    );

    let metadata = std::fs::read_to_string(target.path().join(".aichan/backup.json")).unwrap();
    assert!(metadata.contains("last_restore_at"));
    assert!(!metadata.contains(recovery_phrase));
}

#[test]
fn backup_restore_rejects_wrong_recovery_phrase_without_writing_identity() {
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let backup_path = source.path().join("agent.aichan-backup");

    aichan()
        .current_dir(source.path())
        .args([
            "--json",
            "backup",
            "create",
            "--output",
            backup_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    aichan()
        .current_dir(target.path())
        .env("AICHAN_RECOVERY_PHRASE", "aichan-rp-wrong")
        .args([
            "--json",
            "backup",
            "restore",
            "--file",
            backup_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("backup decryption failed"));

    assert!(!target.path().join(".aichan/identity.json").exists());
}
