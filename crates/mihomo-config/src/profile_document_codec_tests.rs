//! Behavior checks for the Mihomo YAML document codec.
//! test-intent: behavior

use super::profile_document_codec::MihomoProfileDocumentCodec;
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCaret, ProfileDocumentCodecPort, ProfileDocumentSnippetInsertion,
    ProfileDocumentSnippetOutcome,
};

#[test]
fn mihomo_codec_reports_valid_and_invalid_yaml_for_its_typed_scope() {
    let codec = MihomoProfileDocumentCodec;
    let scope = codec.scope();
    assert_eq!(scope.engine_id.as_str(), "mihomo");
    assert_eq!(scope.format_id.as_str(), "mihomo-yaml");
    assert_eq!(scope.schema_version.value(), 1);
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
fn mihomo_codec_reports_parser_marker_for_invalid_yaml() {
    let codec = MihomoProfileDocumentCodec;
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
fn mihomo_codec_owns_snippet_splicing_and_local_preflight() {
    let codec = MihomoProfileDocumentCodec;
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
