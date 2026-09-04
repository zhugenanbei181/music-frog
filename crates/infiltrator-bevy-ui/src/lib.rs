//! MusicFrog Infiltrator — Bevy UI frontend shell (M1 shell + the M2
//! Overview page slice).
//!
//! The strategic unified desktop+mobile surface (charter:
//! docs/BEVY_UI_FRONTEND.md). This module hosts the windowed launcher
//! composition only: `DefaultPlugins` is singleton infrastructure and stays
//! out of [`app::ShellPlugin`] so headless tests exercise the real shell
//! without a window.
//!
//! Slice layout: [`app`] is the window chrome (and the shell's theme +
//! mode-segment affordances), [`route`] the bounded subtree router over
//! the shell's `ContentSlot`, [`projection`] the pure data seam the
//! Overview page renders, [`history`] the rate-history ring behind that
//! page's trend chart, [`controller`] the live mihomo controller pump
//! behind that seam (BEVY-005), [`pages`] the page modules themselves,
//! [`command`] the UI command sink pipeline, and [`capture`] the headless
//! screenshot forensics seam (env-driven skin/window-size/marker, read only here).

pub mod app;
pub mod capture;
pub mod command;
pub mod command_palette;
pub mod controller;
pub mod domain_state;
pub mod history;
pub mod lifecycle;
pub mod mini_hud;
pub mod pages;
pub mod pipeline;
pub mod projection;
pub mod route;
pub mod shell_scene;
pub mod shortcuts;

use bevy::DefaultPlugins;
use bevy::app::{App, PluginGroup};
use bevy::window::{ExitCondition, Window, WindowPlugin, WindowResolution};
use infiltrator_bevy_widgets::theme::LightDark;

/// Launch the windowed Bevy shell. Desktop: one primary window titled
/// "MusicFrog Infiltrator — Bevy", the sidebar/content shell with a live
/// light/dark affordance, embedded widget typography, AccessKit semantic
/// seeds (published by the windowed composition's winit bridge).
///
/// Capture knobs (see [`capture`]): `INFILTRATOR_BEVY_SKIN` seeds the
/// cold-start theme, `INFILTRATOR_BEVY_WINDOW_SIZE` the window
/// resolution, and `INFILTRATOR_CAPTURE_MARKER` installs the
/// frame-counted readiness writer. All three default to the plain demo
/// launch (dark, 1180x760, no marker).
///
/// Data-source knob (see [`controller`]): `INFILTRATOR_BEVY_CONTROLLER`
/// (plus optional `INFILTRATOR_BEVY_SECRET`) switches the Overview page
/// from the demo fixture to the live mihomo controller pump; unset env
/// keeps the demo frontend, a configured-but-unreachable controller
/// projects the typed unavailable state.
pub fn run() {
    let skin = capture::skin_from_env().unwrap_or(LightDark::Dark);
    let (width, height) = capture::window_size_from_env().unwrap_or((1180, 760));
    let marker = capture::marker_path_from_env();
    let controller = controller::controller_config_from_env();

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "MusicFrog Infiltrator — Bevy".into(),
            resolution: WindowResolution::new(width, height),
            ..Window::default()
        }),
        exit_condition: ExitCondition::OnPrimaryClosed,
        ..WindowPlugin::default()
    }));
    app.add_plugins(app::ShellPlugin::new_with_width(skin, width as f32));
    // The route + page bootstrap: without it the content slot stays empty in
    // the windowed run (headless tests add PagesPlugin explicitly, which is
    // why this regression only shows up on screen). The configured
    // controller swaps the demo fixture for the live pump; unset env keeps
    // the demo default.
    let live_controller = controller.is_some();
    match controller {
        Some(config) => {
            let source = controller::MihomoOverviewSource::spawn(config);
            app.add_plugins(controller::PumpDrainPlugin::new(&source));
            app.add_plugins(route::PagesPlugin::new(source));
        }
        None => {
            app.add_plugins(route::PagesPlugin::default());
        }
    }
    if let Some(path) = marker {
        // Live runs gate the readiness marker on the pump's first delivered
        // snapshot, so the capture never shoots the pre-sample placeholder.
        let plugin = capture::CapturePlugin::new(path);
        let plugin = if live_controller {
            plugin.waiting_for_first_snapshot()
        } else {
            plugin
        };
        app.add_plugins(plugin);
    }
    app.run();
}
