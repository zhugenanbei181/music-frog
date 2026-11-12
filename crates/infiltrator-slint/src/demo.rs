//! Slint demo and capture fixture host.
//!
//! Activated via `--demo` or `INFILTRATOR_DEMO=1`.
//! Provides realistic snapshots without connecting to real cores or external network,
//! and writes the `CAPTURE_READY` marker for background virtual compositor screenshots.

mod baseline;
mod capture_marker;
mod fixtures;
mod outbound;
mod owner;
mod pages;

use super::*;
use crate::capture_scenario::{self, CaptureBounds};
use crate::demo::capture_marker::write_capture_marker;
use crate::mascot_driver::publish_receipt_frame;
use infiltrator_contract::language::LanguagePreference;
use infiltrator_contract::shell_sidebar_preference::SidebarExpansionPreference;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use lurker_core_contract::error::Failure;
use lurker_core_contract::page::PageData;
use std::cell::Cell;
use std::env;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct DemoSettings {
    pub enabled: bool,
    pub page: String,
    pub skin: String,
    pub window_size: (f32, f32),
    /// The user zoom in percent, or `None` to keep the surface's own default.
    /// The page scale is a real user setting (85%…115% in 5% steps) that until
    /// now could not be reached from a capture run at all, so every pixel
    /// judgement about typography was made at 100% only.
    pub page_scale: Option<i32>,
    pub marker: Option<PathBuf>,
    pub scenario: Option<String>,
    /// Scroll the page to its end before the frame is taken. Every portrait
    /// receipt so far stops at the top of the scroll, which left the tail
    /// padding under the floating dock - the one place the dock-to-last-card
    /// gap is visible - unproven in pixels.
    pub scroll_to_end: bool,
    /// A deterministic mascot motion frame for a capture receipt: `starting`,
    /// `running`, `stopping`. Empty means the normal live beat.
    pub mascot_frame: Option<String>,
    pub mascot_elapsed_millis: u32,
    /// The owner fixture's sidebar preference. A capture row can therefore
    /// publish the collapsed rail without a UI-private shortcut or a second
    /// source of the persisted setting.
    pub sidebar_expansion: SidebarExpansionPreference,
    /// The locale the capture's owner snapshot publishes. A capture row states
    /// it explicitly so the pixel corpus can prove both languages rather than
    /// silently inheriting the host's system locale.
    pub language: LanguagePreference,
}

