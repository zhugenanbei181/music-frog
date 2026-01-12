use chrono::Utc;
use serde::Serialize;
use tokio::sync::broadcast;

pub const EVENT_REBUILD_STARTED: &str = "rebuild-started";
pub const EVENT_REBUILD_FINISHED: &str = "rebuild-finished";
pub const EVENT_REBUILD_FAILED: &str = "rebuild-failed";
pub const EVENT_PROFILES_CHANGED: &str = "profiles-changed";
pub const EVENT_CORE_CHANGED: &str = "core-changed";
pub const EVENT_SETTINGS_CHANGED: &str = "settings-changed";
pub const EVENT_DNS_CHANGED: &str = "dns-changed";
pub const EVENT_FAKE_IP_CHANGED: &str = "fake-ip-changed";
pub const EVENT_RULES_CHANGED: &str = "rules-changed";
pub const EVENT_RULE_PROVIDERS_CHANGED: &str = "rule-providers-changed";
pub const EVENT_TUN_CHANGED: &str = "tun-changed";
pub const EVENT_WEBDAV_SYNCED: &str = "webdav-synced";

const EVENT_CHANNEL_SIZE: usize = 64;

#[derive(Debug, Clone, Serialize)]
pub struct AdminEvent {
    pub kind: String,
    pub detail: Option<String>,
    pub timestamp: i64,
}

impl AdminEvent {
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            detail: None,
            timestamp: Utc::now().timestamp_millis(),
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Clone)]
pub struct AdminEventBus {
    sender: broadcast::Sender<AdminEvent>,
}

impl Default for AdminEventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl AdminEventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_CHANNEL_SIZE);
        Self { sender }
    }

    pub fn publish(&self, event: AdminEvent) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AdminEvent> {
        self.sender.subscribe()
    }
}
