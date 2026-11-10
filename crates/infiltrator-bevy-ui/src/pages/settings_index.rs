//! BEVY-GAP-031: the Settings index of the pages that fold under Settings.
//!
//! The navigation registry gives a desktop surface one row per *destination*,
//! and `destination_for` folds DNS, Doctor, App Routing, Sync, Tools, About,
//! Disclaimer and Licenses under `Settings`. That is the contract's decision,
//! and the rail obeys it — which means those pages need a second door, or a
//! seven-row sidebar silently makes eight pages unreachable by pointer. The
//! Bevy rail used to paper over the gap by mounting all fifteen routes, which
//! pushed the stack below the fold; the judged surfaces do not, and neither
//! does this one.
//!
//! **The list is not this file's decision.** The chips come from
//! [`pages_folded_under`], the declared inverse of [`destination_for`], so a
//! page added to the contract appears in the index on every surface at once.
//! Slint states the same fact as its `SettingRow` sections
//! (`ui/pages/page_settings.slint`). A click sends the typed [`RouteChanged`]
//! intent and nothing else: the destination page remains the owner of what it
//! renders, and the rail's active row keeps following `destination_for`, so
//! the index never needs to know which row is current.

use crate::localized_widgets::localized_button_scene;
use crate::route::{Route, RouteChanged};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::system::Commands;
use bevy::ecs::system::Query;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, FlexDirection, FlexWrap, JustifyContent, Node, UiRect, percent,
};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::button::ButtonVariant;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::command_catalogue::ShellPage;
use infiltrator_contract::shell_navigation::pages_folded_under;

/// Marker on one index chip; the payload is the route the chip reaches. The
/// chip set itself is the contract's ([`pages_folded_under`]); this is only the
/// surface's own navigation identity, the same vocabulary a rail row carries.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SettingsIndexButton(pub Route);

/// The index band: one chip per page that folds under `Settings`.
pub fn settings_index_scene(palette: &UiPalette) -> impl Scene + use<> {
    let chips: Vec<Box<dyn Scene>> = pages_folded_under(ShellPage::Settings)
        .into_iter()
        .map(|page| {
            let route = Route::from_shell_page(page);
            Box::new(bsn! {
                @{ localized_button_scene(
                        LocalizedText::plain(page.sidebar_key()),
                        ButtonVariant::Default,
                        palette,
                ) }
                SettingsIndexButton(route)
            }) as Box<dyn Scene>
        })
        .collect();
    bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                flex_wrap: FlexWrap::Wrap,
                column_gap: rem(space::S8),
                row_gap: rem(space::S4),
                padding: UiRect::vertical(rem(space::S4)),
            }
            Children [
                { chips }
            ]
    }
}

/// Navigate to the folded page the chip names.
///
/// The index is a pure consumer: it sends the same typed intent a rail row
/// sends and reads nothing back. The active destination stays the router's
/// fact (`destination_for`), so the index never restates which row is current.
pub fn on_settings_index_activated(
    activate: On<Activate>,
    chips: Query<&SettingsIndexButton>,
    mut commands: Commands,
) {
    let Ok(chip) = chips.get(activate.entity) else {
        return;
    };
    commands.trigger(RouteChanged(chip.0));
}
