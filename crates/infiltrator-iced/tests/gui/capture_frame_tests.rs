//! test-intent: behavior
//! A view or stale readback cannot publish a frame receipt; a frame the
//! surface has not settled on cannot publish one either.
use super::*;
use crate::sidebar_chrome::{SidebarChromeReceipt, shell_chrome_facts};
use crate::view::sidebar::{selected_navigation_route, sidebar_presentation};
use iced::Rectangle;
use iced::Size;
use image::open;
use infiltrator_contract::responsive_viewport::ResponsiveViewportSnapshot;
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
    // The viewport is resolved the way a real resize resolves it, so the tier
    // the receipt publishes is the tier of the frame's own width.
    state.shell.viewport = ResponsiveViewportSnapshot::from_dimensions(720.0, 480.0);
    // The sidebar's boxes are measured from the live layout by the capture
    // geometry probe; a unit test has no widget tree to walk, so it stands in
    // for the probe with the boxes the 720x480 icon rail paints.
    state.shell.sidebar_chrome = SidebarChromeReceipt {
        stack_end: Some(Rectangle {
            x: 0.0,
            y: 54.0,
            width: 64.0,
            height: 344.0,
        }),
        ..Default::default()
    };
    state
}

/// The receipt line the frame above describes itself with, built from the same
/// contract helper the host uses so the assertion cannot drift from the format.
fn expected_chrome_line(state: &AppState) -> String {
    let presentation = sidebar_presentation(state);
    shell_chrome_facts(presentation, state.shell.sidebar_chrome)
        .expect("the measured rail describes its own chrome")
        .to_marker_line()
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
    }
    arm(&mut state);
    state.finish_capture_frame(revision, None, screenshot.clone());
    let line = read_to_string(directory.marker()).unwrap();
    assert_eq!(
        line,
        format!(
            "CAPTURE_READY page=overview skin=dark\n{}",
            expected_chrome_line(&state)
        )
    );
    assert!(line.contains("CAPTURE_CHROME nav_form=rail nav_column_px=64"));
    assert_eq!(
        open(directory.readback()).unwrap().to_rgba8().into_raw(),
        bytes
    );
    state.finish_capture_frame(revision, None, screenshot);
    assert_eq!(read_to_string(directory.marker()).unwrap(), line);
}

#[test]
fn observation_routes_publish_only_after_eight_stable_single_selection_readbacks() {
    use crate::types::app::Route;

    for (route, expected_page) in [
        (Route::Runtime, "runtime"),
        (Route::Traffic, "traffic"),
        (Route::Connections, "connections"),
        (Route::Logs, "logs"),
    ] {
        let directory = CaptureDirectory::new();
        let mut state = capture_state(&directory);
        state.shell.current_route = route;
        assert_eq!(selected_navigation_destination_count(route), 1);
        assert_eq!(
            selected_navigation_route(route),
            match route {
                Route::Runtime => Route::Settings,
                _ => route,
            },
            "capture uses the same contract-owned selected row as rendering"
        );
        let revision = state.surface.revision();
        let bytes = [31u8, 61, 91, 255].repeat(720 * 480);
        let screenshot = Screenshot::new(bytes, Size::new(720, 480), 1.0);

        for readback in 0..SETTLED_READBACKS {
            arm(&mut state);
            state.finish_capture_frame(revision, None, screenshot.clone());
            if readback + 1 < SETTLED_READBACKS {
                assert!(
                    !directory.marker().exists(),
                    "the {route:?} route published before the settled readback window"
                );
            }
        }

        let line = read_to_string(directory.marker()).unwrap();
        assert!(
            line.starts_with(&format!("CAPTURE_READY page={expected_page} ")),
            "the {route:?} route receipt must describe its own page"
        );
    }
}

/// A frame that cannot state its sidebar's own geometry publishes nothing.
///
/// The chrome line is the frame's description of the pane behind its pixels,
/// and a partial description is a refusal rather than a default: a marker
/// written before the geometry probe measured the stack end would put a
/// remembered number into the space-utilization judge's measurement box.
#[test]
fn a_frame_whose_sidebar_boxes_were_never_measured_publishes_nothing() {
    let directory = CaptureDirectory::new();
    let mut state = capture_state(&directory);
    state.shell.sidebar_chrome = SidebarChromeReceipt::default();
    let revision = state.surface.revision();
    let bytes = [31u8, 61, 91, 255].repeat(720 * 480);
    let screenshot = Screenshot::new(bytes, Size::new(720, 480), 1.0);
    let _ = state.view();
    for _ in 0..=(SETTLED_READBACKS + 1) {
        arm(&mut state);
        state.finish_capture_frame(revision, None, screenshot.clone());
    }
    assert!(
        !directory.marker().exists(),
        "an unmeasured sidebar must not be described by a frame"
    );
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
    }
    // The published frame follows the surface: the file always shows the
    // bytes the compositor shows now, even while the receipt waits for the
    // surface to settle.
    assert_eq!(
        open(directory.readback()).unwrap().to_rgba8().into_raw(),
        bytes
    );
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
        format!(
            "CAPTURE_READY page=overview skin=dark\n{}",
            expected_chrome_line(&state)
        )
    );
}
