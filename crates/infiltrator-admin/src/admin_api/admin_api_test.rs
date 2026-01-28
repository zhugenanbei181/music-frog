#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt; // for `oneshot`, `ready`, and `call`
    use std::sync::{Arc, Mutex};
    use crate::admin_api::*;
    use infiltrator_core::AppSettings;
    use infiltrator_http::HttpClient;

    #[derive(Clone)]
    struct MockContext {
        rebuild_count: Arc<Mutex<usize>>,
    }

    #[async_trait::async_trait]
    impl AdminApiContext for MockContext {
        async fn rebuild_runtime(&self) -> anyhow::Result<()> {
            let mut count = self.rebuild_count.lock().unwrap();
            *count += 1;
            Ok(())
        }
        async fn set_use_bundled_core(&self, _enabled: bool) {}
        async fn refresh_core_version_info(&self) {}
        async fn notify_subscription_update(&self, _p: String, _s: bool, _m: Option<String>) {}
        async fn editor_path(&self) -> Option<String> { None }
        async fn set_editor_path(&self, _path: Option<String>) {}
        async fn pick_editor_path(&self) -> Option<String> { None }
        async fn open_profile_in_editor(&self, _name: &str) -> anyhow::Result<()> { Ok(()) }
        async fn get_app_settings(&self) -> AppSettings { AppSettings::default() }
        async fn save_app_settings(&self, _s: AppSettings) -> anyhow::Result<()> { Ok(()) }
    }

    fn setup_app() -> axum::Router {
        let ctx = MockContext {
            rebuild_count: Arc::new(Mutex::new(0)),
        };
        let bus = events::AdminEventBus::new();
        let state = AdminApiState::new(ctx, bus);
        router(state)
    }

    #[tokio::test]
    async fn test_get_profiles_route() {
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
    }

    #[tokio::test]
    async fn test_get_settings_route() {
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
    }

    #[tokio::test]
    async fn test_import_profile_integration() {
        let mut server = mockito::Server::new_async().await;
        let mock_yaml = "port: 7890\nmode: rule";
        let _m = server.mock("GET", "/sub")
            .with_status(200)
            .with_body(mock_yaml)
            .create_async().await;

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

        assert_eq!(response.status(), StatusCode::OK);
        
        // Verify file was saved
        let config_path = temp_dir.path().join("configs").join("test-import.yaml");
        assert!(config_path.exists());
        
        mihomo_platform::clear_home_dir_override();
    }
}
