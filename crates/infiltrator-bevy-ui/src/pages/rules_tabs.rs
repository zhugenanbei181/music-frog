//! DUAL-11-14: the Rules workspace partitions (Iced's page tabs mirrored as
//! Bevy visibility partitions).
//!
//! Iced splits the rules workspace into four tabs (list / providers / JSON
//! editors / tracer). Bevy mounts the same four partitions in one scrolling
//! page and switches which one is visible — the capability set is identical,
//! only the presentation differs. The partition identity and order come from
//! [`infiltrator_contract::rules_workspace::RulesTab`], so a tab added to the
//! contract cannot silently exist on one surface only.

use crate::pages::rules_json::RulesJsonState;
use crate::pages::rules_statistics::RulesStatisticsState;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::Commands;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::scene::{Scene, bsn};
use bevy::ui::InteractionDisabled;
use bevy::ui::prelude::{
    AlignItems, BorderRadius, Display, FlexDirection, JustifyContent, Node, UiRect, percent,
};
use bevy::ui_widgets::{ControlOrientation, Tab, TabActivation, TabList, ValueChange};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::rules_workspace::RulesTab;

/// Active partition of the rules workspace.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesTabState {
    pub tab: RulesTab,
}

/// Marker for the Rules workspace tab-list root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesTabStrip;

/// Marker on one tab chip button; the payload is the shared partition index.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesTabChip(pub usize);

/// Marker on a tab chip's label text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesTabChipLabel(pub usize);

/// One partition container: only the active partition is displayed.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesTabBody(pub RulesTab);

fn tab_chip_scene(tab: RulesTab, palette: &UiPalette) -> Box<dyn Scene> {
    let index = tab.index();
    let selected = index == 0;
    Box::new(bsn! {
            Node {
                flex_grow: 1.0,
                min_height: rem(palette.control_height_px),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(rem(space::S12)),
                border_radius: BorderRadius::MAX,
            }
            ThemeBackgroundColor({ if selected { Token::Accent } else { Token::SurfaceElevated } })
            Tab
            RulesTabChip(index)
            LocalizedLabel::plain(tab.i18n_key())
            Children [
                LocalizedText::plain(tab.i18n_key())
                TextRole(Role::Body)
                ThemeTextColor({ if selected { Token::OnAccent } else { Token::InkDim } })
                RulesTabChipLabel(index)
            ]
    })
}

/// The tab bar scene. Scene-constructed once per mount; the active chip is
/// restamped in place by [`sync_rules_tabs`].
pub fn rules_tabs_scene(palette: &UiPalette) -> impl Scene + use<> {
    let chips: Vec<Box<dyn Scene>> = RulesTab::ALL
        .iter()
        .map(|tab| tab_chip_scene(*tab, palette))
        .collect();
    bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: rem(space::S8),
                padding: UiRect::all(rem(space::S4)),
                border_radius: BorderRadius::all(rem(palette.card_radius_px)),
            }
            ThemeBackgroundColor({ Token::Surface })
            RulesTabStrip
            TabList {
                orientation: ControlOrientation::Horizontal,
                activation: TabActivation::Automatic,
            }
            Children [
                { chips }
            ]
    }
}

/// Wrap one partition body so [`sync_rules_tabs`] can show/hide it.
pub fn tab_body_scene(tab: RulesTab, content: Box<dyn Scene>) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S16),
            }
            RulesTabBody(tab)
            Children [
                { vec![content] }
            ]
    })
}

/// DUAL-11-14: show only the active partition and swap chip fill/label ink
/// tokens in place (official `Theme*` repaint on insert — norm ②/③).
pub fn sync_rules_tabs(
    state: Option<Res<RulesTabState>>,
    statistics: Option<Res<RulesStatisticsState>>,
    mut commands: Commands,
    mut bodies: Query<(&mut Node, &RulesTabBody)>,
    chips: Query<(
        Entity,
        &RulesTabChip,
        &Children,
        Option<&InteractionDisabled>,
    )>,
    labels: Query<Entity, With<RulesTabChipLabel>>,
) {
    let Some(state) = state else {
        return;
    };
    let tabs_disabled = statistics.is_some_and(|state| state.model.confirmation.is_some());
    for (mut node, body) in &mut bodies {
        let want = if body.0 == state.tab {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
        }
    }
    for (entity, chip, children, disabled) in &chips {
        if tabs_disabled && disabled.is_none() {
            commands.entity(entity).insert(InteractionDisabled);
        } else if !tabs_disabled && disabled.is_some() {
            commands.entity(entity).remove::<InteractionDisabled>();
        }
        let selected = RulesTab::from_index(chip.0) == state.tab;
        commands
            .entity(entity)
            .insert(ThemeBackgroundColor(if selected {
                Token::Accent.theme_token()
            } else {
                Token::SurfaceElevated.theme_token()
            }));
        let ink = if selected {
            Token::OnAccent
        } else {
            Token::InkDim
        };
        for child in children.iter() {
            if labels.contains(*child) {
                commands
                    .entity(*child)
                    .insert(ThemeTextColor(ink.theme_token()));
            }
        }
    }
}

/// Select the clicked partition. Switching tabs blurs the JSON editor buffer
/// so a hidden editor can never keep consuming keystrokes.
pub fn on_rules_tab_activated(
    change: On<ValueChange<Option<Entity>>>,
    statistics: Option<Res<RulesStatisticsState>>,
    strips: Query<(), With<RulesTabStrip>>,
    chips: Query<&RulesTabChip>,
    mut state: Option<ResMut<RulesTabState>>,
    mut json: Option<ResMut<RulesJsonState>>,
) {
    if statistics.is_some_and(|state| state.model.confirmation.is_some()) {
        return;
    }
    if !strips.contains(change.source) {
        return;
    }
    let Some(selected) = change.value else {
        return;
    };
    let Ok(chip) = chips.get(selected) else {
        return;
    };
    let Some(state) = state.as_deref_mut() else {
        return;
    };
    let tab = RulesTab::from_index(chip.0);
    if state.tab == tab {
        return;
    }
    state.tab = tab;
    if let Some(json) = json.as_deref_mut() {
        json.focused = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_shared::locales::{Lang, Localizer};

    #[test]
    fn every_shared_partition_has_a_chip_label_and_a_body_marker() {
        assert_eq!(RulesTab::ALL.len(), 4);
        let labels: Vec<String> = RulesTab::ALL
            .iter()
            .map(|tab| Lang("zh-CN").tr(tab.i18n_key()).into_owned())
            .collect();
        let mut dedup = labels.clone();
        dedup.sort_unstable();
        dedup.dedup();
        assert_eq!(dedup.len(), labels.len(), "labels must be distinct");
        assert!(labels.iter().all(|label| !label.is_empty()));
        for tab in RulesTab::ALL {
            let body = RulesTabBody(tab);
            assert_eq!(body.0, tab);
        }
    }

    #[test]
    fn default_partition_is_the_shared_default() {
        assert_eq!(RulesTabState::default().tab, RulesTab::default());
        assert_eq!(RulesTabState::default().tab, RulesTab::List);
        assert_eq!(Lang("zh-CN").tr(RulesTab::List.i18n_key()), "规则列表");
        assert_eq!(
            Lang("zh-CN").tr(RulesTab::JsonEditors.i18n_key()),
            "JSON 编辑器"
        );
    }
}
