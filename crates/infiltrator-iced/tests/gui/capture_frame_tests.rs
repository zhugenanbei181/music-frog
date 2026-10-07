//! test-intent: behavior
//! A view or stale readback cannot publish a frame receipt; current RGBA is saved intact.
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
}
impl Drop for CaptureDirectory {
    fn drop(&mut self) {
        let _ = remove_dir_all(&self.0);
    }
}

#[test]
fn redraw_gate_rejects_stale_or_malformed_readback_and_saves_the_current_native_bytes_once() {
    let directory = CaptureDirectory::new();
    let marker = directory.0.join("marker.log");
    let (mut state, _) = AppState::new();
    state.shell.demo = true;
    state.shell.capture_marker = Some(marker.clone());
    state.shell.viewport.width_px = 720.0;
    state.shell.viewport.height_px = 480.0;
    let revision = state.surface.revision();
    let bytes = [31u8, 61, 91, 255].repeat(720 * 480);
    let screenshot = Screenshot::new(bytes.clone(), Size::new(720, 480), 1.0);
    let _ = state.view();
    assert!(!marker.exists());
    assert_eq!(state.capture_frame_task().units(), 0);
    assert!(state.capture_frame_task().units() > 0);
    state.finish_capture_frame(revision + 1, None, screenshot.clone());
    assert!(!marker.exists());
    assert!(!directory.0.join("rendered-frame.png").exists());
    assert_eq!(state.capture_frame_task().units(), 0);
    assert!(state.capture_frame_task().units() > 0);
    state.finish_capture_frame(
        revision,
        None,
        Screenshot::new(vec![0], Size::new(720, 480), 1.0),
    );
    assert!(!marker.exists());
    assert!(state.capture_frame_task().units() > 0);
    state.finish_capture_frame(revision, None, screenshot.clone());
    let line = read_to_string(&marker).unwrap();
    assert_eq!(line, "CAPTURE_READY page=overview skin=dark\n");
    assert_eq!(
        open(directory.0.join("rendered-frame.png"))
            .unwrap()
            .to_rgba8()
            .into_raw(),
        bytes
    );
    state.finish_capture_frame(revision, None, screenshot);
    assert_eq!(read_to_string(&marker).unwrap(), line);
}
