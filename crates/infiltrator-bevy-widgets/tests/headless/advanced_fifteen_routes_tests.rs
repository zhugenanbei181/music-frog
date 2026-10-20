//! Headless coverage for the fifteen advanced widget routes.
//!
//! The cases are grouped by business seam in the sibling modules next to this
//! file: `platform_foundations`, `typed_stage_guards`, `ecosystem_rounds` and
//! `self_heal_and_sandbox`. This module owns the shared imports every group
//! builds on.

#[path = "advanced_fifteen_routes_tests/ecosystem_rounds.rs"]
mod ecosystem_rounds;
#[path = "advanced_fifteen_routes_tests/platform_foundations.rs"]
mod platform_foundations;
#[path = "advanced_fifteen_routes_tests/self_heal_and_sandbox.rs"]
mod self_heal_and_sandbox;
#[path = "advanced_fifteen_routes_tests/typed_stage_guards.rs"]
mod typed_stage_guards;

use bevy::color::Color;
use bevy::math::Vec2;
use bevy::text::Justify;
use bevy::ui::prelude::{AlignItems, FlexDirection, JustifyContent, UiRect, Val};
use infiltrator_bevy_widgets::abi::{
    DesktopRunnerHost, UniversalRunnerHost, WIDGET_ABI_VERSION, is_abi_compatible,
};
use infiltrator_bevy_widgets::auto_heal::{AutoHealWizardState, DiagnosticAnomaly};
use infiltrator_bevy_widgets::bidi::{FontVariationAxes, LayoutDirection};
use infiltrator_bevy_widgets::boot_cache::{BootPipelineCache, StaticByteTable};
use infiltrator_bevy_widgets::chaos::{ChaosFaultConfig, MonkeyExplorerBot};
use infiltrator_bevy_widgets::gamepad_ui::GamepadScrollState;
use infiltrator_bevy_widgets::mobile_view::{
    CameraPixelFormat, CameraTextureFeed, NativePlatformViewSlot, PlatformViewTransition,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::sandbox::SandboxedWidgetInstance;
use infiltrator_bevy_widgets::theme::Theme;
use infiltrator_bevy_widgets::theme_export::{ThemeExportFormat, export_palette_tokens};
use infiltrator_bevy_widgets::windowing::{
    DockPanel, DockSlot, PipOverlaySession, PipOverlayState, WindowRegistry,
};
use std::time;
