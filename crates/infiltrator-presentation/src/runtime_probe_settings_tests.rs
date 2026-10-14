use crate::runtime_probe_settings::{parse_draft, project_settings};
use crate::runtime_probe_settings_editor::RuntimeProbeSettingsDraft;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::runtime_settings::{RuntimeProbeSettings, RuntimeProbeSettingsState};
use infiltrator_domain::settings::{AppSettings, RuntimePanelConfig};

#[test]
fn draft_validation_is_engine_neutral_and_does_not_invent_blank_values() {
    let settings = parse_draft(&RuntimeProbeSettingsDraft {
        url: " https://probe.example.test/path ".into(),
        timeout_ms: "60000".into(),
    })
    .unwrap();
    assert_eq!(settings.url(), "https://probe.example.test/path");
    assert_eq!(settings.timeout_ms(), 60_000);

    for draft in [
        RuntimeProbeSettingsDraft {
            url: "relative/path".into(),
            timeout_ms: "5000".into(),
        },
        RuntimeProbeSettingsDraft {
            url: "https://probe.example.test".into(),
            timeout_ms: "0".into(),
        },
        RuntimeProbeSettingsDraft {
            url: String::new(),
            timeout_ms: "5000".into(),
        },
    ] {
        assert_eq!(
            parse_draft(&draft).unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }
}

#[test]
fn unavailable_settings_never_project_synthetic_defaults() {
    let unavailable = project_settings(None);
    assert!(matches!(
        unavailable,
        RuntimeProbeSettingsState::Unavailable { .. }
    ));

    let settings = AppSettings {
        runtime_panel: RuntimePanelConfig {
            probe_settings: RuntimeProbeSettings::new("https://probe.example.test/204", 9_000)
                .unwrap(),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        project_settings(Some(&Ok(settings))),
        RuntimeProbeSettingsState::Available {
            settings: RuntimeProbeSettings::new("https://probe.example.test/204", 9_000).unwrap(),
        }
    );
}
