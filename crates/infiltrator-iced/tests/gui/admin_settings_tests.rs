//! Admin Web UI server lifecycle glue (headless: under cfg(test) no real
//! server ever starts, and dropped Tasks never persist settings to disk).
//! Mounted via `src/test_mounts.rs` (crate root).
//! test-intent: behavior

use crate::{AppState, Message};
use infiltrator_core::settings::{AdminServerConfig, AppSettings, RuntimePanelConfig};

#[test]
fn test_admin_settings_toggle_drives_lifecycle_bookkeeping() {
    let (mut state, _) = AppState::new();
    // Startup defaults mirror src-tauri: the feature starts enabled.
    assert!(state.admin_enabled);
    assert_eq!(state.admin_port, crate::admin_server::ADMIN_DEFAULT_PORT);

    // First lifecycle pass at the defaults records a Start.
    let _ = state.update(Message::SetAdminEnabled(true));
    assert_eq!(
        state.admin_server.started_config(),
        Some(AdminServerConfig {
            enabled: true,
            port: crate::admin_server::ADMIN_DEFAULT_PORT,
        }),
        "settings toggle should produce a start intent"
    );
    assert!(!state.admin_server.is_running(), "tests never start the server");

    // Toggling off records a Stop: the pending-start bookkeeping is cleared
    // and no handle ever exists in tests.
    let _ = state.update(Message::SetAdminEnabled(false));
    assert!(!state.admin_enabled);
    assert_eq!(
        state.admin_server.started_config(),
        None,
        "disable should produce a stop intent"
    );
    assert!(!state.admin_server.is_running());

    // Re-enabling records a Start again.
    let _ = state.update(Message::SetAdminEnabled(true));
    assert_eq!(
        state.admin_server.started_config(),
        Some(AdminServerConfig {
            enabled: true,
            port: crate::admin_server::ADMIN_DEFAULT_PORT,
        })
    );

    // Applying the same config again is a no-op: bookkeeping unchanged.
    let _ = state.update(Message::ApplyAdminSettings);
    assert_eq!(
        state.admin_server.started_config(),
        Some(AdminServerConfig {
            enabled: true,
            port: crate::admin_server::ADMIN_DEFAULT_PORT,
        })
    );
}

#[test]
fn test_admin_port_change_drives_restart_bookkeeping() {
    let (mut state, _) = AppState::new();
    // Establish the pending start at the default port.
    let _ = state.update(Message::SetAdminEnabled(true));

    let _ = state.update(Message::UpdateAdminPort("25300".into()));
    let _ = state.update(Message::ApplyAdminSettings);
    assert_eq!(state.admin_port, 25300);
    assert_eq!(state.admin_port_input, "25300");
    assert_eq!(
        state.admin_server.started_config(),
        Some(AdminServerConfig {
            enabled: true,
            port: 25300,
        }),
        "port change should produce a restart intent (new config recorded)"
    );

    // A port that is not a usable TCP port is rejected, config unchanged.
    let _ = state.update(Message::UpdateAdminPort("70000".into()));
    let _ = state.update(Message::ApplyAdminSettings);
    assert_eq!(state.admin_port, 25300, "invalid port must not apply");

    let _ = state.update(Message::UpdateAdminPort("0".into()));
    let _ = state.update(Message::ApplyAdminSettings);
    assert_eq!(state.admin_port, 25300, "port 0 must not apply");
}

#[test]
fn test_parse_admin_port_rejects_non_ports() {
    assert_eq!(AppState::parse_admin_port("25210"), Some(25210));
    assert_eq!(AppState::parse_admin_port(" 8080 "), Some(8080));
    assert_eq!(AppState::parse_admin_port(""), None);
    assert_eq!(AppState::parse_admin_port("abc"), None);
    assert_eq!(AppState::parse_admin_port("0"), None, "port 0 is not usable");
    assert_eq!(AppState::parse_admin_port("-1"), None);
    assert_eq!(AppState::parse_admin_port("70000"), None, "beyond u16");
}

#[test]
fn test_settings_loaded_applies_runtime_panel_state() {
    let (mut state, _) = AppState::new();
    let settings = AppSettings {
        runtime_panel: RuntimePanelConfig {
            auto_refresh: false,
            delay_sort: "name_desc".into(),
            delay_test_url: "https://example.com/generate_204".into(),
            delay_timeout_ms: 1200,
            connection_filter: "api".into(),
            connection_sort: "host_asc".into(),
        },
        ..AppSettings::default()
    };

    let _ = state.update(Message::SettingsLoaded(Ok(settings)));
    assert!(!state.runtime_auto_refresh);
    assert_eq!(state.proxy_delay_sort, "name_desc");
    assert_eq!(
        state.runtime_delay_test_url,
        "https://example.com/generate_204"
    );
    assert_eq!(state.runtime_delay_timeout_ms, "1200");
    assert_eq!(state.runtime_connection_filter, "api");
    assert_eq!(state.runtime_connection_sort, "host_asc");
}

#[test]
fn test_settings_loaded_applies_admin_config() {
    let (mut state, _) = AppState::new();
    // A settings file without an admin section keeps serde defaults
    // (back-compat), which mirror the legacy Tauri client.
    let _ = state.update(Message::SettingsLoaded(Ok(AppSettings::default())));
    assert!(state.admin_enabled);
    assert_eq!(
        state.admin_server.started_config(),
        Some(AdminServerConfig::default()),
        "loading enabled settings produces a start intent"
    );

    // Custom admin config flows through the same path (and restarts because
    // the desired config changed).
    let mut settings = AppSettings::default();
    settings.admin = AdminServerConfig {
        enabled: true,
        port: 26000,
    };
    let _ = state.update(Message::SettingsLoaded(Ok(settings)));
    assert_eq!(state.admin_port, 26000);
    assert_eq!(state.admin_port_input, "26000");
    assert_eq!(
        state.admin_server.started_config(),
        Some(AdminServerConfig {
            enabled: true,
            port: 26000,
        })
    );

    // Disabled settings produce a stop (bookkeeping cleared).
    let mut settings = AppSettings::default();
    settings.admin.enabled = false;
    let _ = state.update(Message::SettingsLoaded(Ok(settings)));
    assert!(!state.admin_enabled);
    assert_eq!(state.admin_server.started_config(), None);
}

#[test]
fn test_external_settings_loaded_resyncs_admin_lifecycle() {
    let (mut state, _) = AppState::new();
    let mut settings = AppSettings::default();
    settings.admin = AdminServerConfig {
        enabled: true,
        port: 27000,
    };
    // The WebUI save path reloads settings without the WebDAV startup sync.
    let _ = state.update(Message::ExternalSettingsLoaded(Ok(settings)));
    assert_eq!(state.admin_port, 27000);
    assert_eq!(
        state.admin_server.started_config(),
        Some(AdminServerConfig {
            enabled: true,
            port: 27000,
        })
    );

    // An externally disabled server stops the bookkeeping.
    let mut settings = AppSettings::default();
    settings.admin.enabled = false;
    let _ = state.update(Message::ExternalSettingsLoaded(Ok(settings)));
    assert_eq!(state.admin_server.started_config(), None);
}
