//! Capture readiness follows native redraws and GPU readback, never a view construction.
#[cfg(any(feature = "capture-tools", test))]
use crate::capture_scenario;
use crate::state::AppState;
use crate::types::message::Message;
use iced::window::Screenshot;
use iced::{Rectangle, Task, window};
use image::{ImageFormat, RgbaImage};
#[cfg(any(feature = "capture-tools", test))]
use std::sync::atomic::Ordering;

/// How many consecutive native readbacks must repeat the same pixels before
/// the capture publishes that frame.
///
/// The compositor receipt is compared against the published readback, so the
/// capture may only publish a frame the compositor shows as well. A pointer
/// that settles on a control, an entrance that finishes, or any other late
/// visual change lands *after* the first readback: the surface then renders a
/// different native frame and the compositor shows that one. Requiring the
/// surface to repeat the same frame for a full window of readbacks proves the
/// surface has settled, so the published frame is the frame the compositor
/// still shows.
pub(crate) const SETTLED_READBACKS: u8 = 8;

/// The native frame the capture published.
#[derive(Default)]
pub struct CaptureFrameGate {
    revision: u64,
    bounds: Option<Rectangle>,
    redraws: u8,
    pending: bool,
    ready: bool,
    /// The pixels of the previous native readback.
    previous: Option<Vec<u8>>,
    /// How many consecutive readbacks repeated [`Self::previous`].
    repeated: u8,
}

impl CaptureFrameGate {
    #[cfg(any(feature = "capture-tools", test))]
    pub(crate) fn is_ready(&self) -> bool {
        self.ready
    }
}

#[cfg(test)]
#[path = "../tests/gui/capture_frame_tests.rs"]
mod tests;

impl AppState {
    #[cfg(any(feature = "capture-tools", test))]
    fn capture_state_ready(&self) -> bool {
        self.shell.demo
            && self.shell.capture_marker.is_some()
            && !self.shell.capture_marker_written.load(Ordering::SeqCst)
            && self
                .shell
                .capture_scenario
                .is_none_or(|feature| capture_scenario::business_opened(self, feature))
    }

    #[cfg(not(any(feature = "capture-tools", test)))]
    fn capture_state_ready(&self) -> bool {
        false
    }

    pub(crate) fn capture_frame_task(&mut self) -> Task<Message> {
        // The capture path stays open while the demo holds a marker file: the
        // published frame has to stay current, because the compositor
        // screenshots the window after the surface settled. A frame the
        // compositor already left behind proves nothing.
        if !self.shell.demo || self.shell.capture_marker.is_none() {
            self.shell.capture_frame = CaptureFrameGate::default();
            return Task::none();
        }
        let revision = self.surface.revision();
        let bounds = self.shell.capture_region_bounds;
        let gate = &mut self.shell.capture_frame;
        if gate.revision != revision || gate.bounds != bounds {
            *gate = CaptureFrameGate {
                revision,
                bounds,
                ..Default::default()
            };
        }
        if gate.pending {
            return Task::none();
        }
        gate.redraws = gate.redraws.saturating_add(1);
        // The first redraw can still precede the state change's rebuilt view.
        // The next native redraw draws that view; readback runs on the renderer.
        if gate.redraws < 2 {
            return Task::none();
        }
        gate.pending = true;
        window::latest().then(move |id| match id {
            Some(id) => {
                window::screenshot(id).map(move |screenshot| Message::CaptureFrameRendered {
                    revision,
                    bounds,
                    screenshot,
                })
            }
            None => Task::none(),
        })
    }

    pub(crate) fn finish_capture_frame(
        &mut self,
        revision: u64,
        bounds: Option<Rectangle>,
        screenshot: Screenshot,
    ) {
        let valid = revision == self.surface.revision()
            && bounds == self.shell.capture_region_bounds
            && self.shell.capture_frame.pending
            && screenshot.scale_factor == 1.0
            && screenshot.size.width == self.shell.viewport.width_px.round() as u32
            && screenshot.size.height == self.shell.viewport.height_px.round() as u32;
        self.shell.capture_frame.pending = false;
        if !valid {
            self.shell.capture_frame.redraws = 0;
            self.shell.capture_frame.previous = None;
            self.shell.capture_frame.repeated = 0;
            return;
        }
        let Some(parent) = self
            .shell
            .capture_marker
            .as_ref()
            .and_then(|path| path.parent())
        else {
            return;
        };
        // The readback tells whether the surface still renders the frame the
        // previous readback showed. A surface that changed resets the window:
        // the capture keeps reading back instead of publishing a frame the
        // compositor has already left behind.
        let pixels = screenshot.rgba.as_ref();
        let gate = &mut self.shell.capture_frame;
        if gate.previous.as_deref() == Some(pixels) {
            gate.repeated = gate.repeated.saturating_add(1);
        } else {
            gate.previous = Some(pixels.to_vec());
            gate.repeated = 1;
        }
        // The published frame follows the surface: every frame that differs
        // from the last one is written, so the file always shows what the
        // compositor shows now.
        if gate.repeated == 1 {
            let Some(image) = RgbaImage::from_raw(
                screenshot.size.width,
                screenshot.size.height,
                screenshot.rgba.to_vec(),
            ) else {
                return;
            };
            if let Err(error) =
                image.save_with_format(parent.join("rendered-frame.png"), ImageFormat::Png)
            {
                eprintln!("native capture readback could not be saved: {error}");
                return;
            }
        }
        if gate.repeated < SETTLED_READBACKS {
            return;
        }
        if !self.capture_state_ready() {
            return;
        }
        self.shell.capture_frame.ready = true;
        #[cfg(any(feature = "capture-tools", test))]
        self.write_capture_marker();
    }
}
