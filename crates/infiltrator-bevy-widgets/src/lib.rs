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

pub mod button;
pub mod chart;
pub mod checkbox;
pub mod fonts;
pub mod icon;
pub mod icon_tile;
pub mod list;
pub mod menu;
pub mod nav;
pub mod palette;
pub mod popover;
pub mod radio;
pub mod scrollarea;
pub mod slider;
pub mod stat_chip;
pub mod surface;
pub mod switch;
pub mod text;
pub mod text_input;
pub mod theme;

use bevy::app::{App, Plugin, Update};
use bevy::ecs::schedule::IntoScheduleConfigs;

use crate::palette::UiPalette;

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
        app.add_message::<menu::MenuNavEvent>();
        app.add_message::<menu::MenuOutcome>();
        app.add_message::<list::VirtualListScroll>();
        app.add_message::<list::VirtualListSelect>();
        app.add_systems(
            Update,
            (
                button::sync_control_visuals,
                button::sync_control_labels,
                checkbox::sync_checkbox_visuals,
                radio::sync_radio_visuals,
                slider::sync_slider_visuals,
                text_input::sync_text_fields,
                text_input::sync_field_carets,
                text_input::sync_ime_cursor_areas,
                icon::sync_icon_tints,
                icon_tile::sync_icon_tile_visuals,
                nav::sync_nav_visuals,
                stat_chip::sync_stat_chip_visuals,
                surface::sync_surface_visuals,
                // The nav advance must land before the repaint reads the
                // highlight, so a move paints in the frame it happens.
                (menu::advance_menus, menu::sync_menu_visuals).chain(),
                popover::sync_popover_visuals,
                list::sync_list_visuals,
                list::advance_virtual_lists,
                // The selection bit must land before the nav repaint reads
                // it, so a flip paints in the same frame it happens.
                list::sync_list_selection.before(nav::sync_nav_visuals),
                chart::sync_charts,
                scrollarea::focus_avoidance_auto_scroll_system,
            ),
        );
    }
}
