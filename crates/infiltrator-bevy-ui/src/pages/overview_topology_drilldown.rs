//! The Overview page's topology drilldown panel (UI-04-05).
//!
//! Activating a topology stage opens a drilldown **on the Overview page**:
//! the activated stage's own facts, the shared drilldown keyword and the
//! shared navigation target are rendered here. The business therefore shows
//! its result on its own page instead of only routing away to another one.
//!
//! The drilldown state itself is the shared [`TopologyDrilldownFilter`]; this
//! module only renders it and publishes the two typed intents the panel
//! offers — navigate to the shared target, or close the drilldown.

use crate::pages::overview::{LastOverviewProjection, OverviewScrubberStatus};
use crate::pages::overview_speedtest::{
    OverviewSpeedtestConcurrencyText, OverviewSpeedtestDeadText, OverviewSpeedtestDetailBodyText,
    OverviewSpeedtestHistoryText, OverviewSpeedtestMetricsText, OverviewSpeedtestText,
};
use crate::pages::overview_topology::TopologyDrilldownFilter;
use crate::route::{Route, RouteChanged};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BorderRadius, Display, FlexDirection, JustifyContent, Node, UiRect, percent,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::traffic_topology::{
    TrafficTopologyNavigationTarget, TrafficTopologyStage,
};
use infiltrator_presentation::traffic_topology_projection::navigation_target_for_stage;

/// The drilldown panel root: the Overview page's own topology result.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TopologyDrilldownPanel;

/// The drilldown's result line: the activated stage and its own facts.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TopologyDrilldownText;

/// The drilldown's shared keyword line (the injected filter query).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TopologyDrilldownKeywordText;

/// The control that navigates to the shared navigation target of the stage.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TopologyDrilldownNavigateButton;

/// The control that closes the drilldown.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TopologyDrilldownCloseButton;

/// The drilldown panel, mounted inside the Overview topology card.
///
/// The panel starts hidden: a drilldown only exists once a stage was
/// activated, and the shared filter is the only thing that says so.
pub fn drilldown_panel_scene(palette: &UiPalette) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                width: percent(100),
                display: Display::None,
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S6),
                padding: UiRect::all(rem(space::S8)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            TopologyDrilldownPanel
            Children [
                Text({ String::new() }) TopologyDrilldownText TextRole(Role::BodyStrong)
                --
                Text({ String::new() }) TopologyDrilldownKeywordText TextRole(Role::Mono)
                --
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::FlexEnd,
                    column_gap: rem(space::S8),
                }
                Children [
                    Node {
                        min_height: rem(palette.control_height_px),
                        padding: UiRect::horizontal(rem(space::S12)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                    }
                    ThemeBackgroundColor({ Token::Accent })
                    TopologyDrilldownNavigateButton
                    Button
                    Children [
                        Text({ "→".to_owned() }) TextRole(Role::Body)
                    ]
                    --
                    Node {
                        min_height: rem(palette.control_height_px),
                        padding: UiRect::horizontal(rem(space::S12)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                    }
                    ThemeBackgroundColor({ Token::SurfaceElevated })
                    TopologyDrilldownCloseButton
                    Button
                    Children [
                        Text({ "×".to_owned() }) TextRole(Role::Body)
                    ]
                ]
            ]
    })
}

/// Restamp the drilldown panel from the shared drilldown filter.
///
/// Disjoint filter for the drilldown panel's own lines.
type DrilldownTextFilter = (
    With<TopologyDrilldownText>,
    Without<TopologyDrilldownKeywordText>,
    Without<OverviewScrubberStatus>,
    Without<OverviewSpeedtestText>,
    Without<OverviewSpeedtestMetricsText>,
    Without<OverviewSpeedtestDeadText>,
    Without<OverviewSpeedtestHistoryText>,
    Without<OverviewSpeedtestConcurrencyText>,
    Without<OverviewSpeedtestDetailBodyText>,
);

/// Disjoint filter for the drilldown panel's shared keyword line.
type DrilldownKeywordTextFilter = (
    With<TopologyDrilldownKeywordText>,
    Without<TopologyDrilldownText>,
    Without<OverviewScrubberStatus>,
    Without<OverviewSpeedtestText>,
    Without<OverviewSpeedtestMetricsText>,
    Without<OverviewSpeedtestDeadText>,
    Without<OverviewSpeedtestHistoryText>,
    Without<OverviewSpeedtestConcurrencyText>,
    Without<OverviewSpeedtestDetailBodyText>,
);

