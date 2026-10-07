//! Native scroll position follows the shared policy after layout, without global ECS access.
use crate::pages::logs::{LogsPageRoot, PauseLogsButton};
use crate::pages::logs_search::LogsViewState;
use bevy::app::{App, Plugin, PostUpdate};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, With};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Local, Query, ResMut};
use bevy::ui::{ComputedNode, ScrollPosition, UiSystems};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::localization::LocalizedText;

#[derive(Component, Clone, Copy, Default)]
pub struct LogFollowLabel;
#[derive(QueryData)]
#[query_data(mutable)]
struct FollowViewport {
    entity: Entity,
    computed: &'static ComputedNode,
    position: &'static mut ScrollPosition,
}
pub struct LogsFollowPlugin;
impl Plugin for LogsFollowPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(toggle_follow)
            .add_systems(PostUpdate, replay.after(UiSystems::PostLayout));
    }
}
fn toggle_follow(
    activate: On<Activate>,
    buttons: Query<(), With<PauseLogsButton>>,
    mut view: ResMut<LogsViewState>,
) {
    if buttons.contains(activate.entity) {
        view.0.follow.toggle_follow();
    }
}
fn replay(
    mut view: ResMut<LogsViewState>,
    mut roots: Query<FollowViewport, With<LogsPageRoot>>,
    mut labels: Query<&mut LocalizedText, With<LogFollowLabel>>,
    mut visuals: Query<&mut ControlVisual, With<PauseLogsButton>>,
    mut previous_root: Local<Option<Entity>>,
) {
    for mut viewport in &mut roots {
        let scale = viewport.computed.inverse_scale_factor;
        let visible = viewport.computed.size().y * scale;
        let content = viewport.computed.content_size().y * scale;
        if visible <= 0.0 {
            continue;
        }
        let extent = (content - visible).max(0.0);
        if *previous_root != Some(viewport.entity) {
            viewport.position.y = view.0.follow.restored_offset(extent);
            *previous_root = Some(viewport.entity);
        } else {
            view.0
                .follow
                .observe_viewport(viewport.position.y, content, visible);
        }
        if view.0.follow.should_follow() {
            viewport.position.y = extent;
        }
        view.0
            .follow
            .observe_viewport(viewport.position.y, content, visible);
    }
    for mut label in &mut labels {
        *label = LocalizedText::plain(view.0.follow.label_key());
    }
    for mut visual in &mut visuals {
        visual.0 = !view.0.follow.should_follow();
    }
}
