//! test-intent: behavior
//! A view or stale readback cannot publish a frame receipt; a frame the
//! surface has not settled on cannot publish one either.
use super::*;
use iced::Size;
use image::open;
use std::env::temp_dir;
use std::fs::{create_dir_all, read_to_string, remove_dir_all};
use std::path::PathBuf;
use std::process::id;
use std::sync::atomic::AtomicU64;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct CaptureDirectory(PathBuf);
impl CaptureDirectory {
    fn new() -> Self {
        let path = temp_dir().join(format!(
            "musicfrog-render-frame-{}-{}",
            id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        create_dir_all(&path).unwrap();
        Self(path)
    }
    fn marker(&self) -> PathBuf {
        self.0.join("marker.log")
    }
    fn readback(&self) -> PathBuf {
        self.0.join("rendered-frame.png")
    }
}
impl Drop for CaptureDirectory {
    fn drop(&mut self) {
        let _ = remove_dir_all(&self.0);
    }
}

fn capture_state(directory: &CaptureDirectory) -> AppState {
    let (mut state, _) = AppState::new();
    state.shell.demo = true;
    state.shell.capture_marker = Some(directory.marker());
    state.shell.viewport.width_px = 720.0;
    state.shell.viewport.height_px = 480.0;
    state
}

/// Arm the native readback: the gate waits for the rebuilt view before it
/// asks the renderer for the frame.
fn arm(state: &mut AppState) {
    for _ in 0..4 {
        if state.capture_frame_task().units() > 0 {
            return;
        }
    }
    panic!("the capture gate never armed a native readback");
}

#[test]
fn redraw_gate_rejects_stale_or_malformed_readback_and_saves_the_current_native_bytes_once() {
    let directory = CaptureDirectory::new();
    let mut state = capture_state(&directory);
    let revision = state.surface.revision();
    let bytes = [31u8, 61, 91, 255].repeat(720 * 480);
    let screenshot = Screenshot::new(bytes.clone(), Size::new(720, 480), 1.0);
    let _ = state.view();
    assert!(!directory.marker().exists());
    assert_eq!(state.capture_frame_task().units(), 0);
    assert!(state.capture_frame_task().units() > 0);
    state.finish_capture_frame(revision + 1, None, screenshot.clone());
    assert!(!directory.marker().exists());
    assert!(!directory.readback().exists());
    arm(&mut state);
    state.finish_capture_frame(
        revision,
        None,
        Screenshot::new(vec![0], Size::new(720, 480), 1.0),
    );
    assert!(!directory.marker().exists());
    // The receipt is published only once the surface repeats the frame: a
    // readback that still changes means the compositor would show another
    // native frame than the one the capture publishes.
    for _ in 0..(SETTLED_READBACKS - 1) {
        arm(&mut state);
        state.finish_capture_frame(revision, None, screenshot.clone());
        assert!(!directory.marker().exists());
        assert!(!directory.readback().exists());
    }
    arm(&mut state);
    state.finish_capture_frame(revision, None, screenshot.clone());
    let line = read_to_string(directory.marker()).unwrap();
    assert_eq!(line, "CAPTURE_READY page=overview skin=dark\n");
    assert_eq!(
        open(directory.readback()).unwrap().to_rgba8().into_raw(),
        bytes
    );
    state.finish_capture_frame(revision, None, screenshot);
    assert_eq!(read_to_string(directory.marker()).unwrap(), line);
}

#[test]
fn a_frame_the_surface_still_changes_never_publishes_a_receipt() {
    let directory = CaptureDirectory::new();
    let mut state = capture_state(&directory);
    let revision = state.surface.revision();
    let bytes = [31u8, 61, 91, 255].repeat(720 * 480);
    let screenshot = Screenshot::new(bytes.clone(), Size::new(720, 480), 1.0);
    let moved = Screenshot::new(vec![7u8; 720 * 480 * 4], Size::new(720, 480), 1.0);
    let _ = state.view();
    // A late change (a pointer that lands on a control) restarts the settle
    // window: the capture keeps reading back instead of publishing the frame
    // the compositor has already left behind.
    for index in 0..(SETTLED_READBACKS * 2) {
        arm(&mut state);
        let frame = if index % 2 == 0 {
            moved.clone()
        } else {
            screenshot.clone()
        };
        state.finish_capture_frame(revision, None, frame);
        assert!(!directory.marker().exists());
        assert!(!directory.readback().exists());
    }
    // The surface settles: the capture publishes the frame it repeats.
    arm(&mut state);
    state.finish_capture_frame(revision, None, moved.clone());
    assert!(!directory.marker().exists());
    for _ in 0..(SETTLED_READBACKS - 1) {
        arm(&mut state);
        state.finish_capture_frame(revision, None, screenshot.clone());
        assert!(!directory.marker().exists());
    }
    arm(&mut state);
    state.finish_capture_frame(revision, None, screenshot.clone());
    assert_eq!(
        read_to_string(directory.marker()).unwrap(),
        "CAPTURE_READY page=overview skin=dark\n"
    );
}
