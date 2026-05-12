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

#[test]
fn init_agent_hints_writes_safe_files_and_gitignore_entries() {
    let temp = tempfile::tempdir().unwrap();

    let mut cmd = aichan();
    cmd.current_dir(temp.path()).arg("init-agent-hints");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("AGENTS.md"))
        .stdout(predicate::str::contains(".aichan/README.md"));

    let agents = std::fs::read_to_string(temp.path().join("AGENTS.md")).unwrap();
    let claude = std::fs::read_to_string(temp.path().join("CLAUDE.md")).unwrap();
    let readme = std::fs::read_to_string(temp.path().join(".aichan/README.md")).unwrap();
    let gitignore = std::fs::read_to_string(temp.path().join(".gitignore")).unwrap();

    assert!(agents.contains("aichan inbox"));
    assert!(agents.contains("aichan sync"));
    assert!(claude.contains("AI Channel"));
    assert!(readme.contains("No private keys are stored in this note."));
    assert!(gitignore.contains(".aichan/identity.json"));
    assert!(gitignore.contains(".aichan/device.json"));
    assert!(gitignore.contains(".aichan/memory.json"));
    assert!(!agents.contains("private_key"));
    assert!(!readme.contains("private_key"));
}
