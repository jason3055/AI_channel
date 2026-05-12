use aichan_core::LocalStateDir;

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