/// The panel is the Overview page's own answer to a stage activation: the
/// stage's facts, the shared keyword and the shared navigation target. A
/// drilldown that was never opened leaves the panel hidden.
pub fn sync_topology_drilldown(
    drilldown: Option<Res<TopologyDrilldownFilter>>,
    last: Option<Res<LastOverviewProjection>>,
    mut panels: Query<&mut Node, With<TopologyDrilldownPanel>>,
    // The drilldown writes its own texts; every other text of the overview
    // belongs to another system, so the queries are disjoint (the same
    // discipline the speedtest's text filters use).
    mut texts: Query<&mut Text, DrilldownTextFilter>,
    mut keywords: Query<&mut Text, DrilldownKeywordTextFilter>,
    mut buttons: Query<&mut ButtonDisabled, With<TopologyDrilldownNavigateButton>>,
) {
    let stage = drilldown.as_ref().and_then(|drilldown| drilldown.stage);
    let Some(stage) = stage else {
        for mut panel in &mut panels {
            if panel.display != Display::None {
                panel.display = Display::None;
            }
        }
        return;
    };

    for mut panel in &mut panels {
        if panel.display != Display::Flex {
            panel.display = Display::Flex;
        }
    }

    let result = stage_result(stage, last.as_deref());
    for mut text in &mut texts {
        if text.0 != result {
            text.0 = result.clone();
        }
    }
    let keyword = drilldown
        .as_ref()
        .and_then(|drilldown| drilldown.filter_query.clone())
        .unwrap_or_default();
    for mut text in &mut keywords {
        if text.0 != keyword {
            text.0 = keyword.clone();
        }
    }
    let reachable = navigation_target_for_stage(stage).is_some();
    for mut button in &mut buttons {
        if button.0 == reachable {
            button.0 = !reachable;
        }
    }
}

/// The activated stage's own result line: its label and its detail.
fn stage_result(stage: TrafficTopologyStage, last: Option<&LastOverviewProjection>) -> String {
    let topology = last
        .and_then(|last| last.0.as_ref())
        .map(|projection| &projection.traffic_topology);
    let Some(node) = topology.and_then(|snapshot| snapshot.node(stage)) else {
        return stage.as_str().to_owned();
    };
    if node.detail.is_empty() {
        node.label.clone()
    } else {
        format!("{} · {}", node.label, node.detail)
    }
}

/// Navigate to the shared navigation target of the open drilldown.
pub(crate) fn on_drilldown_navigate_activated(
    activate: On<Activate>,
    buttons: Query<(), With<TopologyDrilldownNavigateButton>>,
    drilldown: Option<Res<TopologyDrilldownFilter>>,
    mut commands: Commands,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let Some(stage) = drilldown.as_ref().and_then(|drilldown| drilldown.stage) else {
        return;
    };
    let Some(target) = navigation_target_for_stage(stage) else {
        return;
    };
    commands.trigger(RouteChanged(route_for_target(target)));
}

/// Close the drilldown: the shared filter is cleared, so the panel hides.
pub(crate) fn on_drilldown_close_activated(
    activate: On<Activate>,
    buttons: Query<(), With<TopologyDrilldownCloseButton>>,
    mut drilldown: Option<ResMut<TopologyDrilldownFilter>>,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    if let Some(ref mut drilldown) = drilldown {
        drilldown.stage = None;
        drilldown.filter_query = None;
    }
}

/// The route of one shared navigation target.
fn route_for_target(target: TrafficTopologyNavigationTarget) -> Route {
    match target {
        TrafficTopologyNavigationTarget::Settings => Route::Settings,
        TrafficTopologyNavigationTarget::Rules => Route::Rules,
        TrafficTopologyNavigationTarget::Proxies => Route::Proxies,
    }
}

/// The shared navigation target of one stage, for the page's own tests.
pub fn navigation_target(stage: TrafficTopologyStage) -> Option<TrafficTopologyNavigationTarget> {
    navigation_target_for_stage(stage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_stage_maps_to_a_shared_navigation_target() {
        for stage in TrafficTopologyStage::ALL {
            assert!(
                navigation_target(stage).is_some(),
                "{stage:?} has a shared navigation target"
            );
        }
    }

    #[test]
    fn the_shared_targets_map_onto_the_page_vocabulary() {
        assert_eq!(
            route_for_target(TrafficTopologyNavigationTarget::Settings),
            Route::Settings
        );
        assert_eq!(
            route_for_target(TrafficTopologyNavigationTarget::Rules),
            Route::Rules
        );
        assert_eq!(
            route_for_target(TrafficTopologyNavigationTarget::Proxies),
            Route::Proxies
        );
    }
}
