use super::*;
use infiltrator_contract::protocol_fidelity::ProtocolDraft;
use infiltrator_contract::protocol_trust::TlsTrustParams;
use infiltrator_ports::profile_document_codec::{
    ProfileConfigurationQuery, ProfileConfigurationUpdate,
};
use lurker_core_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileDocumentRevision, ProfileDocumentSourceIdentity,
    ProfileDocumentSourceKind, ProfileId, ProfileSourceId,
};
use singbox_driver::codec::profile_document_codec::PROFILE_FORMAT;

fn identity(codec: &SingBoxProfileConfigurationCodec) -> ProfileDocumentIdentity {
    let scope = codec.scope();
    ProfileDocumentIdentity::new(
        ProfileId::new("sing-box-profile").unwrap(),
        scope.engine_id,
        ProfileDocumentSourceIdentity::new(
            ProfileDocumentSourceKind::Local,
            ProfileSourceId::new("sing-box-local").unwrap(),
        ),
        ProfileDocumentRevision::new("revision-1").unwrap(),
        scope.format_id,
        scope.schema_version,
    )
}

#[test]
fn product_codec_delegates_to_the_sing_box_json_inspection() {
    let codec = SingBoxProfileConfigurationCodec::new();
    let scope = codec.scope();
    assert_eq!(scope.engine_id.as_str(), "sing-box");
    assert_eq!(scope.format_id.as_str(), PROFILE_FORMAT);
    assert_eq!(
        codec.inspect(br#"{"log":{"level":"warn"}}"#).unwrap(),
        DocumentValidation::Valid
    );
    assert!(matches!(
        codec.insert_snippet(
            "{}",
            "mihomo-only",
            ProfileDocumentCaret { line: 1, column: 0 }
        ),
        Err(PortError::Unsupported { .. })
    ));
}

#[test]
fn mihomo_only_profile_queries_fail_closed_for_sing_box() {
    let codec = SingBoxProfileConfigurationCodec::new();
    let identity = identity(&codec);

    for query in [
        ProfileConfigurationQuery::ProtocolNodes,
        ProfileConfigurationQuery::TrustAnchors,
    ] {
        assert!(matches!(
            codec.read_configuration(&identity, br#"{}"#, query),
            Err(PortError::Unsupported { .. })
        ));
    }
}

#[test]
fn mihomo_only_profile_updates_fail_closed_for_sing_box() {
    let codec = SingBoxProfileConfigurationCodec::new();
    let identity = identity(&codec);

    for update in [
        ProfileConfigurationUpdate::UpsertProtocolNode(ProtocolDraft::new("vless")),
        ProfileConfigurationUpdate::TrustAnchors(TlsTrustParams::default()),
    ] {
        assert!(matches!(
            codec.apply_configuration(&identity, br#"{}"#, update),
            Err(PortError::Unsupported { .. })
        ));
    }
}
