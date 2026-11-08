//! Deterministic CAPTURE_READY marker for a demo frame.
//!
//! The marker names the page and scenario a real compositor frame proves; it
//! lives outside `demo.rs` so the demo host keeps its line budget and the marker
//! owns its once-only latch.

use super::DemoSettings;
use crate::AppWindow;
use crate::capture_scenario::CaptureBounds;
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};

static CAPTURE_MARKER_WRITTEN: AtomicBool = AtomicBool::new(false);

pub fn write_capture_marker(
    settings: &DemoSettings,
    _window: &AppWindow,
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
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        if file.write_all(line.as_bytes()).is_ok() {
            CAPTURE_MARKER_WRITTEN.store(true, Ordering::SeqCst);
        }
    }
}
