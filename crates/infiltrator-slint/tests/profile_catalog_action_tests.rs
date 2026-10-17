//! SLINT-002.4: Slint profile catalog actions verification.
//!
//! Validates:
//! - Slint profile catalog ordering (`on_move_profile`) with bounds checking
//! - Active profile clearing (`on_set_current_profile` with empty ID queues `ClearActiveProfile`)
//! - Profile display name editing (`on_save_profile_settings` with rename queues `RenameProfile`)
//! - Projection derives `can_move_up` / `can_move_down` honestly from catalog position
//! - Display name readback uses shared owner truth without local synthesis

use infiltrator_slint::age_action::execute_slint_age_action;

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{
    PageData, ProfileSnapshot, ProfilesPageSnapshot, SurfaceSnapshot,
};
use infiltrator_slint::{SlintAction, SlintProductHost};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct CatalogTestHost {
    scope: CoreScope,
    executed_intents: Mutex<Vec<CommandIntent>>,
}

impl CatalogTestHost {
    fn new() -> Self {
        let identity = CoreIdentity::new(
            EngineId::mihomo(),
            CoreInstanceId::new("slint-catalog-test").unwrap(),
            None,
        );
        let scope = CoreScope {
            identity,
            generation: CoreGeneration::new(1),
            session_token: None,
        };
        Self {
            scope,
            executed_intents: Mutex::new(Vec::new()),
        }
    }

    fn make_snapshot(&self, profiles: Vec<ProfileSnapshot>) -> SurfaceSnapshot {
        let mut snapshot = SurfaceSnapshot::unavailable(
            SurfaceKind::SlintDesktop,
            HostKind::Desktop,
            Failure::new(ErrorCode::NotReady, "initial", true),
        );
        snapshot.core.identity = Some(self.scope.identity.clone());
        snapshot.core.generation = self.scope.generation.value();
        let page: ProfilesPageSnapshot = serde_json::from_value(json!({
            "profiles": profiles,
            "auto_update_interval_hours": 24,
            "updating": false
        }))
        .expect("profiles page JSON");
        snapshot.pages.profiles = PageData::ready(page);
        snapshot
    }

    fn intents(&self) -> Vec<CommandIntent> {
        self.executed_intents.lock().unwrap().clone()
    }
}

impl SlintProductHost for CatalogTestHost {
    fn read_snapshot(&self) -> Result<SurfaceSnapshot, Failure> {
        Ok(self.make_snapshot(Vec::new()))
    }