impl DemoSettings {
    pub fn from_env() -> Self {
        let enabled = env::args().any(|arg| arg == "--demo")
            || env::var("INFILTRATOR_DEMO").is_ok_and(|v| v.trim() == "1");

        let raw_page = env::var("INFILTRATOR_PAGE").unwrap_or_else(|_| "overview".into());
        let page = match raw_page.trim().to_ascii_lowercase().as_str() {
            "overview" | "home" => "Home",
            "proxies" | "proxy" => "Proxy",
            "profiles" | "config" => "Config",
            "settings" => "Settings",
            "runtime" | "network" => "Network",
            "connections" => "Connections",
            "logs" => "Logs",
            "rules" | "rules-providers" => "Rules",
            "traffic" => "Traffic",
            "app_settings" | "app-settings" => "AppSettings",
            "about" => "About",
            "dns" => "Dns",
            "editor" => "Editor",
            "filters" | "filter" => "Filters",
            "doctor" => "Doctor",
            "sync" => "Sync",
            "app_routing" | "app-routing" => "AppRouting",
            "connections_workbench" | "connections-workbench" => "ConnectionsWorkbench",
            "logs_workbench" | "logs-workbench" => "LogsWorkbench",
            "runtime_workbench" | "runtime-workbench" => "RuntimeWorkbench",
            "proxies_workbench" | "proxies-workbench" => "ProxiesWorkbench",
            "rules_workbench" | "rules-workbench" => "RulesWorkbench",
            "settings_workbench" | "settings-workbench" => "SettingsWorkbench",
            "shell_workbench" | "shell-workbench" => "ShellWorkbench",
            "overview_workbench" | "overview-workbench" => "OverviewWorkbench",
            // The list pages a capture can reach straight from the demo snapshot.
            // Without these the surface routed them but no receipt could ever be
            // taken of them, so a change there was unprovable by construction.
            "meta" => "Meta",
            "overrides" => "Overrides",
            "providers" => "Providers",
            "licenses" => "Licenses",
            // Fail closed: an unaddressable or misspelled page used to fall back
            // to Home, which let a receipt be filed under a scenario whose pixels
            // were a different page altogether.
            other => panic!(
                "INFILTRATOR_PAGE={other} names no page the Slint surface can open from a snapshot; \
                 pages that need a selected id (ConnectionDetail, LicenseDetail, ProfileSettings) \
                 are reached by a business gesture, not by this map"
            ),
        }
        .to_string();

        let raw_skin = env::var("INFILTRATOR_SKIN").unwrap_or_else(|_| "dark".into());
        let skin = match raw_skin.trim().to_ascii_lowercase().as_str() {
            "light" => "light",
            "forest" => "forest",
            "amoled" => "amoled",
            _ => "dark",
        }
        .to_string();

        let window_size = env::var("INFILTRATOR_WINDOW_SIZE")
            .ok()
            .and_then(|raw| {
                let parts: Vec<&str> = raw.split('x').collect();
                if parts.len() == 2 {
                    if let (Ok(w), Ok(h)) = (parts[0].parse::<f32>(), parts[1].parse::<f32>()) {
                        return Some((w, h));
                    }
                }
                None
            })
            .unwrap_or((1180.0, 780.0));

        let marker = env::var("INFILTRATOR_CAPTURE_MARKER")
            .ok()
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty());

        let scenario = env::var("INFILTRATOR_SCENARIO")
            .ok()
            .filter(|v| !v.trim().is_empty());

        let scroll_to_end = env::var("INFILTRATOR_SCROLL").is_ok_and(|v| v.trim() == "end");
        let mascot_frame = env::var("INFILTRATOR_MASCOT_FRAME")
            .ok()
            .filter(|v| matches!(v.trim(), "starting" | "running" | "stopping"))
            .map(|v| v.trim().to_owned());
        let mascot_elapsed_millis = env::var("INFILTRATOR_MASCOT_MILLIS")
            .ok()
            .and_then(|raw| raw.trim().parse::<u32>().ok())
            .unwrap_or(225);

        let page_scale = env::var("INFILTRATOR_PAGE_SCALE")
            .ok()
            .and_then(|raw| raw.trim().parse::<i32>().ok())
            .filter(|percent| (85..=115).contains(percent));

        let sidebar_expansion = match env::var("INFILTRATOR_SIDEBAR_EXPANSION")
            .ok()
            .as_deref()
            .map(str::trim)
        {
            Some("collapsed") => SidebarExpansionPreference::Collapsed,
            Some("expanded") | None => SidebarExpansionPreference::Expanded,
            Some(other) => {
                panic!("INFILTRATOR_SIDEBAR_EXPANSION={other} must be expanded or collapsed")
            }
        };

        let language = match env::var("INFILTRATOR_CAPTURE_LANG")
            .ok()
            .as_deref()
            .map(str::trim)
        {
            Some("en-US") => LanguagePreference::English,
            Some("zh-CN") | None => LanguagePreference::SimplifiedChinese,
            Some(other) => {
                panic!("INFILTRATOR_CAPTURE_LANG={other} must be zh-CN or en-US")
            }
        };

        Self {
            enabled,
            page,
            skin,
            window_size,
            page_scale,
            marker,
            scenario,
            scroll_to_end,
            mascot_frame,
            mascot_elapsed_millis,
            sidebar_expansion,
            language,
        }
    }
}

