//! Full immutable group rows from the shared owner; native language replay keeps entities.
use crate::pages::connections_view::{ConnAggregationSummary, ConnectionsViewState};
use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexWrap, Node, UiRect, percent, px,
};
use bevy::ui::widget::Text;
use infiltrator_application::connection_grouping::ConnectionGroupRow;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::SurfacePanel;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

#[derive(Component, Clone, Copy, Default)]
pub struct ConnectionGroupsRoot;
#[derive(Component, Clone, Debug, Default)]
pub struct ConnectionGroupCard(pub String);
#[derive(Resource, Default)]
struct RenderedGroups {
    root: Option<Entity>,
    rows: Vec<ConnectionGroupRow>,
}

pub struct ConnectionsGroupsPlugin;
impl Plugin for ConnectionsGroupsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderedGroups>()
            .add_systems(Update, replay_groups);
    }
}
fn row_scene(row: &ConnectionGroupRow, palette: &UiPalette) -> impl Scene + use<> {
    let row = row.clone();
    let bg = palette.surface;
    bsn! {
        Node {width:percent(100),flex_wrap:FlexWrap::Wrap,align_items:AlignItems::Center,column_gap:px(space::S12),row_gap:px(space::S4),padding:UiRect::all(px(space::S8)),border_radius:BorderRadius::all(px(4.0))}
        BackgroundColor(bg) SurfacePanel
        ConnectionGroupCard({row.key.clone()})
        Children [
            Node {flex_grow:1.0,min_width:px(0.0)}
            Text({row.key.clone()}) TextRole(Role::BodyStrong)
            --
            LocalizedText::new("conn_aggregate_count",vec![("count",row.count.to_string())]) TextRole(Role::Caption)
            --
            Text({row.traffic.clone()}) TextRole(Role::Mono)
        ]
    }
}
#[derive(SystemParam)]
struct GroupPresentation<'w, 's> {
    view: Res<'w, ConnectionsViewState>,
    rendered: ResMut<'w, RenderedGroups>,
    roots: Query<'w, 's, Entity, With<ConnectionGroupsRoot>>,
    summaries: Query<'w, 's, &'static mut LocalizedText, With<ConnAggregationSummary>>,
    palette: Res<'w, UiPalette>,
}
fn replay_groups(presentation: GroupPresentation, mut commands: Commands) {
    let GroupPresentation {
        view,
        mut rendered,
        roots,
        mut summaries,
        palette,
    } = presentation;
    let copy = LocalizedText::new(
        view.groups.summary_key(),
        vec![("count", view.groups.rows().len().to_string())],
    );
    for mut label in &mut summaries {
        if label.key != copy.key || label.params != copy.params {
            *label = copy.clone();
        }
    }
    let Ok(root) = roots.single() else {
        rendered.root = None;
        return;
    };
    let rows = view.groups.rows();
    if rendered.root == Some(root) && rendered.rows == rows {
        return;
    }
    commands.entity(root).despawn_children();
    for row in rows {
        commands
            .spawn_scene(row_scene(row, &palette))
            .insert(ChildOf(root));
    }
    rendered.root = Some(root);
    rendered.rows = rows.to_vec();
}
