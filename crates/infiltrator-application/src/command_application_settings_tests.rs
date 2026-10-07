//! Headless tests for the shared `UpdateSetting` write path: appearance
//! preferences and global shortcut capture (DUAL-15-06/15-09).
//!
//! An in-memory `SettingsStore` proves the application validates and
//! canonicalises both values instead of persisting whatever string a surface
//! happened to send.

use super::*;
use async_trait::async_trait;
use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::mini_hud::MiniHudPlacement;
use infiltrator_contract::proxy_probe_options::ProxyProbeOptions;
use infiltrator_contract::shortcuts::ShortcutAction;
use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::error::PortError;
use infiltrator_ports::settings_store::SettingsStore;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FakeSettingsStore {
    settings: Mutex<AppSettings>,
    reject_save: AtomicBool,
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
        if self.reject_save.load(Ordering::SeqCst) {
            return Err(PortError::Io("probe settings write denied".into()));
        }
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
        .find(|binding| binding.action == ShortcutAction::ToggleTun)
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

#[tokio::test]
async fn mini_hud_placement_writes_are_validated_per_field() {
    let (application, store) = settings_application();

    for (key, value) in [("mini_hud.x", "320"), ("mini_hud.y", "-40")] {
        application
            .execute(CommandIntent::UpdateSetting {
                key: key.to_owned(),
                value: value.to_owned(),
            })
            .await
            .expect("coordinate write");
    }
    application
        .execute(CommandIntent::UpdateSetting {
            key: "mini_hud.pinned".to_owned(),
            value: "true".to_owned(),
        })
        .await
        .expect("pin write");

    let placement = store.settings.lock().expect("lock").mini_hud;
    assert_eq!(
        placement,
        MiniHudPlacement {
            x: 320,
            y: -40,
            pinned: true,
        }
    );

    let bad_coordinate = application
        .execute(CommandIntent::UpdateSetting {
            key: "mini_hud.x".to_owned(),
            value: "left".to_owned(),
        })
        .await
        .expect_err("reject a non-numeric coordinate");
    assert_eq!(bad_coordinate.code, ErrorCode::InvalidInput);

    let bad_field = application
        .execute(CommandIntent::UpdateSetting {
            key: "mini_hud.z".to_owned(),
            value: "1".to_owned(),
        })
        .await
        .expect_err("reject an unknown HUD field");
    assert_eq!(bad_field.code, ErrorCode::InvalidInput);
    // Rejected writes leave the stored placement untouched.
    assert_eq!(store.settings.lock().expect("lock").mini_hud.x, 320);
}

#[tokio::test]
async fn probe_options_persist_atomically_preserve_unrelated_settings_and_refuse_bad_or_failed_writes()
 {
    let (application, store) = settings_application();
    let original = store.settings.lock().unwrap().clone();
    let options = ProxyProbeOptions {
        test_url: "https://probe.example.test/check".into(),
        timeout_ms: 32767,
    };
    let intent = CommandIntent::SetProxyProbeOptions {
        options: options.clone(),
    };
    application.execute(intent.clone()).await.unwrap();
    let stored = store.settings.lock().unwrap().clone();
    assert_eq!(stored.runtime_panel.delay_test_url, options.test_url);
    assert_eq!(stored.runtime_panel.delay_timeout_ms, options.timeout_ms);
    assert_eq!(stored.theme, original.theme);
    assert_eq!(
        stored.runtime_panel.connection_sort,
        original.runtime_panel.connection_sort
    );
    for timeout in [0, 32768, 60000] {
        let failure = application
            .execute(CommandIntent::SetProxyProbeOptions {
                options: ProxyProbeOptions {
                    timeout_ms: timeout,
                    ..options.clone()
                },
            })
            .await
            .unwrap_err();
        assert_eq!(failure.code, ErrorCode::InvalidInput);
        assert_eq!(*store.settings.lock().unwrap(), stored);
    }
    store.reject_save.store(true, Ordering::SeqCst);
    let failure = application
        .execute(CommandIntent::SetProxyProbeOptions {
            options: ProxyProbeOptions::default(),
        })
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::Storage);
    assert!(failure.retryable);
    assert_eq!(*store.settings.lock().unwrap(), stored);
    store.reject_save.store(false, Ordering::SeqCst);
    application.execute(intent.clone()).await.unwrap();
    let failure = CommandApplication::new().execute(intent).await.unwrap_err();
    assert_eq!(failure.code, ErrorCode::NotReady);
}

#[tokio::test]
async fn language_commands_canonicalize_and_persist_only_after_success_without_overwriting_unrelated_settings()
 {
    let (application, store) = settings_application();
    for (raw, expected) in [("en", "en-US"), (" ZH_cn ", "zh-CN"), ("SYSTEM", "system")] {
        application
            .execute(CommandIntent::UpdateSetting {
                key: "language".into(),
                value: raw.into(),
            })
            .await
            .unwrap();
        assert_eq!(store.settings.lock().unwrap().language, expected);
    }
    let original = store.settings.lock().unwrap().clone();
    for invalid in ["", "unsupported", "zh-TW"] {
        assert_eq!(
            application
                .execute(CommandIntent::UpdateSetting {
                    key: "language".into(),
                    value: invalid.into()
                })
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput
        );
        assert_eq!(*store.settings.lock().unwrap(), original);
    }
    store.reject_save.store(true, Ordering::SeqCst);
    assert_eq!(
        application
            .execute(CommandIntent::SetLanguage {
                preference: LanguagePreference::English
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::Storage
    );
    assert_eq!(*store.settings.lock().unwrap(), original);
    store.reject_save.store(false, Ordering::SeqCst);
    application
        .execute(CommandIntent::SetLanguage {
            preference: LanguagePreference::English,
        })
        .await
        .unwrap();
    let mut expected = original;
    expected.language = "en-US".into();
    assert_eq!(*store.settings.lock().unwrap(), expected);
    assert_eq!(
        CommandApplication::new()
            .execute(CommandIntent::SetLanguage {
                preference: LanguagePreference::English
            })
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotReady
    );
}
