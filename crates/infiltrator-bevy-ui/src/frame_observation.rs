//! Resolve a product-owned frame-timing request into the shared Bevy owner.

use bevy::app::App;
use infiltrator_contract::frame_observation::ANDROID_FRAME_OBSERVATION_REQUEST;
use infiltrator_contract::frame_observation::AndroidFrameObservationRequest;
use std::fs;
use std::path::PathBuf;
use zhuge_ui_core::frame_timing::record::FrameTimingIdentity;
use zhuge_ui_kit_bevy::frame_timing::{
    BevyFrameTimingEnv, FrameReceiptStyle, bevy_frame_timing_plugin_resolved,
};

const SINK_VAR: &str = "INFILTRATOR_ANDROID_FRAME_TIMING_SINK";
const BUDGET_VAR: &str = "INFILTRATOR_ANDROID_FRAME_TIMING_BUDGET_MS";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameObservationSelection {
    None,
    Android,
}

/// Read the one-shot host request from the app's internal data directory.
///
/// A missing or invalid request produces no plugin and no file: a soak that
/// forgot to bind its frame sink is still reported as unobserved, never as a
/// fabricated zero frame.
#[must_use]
pub fn android_request(
    internal_data_path: Option<PathBuf>,
) -> Option<AndroidFrameObservationRequest> {
    let root = internal_data_path?;
    let request_path = root.join(ANDROID_FRAME_OBSERVATION_REQUEST);
    let raw = fs::read_to_string(request_path).ok()?;
    let request: AndroidFrameObservationRequest = serde_json::from_str(&raw).ok()?;
    request.is_valid().then_some(request)
}

/// Install the owner for an Android request.
///
/// Desktop/capture compositions keep their existing env-bound owner; this
/// function does not create a second desktop sink authority.
pub fn install(
    app: &mut App,
    selection: FrameObservationSelection,
    android_internal_data_path: Option<PathBuf>,
) -> bool {
    match selection {
        FrameObservationSelection::None => false,
        FrameObservationSelection::Android => {
            install_android_from_app_path(app, android_internal_data_path)
        }
    }
}

fn install_android_from_app_path(app: &mut App, root: Option<PathBuf>) -> bool {
    let Some(root) = root else {
        return false;
    };
    let Some(request) = android_request(Some(root.clone())) else {
        return false;
    };
    let env = BevyFrameTimingEnv {
        sink_var: SINK_VAR,
        budget_var: BUDGET_VAR,
        default_budget_ms: 16.0,
        receipt_vars: &[],
        receipt_style: FrameReceiptStyle::RunLevel,
        identity: FrameTimingIdentity::new("musicfrog", "musicfrog-android-shell", "bevy"),
        max_samples: 240,
    };
    app.add_plugins(bevy_frame_timing_plugin_resolved(
        &env,
        root.join(request.sink),
        None,
        Some(request.budget_ms),
        request.capture_receipts.clone(),
    ));
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs, process};

    #[test]
    fn a_missing_or_invalid_request_installs_nothing() {
        let dir = env::temp_dir().join(format!("mf-frame-request-{}", process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert!(android_request(Some(dir.clone())).is_none());
        fs::write(dir.join(ANDROID_FRAME_OBSERVATION_REQUEST), "not json").unwrap();
        assert!(android_request(Some(dir.clone())).is_none());
        fs::write(
            dir.join(ANDROID_FRAME_OBSERVATION_REQUEST),
            r#"{"sink":"../escape.jsonl","budget_ms":16.0,"capture_receipts":[]}"#,
        )
        .unwrap();
        assert!(android_request(Some(dir.clone())).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_valid_relative_request_is_resolved() {
        let dir = env::temp_dir().join(format!("mf-frame-request-valid-{}", process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(ANDROID_FRAME_OBSERVATION_REQUEST),
            r#"{"sink":"frame-timing.jsonl","budget_ms":20.0,"capture_receipts":["receipt.txt"]}"#,
        )
        .unwrap();
        let request = android_request(Some(dir.clone()));
        assert_eq!(
            request,
            Some(AndroidFrameObservationRequest {
                sink: PathBuf::from("frame-timing.jsonl"),
                budget_ms: 20.0,
                capture_receipts: vec!["receipt.txt".to_owned()],
            })
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
