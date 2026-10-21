//! Behaviour of the isolated profile-document codec fixture.

use infiltrator_test_support::profile_document_codec_fixtures::ProfileDocumentCodecCapture;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCaret, ProfileDocumentCodecPort, ProfileDocumentSnippetOutcome,
};

const DOCUMENT: &str =
    "mixed-port: 7890\nproxies: []\nproxy-groups: []\nrules:\n  - MATCH,DIRECT\n";

#[test]
fn a_valid_document_inspects_as_valid() {
    let codec = ProfileDocumentCodecCapture::with_snippets();

    let validation = codec.inspect(DOCUMENT.as_bytes()).expect("inspection");

    assert_eq!(validation, DocumentValidation::Valid);
}

#[test]
fn an_invalid_document_reports_the_parser_position() {
    let codec = ProfileDocumentCodecCapture::with_snippets();

    let validation = codec
        .inspect(b"port: 7890\nmode: : rule\n")
        .expect("inspection");

    match validation {
        DocumentValidation::Invalid { diagnostics } => {
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics[0].line.is_some());
            assert!(!diagnostics[0].message.is_empty());
        }
        other => panic!("expected an invalid document, got {other:?}"),
    }
}

#[test]
fn a_snippet_is_spliced_at_the_caret() {
    let codec = ProfileDocumentCodecCapture::with_snippets();

    let outcome = codec
        .insert_snippet(
            DOCUMENT,
            "rule-domain",
            ProfileDocumentCaret { line: 5, column: 0 },
        )
        .expect("snippet insertion");

    match outcome {
        ProfileDocumentSnippetOutcome::Inserted(insertion) => {
            assert_eq!(insertion.snippet_id, "rule-domain");
            assert!(
                insertion
                    .content
                    .contains("DOMAIN-SUFFIX,google.com,PROXIES")
            );
            assert_eq!(insertion.validation, DocumentValidation::Valid);
        }
        ProfileDocumentSnippetOutcome::Rejected { .. } => {
            panic!("the snippet must be inserted into a valid document")
        }
    }
}

#[test]
fn an_unknown_snippet_is_a_typed_failure() {
    let codec = ProfileDocumentCodecCapture::with_snippets();

    let error = codec
        .insert_snippet(
            DOCUMENT,
            "not-a-snippet",
            ProfileDocumentCaret { line: 1, column: 0 },
        )
        .expect_err("an unknown snippet must be refused");

    assert!(matches!(error, PortError::Rejected(_)));
}

#[test]
fn a_codec_without_snippets_refuses_insertion() {
    let codec = ProfileDocumentCodecCapture::without_snippets();

    let error = codec
        .insert_snippet(
            DOCUMENT,
            "rule-domain",
            ProfileDocumentCaret { line: 5, column: 0 },
        )
        .expect_err("the insertion must be refused");

    match error {
        PortError::Unsupported { capability, .. } => {
            assert_eq!(capability, Capability::Profiles);
        }
        other => panic!("expected typed unsupported, got {other:?}"),
    }
}
