//! Product wiring checks for the Mihomo configuration codec.
//! test-intent: behavior

use super::profile_document_codec::MihomoProfileConfigurationCodec;
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCaret, ProfileDocumentCodecPort, ProfileDocumentSnippetInsertion,
    ProfileDocumentSnippetOutcome,
};

#[test]
fn product_codec_delegates_the_neutral_scope_and_inspection_to_the_driver() {
    let codec = MihomoProfileConfigurationCodec::new();
    let scope = codec.scope();
    assert_eq!(scope.engine_id.as_str(), "mihomo");
    assert_eq!(scope.format_id.as_str(), "mihomo-yaml");
    assert_eq!(scope.schema_version.value(), 1);
    assert_eq!(
        codec.inspect(b"mode: rule\n").expect("valid YAML"),
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
fn product_codec_injects_the_shared_snippet_catalogue_into_the_driver_splice() {
    let codec = MihomoProfileConfigurationCodec;
    let insertion = codec
        .insert_snippet(
            "proxies:\n  - name: existing\n    type: ss\nrules:\n  - MATCH,DIRECT\n",
            "ss",
            ProfileDocumentCaret {
                line: 3,
                column: 12,
            },
        )
        .expect("the shared catalogue owns the ss body");
    let ProfileDocumentSnippetOutcome::Inserted(ProfileDocumentSnippetInsertion {
        snippet_id,
        content,
        validation,
        ..
    }) = insertion
    else {
        panic!("a valid catalogue splice must be inserted");
    };
    assert_eq!(snippet_id, "ss");
    assert_eq!(validation, DocumentValidation::Valid);
    assert!(content.contains("- name: SS-Node"));
}

#[test]
fn product_codec_rejects_an_unknown_snippet_id() {
    let codec = MihomoProfileConfigurationCodec;
    assert!(
        codec
            .insert_snippet(
                "mode: rule\n",
                "not-a-snippet",
                ProfileDocumentCaret { line: 1, column: 0 }
            )
            .is_err()
    );
}
