//! Behaviour of the isolated subscription-import port fixture.

use infiltrator_ports::subscription_import::SubscriptionImportPort;
use infiltrator_test_support::subscription_import_fixtures::{
    CAPTURE_CLIPBOARD_TEXT, CAPTURE_FILE_DOCUMENT, CAPTURE_FILE_PATH, SubscriptionImportCapture,
};
use lurker_core_contract::capability::Capability;
use lurker_core_ports::error::PortError;

#[tokio::test]
async fn the_local_file_is_read() {
    let host = SubscriptionImportCapture::new();

    let document = host
        .read_local_file(CAPTURE_FILE_PATH)
        .await
        .expect("local import document");

    assert_eq!(document, CAPTURE_FILE_DOCUMENT);
}

#[tokio::test]
async fn an_unknown_path_is_not_found() {
    let host = SubscriptionImportCapture::new();

    let error = host
        .read_local_file("/isolated-import/missing.yaml")
        .await
        .expect_err("an unknown path must not be found");

    assert!(matches!(error, PortError::NotFound(_)));
}

#[tokio::test]
async fn the_clipboard_is_read() {
    let host = SubscriptionImportCapture::new();

    let text = host.read_clipboard().await.expect("clipboard text");

    assert_eq!(text, CAPTURE_CLIPBOARD_TEXT);
}

#[tokio::test]
async fn a_host_without_clipboard_reports_typed_unsupported() {
    let host = SubscriptionImportCapture::without_clipboard();

    let error = host
        .read_clipboard()
        .await
        .expect_err("the clipboard must be refused");

    match error {
        PortError::Unsupported { capability, .. } => {
            assert_eq!(capability, Capability::Profiles);
        }
        other => panic!("expected typed unsupported, got {other:?}"),
    }
    assert!(host.clipboard_text().is_none());
}
