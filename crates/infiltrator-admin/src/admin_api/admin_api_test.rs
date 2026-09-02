#[cfg(test)]
mod tests {
    use crate::admin_api::models::{
        ImportProfilePayload, SaveProfilePayload, SwitchProfilePayload,
    };
    use crate::admin_api::state::{AdminApiContext, AdminApiState};
    use crate::admin_api::*;
    use anyhow::anyhow;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use infiltrator_core::settings::AppSettings;
    use mihomo_api::client::MihomoClient;
    use mihomo_platform::TEST_LOCK;
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt; // for `oneshot`, `ready`, and `call`

    /// set_default smoke-checks the candidate binary; these route tests only
    /// exercise the HTTP plumbing, so a tiny runnable stand-in suffices.
    #[cfg(unix)]
    fn plant_runnable_fake_binary(home: &std::path::Path, version: &str) {
        use std::os::unix::fs::PermissionsExt;
        let dir = home.join("versions").join(version);
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("mihomo");
        std::fs::write(&bin, "#!/bin/sh\necho \"Mihomo Meta v1.19.18 test\"\n").unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[derive(Clone)]
    struct MockContext {
        rebuild_count: Arc<Mutex<usize>>,
        runtime_url: Option<String>,
        latest_stable_version: String,
        latest_stable_date: String,
        settings: Arc<Mutex<AppSettings>>,
        /// 内存版 WebDAV 密码库（替代真实 OS keyring，测试零外部依赖）。
        secrets: Arc<Mutex<std::collections::HashMap<String, String>>>,
    }

    /// 内存密码库的键：service/key 拼接，语义与真实 keyring 一致。
    fn secrets_key() -> String {
        format!(
            "{}/{}",
            infiltrator_core::settings::WEBDAV_CREDENTIAL_SERVICE,
            infiltrator_core::settings::WEBDAV_PASSWORD_KEY
        )
    }

    type SharedSecrets = Arc<Mutex<std::collections::HashMap<String, String>>>;

    #[async_trait::async_trait]
    impl AdminApiContext for MockContext {
        async fn rebuild_runtime(&self) -> anyhow::Result<()> {
            let mut count = self.rebuild_count.lock().unwrap_or_else(|e| e.into_inner());
            *count += 1;
            Ok(())
        }
        async fn set_use_bundled_core(&self, _enabled: bool) {}
        async fn refresh_core_version_info(&self) {}
        async fn latest_stable_core(&self) -> anyhow::Result<(String, String)> {
            Ok((
                self.latest_stable_version.clone(),
                self.latest_stable_date.clone(),
            ))
        }
        async fn notify_subscription_update(&self, _p: String, _s: bool, _m: Option<String>) {}
        async fn editor_path(&self) -> Option<String> {
            None
        }
        async fn set_editor_path(&self, _path: Option<String>) {}
        async fn pick_editor_path(&self) -> Option<String> {
            None
        }
        async fn open_profile_in_editor(&self, _name: &str) -> anyhow::Result<()> {
            Ok(())
        }
        async fn get_app_settings(&self) -> AppSettings {
            self.settings
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        }
        async fn save_app_settings(&self, s: AppSettings) -> anyhow::Result<()> {
            *self.settings.lock().unwrap_or_else(|e| e.into_inner()) = s;
            Ok(())
        }
        async fn webdav_password(&self) -> Option<String> {
            self.secrets
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&secrets_key())
                .cloned()
        }
        async fn set_webdav_password(&self, password: &str) -> anyhow::Result<()> {
            self.secrets
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(secrets_key(), password.to_string());
            Ok(())
        }
        async fn runtime_running(&self) -> bool {
            self.runtime_url.is_some()
        }
        async fn runtime_controller_url(&self) -> Option<String> {
            self.runtime_url.clone()
        }
        async fn stop_runtime(&self) -> anyhow::Result<()> {
            Ok(())
        }
        async fn runtime_client(&self) -> anyhow::Result<MihomoClient> {
            let runtime_url = self
                .runtime_url
                .as_deref()
                .ok_or_else(|| anyhow!("runtime url is not configured"))?;
            MihomoClient::new(runtime_url, None).map_err(|e| anyhow!(e.to_string()))
        }
        async fn system_proxy_enabled(&self) -> bool {
            false
        }
        async fn set_system_proxy_enabled(&self, _enabled: bool) -> anyhow::Result<()> {
            Ok(())
        }
        async fn autostart_enabled(&self) -> bool {
            false
        }
        async fn set_autostart_enabled(&self, _enabled: bool) -> anyhow::Result<()> {
            Ok(())
        }
        fn supports_system_proxy_control(&self) -> bool {
            false
        }
        fn supports_autostart_control(&self) -> bool {
            false
        }
    }

