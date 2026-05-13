use aichan_core::{DeviceFile, IdentityFile, MemoryFile};
use assert_cmd::Command;
use predicates::prelude::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

fn aichan() -> Command {
    Command::cargo_bin("aichan").unwrap()
}

fn start_test_server() -> (tempfile::TempDir, String) {
    for _ in 0..20 {
        let data_dir = tempfile::tempdir().unwrap();
        let addr = reserve_local_addr();
        let base_url = format!("http://{addr}");
        let state =
            aichan_server::ServerState::with_public_base_url(data_dir.path(), base_url.clone())
                .unwrap();
        let run_addr = addr.clone();
        let handle = thread::spawn(move || {
            aichan_server::run(&run_addr, state).unwrap();
        });
        if wait_for_server(&addr, &handle) {
            return (data_dir, base_url);
        }
        let _ = handle.join();
    }
    panic!("test server did not become ready on a free local port");
}

fn reserve_local_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    addr.to_string()
}

fn wait_for_server(addr: &str, handle: &thread::JoinHandle<()>) -> bool {
    for _ in 0..100 {
        if handle.is_finished() {
            return false;
        }
        if let Ok(mut stream) = TcpStream::connect(addr) {
            stream
                .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let mut response = String::new();
            let _ = stream.read_to_string(&mut response);
            if response.contains("200 OK") {
                return true;
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
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

#[test]
fn backup_create_upload_failure_still_prints_recovery_phrase_and_local_backup() {
    let source = tempfile::tempdir().unwrap();
    let backup_path = source.path().join("agent.aichan-backup");

    let output = aichan()
        .current_dir(source.path())
        .args([
            "--json",
            "backup",
            "create",
            "--upload",
            "--base-url",
            "http://127.0.0.1:9",
            "--output",
            backup_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["created"], true);
    assert_eq!(value["hosted"]["uploaded"], false);
    assert!(value["hosted"]["upload_error"]
        .as_str()
        .unwrap()
        .contains("request PUT"));
    assert!(value["recovery_phrase"]
        .as_str()
        .unwrap()
        .starts_with("aichan-rp-"));
    assert!(backup_path.exists());
}

#[test]
fn backup_create_upload_and_hosted_restore_round_trip_same_peer_with_new_device() {
    let (server_dir, base_url) = start_test_server();
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
    memory.profile.nickname = Some("hosted-backup-agent".to_string());
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
            "--upload",
            "--base-url",
            base_url.as_str(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("hosted"))
        .get_output()
        .stdout
        .clone();

    let create_json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let recovery_phrase = create_json["recovery_phrase"].as_str().unwrap();
    let hosted = create_json["hosted"].as_object().unwrap();
    assert_eq!(hosted["uploaded"], true);
    assert!(hosted["backup_lookup_id"]
        .as_str()
        .unwrap()
        .starts_with("bak_"));
    assert!(hosted["generation_id"]
        .as_str()
        .unwrap()
        .starts_with("gen_"));

    let create_text = String::from_utf8(output).unwrap();
    assert!(!create_text.contains("auth_"));

    let source_metadata =
        std::fs::read_to_string(source.path().join(".aichan/backup.json")).unwrap();
    assert!(source_metadata.contains("backup_lookup_id"));
    assert!(source_metadata.contains("last_hosted_generation_id"));
    assert!(!source_metadata.contains(recovery_phrase));

    let hosted_store = std::fs::read_to_string(server_dir.path().join("hosted_backups.json"))
        .expect("hosted backup store should be written");
    assert!(!hosted_store.contains(&source_identity.private_key));
    assert!(!hosted_store.contains(&source_identity.public_key));
    assert!(!hosted_store.contains("hosted-backup-agent"));
    assert!(!hosted_store.contains(recovery_phrase));

    let restore_output = aichan()
        .current_dir(target.path())
        .env("AICHAN_RECOVERY_PHRASE", recovery_phrase)
        .args([
            "--json",
            "backup",
            "restore",
            "--base-url",
            base_url.as_str(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("hosted"))
        .get_output()
        .stdout
        .clone();

    let restore_json: serde_json::Value = serde_json::from_slice(&restore_output).unwrap();
    assert_eq!(restore_json["restored"], true);
    assert_eq!(restore_json["restore_source"], "hosted");
    assert_eq!(
        restore_json["hosted"]["generation_id"],
        create_json["hosted"]["generation_id"]
    );

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
        Some("hosted-backup-agent")
    );

    let target_metadata =
        std::fs::read_to_string(target.path().join(".aichan/backup.json")).unwrap();
    assert!(target_metadata.contains("backup_lookup_id"));
    assert!(target_metadata.contains("last_hosted_generation_id"));
    assert!(target_metadata.contains("hosted:"));
    assert!(!target_metadata.contains(recovery_phrase));
}
