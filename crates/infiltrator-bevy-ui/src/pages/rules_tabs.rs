//! DUAL-11-14: the Rules workspace partitions (Iced's page tabs mirrored as
//! Bevy visibility partitions).
//!
//! Iced splits the rules workspace into four tabs (list / providers / JSON
//! editors / tracer). Bevy mounts the same four partitions in one scrolling
//! page and switches which one is visible — the capability set is identical,
//! only the presentation differs. The partition identity and order come from
//! [`infiltrator_contract::rules_workspace::RulesTab`], so a tab added to the
//! contract cannot silently exist on one surface only.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, Display, FlexDirection, JustifyContent, Node,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::rules_workspace::RulesTab;

/// Active partition of the rules workspace.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesTabState {
    pub tab: RulesTab,
}

/// One partition container: only the active partition is displayed.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesTabBody(pub RulesTab);

/// Marker on one tab chip button; the payload is the shared partition index.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesTabChip(pub usize);

/// Marker on a tab chip's label text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesTabChipLabel(pub usize);

/// Bare-Chinese label of a shared partition (Bevy presentation convention).
pub const fn tab_label_zh(tab: RulesTab) -> &'static str {
    match tab {
        RulesTab::List => "规则列表",
        RulesTab::Providers => "提供者",
        RulesTab::JsonEditors => "JSON 编辑器",
        RulesTab::Tracer => "分流追踪",
    }
}

fn tab_chip_scene(tab: RulesTab, palette: &UiPalette) -> Box<dyn Scene> {
    let index = tab.index();
    let selected = index == 0;
    let background = if selected {
        palette.accent
    } else {
        palette.surface_elevated
    };
    let ink = if selected {
        palette.on_accent
    } else {
        palette.ink_dim
    };
    Box::new(bsn! {
        Node {
            flex_grow: 1.0,
            min_height: px(palette.control_height_px),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::horizontal(Val::Px(space::S12)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ background })
        Button
        RulesTabChip(index)
        Children [
            (
                Text({ tab_label_zh(tab).to_owned() })
                TextRole(Role::Body)
                TextColor({ ink })
                RulesTabChipLabel(index)
            ),
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
            column_gap: Val::Px(space::S8),
            padding: UiRect::all(Val::Px(space::S4)),
            border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
        }
        BackgroundColor({ palette.surface })
        Children [
            { chips },
        ]
    }
}

/// Wrap one partition body so [`sync_rules_tabs`] can show/hide it.
pub fn tab_body_scene(tab: RulesTab, content: Box<dyn Scene>) -> Box<dyn Scene> {
    Box::new(bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S16),
        }
        RulesTabBody(tab)
        Children [
            { vec![content] },
        ]
    })
}

/// DUAL-11-14: show only the active partition and restamp the chip fills and
/// label inks from the palette.
pub fn sync_rules_tabs(
    state: Option<Res<RulesTabState>>,
    palette: Res<UiPalette>,
    mut bodies: Query<(&mut Node, &RulesTabBody)>,
    mut chips: Query<(&mut BackgroundColor, &RulesTabChip, &Children)>,
    mut labels: Query<&mut TextColor, bevy::ecs::query::With<RulesTabChipLabel>>,
) {
    let Some(state) = state else {
        return;
    };
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
    for (mut background, chip, children) in &mut chips {
        let selected = RulesTab::from_index(chip.0) == state.tab;
        let want = if selected {
            palette.accent
        } else {
            palette.surface_elevated
        };
        if background.0 != want {
            background.0 = want;
        }
        let ink = if selected {
            palette.on_accent
        } else {
            palette.ink_dim
        };
        for child in children.iter() {
            if let Ok(mut color) = labels.get_mut(*child)
                && color.0 != ink
            {
                color.0 = ink;
            }
        }
    }
}

/// Select the clicked partition. Switching tabs blurs the JSON editor buffer
/// so a hidden editor can never keep consuming keystrokes.
pub fn on_rules_tab_activated(
    activate: On<Activate>,
    chips: Query<&RulesTabChip>,
    mut state: Option<ResMut<RulesTabState>>,
    mut json: Option<ResMut<crate::pages::rules_json::RulesJsonState>>,
) {
    let Ok(chip) = chips.get(activate.entity) else {
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

    #[test]
    fn every_shared_partition_has_a_chip_label_and_a_body_marker() {
        assert_eq!(RulesTab::ALL.len(), 4);
        let labels: Vec<&str> = RulesTab::ALL.iter().map(|tab| tab_label_zh(*tab)).collect();
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
        assert_eq!(tab_label_zh(RulesTab::List), "规则列表");
        assert_eq!(tab_label_zh(RulesTab::JsonEditors), "JSON 编辑器");
    }
}
