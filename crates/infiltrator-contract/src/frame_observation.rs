//! Host-resolved opt-in frame-timing request (R152).
//!
//! Android NativeActivity has no inherited process environment contract. A
//! host writes this one-shot request into the app's internal data directory;
//! the Bevy product shell resolves it into the shared `zhuge-ui-kit-bevy`
//! frame-timing owner. A normal launch has no request file, no recorder and no
//! JSONL sink.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// File the host writes inside `AndroidApp::internal_data_path()`.
pub const ANDROID_FRAME_OBSERVATION_REQUEST: &str = "android-frame-observation.json";

/// Host-owned request for one product frame-timing sink.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AndroidFrameObservationRequest {
    /// Relative path under the app's internal data directory.
    pub sink: PathBuf,
    /// Stutter budget in milliseconds; must be finite and positive.
    pub budget_ms: f32,
    /// Run-level capture receipts bound to stuttering frames.
    pub capture_receipts: Vec<String>,
}

impl AndroidFrameObservationRequest {
    /// Reject empty paths, parent escapes, non-finite budgets and zero budgets.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let relative = Path::new(&self.sink);
        !relative.as_os_str().is_empty()
            && relative.is_relative()
            && !relative
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
            && self.budget_ms.is_finite()
            && self.budget_ms > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_relative_sink_with_a_positive_budget_is_a_request() {
        let base = AndroidFrameObservationRequest {
            sink: PathBuf::from("frame-timing.jsonl"),
            budget_ms: 16.0,
            capture_receipts: Vec::new(),
        };
        assert!(base.is_valid());
        let mut invalid = base.clone();
        invalid.sink = PathBuf::from("/absolute.jsonl");
        assert!(!invalid.is_valid());
        invalid = base.clone();
        invalid.sink = PathBuf::from("../escape.jsonl");
        assert!(!invalid.is_valid());
        invalid = base.clone();
        invalid.budget_ms = 0.0;
        assert!(!invalid.is_valid());
        invalid = base;
        invalid.budget_ms = f32::NAN;
        assert!(!invalid.is_valid());
    }
}
