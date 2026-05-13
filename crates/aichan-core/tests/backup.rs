use aichan_core::{derive_hosted_backup_locator, generate_recovery_phrase};

#[test]
fn hosted_backup_locator_is_stable_and_non_secret() {
    let recovery_phrase = generate_recovery_phrase();

    let first = derive_hosted_backup_locator(&recovery_phrase).unwrap();
    let second = derive_hosted_backup_locator(&recovery_phrase).unwrap();

    assert_eq!(first, second);
    assert!(first.backup_lookup_id.starts_with("bak_"));
    assert!(first.backup_auth_token.starts_with("auth_"));
    assert!(!first.backup_lookup_id.contains(&recovery_phrase));
    assert!(!first.backup_auth_token.contains(&recovery_phrase));
    assert_ne!(first.backup_lookup_id, first.backup_auth_token);
}

#[test]
fn hosted_backup_locator_rejects_invalid_recovery_phrase_format() {
    let error = derive_hosted_backup_locator("not-a-recovery-phrase").unwrap_err();

    assert!(error
        .to_string()
        .contains("recovery phrase has invalid format"));
}
