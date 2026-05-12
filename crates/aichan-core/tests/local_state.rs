use aichan_core::{derive_peer_id, IdentityFile, LocalStateDir};

#[test]
fn local_state_paths_point_under_dot_aichan() {
    let temp = tempfile::tempdir().unwrap();
    let state = LocalStateDir::new(temp.path());

    assert_eq!(state.root(), temp.path().join(".aichan"));
    assert_eq!(
        state.identity_path(),
        temp.path().join(".aichan/identity.json")
    );
    assert_eq!(state.device_path(), temp.path().join(".aichan/device.json"));
    assert_eq!(state.memory_path(), temp.path().join(".aichan/memory.json"));
    assert_eq!(state.config_path(), temp.path().join(".aichan/config.json"));
    assert_eq!(
        state.backup_metadata_path(),
        temp.path().join(".aichan/backup.json")
    );
    assert_eq!(
        state.inbox_cache_dir(),
        temp.path().join(".aichan/inbox-cache")
    );
}

#[test]
fn ensure_dirs_creates_root_and_cache_dirs() {
    let temp = tempfile::tempdir().unwrap();
    let state = LocalStateDir::new(temp.path());

    state.ensure_dirs().unwrap();

    assert!(state.root().is_dir());
    assert!(state.inbox_cache_dir().is_dir());
}

#[test]
fn derive_peer_id_is_stable_and_public_key_based() {
    let public_key = [7_u8; 32];
    let first = derive_peer_id(&public_key);
    let second = derive_peer_id(&public_key);

    assert_eq!(first, second);
    assert!(first.as_str().starts_with("peer_"));
    assert_eq!(first.as_str().len(), 29);
}

#[test]
fn identity_create_or_load_reuses_existing_identity() {
    let temp = tempfile::tempdir().unwrap();
    let state = LocalStateDir::new(temp.path());

    let first = IdentityFile::create_or_load(&state).unwrap();
    let second = IdentityFile::create_or_load(&state).unwrap();

    assert_eq!(first.peer_id, second.peer_id);
    assert_eq!(first.public_key, second.public_key);
    assert_eq!(first.private_key, second.private_key);
    assert!(!first.private_key_encrypted);
}

#[test]
fn identity_read_rejects_mismatched_peer_id() {
    let temp = tempfile::tempdir().unwrap();
    let state = LocalStateDir::new(temp.path());
    IdentityFile::create_or_load(&state).unwrap();
    let path = state.identity_path();
    let mut identity: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    identity["peer_id"] = serde_json::Value::String(derive_peer_id(&[9_u8; 32]).to_string());
    std::fs::write(&path, serde_json::to_vec_pretty(&identity).unwrap()).unwrap();

    assert!(IdentityFile::read_from(path).is_err());
}

#[test]
fn identity_read_rejects_invalid_key_encoding_or_length() {
    for (field, value) in [
        ("public_key", "not base64!"),
        ("public_key", "AQID"),
        ("private_key", "not base64!"),
        ("private_key", "AQID"),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let state = LocalStateDir::new(temp.path());
        IdentityFile::create_or_load(&state).unwrap();
        let path = state.identity_path();
        let mut identity: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        identity[field] = serde_json::Value::String(value.to_string());
        std::fs::write(&path, serde_json::to_vec_pretty(&identity).unwrap()).unwrap();

        assert!(IdentityFile::read_from(path).is_err(), "{field}={value}");
    }
}

#[test]
fn identity_read_rejects_private_key_that_does_not_match_public_key() {
    let public_temp = tempfile::tempdir().unwrap();
    let public_state = LocalStateDir::new(public_temp.path());
    IdentityFile::create_or_load(&public_state).unwrap();
    let private_temp = tempfile::tempdir().unwrap();
    let private_state = LocalStateDir::new(private_temp.path());
    IdentityFile::create_or_load(&private_state).unwrap();

    let path = public_state.identity_path();
    let private_identity: serde_json::Value =
        serde_json::from_slice(&std::fs::read(private_state.identity_path()).unwrap()).unwrap();
    let mut identity: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    identity["private_key"] = private_identity["private_key"].clone();
    std::fs::write(&path, serde_json::to_vec_pretty(&identity).unwrap()).unwrap();

    assert!(IdentityFile::read_from(path).is_err());
}

#[cfg(unix)]
#[test]
fn identity_file_is_written_with_restrictive_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let state = LocalStateDir::new(temp.path());

    IdentityFile::create_or_load(&state).unwrap();

    let mode = std::fs::metadata(state.identity_path())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}
