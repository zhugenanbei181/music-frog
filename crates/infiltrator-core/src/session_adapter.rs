use crate::redact::mask_secret;
use anyhow::{Result, anyhow};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Represents the high-level state of the Core Session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Running,
    Stopped,
    Starting,
    Failed(String),
}

/// A frozen snapshot of the session state for UI or Admin consumption.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSessionSnapshot {
    pub state: SessionState,
    pub generation: u64,
    pub active_profile: Option<String>,
    pub endpoint: Option<String>,
    pub secret_masked: Option<String>,
}

/// Internal session manager holding the state.
#[derive(Debug)]
pub struct SessionManager {
    pub state: SessionState,
    pub generation: u64,
    pub active_profile: Option<String>,
    pub endpoint: Option<String>,
    pub secret: Option<String>,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            state: SessionState::Stopped,
            generation: 0,
            active_profile: None,
            endpoint: None,
            secret: None,
        }
    }
}

/// High-level standardized adapter bridging CoreSession to Iced, Admin API, and Android UniFFI.
#[derive(Clone)]
pub struct CoreSessionAdapter {
    manager: Arc<Mutex<SessionManager>>,
}

impl Default for CoreSessionAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreSessionAdapter {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(Mutex::new(SessionManager::new())),
        }
    }

    /// Allows passing an external manager if required.
    pub fn with_manager(manager: Arc<Mutex<SessionManager>>) -> Self {
        Self { manager }
    }

    /// Start a new session with the given profile and configuration content.
    pub async fn start_with_profile(
        &self,
        profile_name: &str,
        config_content: &str,
    ) -> Result<SessionState> {
        let mut mgr = self.manager.lock().await;

        if mgr.state == SessionState::Starting {
            return Err(anyhow!("Session is already starting"));
        }

        mgr.generation += 1;
        mgr.state = SessionState::Starting;
        mgr.active_profile = Some(profile_name.to_string());

        let secret = extract_secret(config_content);
        if !secret.is_empty() {
            mgr.secret = Some(secret);
        }
        mgr.endpoint = Some("127.0.0.1:9090".to_string());

        // In a real integration, we would await core process spin-up here.
        mgr.state = SessionState::Running;

        Ok(mgr.state.clone())
    }

    /// Restart the session with a new configuration.
    pub async fn restart_with_config(&self, config_content: &str) -> Result<SessionState> {
        let mut mgr = self.manager.lock().await;

        if mgr.state == SessionState::Starting {
            return Err(anyhow!("Session is already starting"));
        }

        mgr.generation += 1;
        mgr.state = SessionState::Starting;

        let secret = extract_secret(config_content);
        if !secret.is_empty() {
            mgr.secret = Some(secret);
        }

        // In a real integration, we would await core process restart here.
        mgr.state = SessionState::Running;
        Ok(mgr.state.clone())
    }

    /// Stop the currently active session.
    pub async fn stop_session(&self) -> Result<()> {
        let mut mgr = self.manager.lock().await;

        if mgr.state == SessionState::Starting {
            return Err(anyhow!("Cannot stop while starting"));
        }

        if mgr.state != SessionState::Stopped {
            mgr.generation += 1;
            mgr.state = SessionState::Stopped;
        }

        Ok(())
    }

    /// Helper for testing/internal failures: force a failed state.
    pub async fn set_failed(&self, error_message: String) {
        let mut mgr = self.manager.lock().await;
        mgr.state = SessionState::Failed(error_message);
    }

    /// Retrieve a frozen snapshot of the session state, masking secrets.
    pub async fn get_snapshot(&self) -> CoreSessionSnapshot {
        let mgr = self.manager.lock().await;

        let secret_masked = mgr.secret.as_ref().map(|s| mask_secret(s, s));

        // Enforce error token masking via crate::redact
        let masked_state = if let SessionState::Failed(ref err) = mgr.state {
            if let Some(ref sec) = mgr.secret {
                SessionState::Failed(mask_secret(err, sec))
            } else {
                mgr.state.clone()
            }
        } else {
            mgr.state.clone()
        };

        CoreSessionSnapshot {
            state: masked_state,
            generation: mgr.generation,
            active_profile: mgr.active_profile.clone(),
            endpoint: mgr.endpoint.clone(),
            secret_masked,
        }
    }

    /// Verify whether a previously captured generation matches the current generation.
    pub async fn verify_generation(&self, generation: u64) -> bool {
        let mgr = self.manager.lock().await;
        mgr.generation == generation
    }
}

/// Extracts a rudimentary secret from configuration lines.
fn extract_secret(config: &str) -> String {
    for line in config.lines() {
        if line.trim().starts_with("secret:") {
            return line.trim().trim_start_matches("secret:").trim().to_string();
        }
    }
    "".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_lifecycle() {
        let adapter = CoreSessionAdapter::new();

        let snap = adapter.get_snapshot().await;
        assert_eq!(snap.state, SessionState::Stopped);

        let state = adapter
            .start_with_profile("test_profile", "secret: my_token_123")
            .await
            .unwrap();
        assert_eq!(state, SessionState::Running);

        let snap = adapter.get_snapshot().await;
        assert_eq!(snap.state, SessionState::Running);
        assert_eq!(snap.active_profile.as_deref(), Some("test_profile"));

        adapter.stop_session().await.unwrap();
        let snap = adapter.get_snapshot().await;
        assert_eq!(snap.state, SessionState::Stopped);
    }

    #[tokio::test]
    async fn test_generation_fencing() {
        let adapter = CoreSessionAdapter::new();
        assert!(adapter.verify_generation(0).await);
        assert!(!adapter.verify_generation(1).await);

        adapter.start_with_profile("p1", "").await.unwrap();
        assert!(adapter.verify_generation(1).await);

        adapter.restart_with_config("").await.unwrap();
        assert!(adapter.verify_generation(2).await);

        adapter.stop_session().await.unwrap();
        assert!(adapter.verify_generation(3).await);
    }

    #[tokio::test]
    async fn test_secret_masking() {
        let adapter = CoreSessionAdapter::new();
        adapter
            .start_with_profile("p2", "secret: my_super_secret")
            .await
            .unwrap();

        let snap = adapter.get_snapshot().await;
        // mask_secret("my_super_secret", "my_super_secret") returns "***"
        assert_eq!(snap.secret_masked.as_deref(), Some("***"));

        // test error token masking
        adapter
            .set_failed("Connection failed due to invalid token: my_super_secret".to_string())
            .await;

        let snap = adapter.get_snapshot().await;
        if let SessionState::Failed(msg) = snap.state {
            assert!(msg.contains("***"));
            assert!(!msg.contains("my_super_secret"));
        } else {
            panic!("Expected failed state");
        }
    }

    #[tokio::test]
    async fn test_concurrent_protection() {
        let manager = Arc::new(Mutex::new(SessionManager::new()));

        // Force state to Starting
        {
            let mut guard = manager.lock().await;
            guard.state = SessionState::Starting;
        }

        let adapter = CoreSessionAdapter::with_manager(manager);

        // Start should fail
        let res = adapter.start_with_profile("p3", "").await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "Session is already starting");

        // Stop should fail
        let res2 = adapter.stop_session().await;
        assert!(res2.is_err());
        assert_eq!(res2.unwrap_err().to_string(), "Cannot stop while starting");
    }
}
