//! Business-agnostic Bevy UI widget layer for MusicFrog.
//!
//! Charter (docs/BEVY_UI_FRONTEND.md): static structure composes
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

pub mod abi;
pub mod accordion;
pub mod adaptive_modal;
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
pub mod filter;
pub mod fluid_grid;
pub mod focus;
pub mod fonts;
pub mod gamepad_ui;
pub mod gesture;
pub mod haptics;
pub mod i18n;
pub mod icon;
pub mod icon_tile;
pub mod list;
pub mod master_detail;
pub mod menu;
pub mod mobile_view;
pub mod modal;
pub mod motion;
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
pub mod shader_fx;
pub mod signal_dag;
pub mod slider;
pub mod smart_truncate;
pub mod splitter;
pub mod stat_chip;
pub mod surface;
pub mod switch;
pub mod tabs;
pub mod text;
pub mod text_input;
pub mod theme;
pub mod theme_export;
pub mod toast;
pub mod tooltip;
pub mod tsdb;
pub mod windowing;

use bevy::app::{App, Plugin, Update};
use bevy::ecs::schedule::IntoScheduleConfigs;

use crate::palette::UiPalette;
use crate::responsive::{Density, ResponsiveContext};
use crate::theme::Breakpoint;

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
        app.insert_resource(self.palette);
        app.init_resource::<ResponsiveContext>();
        app.init_resource::<Density>();
        app.init_resource::<Breakpoint>();
        app.init_resource::<master_detail::MasterDetailState>();
        app.init_resource::<adaptive_modal::ModalState>();
        app.init_resource::<chart::ring_buffer::TelemetryCadenceManager>();
        app.init_resource::<toast::ToastQueue>();

        // Embedded faces ride the Assets<Font> store every bevy_ui app
        // already carries; a composition without one falls back to the
        // default handles (system faces) instead of failing.
        let fonts = app
            .world_mut()
            .get_resource_mut::<bevy::asset::Assets<bevy::text::Font>>()
            .map(|mut fonts| fonts::FontSources::embedded(&mut fonts));
        if let Some(sources) = fonts {
            app.insert_resource(sources);
        } else {
            app.init_resource::<fonts::FontSources>();
        }

        // Icon plates load through the host's asset server — only when the
        // host actually carries an image store (every render-backed
        // composition does); anything leaner renders icons as invisible
        // squares (never panics).
        let server = app.world().contains_resource::<bevy::asset::AssetServer>();
        let images = app
            .world()
            .contains_resource::<bevy::asset::Assets<bevy::image::Image>>();
        if server && images {
            let sources =
                icon::IconSources::load(app.world().resource::<bevy::asset::AssetServer>());
            app.insert_resource(sources);
        } else {
            app.init_resource::<icon::IconSources>();
        }

        app.add_observer(text::style_text_roles);
        app.add_observer(switch::apply_theme);
        app.add_observer(icon::stamp_icon_plate);
        app.add_observer(responsive::on_density_switch);
        app.add_observer(master_detail::on_master_item_button_activated);
        app.add_observer(master_detail::on_master_item_selected);
        app.add_observer(master_detail::on_master_back_activated);
        app.add_observer(adaptive_modal::on_modal_close_activated);
        app.add_observer(adaptive_modal::on_modal_open);
        app.add_observer(adaptive_modal::on_modal_close);

        app.add_message::<menu::MenuNavEvent>();
        app.add_message::<menu::MenuOutcome>();
        app.add_message::<list::VirtualListScroll>();
        app.add_message::<list::VirtualListFling>();
        app.add_message::<list::VirtualListSelect>();
        app.add_message::<radio::RadioGroupNavEvent>();
        app.add_message::<combobox::ComboboxNavEvent>();
        app.add_message::<combobox::ComboboxOutcomeEvent>();
        app.add_message::<tabs::TabSelectEvent>();
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
                text_input::sync_text_fields,
                text_input::sync_field_borders,
                text_input::sync_field_carets,
                text_input::sync_ime_cursor_areas,
                icon::sync_icon_tints,
            ),
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
                list::sync_list_visuals,
                list::advance_virtual_lists,
                list::sync_list_selection.before(nav::sync_nav_visuals),
                chart::sync_charts,
                chart::donut::sync_donut_charts,
                chart::histogram::sync_histogram_charts,
            ),
        );

        app.add_systems(
            Update,
            (
                chart::topology::sync_topology_charts,
                chart::ring_buffer::update_telemetry_cadence,
                scrollarea::focus_avoidance_auto_scroll_system,
                responsive::sync_responsive_context_from_window,
                fluid_grid::sync_fluid_grid_layout,
                master_detail::sync_master_detail_layout,
                smart_truncate::sync_smart_truncate_text,
                adaptive_modal::sync_adaptive_modal_morphology,
                density::sync_adaptive_density_styles,
            ),
        );

        app.add_systems(
            Update,
            (
                (combobox::advance_combobox, combobox::sync_combobox_visuals).chain(),
                (
                    tabs::advance_segmented_control,
                    tabs::sync_segmented_control_visuals,
                )
                    .chain(),
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
    }
}