pub struct DemoProductHost {
    snapshot: Mutex<SurfaceSnapshot>,
}

impl DemoProductHost {
    pub fn new(settings: &DemoSettings) -> Self {
        let snapshot = build_demo_snapshot(settings);
        Self {
            snapshot: Mutex::new(snapshot),
        }
    }
}

impl SlintProductHost for DemoProductHost {
    fn read_snapshot(&self) -> Result<SurfaceSnapshot, Failure> {
        Ok(self
            .snapshot
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| {
                SurfaceSnapshot::unavailable(
                    SurfaceKind::SlintDesktop,
                    HostKind::Desktop,
                    Failure::unsupported("the demo snapshot is poisoned"),
                )
            }))
    }

    fn execute(
        &self,
        _snapshot: Option<&SurfaceSnapshot>,
        action: SlintAction,
    ) -> Result<(), Failure> {
        self.execute_typed(_snapshot, action).map(|_| ())
    }

    fn execute_typed(
        &self,
        _snapshot: Option<&SurfaceSnapshot>,
        action: SlintAction,
    ) -> Result<CommandOutput, Failure> {
        let mut guard = self
            .snapshot
            .lock()
            .map_err(|_| Failure::unsupported("the demo snapshot is poisoned"))?;
        owner::respond(&mut guard, action)
    }

    fn autostart_enabled(&self) -> Option<bool> {
        Some(true)
    }
}

fn build_demo_snapshot(settings: &DemoSettings) -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::unsupported("demo snapshot provides explicit fixtures"),
    );
    baseline::apply_runtime_baseline(&mut snapshot);

    let theme_str = baseline::demo_theme_str(&settings.skin);
    baseline::apply_theme_readback(&mut snapshot, theme_str);

    snapshot.pages.settings = PageData::ready(pages::settings_page(
        theme_str,
        settings.sidebar_expansion,
        settings.language,
    ));
    snapshot.language_settings.preference = Some(settings.language);
    snapshot.pages.profiles = PageData::ready(pages::profiles_page());
    baseline::apply_workbench_read_models(&mut snapshot);
    snapshot.pages.connections = PageData::ready(pages::connections_page());
    snapshot.pages.rules = PageData::ready(pages::rules_page());
    snapshot.pages.logs = PageData::ready(pages::logs_page());
    snapshot.pages.overview = PageData::ready(pages::overview_page());

    snapshot
}

/// Runs the demo fixture host with the real product wiring.
///
/// The demo registers the product callbacks and answers the commands the
/// capture's gestures send, so the capture drives the same code the running
/// product drives.
pub fn run_demo(settings: DemoSettings) -> Result<(), slint::PlatformError> {
    let host = Arc::new(DemoProductHost::new(&settings));
    let snapshot = host
        .read_snapshot()
        .map_err(|failure| slint::PlatformError::OtherError(Box::new(failure)))?;
    crate::run_with_host_prepared(host, move |window, workbenches| {
        prepare_demo_window(window, workbenches, &settings, &snapshot);
    })
}

/// Prepares the demo window before the owner worker starts.
fn prepare_demo_window(
    window: &AppWindow,
    workbenches: &Arc<Workbenches>,
    settings: &DemoSettings,
    snapshot: &SurfaceSnapshot,
) {
    // A capture run has no pointing device: the nested compositor parks its
    // pointer at the output centre, which would otherwise publish a hovered
    // control that no user ever hovered.
    window.set_capture_pointer_absent(settings.marker.is_some());
    window.window().set_size(slint::LogicalSize::new(
        settings.window_size.0,
        settings.window_size.1,
    ));
    lumen_projection::project_snapshot(window, snapshot, Some(true));
    lumen_projection::project_workbench(window, snapshot, workbenches);

    // Explicitly enforce the page after projection; the theme rides the
    // snapshot's published readback, so there is no second theme writer.
    set_page(window, &settings.page);
    lumen_projection::project_workbench(window, snapshot, workbenches);
    open_proxies_group(window, snapshot);
    window.set_notice("".into());
    // The zoom is applied through the same setter the sheet's "应用" button
    // uses, so a receipt at 105% rides the real `page-scale-percent` →
    // `LumenUiScale.unit` path instead of a second, harness-only scaling route.
    if let Some(percent) = settings.page_scale {
        window.set_page_scale_percent(percent);
    }

    // A receipt frame is published after the page project so the driver's
    // normal timer cannot overwrite it with the live beat.
    if let Some(phase) = &settings.mascot_frame {
        publish_receipt_frame(window, phase, settings.mascot_elapsed_millis);
    }

    install_capture_timer(window, Arc::clone(workbenches), settings, snapshot.clone());
}

