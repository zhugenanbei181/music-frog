//! Reconcile row topology at the deferred scene boundary; stable rows restamp in place.
use crate::pages::connections::{
    CloseConnectionButton, ConnChainHopText, ConnSpeedText, ConnectionsProjectionUpdated,
};
use crate::pages::connections_row::connection_row_scene;
use crate::pages::connections_view::{
    ConnRowsContainer, ConnectionRow, ConnectionsViewState, apply_connection_row_order,
};
use crate::pages::connections_virtual::ConnectionsVirtualNodes;
use crate::pages::overview::format_rate;
use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, QueryFilter, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::scene::CommandsSceneExt;
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_contract::connection::ConnectionStreamPhase;
use infiltrator_contract::surface_snapshot::ConnectionSnapshot;
use infiltrator_domain::connection_view;

#[derive(Component, Default)]
pub struct ConnectionRowText;
#[derive(QueryData)]
#[query_data(mutable)]
struct RowCopy {
    text: &'static mut Text,
    speed: Option<&'static ConnSpeedText>,
    chain: Option<&'static ConnChainHopText>,
}

#[derive(Resource, Default)]
struct RowTopology {
    root: Option<Entity>,
    identities: Vec<(String, usize)>,
}
pub struct ConnectionsRowsPlugin;
impl Plugin for ConnectionsRowsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RowTopology>().add_observer(reconcile);
    }
}
#[derive(SystemParam)]
struct RowSurface<'w, 's> {
    roots: Query<'w, 's, Entity, With<ConnRowsContainer>>,
    containers: Query<'w, 's, &'static mut Children, With<ConnRowsContainer>>,
    subtrees: Query<'w, 's, &'static Children, Without<ConnRowsContainer>>,
    markers: Query<'w, 's, &'static ConnectionRow>,
    copies: Query<'w, 's, RowCopy, With<ConnectionRowText>>,
    buttons: Query<'w, 's, &'static mut CloseConnectionButton>,
    virtual_containers: Query<'w, 's, (), With<ConnectionsVirtualNodes>>,
    view: Res<'w, ConnectionsViewState>,
    palette: Res<'w, UiPalette>,
    topology: ResMut<'w, RowTopology>,
}

/// Restamp the mounted rows in place from the live projection: the shared
/// reducers own the values, the markers keep pointing at the same connection.
fn restamp_live_rows<F: QueryFilter>(
    rows: &[ConnectionSnapshot],
    copies: &mut Query<RowCopy, F>,
    buttons: &mut Query<&mut CloseConnectionButton>,
) {
    for mut copy in copies.iter_mut() {
        let value = if let Some(speed) = copy.speed {
            rows.get(speed.0).map(|row| {
                format!(
                    "↑ {}  ↓ {}",
                    format_rate(row.upload_bps),
                    format_rate(row.download_bps)
                )
            })
        } else if let Some(chain) = copy.chain {
            rows.get(chain.row)
                .map(connection_view::route_chain)
                .and_then(|route| route.hops().get(chain.hop).cloned())
        } else {
            None
        };
        if let Some(value) = value {
            copy.text.0 = value;
        }
    }
    for mut button in buttons.iter_mut() {
        if let Some(row) = rows.get(button.connection_idx) {
            button.connection_id = row.id.clone();
        }
    }
}

fn reconcile(
    update: On<ConnectionsProjectionUpdated>,
    surface: RowSurface,
    mut commands: Commands,
) {
    if update.0.stream_phase != ConnectionStreamPhase::Live {
        return;
    }
    let RowSurface {
        roots,
        mut containers,
        subtrees,
        markers,
        mut copies,
        mut buttons,
        virtual_containers,
        view,
        palette,
        mut topology,
    } = surface;
    let Ok(root) = roots.single() else {
        topology.root = None;
        return;
    };
    let rows = &update.0.connections;
    if !virtual_containers.is_empty() {
        // BANDROID-010: the recycler owns the row topology whenever its
        // container is mounted, so the full-mount reconcile must not respawn
        // the whole collection. Mounted rows still restamp in place.
        restamp_live_rows(rows, &mut copies, &mut buttons);
        return;
    }
    let identities: Vec<_> = rows
        .iter()
        .map(|row| (row.id.clone(), connection_view::route_chain(row).len()))
        .collect();
    if topology.root == Some(root) && topology.identities == identities {
        restamp_live_rows(rows, &mut copies, &mut buttons);
        apply_connection_row_order(&update.0, view.sort, &mut containers, &markers, &subtrees);
        return;
    }
    commands.entity(root).despawn_children();
    let mut sorted = rows.clone();
    connection_view::sort_connections(&mut sorted, view.sort);
    for row in &sorted {
        let index = rows
            .iter()
            .position(|source| source.id == row.id)
            .expect("sorted source identity");
        commands
            .spawn_scene(connection_row_scene(index, row, &palette))
            .insert(ChildOf(root));
    }
    topology.root = Some(root);
    topology.identities = identities;
}
