//! Bevy Settings scene piece for the local-only offline-startup preflight.
//!
//! Keeping the offline-first row beside its projection-specific scene keeps
//! `settings_core` below the source-size budget without moving any business
//! decision into the widget layer.

use super::settings_core::{SettingsLine, SettingsLineKind};
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderRadius;
use bevy::ui::prelude::{AlignItems, BackgroundColor, JustifyContent, Node, UiRect, Val, percent};
use bevy::ui::widget::Text;
use infiltrator_application::settings_status_projection::format_offline_startup;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::offline_startup::OfflineStartupSnapshot;

pub(super) fn offline_startup_row_scene(
    snapshot: &OfflineStartupSnapshot,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let status = format_offline_startup(snapshot, UiLocale::default().code());
    Box::new(bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(space::S8)),
                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
            }
            BackgroundColor({ palette.surface_elevated })
            Children [
                LocalizedText::plain("settings_offline_startup_title") TextRole(Role::Body)
                --
                Text(status) SettingsLine(SettingsLineKind::OfflineStartup) TextRole(Role::Mono)
            ]
    })
}
