use super::*;
use infiltrator_ports::engine_version_backend_factory::EngineVersionBackendFactory;
use tempfile::tempdir;

#[test]
fn one_owner_registry_supplies_version_and_binary_selection_ports_for_each_engine() {
    let home = tempdir().expect("owner home");
    let owner = AndroidCoreVersionOwner::build_at_home(home.path().to_path_buf())
        .expect("host target has registered Mihomo and sing-box factories");

    for engine_id in [
        MihomoVersionPort::engine_identifier(),
        singbox_api::engine_id(),
    ] {
        let application = owner
            .application
            .version_application(&engine_id)
            .expect("version use case resolves through the owner factory");
        let version_port = owner
            .factory
            .version_port(&engine_id)
            .expect("version port is registered for the engine");
        let binary_selection = owner
            .factory
            .binary_selection_port(&engine_id)
            .expect("selector reuses the same engine factory");
        assert_eq!(application.engine_id(), &engine_id);
        assert_eq!(version_port.engine_id(), engine_id);
        assert_eq!(binary_selection.engine_id(), engine_id);
    }
}

#[test]
fn owner_registry_fails_closed_for_unknown_engines() {
    let home = tempdir().expect("owner home");
    let owner = AndroidCoreVersionOwner::build_at_home(home.path().to_path_buf())
        .expect("host target has registered version factories");
    let unknown = EngineId::new("unknown-engine").expect("valid engine ID");

    assert!(matches!(
        owner.factory.version_port(&unknown),
        Err(PortError::Unsupported { .. })
    ));
    assert!(matches!(
        owner.factory.binary_selection_port(&unknown),
        Err(PortError::Unsupported { .. })
    ));
}
