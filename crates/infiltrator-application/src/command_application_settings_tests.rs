//! Headless tests for the shared `UpdateSetting` write path: appearance
//! preferences and global shortcut capture (DUAL-15-06/15-09).
//!
//! An in-memory `SettingsStore` proves the application validates and
//! canonicalises both values instead of persisting whatever string a surface
//! happened to send.

use super::*;
use async_trait::async_trait;
use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::error::PortError;
use infiltrator_ports::settings_store::SettingsStore;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FakeSettingsStore {
    settings: Mutex<AppSettings>,
}

#[async_trait]
impl SettingsStore for FakeSettingsStore {
    async fn load(&self) -> Result<AppSettings, PortError> {
        Ok(self.settings.lock().expect("settings lock").clone())
    }

    async fn load_hydrated(&self) -> Result<AppSettings, PortError> {
        self.load().await
    }

    async fn save(&self, settings: &AppSettings) -> Result<(), PortError> {
        *self.settings.lock().expect("settings lock") = settings.clone();
        Ok(())
    }
}

fn settings_application() -> (CommandApplication, Arc<FakeSettingsStore>) {
    let store = Arc::new(FakeSettingsStore::default());
    let application =
        CommandApplication::new().with_settings(SettingsApplication::new(store.clone()));
    (application, store)
}

#[tokio::test]
async fn theme_writes_are_validated_and_canonicalised() {
    let (application, store) = settings_application();

    application
        .execute(CommandIntent::UpdateSetting {
            key: "theme".to_owned(),
            value: "EyeForest".to_owned(),
        })
        .await
        .expect("aliases resolve to a canonical skin");
    assert_eq!(store.settings.lock().expect("lock").theme, "forest");

    application
        .execute(CommandIntent::UpdateSetting {
            key: "theme".to_owned(),
            value: "system".to_owned(),
        })
        .await
        .expect("system is a preference");
    assert_eq!(store.settings.lock().expect("lock").theme, "system");

    let failure = application
        .execute(CommandIntent::UpdateSetting {
            key: "theme".to_owned(),
            value: "banana".to_owned(),
        })
        .await
        .expect_err("unknown skins are rejected");
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(store.settings.lock().expect("lock").theme, "system");
}

#[tokio::test]
async fn shortcut_capture_persists_and_rejects_conflicts() {
    let (application, store) = settings_application();

    application
        .execute(CommandIntent::UpdateSetting {
            key: "shortcut.toggle_tun".to_owned(),
            value: "Ctrl+Alt+Y".to_owned(),
        })
        .await
        .expect("free chord capture");
    let settings = store.settings.lock().expect("lock").clone();
    let captured = settings
        .shortcuts
        .iter()
        .find(|binding| {
            binding.action == infiltrator_contract::shortcuts::ShortcutAction::ToggleTun
        })
        .expect("captured binding")
        .chord
        .display_string(false);
    assert_eq!(captured, "Ctrl+Alt+Y");

    let conflict = application
        .execute(CommandIntent::UpdateSetting {
            key: "shortcut.toggle_mini_hud".to_owned(),
            value: "Ctrl+Alt+Y".to_owned(),
        })
        .await
        .expect_err("occupied chord");
    assert_eq!(conflict.code, ErrorCode::InvalidInput);
    assert!(conflict.message.contains("shortcut conflict"));

    let unknown = application
        .execute(CommandIntent::UpdateSetting {
            key: "shortcut.banana".to_owned(),
            value: "Ctrl+K".to_owned(),
        })
        .await
        .expect_err("unknown action");
    assert_eq!(unknown.code, ErrorCode::InvalidInput);

    let malformed = application
        .execute(CommandIntent::UpdateSetting {
            key: "shortcut.toggle_tun".to_owned(),
            value: "Ctrl+Alt+Delete".to_owned(),
        })
        .await
        .expect_err("multi-letter key");
    assert_eq!(malformed.code, ErrorCode::InvalidInput);
}
