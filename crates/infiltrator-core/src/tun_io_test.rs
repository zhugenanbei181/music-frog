use super::save_tun_config;
use infiltrator_domain::tun::TunConfigPatch;

#[tokio::test]
async fn test_tun_io_follows_configs_dir_redirect() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().to_path_buf();
    let cloud = home.join("cloud-sync").join("profiles");
    std::fs::create_dir_all(&cloud).unwrap();
    let guard = crate::settings_io::test_support::RedirectGuard::acquire(home.clone()).await;
    guard
        .set_configs_dir(&home, Some(cloud.to_str().unwrap()))
        .await;

    let seed = crate::settings_io::app_config_manager().await.unwrap();
    seed.save("main", "port: 7890\n").await.unwrap();
    seed.set_current("main").await.unwrap();

    let saved = save_tun_config(TunConfigPatch {
        enable: Some(true),
        ..TunConfigPatch::default()
    })
    .await
    .unwrap();
    assert_eq!(saved.enable, Some(true));

    let loaded = crate::tun_io::load_tun_config().await.unwrap();
    assert_eq!(loaded.enable, Some(true));
    assert!(cloud.join("main.yaml").is_file());
    assert!(!home.join("configs").exists());
}
