    use super::*;
    use infiltrator_contract::runtime_settings::{RuntimeDelaySortOrder, RuntimeProbeSettings};
    use infiltrator_domain::settings::AdminServerConfig;
    #[cfg(test)]
    use infiltrator_ports::error::PortError;
    use mihomo_config::manager::paths::engine_config_dir;
    #[cfg(test)]
    use std::collections::HashMap;
    #[cfg(test)]
    use std::fs;
    use std::path::PathBuf;
    #[cfg(test)]
    use std::result;
    #[cfg(test)]
    use std::sync::Mutex;

    #[test]
    fn test_app_settings_default() {
        let settings = AppSettings::default();
        assert!(settings.use_bundled_core);
        assert!(settings.notifications_enabled);
        assert_eq!(settings.language, "zh-CN");
        assert_eq!(settings.theme, "system");
        assert!(settings.runtime_panel.auto_refresh);
        assert_eq!(
            settings.runtime_panel.delay_sort_order,
            RuntimeDelaySortOrder::LatencyAscending
        );
        assert_eq!(
            settings.runtime_panel.probe_settings,
            RuntimeProbeSettings::default()
        );
        assert_eq!(
            settings.settings_schema_version,
            CURRENT_SETTINGS_SCHEMA_VERSION
        );
        assert_eq!(settings.runtime_panel.connection_sort, "download_desc");
    }

    #[tokio::test]
    async fn legacy_runtime_settings_migrate_once_to_typed_versioned_values() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        fs::write(
            &settings_file,
            r#"[runtime_panel]
auto_refresh = false
delay_sort = "name_desc"
delay_test_url = "https://probe.example.test/check"
delay_timeout_ms = 60000
"#,
        )
        .unwrap();

        let migrated = load_settings(&settings_file).await.unwrap();
        assert_eq!(
            migrated.settings_schema_version,
            CURRENT_SETTINGS_SCHEMA_VERSION
        );
        assert_eq!(
            migrated.runtime_panel.delay_sort_order,
            RuntimeDelaySortOrder::NameDescending
        );
        assert_eq!(
            migrated.runtime_panel.probe_settings,
            RuntimeProbeSettings::new("https://probe.example.test/check", 60_000).unwrap()
        );

        let canonical = fs::read_to_string(&settings_file).unwrap();
        assert!(canonical.contains("settings_schema_version = 2"));
        assert!(canonical.contains(r#"delay_sort_order = "name_descending""#));
        assert!(canonical.contains("timeout_ms = 60000"));
        for retired in ["delay_sort =", "delay_test_url =", "delay_timeout_ms ="] {
            assert!(
                !canonical.contains(retired),
                "retired key remains: {retired}"
            );
        }

        let second_load = load_settings(&settings_file).await.unwrap();
        assert_eq!(second_load, migrated);
        assert_eq!(fs::read_to_string(&settings_file).unwrap(), canonical);
        let (_, migrated_again) = parse_settings_toml(&canonical).unwrap();
        assert!(
            !migrated_again,
            "schema v2 must not run the v1 migration again"
        );
    }

    #[test]
    fn explicit_schema_v1_runtime_settings_upgrade_to_v2_typed_values() {
        let legacy = r#"settings_schema_version = 1

[runtime_panel]
delay_sort = "delay_desc"
delay_test_url = "https://probe.example.test/v1"
delay_timeout_ms = 45000
"#;

        let (settings, migrated) = parse_settings_toml(legacy).unwrap();

        assert!(migrated);
        assert_eq!(
            settings.settings_schema_version,
            CURRENT_SETTINGS_SCHEMA_VERSION
        );
        assert_eq!(
            settings.runtime_panel.delay_sort_order,
            RuntimeDelaySortOrder::LatencyDescending
        );
        assert_eq!(
            settings.runtime_panel.probe_settings,
            RuntimeProbeSettings::new("https://probe.example.test/v1", 45_000).unwrap()
        );
    }

    #[tokio::test]
    async fn invalid_legacy_runtime_settings_fail_closed_without_rewriting_the_file() {
        let invalid_documents = [
            r#"[runtime_panel]
delay_test_url = "file:///etc/passwd"
"#,
            "[runtime_panel]\ndelay_timeout_ms = 0\n",
            r#"[runtime_panel]
delay_sort = "unknown"
"#,
        ];
        for invalid in invalid_documents {
            let temp_dir = tempfile::tempdir().unwrap();
            let settings_file = temp_dir.path().join("settings.toml");
            fs::write(&settings_file, invalid).unwrap();

            assert!(load_settings(&settings_file).await.is_err());
            assert_eq!(fs::read_to_string(&settings_file).unwrap(), invalid);
        }
    }

    #[tokio::test]
    async fn legacy_json_settings_keep_nullable_fields_and_migrate_runtime_values_to_toml() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        let legacy_json_path = settings_file.with_extension("json");
        fs::write(
            &legacy_json_path,
            r#"{
                "editor_path": null,
                "configs_dir": null,
                "language": "en-US",
                "runtime_panel": {
                    "auto_refresh": false,
                    "delay_sort": "delay_desc",
                    "delay_test_url": "https://probe.example.test/json",
                    "delay_timeout_ms": 42000
                }
            }"#,
        )
        .unwrap();

        let loaded = load_settings(&settings_file).await.unwrap();
        assert_eq!(loaded.language, "en-US");
        assert_eq!(
            loaded.settings_schema_version,
            CURRENT_SETTINGS_SCHEMA_VERSION
        );
        assert_eq!(
            loaded.runtime_panel.delay_sort_order,
            RuntimeDelaySortOrder::LatencyDescending
        );
        assert_eq!(
            loaded.runtime_panel.probe_settings,
            RuntimeProbeSettings::new("https://probe.example.test/json", 42_000).unwrap()
        );
        let canonical = fs::read_to_string(&settings_file).unwrap();
        assert!(canonical.contains("settings_schema_version = 2"));
        assert!(!canonical.contains("delay_test_url"));
        assert!(!canonical.contains("delay_timeout_ms"));
    }

    #[test]
    fn test_admin_server_config_default_enabled() {
        let admin = AdminServerConfig::default();
        assert!(
            admin.enabled,
            "admin server is on by default for the desktop product"
        );
        assert_eq!(admin.port, 25210);
    }

    #[tokio::test]
    async fn test_settings_without_admin_section_get_defaults() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        // A pre-admin settings file: must deserialize with admin defaults
        // instead of failing, so old installs keep working (serde back-compat).
        let legacy = "[runtime_panel]\nauto_refresh = true\n";
        fs::write(&settings_file, legacy).unwrap();

        let loaded = load_settings(&settings_file).await.unwrap();
        assert_eq!(loaded.admin, AdminServerConfig::default());
    }

    #[tokio::test]
    async fn test_save_and_load_admin_section() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");

        let mut settings = AppSettings::default();
        settings.admin.enabled = false;
        settings.admin.port = 3000;

        save_settings(&settings_file, &settings).await.unwrap();
        let loaded = load_settings(&settings_file).await.unwrap();
        assert!(!loaded.admin.enabled);
        assert_eq!(loaded.admin.port, 3000);
    }

    #[test]
    fn test_settings_path() {
        let base_dir = PathBuf::from("test_dir");
        let path = settings_path(&base_dir).expect("valid base dir should work");
        assert_eq!(path, base_dir.join("settings.toml"));

        let empty_dir = PathBuf::from("");
        assert!(settings_path(&empty_dir).is_err());
    }

    #[tokio::test]
    async fn test_save_and_load_settings() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");

        let mut settings = AppSettings {
            language: "en-US".to_string(),
            ..AppSettings::default()
        };
        settings.webdav.enabled = true;

        save_settings(&settings_file, &settings).await.unwrap();

        let loaded = load_settings(&settings_file).await.unwrap();
        assert_eq!(loaded.language, "en-US");
        assert!(loaded.webdav.enabled);
        assert!(loaded.runtime_panel.auto_refresh);
    }

    /// serde 往返：configs_dir 存入 settings.toml 后原样读回。
    #[tokio::test]
    async fn test_configs_dir_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");

        let settings = AppSettings {
            configs_dir: Some("~/Library/Mobile Documents/iCloud~Drive/Profiles".to_string()),
            ..AppSettings::default()
        };

        save_settings(&settings_file, &settings).await.unwrap();
        let loaded = load_settings(&settings_file).await.unwrap();
        assert_eq!(
            loaded.configs_dir,
            Some("~/Library/Mobile Documents/iCloud~Drive/Profiles".to_string())
        );

        let raw = fs::read_to_string(&settings_file).unwrap();
        assert!(raw.contains("configs_dir"), "field must be persisted");
    }

    /// 未设置时字段不得写进 settings.toml（skip_serializing_if），否则每次
    /// 保存都会给旧配置文件引入新键。
    #[tokio::test]
    async fn test_configs_dir_absent_from_toml_when_none() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");

        save_settings(&settings_file, &AppSettings::default())
            .await
            .unwrap();
        let raw = fs::read_to_string(&settings_file).unwrap();
        assert!(!raw.contains("configs_dir"));
    }

    /// 旧版 settings.toml 没有 configs_dir 键：必须反序列化为 None 而不是报错。
    #[tokio::test]
    async fn test_legacy_settings_without_configs_dir_load_as_none() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        fs::write(&settings_file, "language = \"en-US\"\n").unwrap();

        let loaded = load_settings(&settings_file).await.unwrap();
        assert_eq!(loaded.configs_dir, None);
        assert_eq!(loaded.language, "en-US");
    }

    /// 旧版 settings.toml 没有 notifications_enabled 键：必须反序列化为 true
    /// 而不是报错（serde 向后兼容约定，同 :260 注释的 admin 缺省先例）。
    #[tokio::test]
    async fn test_legacy_settings_without_notifications_enabled_default_true() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        fs::write(&settings_file, "language = \"en-US\"\n").unwrap();

        let loaded = load_settings(&settings_file).await.unwrap();
        assert!(loaded.notifications_enabled);
        assert_eq!(loaded.language, "en-US");
    }

    /// serde 往返：显式关闭通知后存盘再读回仍为关闭，且字段必须落盘。
    #[tokio::test]
    async fn test_notifications_enabled_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");

        let settings = AppSettings {
            notifications_enabled: false,
            ..AppSettings::default()
        };
        save_settings(&settings_file, &settings).await.unwrap();

        let loaded = load_settings(&settings_file).await.unwrap();
        assert!(!loaded.notifications_enabled);

        let raw = fs::read_to_string(&settings_file).unwrap();
        assert!(
            raw.contains("notifications_enabled"),
            "field must be persisted"
        );
    }

    /// 工厂跟随 settings 的 `configs_dir`：显式 home 与无参工厂（走全局
    /// home override）必须得到同一重定向结果，且默认目录不被创建。
    #[tokio::test]
    async fn test_app_config_manager_follows_settings_configs_dir() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().to_path_buf();
        let cloud = home.join("cloud").join("profiles");
        fs::create_dir_all(&cloud).unwrap();
        let guard = test_support::RedirectGuard::acquire(home.clone()).await;
        guard
            .set_configs_dir(&home, Some(cloud.to_str().unwrap()))
            .await;

        let manager = app_config_manager_in(&home).await.unwrap();
        let engine_dir = engine_config_dir(&cloud, &mihomo_config::engine_id());
        assert_eq!(manager.config_dir(), engine_dir.as_path());

        let manager = app_config_manager().await.unwrap();
        assert_eq!(manager.config_dir(), engine_dir.as_path());
        assert!(!home.join("configs").exists());
    }

    /// `configs_dir` 缺失 / 纯空白时必须与旧行为一致：默认 `<home>/configs`。
    #[tokio::test]
    async fn test_app_config_manager_defaults_when_configs_dir_unset() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().to_path_buf();
        let guard = test_support::RedirectGuard::acquire(home.clone()).await;

        // 无 settings 文件。
        let manager = app_config_manager_in(&home).await.unwrap();
        let engine_dir = engine_config_dir(&home.join("configs"), &mihomo_config::engine_id());
        assert_eq!(manager.config_dir(), engine_dir.as_path());

        // 字段为纯空白等价于未设置。
        guard.set_configs_dir(&home, Some("   ")).await;
        let manager = app_config_manager().await.unwrap();
        assert_eq!(manager.config_dir(), engine_dir.as_path());
    }

    /// 内存凭据存储（仿 session.rs / manager_test 的 MockStore 先例），
    /// `fail_set` 可注入写失败以覆盖迁移的降级路径。
    struct MemoryStore {
        entries: Mutex<HashMap<String, String>>,
        fail_set: bool,
    }

    impl MemoryStore {
        fn working() -> Self {
            Self {
                entries: Mutex::new(HashMap::new()),
                fail_set: false,
            }
        }

        fn failing_set() -> Self {
            Self {
                entries: Mutex::new(HashMap::new()),
                fail_set: true,
            }
        }

        fn get(&self, service: &str, key: &str) -> Option<String> {
            self.entries
                .lock()
                .expect("store lock")
                .get(&format!("{service}/{key}"))
                .cloned()
        }
    }

    #[async_trait::async_trait]
    impl SecureStore for MemoryStore {
        async fn get(&self, service: &str, key: &str) -> result::Result<Option<String>, PortError> {
            Ok(self.get(service, key))
        }

        async fn set(
            &self,
            service: &str,
            key: &str,
            value: &str,
        ) -> result::Result<(), PortError> {
            if self.fail_set {
                return Err(PortError::Failed("injected keyring failure".to_string()));
            }
            self.entries
                .lock()
                .expect("store lock")
                .insert(format!("{service}/{key}"), value.to_string());
            Ok(())
        }

        async fn delete(&self, service: &str, key: &str) -> result::Result<(), PortError> {
            self.entries
                .lock()
                .expect("store lock")
                .remove(&format!("{service}/{key}"));
            Ok(())
        }
    }

    fn legacy_toml_with_password() -> String {
        "[webdav]\nenabled = true\nurl = \"https://dav.example.com\"\nusername = \"user\"\npassword = \"s3cret\"\nsync_interval_mins = 30\n"
            .to_string()
    }

    /// 密码永不落盘：save_settings 的 TOML 输出不得包含 password 键。
    #[tokio::test]
    async fn test_webdav_password_never_serialized() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");

        let mut settings = AppSettings::default();
        settings.webdav.url = "https://dav.example.com".to_string();
        settings.webdav.password = "s3cret".to_string();
        save_settings(&settings_file, &settings).await.unwrap();

        let raw = fs::read_to_string(&settings_file).unwrap();
        assert!(!raw.contains("password"), "plaintext leaked: {raw}");
        // 反序列化缺省键回到空串，不影响其他字段往返。
        let loaded = load_settings(&settings_file).await.unwrap();
        assert_eq!(loaded.webdav.url, "https://dav.example.com");
    }

    /// 迁移主路径：旧明文 toml 加载后 keyring 有值、内存与重写后的文件
    /// 均无明文；第二次加载保持干净且 keyring 值不丢。
    #[tokio::test]
    async fn test_legacy_plaintext_password_migrates_to_keyring() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        fs::write(&settings_file, legacy_toml_with_password()).unwrap();

        let store = MemoryStore::working();
        let loaded = load_settings_with_store(&settings_file, &store)
            .await
            .unwrap();

        assert_eq!(
            store
                .get(WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY)
                .as_deref(),
            Some("s3cret"),
            "password must land in the credential store"
        );
        assert_eq!(loaded.webdav.password, "", "memory mirror must be blanked");

        let raw = fs::read_to_string(&settings_file).unwrap();
        assert!(
            !raw.contains("password"),
            "file must be rewritten clean: {raw}"
        );
        assert!(
            raw.contains("enabled = true"),
            "other fields survive: {raw}"
        );

        // 第二次加载：文件已无明文，keyring 值仍在（不重复迁移）。
        let second = load_settings_with_store(&settings_file, &store)
            .await
            .unwrap();
        assert_eq!(second.webdav.password, "");
        assert_eq!(
            store
                .get(WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY)
                .as_deref(),
            Some("s3cret")
        );
    }

    /// 迁移降级路径：keyring 写失败时保留内存值、不重写文件（下次启动
    /// 重试），加载本身不得报错。
    #[tokio::test]
    async fn test_migration_keeps_plaintext_when_keyring_fails() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        fs::write(&settings_file, legacy_toml_with_password()).unwrap();

        let store = MemoryStore::failing_set();
        let loaded = load_settings_with_store(&settings_file, &store)
            .await
            .unwrap();

        assert_eq!(
            loaded.webdav.password, "s3cret",
            "value must stay in memory so the session keeps working"
        );
        let raw = fs::read_to_string(&settings_file).unwrap();
        assert!(
            raw.contains("s3cret"),
            "file must be left untouched for retry: {raw}"
        );
    }

    /// 水合：干净的 settings 文件 + keyring 有值 → hydrated 加载把密码填回
    /// 内存镜像；普通加载保持空串。
    #[tokio::test]
    async fn test_load_settings_hydrated_fills_password_from_keyring() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.toml");
        save_settings(&settings_file, &AppSettings::default())
            .await
            .unwrap();

        let store = MemoryStore::working();
        store
            .set(
                WEBDAV_CREDENTIAL_SERVICE,
                WEBDAV_PASSWORD_KEY,
                "hydrated-pw",
            )
            .await
            .unwrap();

        let plain = load_settings_with_store(&settings_file, &store)
            .await
            .unwrap();
        assert_eq!(plain.webdav.password, "");

        let hydrated = load_settings_hydrated_with_store(&settings_file, &store)
            .await
            .unwrap();
        assert_eq!(hydrated.webdav.password, "hydrated-pw");

        // 水合值只是内存镜像：文件依旧干净。
        let raw = fs::read_to_string(&settings_file).unwrap();
        assert!(!raw.contains("password"));
    }

    /// 清除是幂等的：空存储上 clear 不得报错，已有条目被移除。
    #[tokio::test]
    async fn test_clear_webdav_password_is_idempotent() {
        let store = MemoryStore::working();
        clear_webdav_password(&store).await;
        assert_eq!(
            store.get(WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY),
            None
        );

        store
            .set(WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY, "pw")
            .await
            .unwrap();
        clear_webdav_password(&store).await;
        assert_eq!(
            store.get(WEBDAV_CREDENTIAL_SERVICE, WEBDAV_PASSWORD_KEY),
            None
        );
    }
