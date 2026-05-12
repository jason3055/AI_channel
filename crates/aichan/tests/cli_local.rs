use assert_cmd::Command;
use predicates::prelude::*;

fn aichan() -> Command {
    Command::cargo_bin("aichan").unwrap()
}

#[test]
fn identity_creates_and_reuses_local_identity() {
    let temp = tempfile::tempdir().unwrap();

    let mut first = aichan();
    first.current_dir(temp.path()).arg("identity");
    first
        .assert()
        .success()
        .stdout(predicate::str::contains("peer_"));

    let identity_path = temp.path().join(".aichan/identity.json");
    assert!(identity_path.exists());

    let first_file = std::fs::read_to_string(&identity_path).unwrap();

    let mut second = aichan();
    second
        .current_dir(temp.path())
        .arg("identity")
        .arg("--json");
    second
        .assert()
        .success()
        .stdout(predicate::str::contains("peer_"));

    let second_file = std::fs::read_to_string(&identity_path).unwrap();
    assert_eq!(first_file, second_file);
}

#[test]
fn status_creates_device_and_memory_without_network() {
    let temp = tempfile::tempdir().unwrap();

    let mut cmd = aichan();
    cmd.current_dir(temp.path()).arg("status");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("device_"))
        .stdout(predicate::str::contains("last_sync_at: never"));

    assert!(temp.path().join(".aichan/device.json").exists());
    assert!(temp.path().join(".aichan/memory.json").exists());
}
