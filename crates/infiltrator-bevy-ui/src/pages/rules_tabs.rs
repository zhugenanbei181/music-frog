//! DUAL-11-14: the Rules workspace partitions (Iced's page tabs mirrored as
//! Bevy visibility partitions).
//!
//! Iced splits the rules workspace into four tabs (list / providers / JSON
//! editors / tracer). Bevy mounts the same four partitions in one scrolling
//! page and switches which one is visible — the capability set is identical,
//! only the presentation differs. The partition identity and order come from
//! [`infiltrator_contract::rules_workspace::RulesTab`], so a tab added to the
//! contract cannot silently exist on one surface only.
//!
//! **Official behaviour layer**: the partition strip is the shared segmented
//! control (`infiltrator_bevy_widgets::tabs`), which mounts the official
//! [`bevy::ui_widgets::TabList`] / [`Tab`] pair. Selection, roving focus,
//! keyboard navigation and the accessibility roles belong to the official
//! widgets; the page only owns the *controlled* selection: the shared rules
//! page model publishes the active partition, and the strip's official
//! [`SelectedTab`] is written back from it.

use crate::pages::page_models::RulesPageModelState;
use crate::pages::rules_json::{RulesJsonPanelRequest, RulesJsonState};
use crate::pages::rules_statistics::RulesStatisticsState;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::scene::{Scene, bsn};
use bevy::ui::InteractionDisabled;
use bevy::ui::prelude::{Display, FlexDirection, Node, percent};
use bevy::ui_widgets::{Activate, SelectedTab, Tab, ValueChange};
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::tabs::{
    SegmentedTab, SegmentedTabLabel, segmented_control_with_labels_scene,
};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::rules_workspace::RulesTab;

/// The active partition of the rules workspace, as the capture layer reads it.
///
/// The tab itself is the shared rules page model's; this resource is the
/// read-only view the capture seam observes, refreshed from the model by
/// [`sync_rules_tabs`].
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesTabState {
    /// The active partition.
    pub tab: RulesTab,
}

/// Marker for the Rules workspace tab-list root (the official `TabList`).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesTabStrip;

/// Marker on one tab chip button; the payload is the shared partition index.
///
/// The official segmented control owns the chip geometry; this marker is the
/// page's own identity for the partition a chip stands for.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesTabChip(pub usize);

/// Marker on a tab chip's label text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesTabChipLabel(pub usize);

/// One partition container: only the active partition is displayed.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesTabBody(pub RulesTab);

/// The label scene of one partition: the shared copy key, the caption role and
/// the marker the shared segmented control paints.
fn tab_chip_scene(tab: RulesTab) -> Box<dyn Scene> {
    let key = tab.i18n_key();
    let index = tab.index();
    Box::new(bsn! {
            LocalizedText { key: { key }, params: { Vec::new() } }
            TextRole(Role::Caption)
            SegmentedTabLabel
            RulesTabChipLabel(index)
    }) as Box<dyn Scene>
}

/// The tab bar scene. The official segmented control is mounted once per mount;
/// the active chip is restamped in place by [`sync_rules_tabs`].
pub fn rules_tabs_scene(palette: &UiPalette) -> impl Scene + use<> {
    let chips: Vec<Box<dyn Scene>> = RulesTab::ALL
        .iter()
        .map(|tab| tab_chip_scene(*tab))
        .collect();
    bsn! {
        @{ segmented_control_with_labels_scene(chips, RulesTab::default().index(), palette) }
        RulesTabStrip
    }
}

/// Wrap one partition body so [`sync_rules_tabs`] can show/hide it.
///
/// The body never shrinks: the page root owns the scroll, so a partition keeps
/// its natural height and the rule list's own viewport cannot overflow into
/// the partition below it.
pub fn tab_body_scene(tab: RulesTab, content: Box<dyn Scene>) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                width: percent(100),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S16),
            }
            RulesTabBody(tab)
            Children [
                { vec![content] }
            ]
    })
}

type UnbridgedTabQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static SegmentedTab), (With<Tab>, Without<RulesTabChip>)>;

type UnlabeledTabQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static SegmentedTab), (With<Tab>, Without<LocalizedLabel>)>;

