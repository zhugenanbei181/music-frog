//! Capture readiness follows native redraws and GPU readback, never a view construction.
use super::interaction;
use crate::state::AppState;
use crate::types::message::Message;
use iced::window::Screenshot;
use iced::{Rectangle, Task, window};
use image::{ImageFormat, RgbaImage};
use std::sync::atomic::Ordering;

#[derive(Default)]
pub struct CaptureFrameGate {
    revision: u64,
    bounds: Option<Rectangle>,
    redraws: u8,
    pending: bool,
    ready: bool,
}

impl CaptureFrameGate {
    pub(crate) fn is_ready(&self) -> bool {
        self.ready
    }
}

#[cfg(test)]
#[path = "../../tests/gui/capture_frame_tests.rs"]
mod tests;

impl AppState {
    fn capture_state_ready(&self) -> bool {
        self.shell.demo
            && self.shell.capture_marker.is_some()
            && !self.shell.capture_marker_written.load(Ordering::SeqCst)
            && self
                .shell
                .capture_scenario
                .is_none_or(|feature| interaction::ready(self, feature))
    }

    pub(crate) fn capture_frame_task(&mut self) -> Task<Message> {
        if !self.capture_state_ready() {
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
        if gate.pending || gate.ready {
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
        let valid = self.capture_state_ready()
            && revision == self.surface.revision()
            && bounds == self.shell.capture_region_bounds
            && self.shell.capture_frame.pending
            && screenshot.scale_factor == 1.0
            && screenshot.size.width == self.shell.viewport.width_px.round() as u32
            && screenshot.size.height == self.shell.viewport.height_px.round() as u32;
        self.shell.capture_frame.pending = false;
        if !valid {
            self.shell.capture_frame.redraws = 0;
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
        self.shell.capture_frame.ready = true;
        self.write_capture_marker();
    }
}