    fn setup_app() -> axum::Router {
        setup_app_with_runtime(None)
    }

    fn setup_app_with_runtime(runtime_url: Option<String>) -> axum::Router {
        let (app, _) = setup_app_with_runtime_and_secrets(runtime_url);
        app
    }

    fn setup_app_with_runtime_and_secrets(
        runtime_url: Option<String>,
    ) -> (axum::Router, SharedSecrets) {
        let ctx = MockContext {
            rebuild_count: Arc::new(Mutex::new(0)),
            runtime_url,
            latest_stable_version: "v1.20.0".to_string(),
            latest_stable_date: "2026-01-01T00:00:00Z".to_string(),
            settings: Arc::new(Mutex::new(AppSettings::default())),
            secrets: Arc::new(Mutex::new(std::collections::HashMap::new())),
        };
        let secrets = ctx.secrets.clone();
        let bus = events::AdminEventBus::new();
        let state = AdminApiState::new(ctx, bus);
        (router(state), secrets)
    }

    fn setup_app_with_auth(token: Option<String>) -> axum::Router {
        let ctx = MockContext {
            rebuild_count: Arc::new(Mutex::new(0)),
            runtime_url: None,
            latest_stable_version: "v1.20.0".to_string(),
            latest_stable_date: "2026-01-01T00:00:00Z".to_string(),
            settings: Arc::new(Mutex::new(AppSettings::default())),
            secrets: Arc::new(Mutex::new(std::collections::HashMap::new())),
        };
        let bus = events::AdminEventBus::new();
        let state = AdminApiState::with_auth_token(ctx, bus, token);
        router(state)
    }

