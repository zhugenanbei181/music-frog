use super::*;
use lurker_core_ports::profile_document_codec::ProfileDocumentCodecPort;

#[test]
fn codec_is_scoped_to_the_sing_box_json_format() {
    let codec = SingBoxProfileDocumentCodec;
    let scope = codec.scope();
    assert_eq!(scope.engine_id.as_str(), "sing-box");
    assert_eq!(scope.format_id.as_str(), PROFILE_FORMAT);
    assert_eq!(scope.schema_version.value(), PROFILE_SCHEMA_VERSION);
}

#[test]
fn codec_accepts_object_roots_and_reports_structured_syntax_diagnostics() {
    let codec = SingBoxProfileDocumentCodec;
    assert_eq!(
        codec.inspect(br#"{"log":{"level":"warn"}}"#).unwrap(),
        DocumentValidation::Valid
    );

    match codec.inspect(br#"{"outbounds":[}"#).unwrap() {
        DocumentValidation::Invalid { diagnostics } => {
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics[0].line.is_some());
            assert!(diagnostics[0].column.is_some());
            assert_eq!(diagnostics[0].message, "invalid sing-box JSON syntax");
        }
        DocumentValidation::Valid => panic!("invalid JSON must not validate"),
    }

    match codec.inspect(b"[]").unwrap() {
        DocumentValidation::Invalid { diagnostics } => {
            assert_eq!(
                diagnostics[0].message,
                "sing-box configuration root must be a JSON object"
            );
            assert!(diagnostics[0].line.is_none());
        }
        DocumentValidation::Valid => panic!("a non-object root must not validate"),
    }
}
