//! Behavior cases for invalid.
//! test-intent: behavior

use super::*;

#[tokio::test]
async fn invalid_names_are_rejected_before_store_access() {
    let application = ProfileApplication::new(Arc::new(FakeStore::default()));
    let failure = application
        .load_profile_info("../outside")
        .await
        .expect_err("path-like name must fail");
    assert_eq!(failure.code, ErrorCode::InvalidInput);
}
