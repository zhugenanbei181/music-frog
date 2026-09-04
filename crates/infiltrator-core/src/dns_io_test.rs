use super::save_dns_config;
use infiltrator_domain::dns::DnsConfigPayload;

#[tokio::test]
async fn test_dns_io_follows_configs_dir_redirect() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().to_path_buf();
    let cloud = home.join("cloud-sync").join("profiles");
    std::fs::create_dir_all(&cloud).unwrap();
    let guard = crate::settings::test_support::RedirectGuard::acquire(home.clone()).await;
    guard
        .set_configs_dir(&home, Some(cloud.to_str().unwrap()))
        .await;

    let seed = crate::settings::app_config_manager().await.unwrap();
    seed.save("main", "port: 7890\n").await.unwrap();
    seed.set_current("main").await.unwrap();

    let saved = save_dns_config(DnsConfigPayload {
        enable: Some(true),
        prefer_h3: Some(true),
        direct_nameserver: Some(vec!["223.5.5.5".to_string()]),
        ..DnsConfigPayload::default()
    })
    .await
    .unwrap();
    assert_eq!(saved.enable, Some(true));
    assert_eq!(saved.prefer_h3, Some(true));
    assert_eq!(saved.direct_nameserver, Some(vec!["223.5.5.5".to_string()]));

    let loaded = crate::dns_io::load_dns_config().await.unwrap();
    assert_eq!(loaded.enable, Some(true));
    assert_eq!(loaded.prefer_h3, Some(true));
    assert_eq!(
        loaded.direct_nameserver,
        Some(vec!["223.5.5.5".to_string()])
    );
    assert!(cloud.join("main.yaml").is_file());
    assert!(!home.join("configs").exists());
}
