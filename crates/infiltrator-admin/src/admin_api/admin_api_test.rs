#[cfg(test)]
mod tests {
    use crate::TEST_LOCK;
    use crate::admin_api::*;
    use anyhow::anyhow;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use infiltrator_core::AppSettings;
    use mihomo_api::MihomoClient;
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt; // for `oneshot`, `ready`, and `call`

    #[derive(Clone)]
    struct MockContext {
        rebuild_count: Arc<Mutex<usize>>,
        runtime_url: Option<String>,
        latest_stable_version: String,
        latest_stable_date: String,
    }

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
            AppSettings::default()
        }
        async fn save_app_settings(&self, _s: AppSettings) -> anyhow::Result<()> {
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
        let ctx = MockContext {
            rebuild_count: Arc::new(Mutex::new(0)),
            runtime_url,
            latest_stable_version: "v1.20.0".to_string(),
            latest_stable_date: "2026-01-01T00:00:00Z".to_string(),
        };
        let bus = events::AdminEventBus::new();
        let state = AdminApiState::new(ctx, bus);
        router(state)
    }

    #[tokio::test]
    async fn test_get_profiles_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());

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

        mihomo_platform::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_get_settings_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());

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

        mihomo_platform::clear_home_dir_override();
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
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());

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

        mihomo_platform::clear_home_dir_override();
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
        mihomo_platform::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_switch_nonexistent_profile_returns_error() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());

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
        mihomo_platform::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_delete_active_profile_rejected() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());

        let manager = mihomo_config::ConfigManager::new().unwrap();
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
        mihomo_platform::clear_home_dir_override();
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
        mihomo_platform::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_get_dns_config_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());
        let manager = mihomo_config::ConfigManager::new().unwrap();
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
        mihomo_platform::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_get_tun_config_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());
        let manager = mihomo_config::ConfigManager::new().unwrap();
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
        mihomo_platform::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_get_rules_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());
        let manager = mihomo_config::ConfigManager::new().unwrap();
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
        mihomo_platform::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_get_proxy_providers_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());
        let manager = mihomo_config::ConfigManager::new().unwrap();
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
        mihomo_platform::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_get_sniffer_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());
        let manager = mihomo_config::ConfigManager::new().unwrap();
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
        mihomo_platform::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_flush_fake_ip_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());
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
        mihomo_platform::clear_home_dir_override();
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
        mihomo_platform::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_download_core_installed_version_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let version = "v1.20.0";
        let version_dir = temp_dir.path().join("versions").join(version);
        tokio::fs::create_dir_all(&version_dir).await.unwrap();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());

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
        mihomo_platform::clear_home_dir_override();
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
        let version_dir = temp_dir.path().join("versions").join(version);
        tokio::fs::create_dir_all(&version_dir).await.unwrap();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());

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

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 2048)
            .await
            .unwrap();
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
        mihomo_platform::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_activate_core_version_route() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let version = "v1.19.0";
        let version_dir = temp_dir.path().join("versions").join(version);
        tokio::fs::create_dir_all(&version_dir).await.unwrap();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());

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
        mihomo_platform::clear_home_dir_override();
    }

    #[tokio::test]
    async fn test_save_rules_route_schedules_rebuild() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        mihomo_platform::set_home_dir_override(temp_dir.path().to_path_buf());

        let manager = mihomo_config::ConfigManager::new().unwrap();
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
        mihomo_platform::clear_home_dir_override();
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
}