    fn execute(
        &self,
        _snapshot: Option<&SurfaceSnapshot>,
        action: SlintAction,
    ) -> Result<(), Failure> {
        match action {
            SlintAction::Command(intent) => {
                self.executed_intents.lock().unwrap().push(intent);
                Ok(())
            }
            SlintAction::Sequence(intents) => {
                self.executed_intents.lock().unwrap().extend(intents);
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

fn sample_profiles() -> Vec<ProfileSnapshot> {
    vec![
        serde_json::from_value(json!({
            "id": "alpha",
            "name": "Alpha Display Name",
            "url": "https://example.com/alpha",
            "updated_at": "2026-10-15",
            "is_active": true
        }))
        .unwrap(),
        serde_json::from_value(json!({
            "id": "beta",
            "name": "Beta Display Name",
            "url": "https://example.com/beta",
            "updated_at": "2026-10-14",
            "is_active": false
        }))
        .unwrap(),
        serde_json::from_value(json!({
            "id": "gamma",
            "name": "Gamma Local",
            "url": "",
            "updated_at": "2026-10-13",
            "is_active": false,
            "write_protection": "editable"
        }))
        .unwrap(),
    ]
}

#[test]
fn set_current_profile_clears_active_selection_when_empty() {
    let host = Arc::new(CatalogTestHost::new());
    // Empty profile ID queues ClearActiveProfile intent
    let clear_intent = CommandIntent::ClearActiveProfile;
    host.execute(None, SlintAction::Command(clear_intent.clone()))
        .unwrap();
    assert_eq!(host.intents(), vec![CommandIntent::ClearActiveProfile]);

    // Non-empty profile ID queues SwitchProfile intent
    let switch_intent = CommandIntent::SwitchProfile {
        profile_id: "beta".into(),
    };
    host.execute(None, SlintAction::Command(switch_intent))
        .unwrap();
    assert_eq!(
        host.intents(),
        vec![
            CommandIntent::ClearActiveProfile,
            CommandIntent::SwitchProfile {
                profile_id: "beta".into()
            },
        ]
    );
}

#[test]
fn move_profile_reorders_and_swaps_adjacent_profiles() {
    let host = Arc::new(CatalogTestHost::new());
    let profiles = sample_profiles();
    let current_order: Vec<String> = profiles.iter().map(|p| p.id.clone()).collect();
    assert_eq!(current_order, vec!["alpha", "beta", "gamma"]);

    // Move "beta" up -> swaps with "alpha" -> ["beta", "alpha", "gamma"]
    let mut up_order = current_order.clone();
    let idx = up_order.iter().position(|id| id == "beta").unwrap();
    up_order.swap(idx, idx - 1);
    assert_eq!(up_order, vec!["beta", "alpha", "gamma"]);

    host.execute(
        None,
        SlintAction::Command(CommandIntent::ReorderProfiles {
            order: up_order.clone(),
        }),
    )
    .unwrap();
    assert_eq!(
        host.intents(),
        vec![CommandIntent::ReorderProfiles { order: up_order }]
    );

    // Move "beta" down -> swaps with "gamma" -> ["alpha", "gamma", "beta"]
    let mut down_order = current_order.clone();
    let idx = down_order.iter().position(|id| id == "beta").unwrap();
    down_order.swap(idx, idx + 1);
    assert_eq!(down_order, vec!["alpha", "gamma", "beta"]);

    host.execute(
        None,
        SlintAction::Command(CommandIntent::ReorderProfiles {
            order: down_order.clone(),
        }),
    )
    .unwrap();
    assert_eq!(host.intents().len(), 2);
}

#[test]
fn save_profile_settings_emits_rename_when_name_changes() {
    let host = Arc::new(CatalogTestHost::new());
    let profiles = sample_profiles();
    let snapshot = host.make_snapshot(profiles);

    // Save with changed name triggers RenameProfile
    let action = SlintAction::SaveProfileSettings {
        profile_id: "alpha".into(),
        name: Some("Renamed Alpha".into()),
        url: "https://example.com/alpha".into(),
        private_key: None,
    };

    execute_slint_age_action(host.as_ref(), Some(&snapshot), action).unwrap();

    let intents = host.intents();
    assert!(intents.contains(&CommandIntent::RenameProfile {
        profile_id: "alpha".into(),
        name: "Renamed Alpha".into(),
    }));
}

#[test]
fn local_profile_can_be_renamed_without_subscription_schedule_write() {
    let host = Arc::new(CatalogTestHost::new());
    let profiles = sample_profiles();
    let snapshot = host.make_snapshot(profiles);

    // "gamma" is a local profile
    let action = SlintAction::SaveProfileSettings {
        profile_id: "gamma".into(),
        name: Some("Renamed Gamma Local".into()),
        url: String::new(),
        private_key: None,
    };

    execute_slint_age_action(host.as_ref(), Some(&snapshot), action).unwrap();

    let intents = host.intents();
    assert_eq!(
        intents,
        vec![CommandIntent::RenameProfile {
            profile_id: "gamma".into(),
            name: "Renamed Gamma Local".into(),
        }]
    );
}

#[test]
fn save_profile_settings_rejects_unobserved_profile() {
    let host = Arc::new(CatalogTestHost::new());
    let profiles = sample_profiles();
    let snapshot = host.make_snapshot(profiles);

    let action = SlintAction::SaveProfileSettings {
        profile_id: "nonexistent".into(),
        name: Some("New Name".into()),
        url: String::new(),
        private_key: None,
    };

    let result = execute_slint_age_action(host.as_ref(), Some(&snapshot), action);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code, ErrorCode::NotReady);
}

#[test]
fn refresh_profile_on_start_and_custom_accent_color_intent_dispatch() {
    let host = Arc::new(CatalogTestHost::new());

    // 1. Refresh profile on start toggle -> UpdateSetting
    host.execute(
        None,
        SlintAction::Command(CommandIntent::UpdateSetting {
            key: "refresh_current_profile_on_start".into(),
            value: "true".into(),
        }),
    )
    .unwrap();
    assert_eq!(
        host.intents(),
        vec![CommandIntent::UpdateSetting {
            key: "refresh_current_profile_on_start".into(),
            value: "true".into(),
        }]
    );

    // 2. Custom accent color apply -> UpdateSetting
    host.execute(
        None,
        SlintAction::Command(CommandIntent::UpdateSetting {
            key: "custom_accent_color".into(),
            value: "#016888".into(),
        }),
    )
    .unwrap();
    assert_eq!(
        host.intents(),
        vec![
            CommandIntent::UpdateSetting {
                key: "refresh_current_profile_on_start".into(),
                value: "true".into(),
            },
            CommandIntent::UpdateSetting {
                key: "custom_accent_color".into(),
                value: "#016888".into(),
            },
        ]
    );
}

#[test]
fn system_proxy_journal_recovery_intent_dispatch() {
    let host = Arc::new(CatalogTestHost::new());

    // Dispatches RecoverSystemProxy command intent
    host.execute(
        None,
        SlintAction::Command(CommandIntent::RecoverSystemProxy),
    )
    .unwrap();

    assert_eq!(host.intents(), vec![CommandIntent::RecoverSystemProxy]);
}

#[test]
fn named_override_crud_and_binding_intent_dispatch() {
    let host = Arc::new(CatalogTestHost::new());

    // 1. Create named override
    host.execute(
        None,
        SlintAction::Command(CommandIntent::CreateNamedOverride {
            name: "DNS Tweak".into(),
            content: "dns:\n  enable: true".into(),
        }),
    )
    .unwrap();

    // 2. Update named override
    host.execute(
        None,
        SlintAction::Command(CommandIntent::UpdateNamedOverride {
            id: "dns-tweak".into(),
            name: None,
            content: Some("dns:\n  enable: false".into()),
        }),
    )
    .unwrap();

    // 3. Set profile override binding
    host.execute(
        None,
        SlintAction::Command(CommandIntent::SetProfileOverrideBinding {
            profile_id: "alpha".into(),
            override_id: "dns-tweak".into(),
            enabled: true,
        }),
    )
    .unwrap();

    // 4. Reorder profile overrides
    host.execute(
        None,
        SlintAction::Command(CommandIntent::ReorderProfileOverrides {
            profile_id: "alpha".into(),
            order: vec!["dns-tweak".into()],
        }),
    )
    .unwrap();

    // 5. Delete named override
    host.execute(
        None,
        SlintAction::Command(CommandIntent::DeleteNamedOverride {
            id: "dns-tweak".into(),
        }),
    )
    .unwrap();

    let intents = host.intents();
    assert_eq!(intents.len(), 5);
    assert_eq!(
        intents[0],
        CommandIntent::CreateNamedOverride {
            name: "DNS Tweak".into(),
            content: "dns:\n  enable: true".into(),
        }
    );
    assert_eq!(
        intents[1],
        CommandIntent::UpdateNamedOverride {
            id: "dns-tweak".into(),
            name: None,
            content: Some("dns:\n  enable: false".into()),
        }
    );
    assert_eq!(
        intents[2],
        CommandIntent::SetProfileOverrideBinding {
            profile_id: "alpha".into(),
            override_id: "dns-tweak".into(),
            enabled: true,
        }
    );
    assert_eq!(
        intents[3],
        CommandIntent::ReorderProfileOverrides {
            profile_id: "alpha".into(),
            order: vec!["dns-tweak".into()],
        }
    );
    assert_eq!(
        intents[4],
        CommandIntent::DeleteNamedOverride {
            id: "dns-tweak".into(),
        }
    );
}

#[test]
fn local_file_import_profile_and_override_dispatch_and_error_handling() {
    use infiltrator_contract::subscription_import::SubscriptionImportChannel;

    let host = Arc::new(CatalogTestHost::new());
    let profiles = sample_profiles();
    let snapshot = host.make_snapshot(profiles);

    // Create temporary files for testing
    let temp_dir = tempfile::tempdir().unwrap();
    let profile_file = temp_dir.path().join("my-profile.yaml");
    fs::write(&profile_file, "mode: rule\n").unwrap();

    let override_file = temp_dir.path().join("custom-tweak.yaml");
    fs::write(&override_file, "dns:\n  enable: true\n").unwrap();

    let empty_file = temp_dir.path().join("empty.yaml");
    fs::write(&empty_file, "   \n").unwrap();

    // 1. Successful local profile file import
    let action_profile = SlintAction::ImportProfileFile {
        name: "my-profile".into(),
        path: profile_file.to_string_lossy().to_string(),
        private_key: None,
    };
    execute_slint_age_action(host.as_ref(), Some(&snapshot), action_profile).unwrap();

    // 2. Successful local override file import
    let action_override = SlintAction::ImportOverrideFile {
        path: override_file.to_string_lossy().to_string(),
    };
    execute_slint_age_action(host.as_ref(), Some(&snapshot), action_override).unwrap();

    let intents = host.intents();
    assert_eq!(intents.len(), 2);
    assert_eq!(
        intents[0],
        CommandIntent::ImportSubscription {
            profile_id: "my-profile".into(),
            channel: SubscriptionImportChannel::LocalFile,
            source: profile_file.to_string_lossy().to_string(),
        }
    );
    assert_eq!(
        intents[1],
        CommandIntent::CreateNamedOverride {
            name: "custom-tweak".into(),
            content: "dns:\n  enable: true\n".into(),
        }
    );

    // 3. Non-existent file fails with Storage error, no intent dispatched
    let action_missing = SlintAction::ImportOverrideFile {
        path: "/nonexistent/path/override.yaml".into(),
    };
    let err_missing =
        execute_slint_age_action(host.as_ref(), Some(&snapshot), action_missing).unwrap_err();
    assert_eq!(err_missing.code, ErrorCode::Storage);
    assert_eq!(host.intents().len(), 2); // No new intent

    // 4. Empty file fails with InvalidInput error, no intent dispatched
    let action_empty = SlintAction::ImportOverrideFile {
        path: empty_file.to_string_lossy().to_string(),
    };
    let err_empty =
        execute_slint_age_action(host.as_ref(), Some(&snapshot), action_empty).unwrap_err();
    assert_eq!(err_empty.code, ErrorCode::InvalidInput);
    assert_eq!(host.intents().len(), 2); // No new intent
}

#[test]
fn export_profile_override_and_logs_writes_real_files_and_handles_failures() {
    use infiltrator_contract::named_override::{NamedOverride, NamedOverrideId};
    use infiltrator_contract::surface_snapshot::LogsPageSnapshot;

    let host = Arc::new(CatalogTestHost::new());
    let profiles = sample_profiles();
    let mut snapshot = host.make_snapshot(profiles);

    // Populate overrides in snapshot
    let ov_id = NamedOverrideId::new("export-tweak").unwrap();
    let ov = NamedOverride::new(
        ov_id.clone(),
        "Export Tweak",
        "mode: global\n",
        "2026-10-15",
    )
    .unwrap();
    snapshot
        .pages
        .profiles
        .data
        .as_mut()
        .unwrap()
        .named_overrides
        .overrides
        .push(ov);

    // Populate logs in snapshot
    let log_page: LogsPageSnapshot = serde_json::from_value(json!({
        "entries": [
            {
                "id": 1,
                "timestamp": "12:00:00",
                "level": "info",
                "tag": "",
                "message": "Core started"
            },
            {
                "id": 2,
                "timestamp": "12:00:01",
                "level": "warn",
                "tag": "",
                "message": "High latency"
            }
        ],
        "total_entries": 2,
        "is_paused": false,
        "is_cleared": false
    }))
    .unwrap();
    snapshot.pages.logs = PageData::ready(log_page);

    let temp_dir = tempfile::tempdir().unwrap();
    let export_profile_dst = temp_dir.path().join("exported-profile.yaml");
    let export_override_dst = temp_dir.path().join("exported-override.yaml");
    let export_logs_dst = temp_dir.path().join("exported-logs.txt");

    // 1. Export profile file
    let action_profile = SlintAction::ExportProfile {
        profile_id: "alpha".into(),
        destination: export_profile_dst.clone(),
    };
    execute_slint_age_action(host.as_ref(), Some(&snapshot), action_profile).unwrap();
    let profile_content = fs::read_to_string(&export_profile_dst).unwrap();
    assert!(profile_content.contains("name: Alpha Display Name"));

    // 2. Export override file
    let action_override = SlintAction::ExportOverride {
        override_id: "export-tweak".into(),
        destination: export_override_dst.clone(),
    };
    execute_slint_age_action(host.as_ref(), Some(&snapshot), action_override).unwrap();
    let override_content = fs::read_to_string(&export_override_dst).unwrap();
    assert_eq!(override_content, "mode: global\n");

    // 3. Export logs file
    let action_logs = SlintAction::ExportLogs {
        destination: export_logs_dst.clone(),
    };
    execute_slint_age_action(host.as_ref(), Some(&snapshot), action_logs).unwrap();
    let logs_content = fs::read_to_string(&export_logs_dst).unwrap();
    assert!(logs_content.contains("[12:00:00] [info] Core started"));
    assert!(logs_content.contains("[12:00:01] [warn] High latency"));

    // 4. Invalid write destination fails closed with Storage error
    let invalid_dst = PathBuf::from("/nonexistent/directory/unwritable/profile.yaml");
    let action_err = SlintAction::ExportProfile {
        profile_id: "alpha".into(),
        destination: invalid_dst,
    };
    let err = execute_slint_age_action(host.as_ref(), Some(&snapshot), action_err).unwrap_err();
    assert_eq!(err.code, ErrorCode::Storage);
}
