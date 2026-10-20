//! Business-agnostic Bevy UI widget layer for MusicFrog.
//!
//! Charter (crates/infiltrator-bevy-ui/docs/BEVY_UI_FRONTEND.md): static structure composes
//! declaratively with `bsn!` scene functions; runtime changes restamp
//! components via observers, never rebuild trees; every color and metric
//! originates in [`theme`] tokens and becomes a bevy value only inside
//! [`palette`]. Behavior primitives come from the official unstyled
//! `bevy_ui_widgets`; the product skin lives here. Each control is a pure
//! function core plus a `*_scene` adapter, so the semantics stay
//! headless-testable.
//!
//! This crate depends on locked bevy only — never on business crates — so a
//! future extraction shared with taskmanager's bevy frontend is a
//! lift-and-shift.

use bevy::input::mouse::MouseScrollPixelsPerLine;
use bevy::picking::pointer::PointerInput;
use bevy::ui_widgets::{Button, ScrollAreaPlugin};
use chart::donut::sync_donut_charts;
use chart::histogram::sync_histogram_charts;
use chart::ring_buffer::{TelemetryCadenceManager, update_telemetry_cadence};
use chart::texture::release_retired;
use chart::topology::{advance_topology_flow, sync_topology_charts};
use globe::advance_globe_rotation;
use globe::report::{GlobeMeasurement, sync_globe_measurement};
use globe::stage::{GlobeIdleBudget, GlobeStage, sync_globe_stage};
use globe::sync_globe_charts;
pub mod abi;
pub mod accordion;
pub mod adaptive_modal;
mod asset_setup;
pub mod auto_heal;
pub mod bidi;
pub mod boot_cache;
pub mod button;
pub mod cadence;
pub mod chaos;
pub mod chart;
pub mod checkbox;
pub mod clipboard_sanitizer;
pub mod combobox;
pub mod context_menu;
pub mod datagrid;
pub mod density;
pub mod desktop;
pub mod drawer;
pub mod editor;
pub mod feathers_theme;
pub mod filter;
pub mod fluid_grid;
pub mod focus;
pub mod fonts;
pub mod gamepad_ui;
pub mod gesture;
pub mod globe;
pub mod haptics;
pub mod i18n;
pub mod icon;
pub mod icon_tile;
pub mod list;
pub mod localization;
pub mod master_detail;
pub mod menu;
pub mod mobile_view;
pub mod modal;
pub mod motion;
pub mod multiline_editor;
pub mod nav;
pub mod palette;
pub mod particle;
pub mod popover;
pub mod radio;
pub mod reactive;
pub mod reorderable;
pub mod responsive;
pub mod sandbox;
pub mod scrollarea;
pub mod selection;
mod shader_assets;
pub mod shader_fx;
pub mod signal_dag;
pub mod slider;
pub mod smart_truncate;
pub mod splitter;
pub mod stat_chip;
pub mod surface;
pub mod surface_shader;
pub mod switch;
pub mod tabs;
pub mod telemetry_gpu;
pub mod text;
pub mod text_input;
pub mod text_runs;
pub mod theme;
pub mod theme_export;
pub mod toast;
pub mod tooltip;
pub mod tsdb;
pub mod windowing;

use crate::button::ButtonDisabled;
use crate::localization::WidgetLocalizationPlugin;
use crate::palette::UiPalette;
use crate::responsive::{Density, ResponsiveContext};
use crate::telemetry_gpu::TelemetryGpuPlugin;
use crate::text_input::native::NativeTextFieldPlugin;
use crate::text_input::render::{
    sync_field_borders, sync_field_carets, sync_ime_cursor_areas, sync_text_fields,
};
use crate::text_runs::TextRunsPlugin;
use crate::theme::Breakpoint;
use crate::theme::REM_BASE;
use bevy::app::{App, Plugin, PostUpdate, PreStartup, Update};
use bevy::asset::AssetApp;
use bevy::ecs::schedule::{ApplyDeferred, IntoScheduleConfigs};
use bevy::feathers::FeathersCorePlugin;
use bevy::input_focus::InputFocusPlugin;
use bevy::render::RenderPlugin;
use bevy::shader::Shader;
use bevy::text::RemSize;
use bevy::ui::UiScale;
use bevy::ui::UiSystems;
use bevy::ui_widgets::{TabPlugin, tablist_self_update};

/// Installs the resolved palette resource, the embedded font sources, the
/// icon plate store, the typography/theme observers and the per-control
/// repaint systems. Frontends add this before spawning any scene.
pub struct WidgetsPlugin {
    palette: UiPalette,
}