    #[tokio::test]
    async fn test_get_profiles_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let app = setup_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/profiles")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "application/json");

        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_get_settings_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let app = setup_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_get_capabilities_route() {
        let _guard = TEST_LOCK.lock().await;
        let app = setup_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/capabilities")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 2048)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["schema_version"], 1);
        assert!(json["runtime"]["status"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_get_runtime_status_route_when_stopped() {
        let _guard = TEST_LOCK.lock().await;
        let app = setup_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/runtime/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 2048)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["running"], false);
    }

    #[tokio::test]
    async fn test_import_profile_integration() {
        let _guard = TEST_LOCK.lock().await;

        let mut server = mockito::Server::new_async().await;
        let mock_yaml = "port: 7890\nmode: rule";
        let _m = server
            .mock("GET", "/sub")
            .with_status(200)
            .with_body(mock_yaml)
            .create_async()
            .await;

        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let app = setup_app();
        let payload = ImportProfilePayload {
            name: "test-import".to_string(),
            url: format!("{}/sub", server.url()),
            activate: Some(true),
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/profiles/import")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let body_str = String::from_utf8_lossy(&body_bytes);

        if status != StatusCode::OK {
            panic!(
                "Import profile failed with status {}. Body: {}",
                status, body_str
            );
        }

        // Verify file was saved
        let config_path = temp_dir.path().join("configs").join("test-import.yaml");
        assert!(config_path.exists());

        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_save_invalid_yaml_returns_400() {
        let _guard = TEST_LOCK.lock().await;
        let app = setup_app();
        let payload = SaveProfilePayload {
            name: "invalid-yaml".to_string(),
            content: "key: : : : value".to_string(), // Invalid YAML
            activate: Some(false),
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/profiles/save")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_switch_nonexistent_profile_returns_error() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let app = setup_app();
        let payload = SwitchProfilePayload {
            name: "i-do-not-exist".to_string(),
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/profiles/switch")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(response.status().is_client_error() || response.status().is_server_error());
        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_delete_active_profile_rejected() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let manager = mihomo_config::manager::ConfigManager::new().unwrap();
        manager.save("active", "port: 7890").await.unwrap();
        manager.set_current("active").await.unwrap();

        let app = setup_app();
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/admin/api/profiles/active")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_get_rebuild_status_reflects_reality() {
        let _guard = TEST_LOCK.lock().await;
        let app = setup_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/rebuild/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap(),
        )
        .unwrap();

        assert_eq!(body["in_progress"], false);
        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_get_dns_config_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());
        let manager = mihomo_config::manager::ConfigManager::new().unwrap();
        manager
            .save("default", "dns:\n  enable: true")
            .await
            .unwrap();

        let app = setup_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/dns")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_get_tun_config_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());
        let manager = mihomo_config::manager::ConfigManager::new().unwrap();
        manager
            .save("default", "tun:\n  enable: true")
            .await
            .unwrap();

        let app = setup_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/tun")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_get_rules_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());
        let manager = mihomo_config::manager::ConfigManager::new().unwrap();
        manager.save("default", "rules:\n  - DIRECT").await.unwrap();

        let app = setup_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/rules")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_get_proxy_providers_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());
        let manager = mihomo_config::manager::ConfigManager::new().unwrap();
        manager
            .save(
                "default",
                "proxy-providers:\n  p1:\n    type: http\n    url: https://example.com/p1.yaml\n",
            )
            .await
            .unwrap();

        let app = setup_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/proxy-providers")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_get_sniffer_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());
        let manager = mihomo_config::manager::ConfigManager::new().unwrap();
        manager
            .save(
                "default",
                "sniffer:\n  enable: true\n  sniff:\n    TLS:\n      ports: [443]\n",
            )
            .await
            .unwrap();

        let app = setup_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/sniffer")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_flush_fake_ip_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());
        let app = setup_app();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/fake-ip/flush")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_list_core_versions_route() {
        let _guard = TEST_LOCK.lock().await;
        let app = setup_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/core/versions")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_download_core_installed_version_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let version = "v1.20.0";
        plant_runnable_fake_binary(temp_dir.path(), version);
        let planted = temp_dir.path().join("versions/v1.20.0/mihomo");
        assert!(planted.exists(), "planted binary missing before request");
        let raw = std::fs::read(&planted).unwrap();
        println!("planted bytes: {:?}", String::from_utf8_lossy(&raw));
        let direct = std::process::Command::new(&planted)
            .arg("-v")
            .output()
            .unwrap();
        println!(
            "direct exec: status={:?} out={:?}",
            direct.status,
            String::from_utf8_lossy(&direct.stdout)
        );
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let app = setup_app();
        let payload = serde_json::json!({ "version": version });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/core/download")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["version"], version);
        assert_eq!(json["downloaded"], false);
        assert_eq!(json["already_installed"], true);
        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_get_latest_stable_core_route() {
        let _guard = TEST_LOCK.lock().await;
        let app = setup_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/core/latest-stable")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["version"], "v1.20.0");
        assert_eq!(json["release_date"], "2026-01-01T00:00:00Z");
    }

    #[tokio::test]
    async fn test_update_stable_core_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let version = "v1.20.0";
        plant_runnable_fake_binary(temp_dir.path(), version);
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let app = setup_app();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/core/update-stable")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), 2048)
            .await
            .unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "response body: {}",
            String::from_utf8_lossy(&body)
        );
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["version"], version);
        assert_eq!(json["downloaded"], false);
        assert_eq!(json["already_installed"], true);
        assert_eq!(json["rebuild_scheduled"], true);

        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        let status_response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/rebuild/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status_response.status(), StatusCode::OK);
        let status_body = axum::body::to_bytes(status_response.into_body(), 2048)
            .await
            .unwrap();
        let status_json: serde_json::Value = serde_json::from_slice(&status_body).unwrap();
        assert_eq!(status_json["in_progress"], false);
        assert_eq!(status_json["last_error"], serde_json::Value::Null);
        assert_eq!(status_json["last_reason"], "core-update-stable");

        let config_file = temp_dir.path().join("config.toml");
        let content = tokio::fs::read_to_string(config_file).await.unwrap();
        assert!(content.contains(&format!("version = \"{}\"", version)));
        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_activate_core_version_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let version = "v1.19.0";
        plant_runnable_fake_binary(temp_dir.path(), version);
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let app = setup_app();
        let payload = serde_json::json!({ "version": version });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/core/activate")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        let status_response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/rebuild/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status_response.status(), StatusCode::OK);
        let status_body = axum::body::to_bytes(status_response.into_body(), 2048)
            .await
            .unwrap();
        let status_json: serde_json::Value = serde_json::from_slice(&status_body).unwrap();
        assert_eq!(status_json["in_progress"], false);
        assert_eq!(status_json["last_error"], serde_json::Value::Null);
        assert_eq!(status_json["last_reason"], "core-activate");

        let config_file = temp_dir.path().join("config.toml");
        let content = tokio::fs::read_to_string(config_file).await.unwrap();
        assert!(content.contains(&format!("version = \"{}\"", version)));
        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_save_rules_route_schedules_rebuild() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let manager = mihomo_config::manager::ConfigManager::new().unwrap();
        manager.save("default", "rules:\n  - DIRECT").await.unwrap();

        let app = setup_app();
        let payload = serde_json::json!({
            "rules": [
                {
                    "rule": "DOMAIN,example.com,DIRECT",
                    "enabled": true
                }
            ]
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/rules")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["rules"][0]["rule"], "DOMAIN,example.com,DIRECT");
        assert_eq!(json["rules"][0]["enabled"], true);

        tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        let status_response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/rebuild/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(status_response.status(), StatusCode::OK);
        let status_body = axum::body::to_bytes(status_response.into_body(), 2048)
            .await
            .unwrap();
        let status_json: serde_json::Value = serde_json::from_slice(&status_body).unwrap();
        assert_eq!(status_json["in_progress"], false);
        assert_eq!(status_json["last_error"], serde_json::Value::Null);
        assert_eq!(status_json["last_reason"], "rules-update");
        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_runtime_connections_route() {
        let _guard = TEST_LOCK.lock().await;
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/connections")
            .with_status(200)
            .with_body(
                r#"{
                    "downloadTotal": 1000,
                    "uploadTotal": 2000,
                    "connections": [{
                        "id":"c1",
                        "metadata": {"network":"tcp", "type":"socks5", "sourceIP":"127.0.0.1", "destinationIP":"8.8.8.8", "sourcePort":"1234", "destinationPort":"443", "host":"", "dnsMode":"normal", "processPath":""},
                        "uploadTotal": 100,
                        "downloadTotal": 200,
                        "start": "2024-01-01T00:00:00Z",
                        "rule": "Match",
                        "rulePayload": ""
                    }]
                }"#,
            )
            .create_async()
            .await;
        let app = setup_app_with_runtime(Some(server.url()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/runtime/connections")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_runtime_memory_route() {
        let _guard = TEST_LOCK.lock().await;
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/memory")
            .with_status(200)
            .with_body(r#"{"inuse":123,"oslimit":456}"#)
            .create_async()
            .await;
        let app = setup_app_with_runtime(Some(server.url()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/runtime/memory")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["inuse"], 123);
        assert_eq!(json["oslimit"], 456);
    }

    #[tokio::test]
    async fn test_runtime_close_single_connection_route() {
        let _guard = TEST_LOCK.lock().await;
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("DELETE", "/connections/c1")
            .with_status(204)
            .create_async()
            .await;
        let app = setup_app_with_runtime(Some(server.url()));
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/admin/api/runtime/connections/c1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_runtime_traffic_route() {
        let _guard = TEST_LOCK.lock().await;
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/connections")
            .with_status(200)
            .with_body(
                r#"{
                    "downloadTotal": 3000,
                    "uploadTotal": 4000,
                    "connections": [
                        {
                            "id":"c1",
                            "metadata": {"network":"tcp", "type":"socks5", "sourceIP":"127.0.0.1", "destinationIP":"8.8.8.8", "sourcePort":"1234", "destinationPort":"443", "host":"", "dnsMode":"normal", "processPath":""},
                            "uploadTotal": 100,
                            "downloadTotal": 200,
                            "start": "2024-01-01T00:00:00Z",
                            "rule": "Match",
                            "rulePayload": ""
                        },
                        {
                            "id":"c2",
                            "metadata": {"network":"tcp", "type":"socks5", "sourceIP":"127.0.0.1", "destinationIP":"8.8.8.8", "sourcePort":"1234", "destinationPort":"443", "host":"", "dnsMode":"normal", "processPath":""},
                            "uploadTotal": 100,
                            "downloadTotal": 200,
                            "start": "2024-01-01T00:00:00Z",
                            "rule": "Match",
                            "rulePayload": ""
                        }
                    ]
                }"#,
            )
            .create_async()
            .await;
        let app = setup_app_with_runtime(Some(server.url()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/runtime/traffic")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["up_total"], 4000);
        assert_eq!(json["down_total"], 3000);
        assert_eq!(json["connections"], 2);
    }

    #[tokio::test]
    async fn test_runtime_logs_invalid_level_returns_400() {
        let _guard = TEST_LOCK.lock().await;
        let app = setup_app_with_runtime(Some("http://127.0.0.1:65535".to_string()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/runtime/logs?level=invalid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_runtime_proxy_delays_route() {
        let _guard = TEST_LOCK.lock().await;
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/proxies")
            .with_status(200)
            .with_body(
                r#"{
                    "proxies": {
                        "GLOBAL": {"type":"Selector","name":"GLOBAL","now":"Proxy-A","all":["Proxy-A","Proxy-B"],"history":[]},
                        "Proxy-A": {"type":"Shadowsocks","name":"Proxy-A","udp":true,"history":[{"time":"2026-02-06T00:00:00Z","delay":120}],"alive":true,"server":"1.1.1.1","port":443,"cipher":"aes-256-gcm"},
                        "Proxy-B": {"type":"Shadowsocks","name":"Proxy-B","udp":true,"history":[],"alive":true,"server":"1.1.1.1","port":443,"cipher":"aes-256-gcm"}
                    }
                }"#,
            )
            .create_async()
            .await;
        let app = setup_app_with_runtime(Some(server.url()));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/runtime/proxies")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["nodes"].as_array().unwrap().len(), 2);
        assert_eq!(json["nodes"][0]["name"], "Proxy-A");
        assert_eq!(json["nodes"][0]["delay_ms"], 120);
        assert_eq!(
            json["default_test_url"],
            "http://www.gstatic.com/generate_204"
        );
        assert_eq!(json["default_timeout_ms"], 5000);
    }

    #[tokio::test]
    async fn test_runtime_proxy_delay_test_route() {
        let _guard = TEST_LOCK.lock().await;
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock(
                "GET",
                mockito::Matcher::Regex("^/proxies/proxy1/delay(\\?.*)?$".to_string()),
            )
            .with_status(200)
            .with_body(r#"{"delay":123}"#)
            .create_async()
            .await;
        let app = setup_app_with_runtime(Some(server.url()));
        let payload = serde_json::json!({ "proxy": "proxy1" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/runtime/delay/test")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["proxy"], "proxy1");
        assert_eq!(json["delay_ms"], 123);
        assert_eq!(json["test_url"], "http://www.gstatic.com/generate_204");
        assert_eq!(json["timeout_ms"], 5000);
    }

    #[tokio::test]
    async fn test_runtime_proxy_delay_test_all_route() {
        let _guard = TEST_LOCK.lock().await;
        let mut server = mockito::Server::new_async().await;
        let _m_proxies = server
            .mock("GET", "/proxies")
            .with_status(200)
            .with_body(
                r#"{
                    "proxies": {
                        "GLOBAL": {"type":"Selector","name":"GLOBAL","now":"Proxy-A","all":["Proxy-A","Proxy-B"],"history":[]},
                        "Proxy-A": {"type":"Shadowsocks","name":"Proxy-A","udp":true,"history":[],"alive":true,"server":"1.1.1.1","port":443,"cipher":"aes-256-gcm"},
                        "Proxy-B": {"type":"Shadowsocks","name":"Proxy-B","udp":true,"history":[],"alive":true,"server":"1.1.1.1","port":443,"cipher":"aes-256-gcm"}
                    }
                }"#,
            )
            .create_async()
            .await;
        let _m_ok = server
            .mock(
                "GET",
                mockito::Matcher::Regex("^/proxies/Proxy-A/delay(\\?.*)?$".to_string()),
            )
            .with_status(200)
            .with_body(r#"{"delay":88}"#)
            .create_async()
            .await;
        let _m_fail = server
            .mock(
                "GET",
                mockito::Matcher::Regex("^/proxies/Proxy-B/delay(\\?.*)?$".to_string()),
            )
            .with_status(500)
            .with_body(r#"{"error":"failed"}"#)
            .create_async()
            .await;

        let app = setup_app_with_runtime(Some(server.url()));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/runtime/delay/test-all")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["success_count"], 1);
        assert_eq!(json["failed_count"], 1);
        assert_eq!(json["results"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_doctor_checks_list_route() {
        let _guard = TEST_LOCK.lock().await;
        let app = setup_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/doctor/checks")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 64 * 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        let checks = json.as_array().expect("checks must be a JSON array");
        assert!(!checks.is_empty(), "expected the full check metadata list");
        assert!(
            checks
                .iter()
                .any(|check| check["id"] == "config.settings_parse"),
            "known check id missing from metadata list"
        );
        for check in checks {
            assert!(check["id"].is_string());
            assert!(check["category"].is_string());
            assert!(check["summary"].is_string());
            assert!(check["fixable"].is_boolean());
            assert!(check["default_enabled"].is_boolean());
        }
    }

    #[tokio::test]
    async fn test_doctor_check_detail_route() {
        let _guard = TEST_LOCK.lock().await;
        let app = setup_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/doctor/checks/config.settings_parse")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(json["id"], "config.settings_parse");
        assert_eq!(json["category"], "config");
        assert!(json["why"].is_string());
        assert!(json["fail_means"].is_string());
        assert!(json["hint"].is_string());
    }

    #[tokio::test]
    async fn test_doctor_check_detail_unknown_id_returns_404() {
        let _guard = TEST_LOCK.lock().await;
        let app = setup_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/doctor/checks/nope.nothing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 2048)
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(json["error"].is_string());
    }

    #[tokio::test]
    async fn test_doctor_run_route_shape_and_filter() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let app = setup_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/doctor")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 64 * 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(json["started_at"].is_u64());
        assert!(json["finished_at"].is_u64());
        assert!(json["exit_code"].is_i64());
        assert!(json["exit_code"].as_i64().unwrap() >= 0);
        let checks = json["checks"].as_array().expect("checks array");
        assert!(!checks.is_empty(), "unfiltered run must execute checks");
        for check in checks {
            assert!(check["id"].is_string());
            assert!(check["status"].is_string());
        }

        let app = setup_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/doctor?only=config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 64 * 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        let checks = json["checks"].as_array().expect("filtered checks array");
        assert!(!checks.is_empty(), "config filter must select checks");
        assert!(
            checks.iter().all(|check| check["category"] == "config"),
            "filter must narrow the report to the config category"
        );

        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_doctor_fix_route_is_idempotent() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let app = setup_app();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/doctor/fix")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let first: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 64 * 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(first["actions"].is_array());

        // Every conservative repair is a no-op once its artifact exists.
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/doctor/fix")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let second: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 64 * 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(
            second["actions"]
                .as_array()
                .expect("actions array")
                .is_empty(),
            "second fix run must change nothing, got: {second}"
        );

        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_bootstrap_route_is_idempotent() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::paths::set_home_dir_override(temp_dir.path().to_path_buf());

        let app = setup_app();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/bootstrap")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 64 * 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        let steps = json["steps"].as_array().expect("steps array");
        assert!(!steps.is_empty(), "bootstrap must report its steps");
        assert!(steps.iter().any(|step| step["id"] == "configs_dir"));
        assert!(steps.iter().any(|step| step["id"] == "default_config"));
        assert!(temp_dir.path().join("configs").is_dir());

        // Second run on the initialized home must skip every step.
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/bootstrap")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 64 * 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        for step in json["steps"].as_array().expect("steps array") {
            assert_eq!(step["executed"], false, "step must be skipped: {step}");
        }

        mihomo_platform::paths::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_settings_configs_dir_roundtrip() {
        let _guard = TEST_LOCK.lock().await;
        let app = setup_app();

        let payload = serde_json::json!({ "configs_dir": "/custom/configs" });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/admin/api/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(json["configs_dir"], "/custom/configs");

        // A blank override means "unset" and must round-trip to null.
        let payload = serde_json::json!({ "configs_dir": "   " });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        // An absent key must leave the stored value untouched.
        let payload = serde_json::json!({ "language": "en-US" });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(json["configs_dir"], serde_json::Value::Null);
        assert_eq!(json["language"], "en-US");
    }

    /// 0.20 OS 系统通知开关：POST 持久化 + GET 回填，且缺省键不动存量值。
    #[tokio::test]
    async fn test_settings_notifications_enabled_roundtrip() {
        let _guard = TEST_LOCK.lock().await;
        let app = setup_app();

        // 缺省开启。
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/admin/api/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(json["notifications_enabled"], true);

        // 显式关闭并回读。
        let payload = serde_json::json!({ "notifications_enabled": false });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/admin/api/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(json["notifications_enabled"], false);

        // 不携带该键的保存不得改动已持久化的关闭状态。
        let payload = serde_json::json!({ "language": "en-US" });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(json["notifications_enabled"], false);
        assert_eq!(json["language"], "en-US");
    }

    /// WebDAV 密码永不落盘/回传：POST 带非空 password → 进内存 keyring、
    /// settings 快照无明文；GET 不回传 password 键；POST 空密码不动既有条目。
    #[tokio::test]
    async fn test_settings_webdav_password_roundtrip() {
        let _guard = TEST_LOCK.lock().await;
        let (app, secrets) = setup_app_with_runtime_and_secrets(None);
        let key = secrets_key();

        // POST：显式非空 password。
        let payload = serde_json::json!({
            "webdav": {
                "enabled": true,
                "url": "https://dav.example.com",
                "username": "user",
                "password": "s3cret"
            }
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        assert_eq!(
            secrets
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&key)
                .map(String::as_str),
            Some("s3cret"),
            "non-empty password must land in the credential store"
        );

        // GET：webdav 在场，但不得携带 password 键。
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/admin/api/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(json["webdav"]["url"], "https://dav.example.com");
        assert!(
            json["webdav"].get("password").is_none(),
            "GET must not echo the password back: {}",
            json["webdav"]
        );

        // POST 不带 password：既有 keyring 条目保持不动。
        let payload = serde_json::json!({
            "webdav": {
                "enabled": true,
                "url": "https://dav.example.com",
                "username": "user"
            }
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            secrets
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&key)
                .map(String::as_str),
            Some("s3cret"),
            "empty/absent password must leave the stored entry untouched"
        );

        // 内存 settings 快照同样无明文（宿主落盘时被 skip_serializing 跳过）。
        let body = app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(body.status(), StatusCode::OK);
        let raw = axum::body::to_bytes(body.into_body(), 4096).await.unwrap();
        assert!(
            !String::from_utf8_lossy(&raw).contains("s3cret"),
            "settings snapshot must not leak plaintext"
        );
    }

    #[tokio::test]
    async fn test_admin_api_token_auth_isolation() {
        let _guard = TEST_LOCK.lock().await;
        let token = "test_super_secret_admin_token_456".to_string();
        let app = setup_app_with_auth(Some(token.clone()));

        // 1. Request with no token -> 401 Unauthorized
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/admin/api/capabilities")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // 2. Request with invalid Bearer token -> 401 Unauthorized
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/admin/api/capabilities")
                    .header("Authorization", "Bearer wrong_token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // 3. Request with valid Bearer token -> 200 OK
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/admin/api/capabilities")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // 4. Request with valid x-admin-token header -> 200 OK
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/admin/api/capabilities")
                    .header("x-admin-token", &token)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // 5. Request with query param token -> 200 OK
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/admin/api/capabilities?token={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // 6. Request with unconfigured auth_token -> allows access
        let open_app = setup_app_with_auth(None);
        let response = open_app
            .oneshot(
                Request::builder()
                    .uri("/admin/api/capabilities")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_script_endpoints_presets_and_execute() {
        let _guard = TEST_LOCK.lock().await;
        let app = setup_app();

        // 1. GET /admin/api/scripts/presets
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/admin/api/scripts/presets")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let presets = json["presets"].as_array().expect("presets array");
        assert_eq!(presets.len(), 4);

        // 2. POST /admin/api/scripts/validate (valid script)
        let val_payload = serde_json::json!({
            "script": "function main(config) { filter_nodes_by_regex(config, 'ad', true); return config; }"
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/scripts/validate")
                    .header("content-type", "application/json")
                    .body(Body::from(val_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["valid"], true);
        assert_eq!(json["entry_point_found"], true);

        // 3. POST /admin/api/scripts/execute
        let exec_payload = serde_json::json!({
            "script": "function main(config) {\n  filter_nodes_by_regex(config, '官网|广告', true);\n  console.log('Filtered ad nodes');\n  return config;\n}",
            "yaml_content": "proxies:\n  - name: \"🇭🇰 香港 01\"\n    type: ss\n  - name: \"官网-广告节点\"\n    type: ss\n",
            "stage": "pre_merge",
            "timeout_ms": 500
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/scripts/execute")
                    .header("content-type", "application/json")
                    .body(Body::from(exec_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["success"], true);
        assert!(
            json["transformed_yaml"]
                .as_str()
                .unwrap()
                .contains("🇭🇰 香港 01")
        );
        assert!(
            !json["transformed_yaml"]
                .as_str()
                .unwrap()
                .contains("官网-广告节点")
        );
        assert_eq!(json["console_logs"][0], "Filtered ad nodes");
    }

    #[tokio::test]
    async fn test_extension_endpoints_package_and_manifest() {
        let _guard = TEST_LOCK.lock().await;
        let app = setup_app();

        // 1. POST /admin/api/extensions/package/export
        let ext_pkg = serde_json::json!({
            "package": {
                "name": "Auto Country Router",
                "version": "1.0.0",
                "author": "Infiltrator",
                "description": "Auto groups nodes by country",
                "stage": "pre_merge",
                "script_code": "function main(config) { auto_country_groups(config); return config; }",
                "mixin_yaml": null,
                "tags": ["country", "auto"]
            }
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/extensions/package/export")
                    .header("content-type", "application/json")
                    .body(Body::from(ext_pkg.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 8192)
            .await
            .unwrap();
        let export_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let exported_json_str = export_json["json"].as_str().unwrap();
        let checksum = export_json["checksum"].as_str().unwrap();
        assert!(!checksum.is_empty());

        // 2. POST /admin/api/extensions/package/import
        let import_payload = serde_json::json!({
            "json": exported_json_str,
            "expected_checksum": checksum
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/extensions/package/import")
                    .header("content-type", "application/json")
                    .body(Body::from(import_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 8192)
            .await
            .unwrap();
        let imported_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(imported_json["package"]["name"], "Auto Country Router");

        // 3. POST /admin/api/extensions/manifest/validate
        let manifest_payload = serde_json::json!({
            "manifest": {
                "id": "ext-test",
                "name": "Extension Test",
                "version": "1.0.0",
                "author": "Dev",
                "description": "Valid test manifest",
                "permissions": ["network_access", "modify_rules"],
                "settings_schema": [
                    {
                        "key": "enable_auto",
                        "label": "Enable Auto",
                        "field_type": "boolean",
                        "default_value": true
                    }
                ]
            }
        });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/api/extensions/manifest/validate")
                    .header("content-type", "application/json")
                    .body(Body::from(manifest_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let res: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(res["valid"], true);
    }
}
