//! Deterministic CAPTURE_READY marker for a demo frame.
//!
//! The marker names the page and scenario a real compositor frame proves; it
//! lives outside `demo.rs` so the demo host keeps its line budget and the marker
//! owns its once-only latch.

use super::DemoSettings;
use crate::AppWindow;
use crate::capture_scenario::CaptureBounds;
use crate::shell_receipt_host::shell_chrome_facts;
use slint::Model;
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};

static CAPTURE_MARKER_WRITTEN: AtomicBool = AtomicBool::new(false);

pub fn write_capture_marker(
    settings: &DemoSettings,
    window: &AppWindow,
    activated: bool,
    bounds: Option<CaptureBounds>,
) {
    if !settings.enabled || CAPTURE_MARKER_WRITTEN.load(Ordering::SeqCst) {
        return;
    }
    let Some(path) = settings.marker.as_ref() else {
        return;
    };

    let marker_page = match settings.page.as_str() {
        "Home" => "overview",
        "Proxy" => "proxies",
        "Config" => "profiles",
        "Network" => "runtime",
        "Rules" => "rules",
        "Connections" => "connections",
        "Logs" => "logs",
        "Settings" => "settings",
        "AppSettings" => "app_settings",
        "Traffic" => "traffic",
        "About" => "about",
        // The plain list pages the demo snapshot can fill without a gesture.
        // They used to fall through to `overview`, so a frame of one of them was
        // labelled as a different page; now they name themselves.
        "Meta" => "meta",
        "Overrides" => "overrides",
        "Providers" => "providers",
        "Licenses" => "licenses",
        // The workbench pages carry the parity businesses of the DNS, editor,
        // filter, doctor, sync and routing panels.
        "Dns" => "dns",
        "Editor" => "editor",
        "Filters" => "filters",
        "Doctor" => "doctor",
        "Sync" => "sync",
        "AppRouting" => "app_routing",
        "ConnectionsWorkbench" => "connections_workbench",
        "LogsWorkbench" => "logs_workbench",
        "RuntimeWorkbench" => "runtime_workbench",
        "ProxiesWorkbench" => "proxies_workbench",
        "RulesWorkbench" => "rules_workbench",
        "SettingsWorkbench" => "settings_workbench",
        "ShellWorkbench" => "shell_workbench",
        "OverviewWorkbench" => "overview_workbench",
        _ => "overview",
    };

    let mut line = format!(
        "CAPTURE_READY page={} skin={}\n",
        marker_page, settings.skin
    );

    if let Some(scenario) = &settings.scenario {
        // A scenario is only published as activated when the surface really
        // opened it and its real geometry is known.
        match (activated, bounds) {
            (true, Some(bounds)) => {
                line = format!(
                    "{} scenario={} activated=true bounds={},{},{},{}\n",
                    line.trim_end(),
                    scenario,
                    bounds.x,
                    bounds.y,
                    bounds.width,
                    bounds.height
                );
            }
            _ => {
                line = format!(
                    "{} scenario={} activated=false\n",
                    line.trim_end(),
                    scenario
                );
            }
        }
    }

    if let Some(parent) = path.parent() {
        let _ = create_dir_all(parent);
    }
    line.push_str(&chrome_line(window));
    line.push_str(&navigation_line(window));
    line.push_str(&localized_copy_line(window));
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        if file.write_all(line.as_bytes()).is_ok() {
            CAPTURE_MARKER_WRITTEN.store(true, Ordering::SeqCst);
        }
    }
}

/// The chrome line for this frame: what the live shell is painting right now.
///
/// The values are read out of the same bindings that placed the pixels - the
/// reserved column, the page margin, the shell air above and below, the dock
/// band, and the navigation stack's own row geometry - then divided by the ui
/// unit those bindings used, so the receipt states logical pixels and the
/// space-utilization judge needs no remembered constant. If the shell cannot
/// say what it drew, the frame is reported rather than measured against a
/// number the judge kept for itself.
/// The chrome line for this frame: what the live shell is painting.
fn chrome_line(window: &AppWindow) -> String {
    shell_chrome_facts(window).to_marker_line()
}

/// The navigation words the live shell published for this frame.
///
/// The capture's locale receipt reads this line back instead of trusting the
/// environment that launched the process: the pixels can only prove a language
/// if the frame itself states which copy table fed its labels.
fn navigation_line(window: &AppWindow) -> String {
    let items = window.get_navigation_items();
    let labels = (0..items.row_count())
        .filter_map(|row| items.row_data(row))
        .map(|item| {
            let encoded = item
                .label
                .as_str()
                .bytes()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            format!("{}={};", item.id, encoded)
        })
        .collect::<String>();
    format!("CAPTURE_NAV labels={labels}\n")
}

/// The localized non-navigation words the live window published.
///
/// `CAPTURE_NAV` proves the navigation model. This line reads three more
/// compiler-driven strings from the same window: the desktop/app-bar page
/// title owner, the mobile vocabulary and the typed takeover-mode label.
/// All are bound directly to `PageTitles`, so the receipt follows the shared
/// PO catalog instead of inventing a second vocabulary in the capture host.
fn localized_copy_line(window: &AppWindow) -> String {
    let copy = [
        ("desktop-title", window.get_capture_page_title()),
        ("mobile-title", window.get_capture_phone_title()),
        ("network-mode", window.get_network_mode_label()),
    ];
    let encoded = copy
        .iter()
        .map(|(key, value)| {
            let bytes = value
                .as_str()
                .bytes()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            format!("{key}={bytes};")
        })
        .collect::<String>();
    format!("CAPTURE_LOCALE copy={encoded}\n")
}