#[derive(SystemParam)]
pub struct RulesTabsSyncParams<'w, 's> {
    state: Option<Res<'w, RulesPageModelState>>,
    view: Option<ResMut<'w, RulesTabState>>,
    statistics: Option<Res<'w, RulesStatisticsState>>,
    json: Option<ResMut<'w, RulesJsonState>>,
    commands: Commands<'w, 's>,
    bodies: Query<'w, 's, (&'static mut Node, &'static RulesTabBody)>,
    strips:
        Query<'w, 's, (Entity, &'static mut SelectedTab, &'static Children), With<RulesTabStrip>>,
    tabs: Query<'w, 's, (Entity, &'static SegmentedTab, Has<InteractionDisabled>), With<Tab>>,
    unbridged: UnbridgedTabQuery<'w, 's>,
    unlabeled: UnlabeledTabQuery<'w, 's>,
}

/// DUAL-11-14: show only the active partition and drive the official strip's
/// selection from the shared page model.
///
/// The official `TabList` is a *controlled* selection: a click only requests a
/// selection (`ValueChange`), and the owner writes [`SelectedTab`]. The shared
/// rules page model is that owner, so the strip's official selection is
/// written from the model's partition — never from a second page-local truth.
pub fn sync_rules_tabs(params: RulesTabsSyncParams) {
    let RulesTabsSyncParams {
        state,
        mut view,
        statistics,
        mut json,
        mut commands,
        mut bodies,
        mut strips,
        tabs,
        unbridged,
        unlabeled,
    } = params;
    // The page's own identity for the official chips: the shared segmented
    // control owns the chip, the page owns which partition a chip stands for.
    // The bridge does not depend on the model, so a chip is always the page's
    // own marker from the first frame.
    for (entity, tab) in &unbridged {
        commands.entity(entity).insert(RulesTabChip(tab.0));
    }
    for (entity, tab) in &unlabeled {
        let key = RulesTab::from_index(tab.0).i18n_key();
        commands.entity(entity).insert(LocalizedLabel::plain(key));
    }
    let Some(state) = state else {
        return;
    };
    if let Some(view) = view.as_deref_mut()
        && view.tab != state.0.tab()
    {
        view.tab = state.0.tab();
    }
    let tabs_disabled = statistics.is_some_and(|state| state.model.confirmation().is_some());
    for (mut node, body) in &mut bodies {
        let want = if body.0 == state.0.tab() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
        }
    }
    // The JSON partition's editor panel reveals itself with the partition and
    // folds back with it: the panel's stage is the only state of that reveal.
    if let Some(json) = json.as_deref_mut() {
        let request = if state.0.tab() == RulesTab::JsonEditors {
            RulesJsonPanelRequest::Reveal
        } else {
            RulesJsonPanelRequest::Hide
        };
        json.panel = json.panel.transition(request);
    }
    let selected_tab = tabs
        .iter()
        .find(|(_, tab, _)| tab.0 == state.0.tab().index())
        .map(|(entity, _, _)| entity);
    for (_, mut selected, children) in &mut strips {
        if let Some(target) = selected_tab
            && children.contains(&target)
            && selected.0 != Some(target)
        {
            selected.0 = Some(target);
        }
    }
    // The official strip disables as a whole: a pending confirmation closes
    // every partition, so every chip closes with it.
    if tabs_disabled {
        for (entity, _, _) in &strips {
            commands.entity(entity).insert(InteractionDisabled);
        }
        for (entity, _, disabled) in &tabs {
            if !disabled {
                commands.entity(entity).insert(InteractionDisabled);
            }
        }
    } else {
        for (entity, _, _) in &strips {
            commands.entity(entity).remove::<InteractionDisabled>();
        }
        for (entity, _, disabled) in &tabs {
            if disabled {
                commands.entity(entity).remove::<InteractionDisabled>();
            }
        }
    }
}

/// Select the clicked partition. Switching tabs blurs the JSON editor buffer
/// so a hidden editor can never keep consuming keystrokes.
///
/// The official strip only *requests* a selection; this observer is the owner
/// that adopts it into the shared rules page model.
pub fn on_rules_tab_activated(
    change: On<ValueChange<Option<Entity>>>,
    statistics: Option<Res<RulesStatisticsState>>,
    strips: Query<(), With<RulesTabStrip>>,
    chips: Query<&RulesTabChip>,
    mut state: Option<ResMut<RulesPageModelState>>,
    mut view: Option<ResMut<RulesTabState>>,
    mut json: Option<ResMut<RulesJsonState>>,
) {
    if statistics.is_some_and(|state| state.model.confirmation().is_some()) {
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
    select_partition(
        RulesTab::from_index(chip.0),
        &mut state,
        &mut view,
        &mut json,
    );
}

/// A partition chip activated directly.
///
/// The official strip answers pointer clicks and keyboard navigation; a caller
/// that addresses a partition by itself (the capture seam, a shell command)
/// activates the chip, and the chip answers the official `Activate` event the
/// same way it answers a click.
pub fn on_rules_tab_chip_activated(
    activate: On<Activate>,
    statistics: Option<Res<RulesStatisticsState>>,
    chips: Query<&RulesTabChip>,
    mut state: Option<ResMut<RulesPageModelState>>,
    mut view: Option<ResMut<RulesTabState>>,
    mut json: Option<ResMut<RulesJsonState>>,
) {
    if statistics.is_some_and(|state| state.model.confirmation().is_some()) {
        return;
    }
    let Ok(chip) = chips.get(activate.entity) else {
        return;
    };
    select_partition(
        RulesTab::from_index(chip.0),
        &mut state,
        &mut view,
        &mut json,
    );
}

/// Adopt a partition into the shared rules page model.
fn select_partition(
    tab: RulesTab,
    state: &mut Option<ResMut<RulesPageModelState>>,
    view: &mut Option<ResMut<RulesTabState>>,
    json: &mut Option<ResMut<RulesJsonState>>,
) {
    let Some(state) = state.as_deref_mut() else {
        return;
    };
    if state.0.tab() == tab {
        return;
    }
    state.0.select_tab(tab);
    if let Some(view) = view.as_deref_mut() {
        view.tab = tab;
    }
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
        let model = RulesPageModelState::default();
        assert_eq!(model.0.tab(), RulesTab::default());
        assert_eq!(model.0.tab(), RulesTab::List);
        assert_eq!(Lang("zh-CN").tr(RulesTab::List.i18n_key()), "规则列表");
        assert_eq!(
            Lang("zh-CN").tr(RulesTab::JsonEditors.i18n_key()),
            "JSON 编辑器"
        );
    }

    /// The strip is the shared segmented control: every partition owns one
    /// chip, and the chip's index is the shared partition index.
    #[test]
    fn every_partition_owns_one_chip_of_the_shared_segmented_control() {
        for tab in RulesTab::ALL {
            let chip = RulesTabChip(tab.index());
            assert_eq!(chip.0, tab.index());
            assert_eq!(RulesTab::from_index(chip.0), tab);
        }
    }
}
