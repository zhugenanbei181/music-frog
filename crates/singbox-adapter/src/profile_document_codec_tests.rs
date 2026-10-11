use super::*;
use infiltrator_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileDocumentRevision, ProfileDocumentSourceIdentity,
    ProfileDocumentSourceKind, ProfileId, ProfileSourceId,
};
use infiltrator_contract::protocol_fidelity::ProtocolDraft;
use infiltrator_contract::protocol_trust::TlsTrustParams;
use infiltrator_ports::profile_document_codec::{
    ProfileConfigurationQuery, ProfileConfigurationUpdate,
};

fn identity(codec: &SingBoxProfileDocumentCodec) -> ProfileDocumentIdentity {
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
fn mihomo_only_profile_queries_fail_closed_for_sing_box() {
    let codec = SingBoxProfileDocumentCodec;
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
    let codec = SingBoxProfileDocumentCodec;
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
