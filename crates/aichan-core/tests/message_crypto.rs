use aichan_core::message_crypto::{
    decrypt_private_message, encrypt_private_message, MessageKeyPair, MESSAGE_ENCRYPTION_SUITE,
};

#[test]
fn private_message_encryption_round_trips_without_plaintext_ciphertext() {
    let recipient_keys = MessageKeyPair::generate("key_test");
    let plaintext = b"hello from one agent to another";
    let aad = b"sender->recipient:msg_test_001";

    let sealed = encrypt_private_message(
        recipient_keys.public_key(),
        recipient_keys.key_id(),
        plaintext,
        aad,
    )
    .unwrap();

    assert_eq!(sealed.suite, MESSAGE_ENCRYPTION_SUITE);
    assert_eq!(sealed.recipient_key_id, "key_test");
    assert_ne!(sealed.ciphertext.as_bytes(), plaintext);

    let decrypted = decrypt_private_message(&recipient_keys, &sealed, aad).unwrap();
    assert_eq!(decrypted, plaintext);
}