impl WidgetsPlugin {
    pub fn new(theme: &theme::Theme) -> Self {
        Self {
            palette: UiPalette::new(theme),
        }
    }
}

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        // Native required components attach at spawn, before any navigation can retire a control.
        app.register_required_components::<Button, responsive::TouchHitbox>()
            .register_required_components::<Button, ButtonDisabled>();
        // The globe enhancement's own stage attaches with its plate: a plate
        // without a stage is a live enhancement.
        app.register_required_components::<globe::GlobePlate, GlobeStage>();
        if !app.is_plugin_added::<WidgetLocalizationPlugin>() {
            app.add_plugins(WidgetLocalizationPlugin);
        }
        app.add_plugins((NativeTextFieldPlugin, TextRunsPlugin));
        app.add_plugins(TelemetryGpuPlugin);
        app.init_resource::<MouseScrollPixelsPerLine>();
        app.add_message::<PointerInput>();
        if !app.is_plugin_added::<ScrollAreaPlugin>() {
            app.add_plugins(ScrollAreaPlugin);
        }
        app.init_resource::<scrollarea::ScrollMotionState>()
            .init_resource::<scrollarea::ScrollMotionPreference>()
            .init_resource::<scrollarea::PointerGestureTracker>()
            .add_observer(scrollarea::track_button_pointer_press)
            .add_observer(scrollarea::cancel_pointer_tap_on_drag_start)
            .add_observer(scrollarea::scrollarea_on_drag_start)
            .add_observer(scrollarea::scrollarea_on_drag)
            .add_observer(scrollarea::scrollarea_on_drag_end)
            .add_observer(scrollarea::scrollarea_on_drag_cancel);

        app.insert_resource(self.palette);
        // Root font size: the single scale knob behind every rem token
        // (capability norm ①). Hosts override this resource to rescale the
        // whole UI; nothing else may change type sizes.
        app.insert_resource(RemSize(REM_BASE));
        // Official two-stage theming (capability norm ②): the palette builds
        // the feathers UiTheme; overwriting that resource repaints every
        // Theme* paint component through the official update_theme systems.
        // FeathersCorePlugin embeds .wesl shader assets; the asset store must
        // exist even in hosts that skip AssetPlugin (headless compositions).
        // RenderPlugin owns the live Shader store and its handle allocator.
        // Reinitializing it invalidates in-flight embedded Feathers shader loads.
        if !app.is_plugin_added::<RenderPlugin>() {
            app.init_asset::<Shader>();
        }
        // Feathers observers read these even in headless hosts that skip
        // UiPlugin / the windowing stack.
        app.init_resource::<UiScale>();
        // Feathers' focus-outline systems read the input-focus resources.
        if !app.is_plugin_added::<InputFocusPlugin>() {
            app.add_plugins(InputFocusPlugin);
        }
        if !app.is_plugin_added::<FeathersCorePlugin>() {
            app.add_plugins(FeathersCorePlugin);
        }
        app.insert_resource(feathers_theme::build_ui_theme(&self.palette));
        app.add_systems(Update, feathers_theme::sync_ui_theme);
        // Official headless tab strip (capability norm ③): selection state,
        // keyboard navigation and a11y roles come from bevy_ui_widgets.
        if !app.is_plugin_added::<TabPlugin>() {
            app.add_plugins(TabPlugin);
        }
        // The official dialog layer (capability norm ⑥): the barrier
        // click-outside, the Escape interception, the focus containment and
        // the dialog stack come from bevy_ui_widgets, so every product
        // surface that mounts a dialog gets them.
        if !app.is_plugin_added::<modal::ModalWidgetsPlugin>() {
            app.add_plugins(modal::ModalWidgetsPlugin);
        }
        app.add_observer(tablist_self_update);
        app.add_observer(tabs::stamp_initial_tab_ink);
        app.init_resource::<ResponsiveContext>();
        app.init_resource::<Density>();
        app.init_resource::<Breakpoint>();
        app.init_resource::<master_detail::MasterDetailState>();
        app.init_resource::<adaptive_modal::ModalState>();
        app.init_resource::<TelemetryCadenceManager>();
        app.init_resource::<GlobeIdleBudget>();
        app.init_resource::<GlobeMeasurement>();
        app.init_resource::<toast::ToastQueue>();

        app.init_resource::<fonts::FontSources>();
        app.init_resource::<icon::IconSources>();
        app.add_systems(
            PreStartup,
            (asset_setup::initialize_fonts, asset_setup::initialize_icons),
        );

        app.add_observer(text::style_text_roles);
        app.add_observer(button::insert_button_disabled);
        app.add_observer(switch::apply_theme);
        app.add_observer(icon::stamp_icon_plate);
        app.add_observer(responsive::on_density_switch);
        app.add_observer(master_detail::on_master_item_button_activated);
        app.add_observer(master_detail::on_master_item_selected);
        app.add_observer(master_detail::on_master_back_activated);
        app.add_observer(adaptive_modal::on_adaptive_modal_request_close);
        app.add_observer(adaptive_modal::on_modal_open);
        app.add_observer(adaptive_modal::on_modal_close);

        app.add_message::<menu::MenuNavEvent>();
        app.add_message::<menu::MenuOutcome>();
        app.add_message::<list::VirtualListScroll>();
        app.add_message::<list::VirtualListSelect>();
        app.add_message::<radio::RadioGroupNavEvent>();
        app.add_message::<combobox::ComboboxNavEvent>();
        app.add_message::<combobox::ComboboxOutcomeEvent>();
        app.add_message::<modal::ModalEvent>();
        app.add_message::<modal::ModalOpenEvent>();
        app.add_message::<modal::ModalCloseEvent>();
        app.add_message::<drawer::DrawerOpenEvent>();
        app.add_message::<drawer::DrawerCloseEvent>();
        app.add_message::<toast::ToastSpawnEvent>();
        app.add_message::<toast::ToastDismissEvent>();
        app.add_message::<accordion::AccordionToggleEvent>();
        app.add_message::<splitter::SplitterDragEvent>();

        app.add_systems(
            Update,
            (
                button::sync_control_visuals,
                button::sync_control_labels,
                checkbox::sync_checkbox_visuals,
                radio::advance_radio_group_navigation,
                radio::sync_radio_visuals,
                slider::sync_slider_visuals,
                slider::sync_range_slider_visuals,
                sync_text_fields,
                sync_field_borders,
                sync_field_carets,
                sync_ime_cursor_areas,
                icon::sync_icon_tints,
            ),
        );
        app.add_systems(
            Update,
            scrollarea::cancel_motion_on_pointer_input.before(scrollarea::advance_scroll_motion),
        );

        // Page projection and retirement queues finish in Update. Sync SDK
        // state from surviving controls, then commit before UI preparation.
        app.add_systems(
            PostUpdate,
            (
                button::sync_button_disabled,
                // The shared switch control owns the knob's geometry: the
                // surface flips the typed position, this re-projects the
                // offset before the layout pass reads it.
                switch::sync_switch_knobs,
                interaction_block::sync_inputs,
                ApplyDeferred,
            )
                .chain()
                .before(UiSystems::Prepare),
        );

        app.add_systems(
            Update,
            (
                icon_tile::sync_icon_tile_visuals,
                nav::sync_nav_visuals,
                stat_chip::sync_stat_chip_visuals,
                surface::sync_surface_visuals,
                (menu::advance_menus, menu::sync_menu_visuals).chain(),
                popover::sync_popover_visuals,
                list::advance_virtual_lists,
                list::sync_list_selection.before(nav::sync_nav_visuals),
                chart::sync_chart_crosshair_tracking,
                chart::sync_charts,
                release_retired,
                sync_donut_charts,
                sync_histogram_charts,
            ),
        );

        app.add_systems(
            Update,
            (
                (advance_topology_flow, sync_topology_charts).chain(),
                (
                    advance_globe_rotation,
                    sync_globe_stage,
                    sync_globe_charts,
                    sync_globe_measurement,
                )
                    .chain(),
                update_telemetry_cadence,
                scrollarea::advance_scroll_motion,
                scrollarea::focus_avoidance_auto_scroll_system,
                responsive::sync_responsive_context_from_window,
                fluid_grid::sync_fluid_grid_layout,
                master_detail::sync_master_detail_layout,
                smart_truncate::sync_smart_truncate_text,
                adaptive_modal::sync_adaptive_modal_morphology,
                density::sync_adaptive_density_styles,
                responsive::sync_touch_hitboxes,
            ),
        );

        app.add_systems(
            Update,
            (
                (combobox::advance_combobox, combobox::sync_combobox_visuals).chain(),
                (tabs::sync_segmented_control_visuals,).chain(),
                modal::sync_modal_visuals,
                drawer::sync_drawer_visuals,
                tooltip::sync_tooltip_visuals,
                (toast::advance_toasts, toast::sync_toast_visuals).chain(),
                (
                    accordion::advance_accordions,
                    accordion::sync_accordion_visuals,
                )
                    .chain(),
            ),
        );

        app.add_systems(
            Update,
            (splitter::advance_splitters, splitter::sync_splitter_visuals).chain(),
        );

        app.add_plugins(motion::SpringAnimationPlugin);
    }
}

pub mod interaction_block;
