use assert_cmd::Command;
use predicates::prelude::*;

fn aichan() -> Command {
    Command::cargo_bin("aichan").unwrap()
}

#[test]
fn version_flag_reports_cli_version() {
    let mut cmd = aichan();
    cmd.arg("--version");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("aichan"))
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn upgrade_dry_run_reports_release_first_plan_without_cargo_logs() {
    let output = aichan()
        .args(["--json", "upgrade", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("release_then_cargo"))
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["upgraded"], false);
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["current_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(value["strategy"], "release_then_cargo");
    assert!(value["release"]["latest_api_url"]
        .as_str()
        .unwrap()
        .contains("/repos/aftershower/AI_channel/releases/latest"));
    assert!(value["release"]["asset_name"]
        .as_str()
        .unwrap()
        .starts_with("aichan-"));
    assert!(value.get("stdout").is_none());
    assert!(value.get("stderr").is_none());

    let command = value["fallback_command"].as_array().unwrap();
    let command = command
        .iter()
        .map(|part| part.as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(command.first().copied(), Some("cargo"));
    assert!(command
        .windows(2)
        .any(|parts| parts == ["install", "--git"]));
    assert!(command.contains(&"https://github.com/aftershower/AI_channel"));
    assert!(command.contains(&"aichan"));
    assert!(command.contains(&"--locked"));
    assert!(command.contains(&"--force"));
}

#[test]
fn identity_creates_and_reuses_local_identity() {
    let temp = tempfile::tempdir().unwrap();

    let mut first = aichan();
    first
        .env("AICHAN_HOME", temp.path())
        .current_dir(temp.path())
        .arg("identity");
    first
        .assert()
        .success()
        .stdout(predicate::str::contains("peer_"));

    let identity_path = temp.path().join(".aichan/identity.json");
    assert!(identity_path.exists());

    let first_file = std::fs::read_to_string(&identity_path).unwrap();

    let mut second = aichan();
    second
        .env("AICHAN_HOME", temp.path())
        .current_dir(temp.path())
        .arg("identity")
        .arg("--json");
    let second_output = second
        .assert()
        .success()
        .stdout(predicate::str::contains("peer_"))
        .get_output()
        .stdout
        .clone();
    let second_json: serde_json::Value = serde_json::from_slice(&second_output).unwrap();
    assert!(second_json.get("private_key").is_none());
    assert!(second_json.get("private_key_encrypted").is_some());

    let second_file = std::fs::read_to_string(&identity_path).unwrap();
    assert_eq!(first_file, second_file);
}

#[test]
fn default_identity_prefers_home_before_project_state() {
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let neutral = tempfile::tempdir().unwrap();

    let home_output = aichan()
        .env("HOME", home.path())
        .current_dir(neutral.path())
        .args(["identity", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let home_json: serde_json::Value = serde_json::from_slice(&home_output).unwrap();
    let home_peer = home_json["peer_id"].as_str().unwrap().to_string();

    let project_output = aichan()
        .env("HOME", home.path())
        .args(["--project-dir"])
        .arg(project.path())
        .args(["identity", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let project_json: serde_json::Value = serde_json::from_slice(&project_output).unwrap();
    let project_peer = project_json["peer_id"].as_str().unwrap().to_string();
    assert_ne!(home_peer, project_peer);

    let default_output = aichan()
        .env("HOME", home.path())
        .current_dir(project.path())
        .args(["identity", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let default_json: serde_json::Value = serde_json::from_slice(&default_output).unwrap();
    assert_eq!(default_json["peer_id"], home_peer);
    assert_eq!(
        default_json["identity_file"],
        home.path()
            .join(".aichan/identity.json")
            .display()
            .to_string()
    );

    let explicit_output = aichan()
        .env("HOME", home.path())
        .args(["--project-dir"])
        .arg(project.path())
        .args(["identity", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let explicit_json: serde_json::Value = serde_json::from_slice(&explicit_output).unwrap();
    assert_eq!(explicit_json["peer_id"], project_peer);
}

#[test]
fn status_creates_device_and_memory_without_network() {
    let temp = tempfile::tempdir().unwrap();

    let mut cmd = aichan();
    cmd.env("AICHAN_HOME", temp.path())
        .current_dir(temp.path())
        .arg("status");

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
    assert!(agents.contains("aichan upgrade"));
    assert!(agents.contains("aichan sync"));
    assert!(claude.contains("AI Channel"));
    assert!(claude.contains("aichan upgrade"));
    assert!(readme.contains("No private keys are stored in this note."));
    assert!(gitignore.contains(".aichan/identity.json"));
    assert!(gitignore.contains(".aichan/device.json"));
    assert!(gitignore.contains(".aichan/memory.json"));
    assert!(gitignore.contains(".aichan/recipient-key-cache.json"));
    assert!(gitignore.contains(".aichan/inbox-cache/"));
    assert!(gitignore.contains(".aichan/peer-messages/"));
    assert!(gitignore.contains(".aichan/transcripts/"));
    assert!(!agents.contains("private_key"));
    assert!(!readme.contains("private_key"));
}

#[test]
fn init_agent_hints_preserves_existing_guidance_files() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join(".aichan")).unwrap();
    std::fs::write(
        temp.path().join("AGENTS.md"),
        "# Existing agent rules\n\nKeep this project-specific guidance.\n",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("CLAUDE.md"),
        "# Existing Claude rules\n\nKeep these Claude-specific notes.\n",
    )
    .unwrap();
    std::fs::write(
        temp.path().join(".aichan/README.md"),
        "# Existing AI Channel notes\n\nKeep this local context.\n",
    )
    .unwrap();

    let mut cmd = aichan();
    cmd.current_dir(temp.path()).arg("init-agent-hints");
    cmd.assert().success();

    let agents = std::fs::read_to_string(temp.path().join("AGENTS.md")).unwrap();
    let claude = std::fs::read_to_string(temp.path().join("CLAUDE.md")).unwrap();
    let readme = std::fs::read_to_string(temp.path().join(".aichan/README.md")).unwrap();

    assert!(agents.contains("Keep this project-specific guidance."));
    assert!(agents.contains("<!-- BEGIN AICHAN -->"));
    assert!(agents.contains("aichan inbox"));
    assert!(agents.contains("aichan upgrade"));
    assert!(claude.contains("Keep these Claude-specific notes."));
    assert!(claude.contains("<!-- BEGIN AICHAN -->"));
    assert!(claude.contains("AI Channel"));
    assert!(claude.contains("aichan upgrade"));
    assert!(readme.contains("Keep this local context."));
    assert!(readme.contains("<!-- BEGIN AICHAN -->"));
    assert!(readme.contains("No private keys are stored in this note."));
}

#[test]
fn init_agent_hints_is_idempotent_for_blocks_and_gitignore() {
    let temp = tempfile::tempdir().unwrap();

    for _ in 0..2 {
        let mut cmd = aichan();
        cmd.current_dir(temp.path()).arg("init-agent-hints");
        cmd.assert().success();
    }

    let agents = std::fs::read_to_string(temp.path().join("AGENTS.md")).unwrap();
    let claude = std::fs::read_to_string(temp.path().join("CLAUDE.md")).unwrap();
    let readme = std::fs::read_to_string(temp.path().join(".aichan/README.md")).unwrap();
    let gitignore = std::fs::read_to_string(temp.path().join(".gitignore")).unwrap();

    assert_eq!(agents.matches("<!-- BEGIN AICHAN -->").count(), 1);
    assert_eq!(agents.matches("<!-- END AICHAN -->").count(), 1);
    assert_eq!(claude.matches("<!-- BEGIN AICHAN -->").count(), 1);
    assert_eq!(claude.matches("<!-- END AICHAN -->").count(), 1);
    assert_eq!(readme.matches("<!-- BEGIN AICHAN -->").count(), 1);
    assert_eq!(readme.matches("<!-- END AICHAN -->").count(), 1);

    for entry in [
        ".aichan/identity.json",
        ".aichan/device.json",
        ".aichan/memory.json",
        ".aichan/recipient-key-cache.json",
        ".aichan/inbox-cache/",
        ".aichan/peer-messages/",
        ".aichan/transcripts/",
    ] {
        assert_eq!(gitignore.lines().filter(|line| *line == entry).count(), 1);
    }
}
