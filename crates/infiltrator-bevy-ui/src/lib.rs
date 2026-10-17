#![allow(clippy::collapsible_match, clippy::large_enum_variant)]
//! MusicFrog Infiltrator — independent Bevy UI surface.
//!
//! This crate owns the ECS UI and accepts host-owned surface, command, and
//! system-capability adapters. Product composition and process entrypoints
//! live in platform host crates. `DefaultPlugins` stays out of
//! [`app::ShellPlugin`] so headless tests exercise the real shell without a
//! window.
//!
//! Slice layout: [`app`] is the window chrome (and the shell's theme +
//! routing-intent affordances), [`route`] the bounded subtree router over
//! the shell's `ContentSlot`, [`projection`] the pure data seam the
//! Overview page renders, [`history`] the rate-history ring behind that
//! page's trend chart, [`controller`] the live mihomo controller pump
//! behind that seam (BEVY-005), [`pages`] the page modules themselves,
//! [`command`] the UI command sink pipeline, and [`capture`] the headless
//! screenshot forensics seam (env-driven skin/window-size/marker, read only here).

use infiltrator_bevy_widgets::shader_fx::ModernSurfacePlugin;
use infiltrator_contract::theme::ThemePreference;
pub mod a11y;
pub mod app;
pub mod appearance;
pub mod cadence;
#[cfg(feature = "capture-tools")]
pub mod capture;
pub mod chrome;
pub mod command;
pub mod command_events;
pub mod command_execution;
pub mod command_palette;
pub mod command_palette_shell;
#[cfg(feature = "capture-tools")]
pub mod controller;
pub mod domain_state;
pub mod gesture;
pub mod history;
pub mod host_capabilities;
pub mod ime;
mod ime_native;
#[cfg(feature = "capture-tools")]
pub mod interaction_capture;
pub mod lifecycle;
pub mod localization;
pub mod localized_widgets;
pub mod mini_hud;
pub mod mini_hud_refresh;
pub mod mini_hud_shell;
pub mod pages;
pub mod pipeline;
pub mod projection;
pub mod route;
pub mod shell_rail;
pub mod shell_readout;
pub mod shell_routing_intent;
pub mod shell_routing_intent_issue;
pub mod shell_scene;
pub mod shell_waveform;
pub mod shortcuts;
pub mod surface;
pub mod toast;
pub mod tray_status;

use crate::command::UiCommandSink;
use crate::command_execution::UnavailableCommandSink;
use crate::pages::connections_clipboard::ClipboardHost;
use crate::surface::host_insets::WindowInsetsInputReceiver;
use bevy::DefaultPlugins;
use bevy::app::{App, PluginGroup};
use bevy::window::{ExitCondition, Window, WindowPlugin, WindowResolution};
use infiltrator_contract::error::Failure;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use std::sync::Arc;
use std::time::Duration;

/// Explicit isolated composition for screenshots and UI development.
#[cfg(feature = "capture-tools")]
pub fn run_demo() {
    let composition = capture_composition();
    run_with_command_sink_and_composition(
        Arc::new(command::DemoCommandSink::accepting()),
        composition,
    );
}

/// Render the actual host initialization failure on every page and reject commands.
pub fn run_unavailable(surface: SurfaceKind, host: HostKind, failure: Failure) {
    let source = surface::UnavailableSurfaceSource::new(surface, host, failure.clone());
    run_with_command_sink_and_composition(
        Arc::new(UnavailableCommandSink::new(failure)),
        SurfaceComposition::Source(Arc::new(source)),
    );
}

/// Commands without a page reader show explicit unavailable data. Fixture hosts use `run_demo`.
pub fn run_with_command_sink(sink: Arc<dyn UiCommandSink>) {
    run_with_command_sink_and_composition(
        sink,
        SurfaceComposition::Source(Arc::new(surface::UnavailableSurfaceSource::default())),
    );
}

/// Launch with host-owned surface and command adapters without constructing a
/// shared application inside the Bevy process. The source must return a cheap
/// cached snapshot; its owner performs transport reads on its own worker.
/// The host also supplies its live typed window-inset receiver; the shell does
/// not use a device-specific bottom offset or assume insets are zero.
pub fn run_with_host_surface_and_command_sink(
    source: Arc<dyn surface::SurfaceSource>,
    sink: Arc<dyn UiCommandSink>,
    insets: WindowInsetsInputReceiver,
    refresh_interval: Duration,
) {
    run_with_command_sink_and_composition(
        sink,
        SurfaceComposition::Host {
            source,
            insets,
            refresh_interval,
        },
    );
}

enum SurfaceComposition {
    Source(Arc<dyn surface::SurfaceSource>),
    Host {
        source: Arc<dyn surface::SurfaceSource>,
        insets: WindowInsetsInputReceiver,
        refresh_interval: Duration,
    },
    #[cfg(feature = "capture-tools")]
    Demo,
    #[cfg(feature = "capture-controller")]
    Controller(controller::ControllerConfig),
}

#[cfg(feature = "capture-tools")]
#[derive(Clone, Copy)]
enum CaptureReadiness {
    Immediate,
    FirstSnapshot,
}

#[cfg(feature = "capture-tools")]
#[derive(Clone, Copy)]
enum InteractionCaptureSource {
    DemoFixture,
    ExternalSurface,
}

