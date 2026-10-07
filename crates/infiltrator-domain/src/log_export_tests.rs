//! Actual byte projection, structural credentials and source identity.
use super::prepare_log_export;
use crate::profile_source::hash_document_bytes;
use crate::redact::redact_line;
use infiltrator_contract::logs::LogSession;
use infiltrator_contract::session::SessionToken;

#[test]
fn export_preserves_record_order_and_utf8_bytes_while_removing_structural_and_short_known_secrets()
{
    let records = vec![
        (11, "WARN[1] https://user:secret@host/x?token=abc&key=xyz".into()),
        (12, "Authorization: Basic ZGVtbzpwYXNzd29yZA==".into()),
        (13, "Proxy-Authorization: Digest username=\"alice\", nonce=\"nonce-secret\", response=\"digest-secret\"".into()),
        (14, "INFO[4] known credential 短; benign café".into()),
    ];
    let artifact = prepare_log_export(
        LogSession {
            generation: 2,
            token: SessionToken::new(19),
        },
        3,
        &records,
        &["短".into()],
    )
    .unwrap();
    assert_eq!(artifact.summary.records, 4);
    assert_eq!(artifact.summary.first_record, Some(11));
    assert_eq!(artifact.summary.last_record, Some(14));
    assert_eq!(artifact.summary.bytes, artifact.content.len());
    assert_eq!(
        artifact.summary.identity.sha256,
        hash_document_bytes(&artifact.content)
    );
    assert_eq!(
        artifact.content,
        "WARN[1] https://user:***@host/x?token=***&key=***\nAuthorization: ***\nProxy-Authorization: ***\nINFO[4] known credential ***; benign café\n"
    );
    for leaked in [
        "secret",
        "abc",
        "xyz",
        "ZGVtb",
        "alice",
        "nonce-secret",
        "digest-secret",
        "短",
    ] {
        assert!(
            !artifact.content.contains(leaked),
            "credential survived redaction"
        );
    }
}

#[test]
fn escaped_quotes_in_json_credentials_cannot_end_redaction_before_the_actual_value_boundary() {
    let raw = r#"{"authorization":"Digest username=\"alice\", nonce=\"credential\"","password":"ab\"escaped-secret","message":"café healthy"}"#;
    let masked = redact_line(raw, &[]);
    assert_eq!(
        masked,
        r#"{"authorization":"***","password":"***","message":"café healthy"}"#
    );
    assert_eq!(redact_line(&masked, &[]), masked);
}
