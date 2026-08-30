//! Demo visual-capture plumbing: the one-shot `CAPTURE_READY` marker file
//! consumed by the screenshot tooling.

use super::{route_env_name, skin_name};
use crate::state::AppState;
use std::sync::atomic::Ordering;

impl AppState {
    /// Append the capture-ready marker, exactly once per process, after the
    /// first real `view()` pass (called from `AppState::view`). Idempotent:
    /// repeat calls are ignored via an atomic flag.
    pub(crate) fn write_capture_marker(&self) {
        if !self.demo {
            return;
        }
        if self.capture_marker_written.swap(true, Ordering::SeqCst) {
            return;
        }
        let Some(path) = self.capture_marker.as_ref() else {
            return;
        };
        let line = format!(
            "CAPTURE_READY page={} skin={}\n",
            route_env_name(self.current_route),
            skin_name(&self.theme),
        );
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            use std::io::Write as _;
            let _ = file.write_all(line.as_bytes());
        }
    }
}
