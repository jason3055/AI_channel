use aichan_core::{
    decrypt_activity_snapshot, derive_activity_locator, encrypt_activity_snapshot, DeviceFile,
    IdentityFile, MemoryFile,
};

#[test]
fn activity_snapshot_encrypts_memory_for_same_identity_without_plaintext() {
    let temp = tempfile::tempdir().unwrap();
    let state = aichan_core::LocalStateDir::new(temp.path());
    let identity = IdentityFile::create_or_load(&state).unwrap();
    let device = DeviceFile::create_or_load(&state).unwrap();
    let mut memory = MemoryFile::create_or_load(&state).unwrap();
    memory.profile.nickname = Some("activity-test-agent".to_string());

    let locator = derive_activity_locator(&identity).unwrap();
    let event = encrypt_activity_snapshot(&identity, device.device_id, &memory).unwrap();
    let event_text = serde_json::to_string(&event).unwrap();

    assert!(locator.bucket_id.starts_with("sync_"));
    assert!(locator.auth_token.starts_with("auth_"));
    assert!(!event_text.contains("activity-test-agent"));
    assert_eq!(event.version, 1);

    let payload = decrypt_activity_snapshot(&identity, &event).unwrap();
    assert_eq!(
        payload.memory.profile.nickname.as_deref(),
        Some("activity-test-agent")
    );
}
