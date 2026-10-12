use super::*;
use tempfile::TempDir;

#[tokio::test]
async fn product_profile_store_registry_uses_the_resolved_shared_configs_root() {
    let home = TempDir::new().unwrap();
    let configs = TempDir::new().unwrap();
    let configs_path = configs.path().to_str().expect("test path is UTF-8");
    let mihomo = Arc::new(
        ConfigManager::with_home_configs_dir_and_store(
            home.path().to_path_buf(),
            Some(configs_path),
            DefaultCredentialStore::default(),
        )
        .expect("Mihomo manager uses the configured shared root"),
    );

    let stores = engine_profile_stores(home.path().to_path_buf(), Arc::clone(&mihomo))
        .expect("register both engine profile stores");
    let registered_stores = stores.all();
    let mut engines: Vec<_> = registered_stores
        .iter()
        .map(|store| store.engine_id())
        .collect();
    engines.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    assert_eq!(
        engines,
        vec![
            MihomoVersionPort::engine_identifier(),
            singbox_api::engine_id()
        ]
    );

    let sing_box = registered_stores
        .iter()
        .find(|store| store.engine_id() == singbox_api::engine_id())
        .expect("sing-box profile store is registered");
    assert!(
        ProfileStore::list_profiles(sing_box.as_ref())
            .await
            .expect("initialize the engine-scoped namespace")
            .is_empty()
    );
    assert!(configs.path().join("engines/sing-box").is_dir());
}

#[test]
fn desktop_version_registry_registers_sing_box_only_when_the_current_target_is_locked() {
    let home = TempDir::new().unwrap();
    let expected_target = SingBoxEngineVersionBackendFactory::current(home.path().to_path_buf());
    let registry = version_factory_for(home.path().to_path_buf()).unwrap();
    let sing_box = singbox_api::engine_id();

    match expected_target {
        Ok(_) => assert!(registry.version_port(&sing_box).is_ok()),
        Err(PortError::Unsupported { .. }) => assert!(matches!(
            registry.version_port(&sing_box),
            Err(PortError::Unsupported { .. })
        )),
        Err(error) => panic!("unexpected sing-box target registration error: {error}"),
    }
    assert!(
        registry
            .version_port(&MihomoVersionPort::engine_identifier())
            .is_ok()
    );
}