/// Opens the proxies page on the group the owner marked as active.
///
/// The proxies page reaches its node list through a group, and the outbound
/// search is only reachable on that list. The demo therefore opens the page
/// the way a user reaches the search, so the route-only render and the
/// activated render differ by the search and by nothing else.
fn open_proxies_group(window: &AppWindow, snapshot: &SurfaceSnapshot) {
    if window.get_page().as_str() != "Proxy" {
        return;
    }
    let Some(group) = snapshot.active_exit.group.as_deref() else {
        return;
    };
    window.set_selected_group(group.into());
}

/// Publishes the capture marker once the business really shows its outcome.
///
/// The marker names the business the capture activated, so a scenario whose
/// business never opened is published as not activated.
fn install_capture_timer(
    window: &AppWindow,
    workbenches: Arc<Workbenches>,
    settings: &DemoSettings,
    snapshot: SurfaceSnapshot,
) {
    let marker_settings = settings.clone();
    let scenario = settings.scenario.clone();
    let weak_window = window.as_weak();
    let activated = Cell::new(false);
    let activation = Rc::new(capture_scenario::Activation::default());
    let previous_bounds = Cell::new(None::<CaptureBounds>);
    let ticks = Cell::new(0u32);
    let scroll_pending = Cell::new(settings.scroll_to_end);
    let compact = settings.window_size.0 < 900.0;
    let timer = Rc::new(slint::Timer::default());
    let timer_for_callback = Rc::clone(&timer);
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            let Some(window) = weak_window.upgrade() else {
                timer_for_callback.stop();
                return;
            };
            ticks.set(ticks.get() + 1);
            if let Some(scenario) = &scenario
                && !activated.get()
            {
                // The activation drives the business's own gesture and waits
                // until the business shows its own outcome.
                activated.set(capture_scenario::activate(
                    &window,
                    scenario,
                    &snapshot,
                    &workbenches,
                    &activation,
                    compact,
                ));
            }
            let bounds = capture_scenario::bounds(&window);
            let stable = bounds.is_some() && previous_bounds.get() == bounds;
            previous_bounds.set(bounds);
            // The window needs a few frames before its first paint, and a
            // scenario needs a settled sheet before its bounds are real.
            if ticks.get() < 5 || (scenario.is_some() && (!stable || !activated.get())) {
                return;
            }
            // The scroll request is served here rather than at prepare time:
            // `scroll-page-to` clamps against the scroll view's *content*
            // height, which is still zero before the first layout passes, so a
            // pre-layout request resolved to "stay at the top" and the frame
            // quietly proved nothing. Returning one tick late lets the scrolled
            // frame paint before the marker releases the screenshot.
            if scroll_pending.get() {
                scroll_pending.set(false);
                window.invoke_scroll_page_to(1_000_000.0);
                return;
            }
            // The frame is republished once the page has settled, so the
            // screenshot shows the requested phase rather than a timer tick.
            if let Some(phase) = &marker_settings.mascot_frame {
                publish_receipt_frame(&window, phase, marker_settings.mascot_elapsed_millis);
            }
            write_capture_marker(&marker_settings, &window, activated.get(), bounds);
            timer_for_callback.stop();
        },
    );
}