#[cfg(feature = "capture-tools")]
impl SurfaceComposition {
    fn capture_readiness(&self) -> CaptureReadiness {
        match self {
            Self::Demo => CaptureReadiness::Immediate,
            Self::Source(_) | Self::Host { .. } => CaptureReadiness::FirstSnapshot,
            #[cfg(feature = "capture-controller")]
            Self::Controller(_) => CaptureReadiness::FirstSnapshot,
        }
    }

    fn interaction_capture_source(&self) -> InteractionCaptureSource {
        match self {
            Self::Demo => InteractionCaptureSource::DemoFixture,
            Self::Source(_) | Self::Host { .. } => InteractionCaptureSource::ExternalSurface,
            #[cfg(feature = "capture-controller")]
            Self::Controller(_) => InteractionCaptureSource::ExternalSurface,
        }
    }
}

#[cfg(feature = "capture-tools")]
fn capture_composition() -> SurfaceComposition {
    #[cfg(feature = "capture-controller")]
    {
        return match controller::controller_config_from_env() {
            Some(config) => SurfaceComposition::Controller(config),
            None => SurfaceComposition::Demo,
        };
    }
    #[cfg(not(feature = "capture-controller"))]
    SurfaceComposition::Demo
}

fn run_with_command_sink_and_composition(
    sink: Arc<dyn UiCommandSink>,
    composition: SurfaceComposition,
) {
    #[cfg(feature = "capture-tools")]
    let preference = capture::skin_from_env()
        .map(appearance::contract_skin_from_widget)
        .map(ThemePreference::Fixed)
        .unwrap_or_default();
    #[cfg(not(feature = "capture-tools"))]
    let preference = ThemePreference::default();
    #[cfg(feature = "capture-tools")]
    let (width, height) = capture::window_size_from_env().unwrap_or((1180, 760));
    #[cfg(not(feature = "capture-tools"))]
    let (width, height) = (1180, 760);
    #[cfg(feature = "capture-tools")]
    let marker = capture::marker_path_from_env();
    #[cfg(feature = "capture-tools")]
    let capture_readiness = composition.capture_readiness();
    #[cfg(feature = "capture-tools")]
    let interaction_capture_source = composition.interaction_capture_source();

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "MusicFrog Infiltrator — Bevy".into(),
            resolution: WindowResolution::new(width, height),
            // DUAL-15-13: the shell runs the shared frameless chrome shape;
            // the mounted chrome bar owns dragging and the window controls.
            decorations: chrome::chrome_shape().os_decorations(),
            transparent: true,
            ..Window::default()
        }),
        exit_condition: ExitCondition::OnPrimaryClosed,
        ..WindowPlugin::default()
    }));
    app.insert_resource(ClipboardHost);
    app.add_plugins(app::ShellPlugin::new_with_width(preference, width as f32));
    app.add_plugins(ModernSurfacePlugin);
    // DUAL-15-08: the real Bevy power knob follows the shared render cadence.
    app.add_plugins(cadence::CadencePlugin);
    // DUAL-15-13 / DUAL-15-02: the frameless chrome path and the honest tray
    // capability report are installed by `ShellPlugin` (so headless
    // compositions get the same facts as the windowed launcher). Adding them
    // here as well duplicated the plugin and panicked the launcher.
    app.add_plugins(command::CommandPumpPlugin::new(sink));
    // Every product, host, demo, and capture composition has its own explicit
    // variant. A missing source is never inferred from a pair of Options or a
    // live-controller boolean.
    match composition {
        SurfaceComposition::Source(source) => {
            app.add_plugins(route::PagesPlugin::new_surface_arc(source));
        }
        SurfaceComposition::Host {
            source,
            insets,
            refresh_interval,
        } => {
            app.insert_resource(insets);
            app.add_plugins(route::PagesPlugin::new_surface_arc(source));
            app.add_plugins(surface::SurfaceSourceRefreshPlugin::new(refresh_interval));
        }
        #[cfg(feature = "capture-tools")]
        SurfaceComposition::Demo => {
            app.add_plugins(route::PagesPlugin::demo());
        }
        #[cfg(feature = "capture-controller")]
        SurfaceComposition::Controller(config) => {
            let source = controller::MihomoOverviewSource::spawn(config);
            app.add_plugins(controller::PumpDrainPlugin::new(&source));
            app.add_plugins(route::PagesPlugin::new(source));
        }
    }
    #[cfg(feature = "capture-tools")]
    if let Some(path) = marker {
        // Live runs gate the readiness marker on the pump's first delivered
        // snapshot, so the capture never shoots the pre-sample placeholder.
        let plugin = capture::CapturePlugin::new(path);
        let plugin = match capture_readiness {
            CaptureReadiness::Immediate => plugin,
            CaptureReadiness::FirstSnapshot => plugin.waiting_for_first_snapshot(),
        };
        app.add_plugins(plugin);
    }
    #[cfg(feature = "capture-tools")]
    if let Some(feature) = interaction_capture::feature_from_environment() {
        match interaction_capture_source {
            InteractionCaptureSource::DemoFixture => {
                interaction_capture::InteractionCapture::install(&mut app, feature);
            }
            InteractionCaptureSource::ExternalSurface => {
                panic!("interaction capture requires the isolated demo composition");
            }
        }
    }
    app.run();
}
