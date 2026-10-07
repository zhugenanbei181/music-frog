//! Behavior cases for auth.
//! test-intent: behavior

use super::*;

#[test]
fn test_auth_token_operations() {
    let token1 = AuthToken::generate();
    let token2 = AuthToken::generate();
    assert_ne!(token1.secret(), token2.secret());
    assert!(token1.verify(token1.secret()));
    assert!(!token1.verify(token2.secret()));
    assert!(!token1.verify("invalid_token_candidate"));

    let manual = AuthToken::new("my-custom-shared-secret");
    assert_eq!(manual.secret(), "my-custom-shared-secret");
    assert!(manual.verify("my-custom-shared-secret"));
}

#[tokio::test]
async fn test_auth_token_file_save_and_load() {
    let temp_dir = TempDir::new().unwrap();
    let token_path = temp_dir.path().join("nested").join("service.token");
    let token = AuthToken::generate();

    token
        .save_to_file(&token_path)
        .await
        .expect("Failed to save auth token");
    assert!(token_path.exists());

    let loaded = AuthToken::load_from_file(&token_path)
        .await
        .expect("Failed to load auth token");
    assert_eq!(token.secret(), loaded.secret());
    assert!(loaded.verify(token.secret()));
}
