//! Behavior checks for the neutral Mihomo YAML document codec.
//! test-intent: behavior

use super::MihomoProfileDocumentCodec;
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCaret, ProfileDocumentCodecPort, ProfileDocumentSnippetInsertion,
    ProfileDocumentSnippetOutcome,
};

fn test_snippet_body(snippet_id: &str) -> Option<&'static str> {
    match snippet_id {
        "ss" => Some(
            "  - name: SS-Node\n    type: ss\n    server: server.example.com\n    port: 8388\n    cipher: aes-256-gcm\n    password: password\n",
        ),
        "select" => Some("  - name: PROXIES\n    type: select\n    proxies:\n      - DIRECT\n"),
        "rule-geoip" => Some("  - GEOIP,CN,DIRECT\n"),
        _ => None,
    }
}

#[test]
fn codec_is_scoped_to_the_mihomo_yaml_format() {
    let codec = MihomoProfileDocumentCodec::default();
    let scope = codec.scope();
    assert_eq!(scope.engine_id, EngineId::mihomo());
    assert_eq!(scope.engine_id.as_str(), "mihomo");
    assert_eq!(scope.format_id.as_str(), super::PROFILE_FORMAT);
    assert_eq!(scope.schema_version.value(), super::PROFILE_SCHEMA_VERSION);
    assert_eq!(scope.schema_version.value(), 1);
}

#[test]
fn codec_reports_valid_and_invalid_yaml_with_typed_diagnostics() {
    let codec = MihomoProfileDocumentCodec::default();
    assert_eq!(
        codec.inspect(b"mode: rule\n").expect("valid YAML"),
        DocumentValidation::Valid
    );
    assert_eq!(
        codec
            .inspect(b"")
            .expect("empty editor buffer is inspectable"),
        DocumentValidation::Valid
    );

    match codec
        .inspect(b"rules: [\n")
        .expect("invalid YAML is a verdict")
    {
        DocumentValidation::Invalid { diagnostics } => {
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics[0].line.is_some());
            assert!(diagnostics[0].column.is_some());
            assert!(!diagnostics[0].message.is_empty());
        }
        DocumentValidation::Valid => panic!("malformed YAML was accepted"),
    }
}

#[test]
fn codec_reports_parser_marker_for_invalid_yaml() {
    let codec = MihomoProfileDocumentCodec::default();
    let invalid = "mode: : rule\n";
    match codec.inspect(invalid.as_bytes()).expect("syntax verdict") {
        DocumentValidation::Invalid { diagnostics } => {
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics[0].line.is_some());
            assert!(diagnostics[0].column.is_some());
            assert!(!diagnostics[0].message.is_empty());
        }
        DocumentValidation::Valid => panic!("malformed YAML was accepted"),
    }
}

#[test]
fn codec_rejects_non_utf8_bytes_before_parsing() {
    let codec = MihomoProfileDocumentCodec::default();
    assert!(matches!(
        codec.inspect(&[0xff, 0xfe]),
        Err(PortError::Failed(_))
    ));
}

#[test]
fn codec_without_a_catalogue_fails_snippet_insertion_closed() {
    let codec = MihomoProfileDocumentCodec::default();
    assert!(matches!(
        codec.insert_snippet(
            "mode: rule\n",
            "ss",
            ProfileDocumentCaret { line: 1, column: 0 }
        ),
        Err(PortError::Unsupported { .. })
    ));
}

#[test]
fn codec_owns_snippet_splicing_and_local_preflight() {
    let codec = MihomoProfileDocumentCodec::with_snippets(test_snippet_body);
    let document = "proxies:\n  - name: existing\n    type: ss\nrules:\n  - MATCH,DIRECT\n";
    let insertion = codec
        .insert_snippet(
            document,
            "ss",
            ProfileDocumentCaret {
                line: 3,
                column: 12,
            },
        )
        .expect("supported Mihomo snippet");
    match insertion {
        ProfileDocumentSnippetOutcome::Inserted(insertion) => {
            assert!(insertion.content.contains("- name: SS-Node"));
            assert_eq!(insertion.validation, DocumentValidation::Valid);
        }
        ProfileDocumentSnippetOutcome::Rejected { validation } => {
            panic!("valid splice was rejected: {validation:?}");
        }
    }

    let broken = codec
        .insert_snippet(
            "mode: rule\n",
            "select",
            ProfileDocumentCaret { line: 1, column: 6 },
        )
        .expect("invalid splice is a neutral verdict");
    assert!(matches!(
        broken,
        ProfileDocumentSnippetOutcome::Rejected {
            validation: DocumentValidation::Invalid { .. }
        }
    ));

    let already_invalid = codec
        .insert_snippet(
            "rules: [\n",
            "rule-geoip",
            ProfileDocumentCaret { line: 1, column: 0 },
        )
        .expect("repair editing remains possible");
    assert!(matches!(
        already_invalid,
        ProfileDocumentSnippetOutcome::Inserted(ProfileDocumentSnippetInsertion {
            validation: DocumentValidation::Invalid { .. },
            ..
        })
    ));
}

#[test]
fn codec_rejects_an_unknown_snippet_id_as_invalid_input() {
    let codec = MihomoProfileDocumentCodec::with_snippets(test_snippet_body);
    assert!(matches!(
        codec.insert_snippet(
            "mode: rule\n",
            "not-a-snippet",
            ProfileDocumentCaret { line: 1, column: 0 }
        ),
        Err(PortError::Rejected(failure))
            if failure.code == ErrorCode::InvalidInput
    ));
}
