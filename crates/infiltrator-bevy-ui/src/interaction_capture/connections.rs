//! Native grouping activation observes complete shared facts and actual query input.
use super::geometry::CaptureGeometry;
use super::scroll::request_scroll;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::pages::connections::ConnHostText;
use crate::pages::connections::{ConnAggregationPill, ConnectionsScrollArea};
use crate::pages::connections_groups::{ConnectionGroupCard, ConnectionGroupsRoot};
use crate::pages::connections_view::{ConnectionRow, ConnectionsViewState};
use crate::route::{ActiveRoute, Route};
use crate::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use bevy::app::App;
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerPress};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use bevy::window::{PrimaryWindow, Window};
use infiltrator_application::connection_grouping_fixtures::grouping_surface;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_contract::parity::FeatureId;
use infiltrator_domain::connection_view::ConnectionGroupingMode;

fn query(feature: FeatureId) -> &'static str {
    if feature == FeatureId::ConnectionsSearchHighlight {
        "API-0"
    } else {
        "CLIENT-0"
    }
}
fn capture_selected(feature: &InteractionCapture, route: &ActiveRoute) -> bool {
    matches!(
        feature.feature,
        FeatureId::ConnectionsGrouping | FeatureId::ConnectionsSearchHighlight
    ) && selected(feature, route, feature.feature, Route::Connections)
}
#[derive(Resource, Default)]
struct GroupCapture(u8);
pub fn install(app: &mut App) {
    app.init_resource::<GroupCapture>();
}
#[derive(SystemParam)]
pub struct GroupControls<'w, 's> {
    capture: ResMut<'w, GroupCapture>,
    route: Res<'w, ActiveRoute>,
    view: Res<'w, ConnectionsViewState>,
    latest: Res<'w, LatestSurfaceSnapshot>,
    pills: Query<'w, 's, (Entity, &'static ConnAggregationPill)>,
    fields: Query<'w, 's, (Entity, &'static NativeTextField, &'static TextField)>,
    groups: Query<'w, 's, Entity, With<ConnectionGroupsRoot>>,
    rows: Query<'w, 's, (Entity, &'static ConnectionRow)>,
    hosts: Query<'w, 's, (Entity, &'static ConnHostText)>,
    colors: Query<'w, 's, &'static TextColor>,
    palette: Res<'w, UiPalette>,
    cards: Query<'w, 's, (&'static ConnectionGroupCard, &'static Children)>,
    texts: Query<'w, 's, &'static Text>,
    scroll: Query<'w, 's, Entity, With<ConnectionsScrollArea>>,
    windows: Query<'w, 's, (Entity, &'static Window), With<PrimaryWindow>>,
}
pub fn activate(
    mut controls: GroupControls,
    feature: Res<InteractionCapture>,
    geometry: CaptureGeometry,
    mut commands: Commands,
) {
    if !capture_selected(&feature, &controls.route) {
        return;
    }
    match controls.capture.0 {
        0 => {
            commands.trigger(SurfaceSnapshotUpdated(grouping_surface(
                controls.latest.0.clone(),
            )));
            controls.capture.0 = 1;
        }
        1 => {
            if let Some((entity, _)) = controls.pills.iter().find(|(_, pill)| {
                pill.0
                    == if feature.feature == FeatureId::ConnectionsSearchHighlight {
                        ConnectionGroupingMode::Flat
                    } else {
                        ConnectionGroupingMode::ByProcess
                    }
            }) {
                commands.trigger(Activate { entity });
                controls.capture.0 = 2;
            }
        }
        2 => {
            let Some((entity, _, _)) = controls.fields.iter().find(|(_, field, _)| field.0 == 10)
            else {
                return;
            };
            let (Ok(scroll), Ok((_, window))) =
                (controls.scroll.single(), controls.windows.single())
            else {
                return;
            };
            let (Some(bounds), Some(viewport)) = (geometry.rect(entity), geometry.rect(scroll))
            else {
                return;
            };
            if !request_scroll(&mut commands, scroll, bounds, viewport) {
                return;
            }
            commands.trigger(PointerPress {
                entity,
                pointer: Pointer::new(
                    PointerId::Mouse,
                    Location {
                        target: NormalizedRenderTarget::None {
                            width: window.width() as u32,
                            height: window.height() as u32,
                        },
                        position: Vec2::new(
                            bounds[0] + bounds[2] / 2.0,
                            bounds[1] + bounds[3] / 2.0,
                        ),
                    },
                ),
                button: PointerButton::Primary,
                hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
                count: 1,
            });
            controls.capture.0 = 3;
        }
        3 => {
            let Ok((window, _)) = controls.windows.single() else {
                return;
            };
            commands.write_message(KeyboardInput {
                key_code: KeyCode::KeyM,
                logical_key: Key::Character(query(feature.feature).into()),
                text: Some(query(feature.feature).into()),
                state: ButtonState::Pressed,
                repeat: false,
                window,
            });
            controls.capture.0 = 4;
        }
        _ => {
            let target = if feature.feature == FeatureId::ConnectionsSearchHighlight {
                controls
                    .rows
                    .iter()
                    .find(|(_, row)| row.0 == 0)
                    .map(|(entity, _)| entity)
            } else {
                controls.groups.single().ok()
            };
            if let (Some(target), Ok(scroll)) = (target, controls.scroll.single())
                && let (Some(bounds), Some(viewport)) =
                    (geometry.rect(target), geometry.rect(scroll))
            {
                request_scroll(&mut commands, scroll, bounds, viewport);
            }
        }
    }
}
pub fn observe(
    controls: GroupControls,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observed: ResMut<ObservedInteraction>,
) {
    if !capture_selected(&feature, &controls.route) || controls.capture.0 != 4 {
        return;
    }
    if feature.feature == FeatureId::ConnectionsSearchHighlight {
        let bounds = search_bounds(&controls, &mut geometry);
        observed.publish(&mut feature, bounds);
        return;
    }
    let state = &controls.view.groups;
    let Some((field, _, text)) = controls.fields.iter().find(|(_, field, _)| field.0 == 10) else {
        return;
    };
    if text.0.text() != query(feature.feature)
        || state.mode() != ConnectionGroupingMode::ByProcess
        || state.rows().len() != 1
        || state.rows()[0].key != "client-0"
        || state.rows()[0].count != 2
        || state.rows()[0].traffic != "↑ 2.00 KB / ↓ 4.00 KB"
    {
        return;
    }
    if controls.latest.0.shell_readout.connections.value != Some(9)
        || !controls.latest.0.shell_readout.connections.current
        || controls.latest.0.core.active_connections != 9
    {
        return;
    }
    let Ok((card, children)) = controls.cards.single() else {
        return;
    };
    if card.0 != "client-0"
        || !children.iter().any(|child| {
            controls
                .texts
                .get(*child)
                .is_ok_and(|text| text.0 == "↑ 2.00 KB / ↓ 4.00 KB")
        })
    {
        return;
    }
    let bounds = (|| {
        geometry.bounds(field, "connections-group-search")?;
        for (entity, _) in &controls.pills {
            geometry.bounds(entity, "connections-group-control")?;
        }
        geometry.bounds(controls.groups.single().ok()?, "connections-group-results")
    })();
    observed.publish(&mut feature, bounds);
}

fn search_bounds(controls: &GroupControls, geometry: &mut CaptureGeometry) -> Option<[f32; 4]> {
    let state = &controls.view.groups;
    if !state.mode().is_flat()
        || state.matched_count() != 2
        || state.source_count() != 9
        || !state.source_current()
    {
        return None;
    }
    let (field, _, text) = controls.fields.iter().find(|(_, field, _)| field.0 == 10)?;
    if text.0.text() != "API-0" || controls.latest.0.shell_readout.connections.value != Some(9) {
        return None;
    }
    geometry.bounds(field, "connections-search-input")?;
    let (host, _) = controls.hosts.iter().find(|(_, marker)| marker.0 == 0)?;
    let root = controls.texts.get(host).ok()?;
    if root.0 != "api-0" || controls.colors.get(host).ok()?.0 != controls.palette.accent {
        return None;
    }
    // The root is the matched prefix; native spans carry the untouched remaining bytes.
    let (row, _) = controls.rows.iter().find(|(_, marker)| marker.0 == 0)?;
    geometry.bounds(host, "connections-highlight-text")?;
    geometry.bounds(row, "connections-search-result")
}
