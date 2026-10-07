//! Demo visual-capture plumbing: the one-shot `CAPTURE_READY` marker file
//! consumed by the screenshot tooling.

use super::interaction;
use super::{route_env_name, skin_name};
use crate::state::AppState;
use std::fs::{OpenOptions, create_dir_all};
use std::sync::atomic::Ordering;

impl AppState {
    /// Append the capture-ready marker after actual native GPU readback. Idempotent:
    /// repeat calls are ignored via an atomic flag.
    pub(crate) fn write_capture_marker(&self) {
        if !self.shell.demo || !self.shell.capture_frame.is_ready() {
            return;
        }
        if self.shell.capture_marker_written.load(Ordering::SeqCst) {
            return;
        }
        let Some(path) = self.shell.capture_marker.as_ref() else {
            return;
        };
        let mut line = format!(
            "CAPTURE_READY page={} skin={}\n",
            route_env_name(self.shell.current_route),
            skin_name(&self.shell.theme),
        );
        if let Some(feature) = self.shell.capture_scenario {
            let scenario = feature.spec().id;
            if !interaction::ready(self, feature) {
                return;
            }
            line = format!("{} scenario={scenario} activated=true", line.trim_end());
            if let Some(bounds) = self.shell.capture_region_bounds {
                line.push_str(&format!(
                    " bounds={},{},{},{}",
                    bounds.x, bounds.y, bounds.width, bounds.height
                ));
            }
            line.push('\n');
        }
        if let Some(parent) = path.parent() {
            let _ = create_dir_all(parent);
        }
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            use std::io::Write as _;
            if file.write_all(line.as_bytes()).is_ok() {
                self.shell
                    .capture_marker_written
                    .store(true, Ordering::SeqCst);
            }
        }
    }
}
