use aichan_core::{DeviceFile, IdentityFile, MemoryFile};
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
fn sync_round_trips_memory_summary_between_restored_devices() {
    let (_server_dir, base_url) = start_test_server();
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
    let backup_output = aichan()
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
        .get_output()
        .stdout
        .clone();
    let backup_json: serde_json::Value = serde_json::from_slice(&backup_output).unwrap();
    let recovery_phrase = backup_json["recovery_phrase"].as_str().unwrap();

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
        .success();

    let source_identity = IdentityFile::read_from(source.path().join(".aichan/identity.json"))
        .expect("source identity should exist");
    let target_identity = IdentityFile::read_from(target.path().join(".aichan/identity.json"))
        .expect("target identity should exist");
    let source_device = DeviceFile::read_from(source.path().join(".aichan/device.json"))
        .expect("source device should exist");
    let target_device = DeviceFile::read_from(target.path().join(".aichan/device.json"))
        .expect("target device should exist");
    assert_eq!(source_identity.peer_id, target_identity.peer_id);
    assert_ne!(source_device.device_id, target_device.device_id);

    let mut source_memory = MemoryFile::read_from(source.path().join(".aichan/memory.json"))
        .expect("source memory should exist");
    source_memory.profile.nickname = Some("synced-source-agent".to_string());
    source_memory.common_tags.push("activity-sync".to_string());
    source_memory.updated_at = Utc::now();
    std::fs::write(
        source.path().join(".aichan/memory.json"),
        serde_json::to_vec_pretty(&source_memory).unwrap(),
    )
    .unwrap();

    aichan()
        .current_dir(source.path())
        .args(["--json", "sync", "--base-url", base_url.as_str()])
        .assert()
        .success()
        .stdout(predicate::str::contains("uploaded"));

    let sync_output = aichan()
        .current_dir(target.path())
        .args(["--json", "sync", "--base-url", base_url.as_str()])
        .assert()
        .success()
        .stdout(predicate::str::contains("applied"))
        .get_output()
        .stdout
        .clone();
    let sync_json: serde_json::Value = serde_json::from_slice(&sync_output).unwrap();
    assert!(sync_json["pulled"].as_u64().unwrap() >= 1);

    let target_memory = MemoryFile::read_from(target.path().join(".aichan/memory.json"))
        .expect("target memory should exist");
    assert_eq!(
        target_memory.profile.nickname.as_deref(),
        Some("synced-source-agent")
    );
    assert!(target_memory
        .common_tags
        .iter()
        .any(|tag| tag == "activity-sync"));
    assert!(target_memory.sync.last_sync_at.is_some());
}

#[test]
fn status_warns_when_device_is_near_sync_window_edge() {
    let temp = tempfile::tempdir().unwrap();
    aichan()
        .current_dir(temp.path())
        .arg("status")
        .assert()
        .success();

    let mut memory = MemoryFile::read_from(temp.path().join(".aichan/memory.json")).unwrap();
    memory.sync.last_sync_at = Some(Utc::now() - chrono::Duration::days(6));
    std::fs::write(
        temp.path().join(".aichan/memory.json"),
        serde_json::to_vec_pretty(&memory).unwrap(),
    )
    .unwrap();

    let output = aichan()
        .current_dir(temp.path())
        .args(["--json", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("sync_warning"))
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["sync_warning"]["level"], "warning");
    assert!(value["sync_warning"]["message"]
        .as_str()
        .unwrap()
        .contains("approaching"));
}
