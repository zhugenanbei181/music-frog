//! test-intent: behavior
//! Native outbound controls use typed IDs and replay accepted preference observations.
use crate::command_harness::TypedOutboundPreferenceSink;
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerCancel, PointerClick, PointerPress};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::text::TextColor;
use bevy::ui::BackgroundColor;
use bevy::ui::widget::Text;
use bevy::ui::{Display, Node, Pressed};
use bevy::ui_widgets::{Activate, Button, ButtonPlugin};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::pages::proxies::{
    FilterAliveToggle, GroupNodesContainer, LastProxiesProjection, NodePinButton,
    ProxyGroupFoldButton, ProxyNodeButton, ToggleViewModeButton,
};
use infiltrator_bevy_ui::pages::proxies_identity::{ProxyCardsContainer, ProxyGroupIdentity};
use infiltrator_bevy_ui::pages::proxies_preferences::{
    FilterAliveIndicator, ProxyFavoriteStar, ProxyViewLabel,
};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{
    DemoSurfaceSource, LatestSurfaceSnapshot, SurfaceSnapshotUpdated, SurfaceSource,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::theme::{ThemePreference, ThemeSkin};
use lurker_core_contract::outbound::{
    details::OutboundSnapshot,
    identity::{OutboundId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionSet,
        OutboundSelectionState, SelectionAvailability,
    },
    preferences::{
        OutboundGroupExpansion, OutboundHealthFilter, OutboundLayout, OutboundUiPreferenceIntent,
        OutboundUiPreferenceRequest,
    },
};
use lurker_core_contract::page::PageData;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread::yield_now;
use std::time::{Duration, Instant};

fn app_with_preferences() -> (App, TypedOutboundPreferenceSink) {
    let source = DemoSurfaceSource::running();
    let snapshot = source.surface_snapshot();
    let preferences = snapshot
        .outbound_ui_preferences
        .data
        .clone()
        .expect("demo has typed outbound preferences");
    let sink = TypedOutboundPreferenceSink::new(preferences);
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::new_with_width(
        ThemePreference::Fixed(ThemeSkin::Dark),
        1180.0,
    ));
    app.add_plugins(PagesPlugin::new_surface(source));
    app.add_plugins(CommandPumpPlugin::new(Arc::new(sink.clone())));
    app.update();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();
    (app, sink)
}

fn publish(app: &mut App, sink: &TypedOutboundPreferenceSink, mut snapshot: SurfaceSnapshot) {
    snapshot.revision = app
        .world()
        .resource::<LatestSurfaceSnapshot>()
        .0
        .revision
        .saturating_add(1);
    snapshot.outbound_ui_preferences = PageData::ready(sink.preferences());
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    app.update();
}

fn apply_preference(
    app: &mut App,
    sink: &TypedOutboundPreferenceSink,
    intent: OutboundUiPreferenceIntent,
) {
    let snapshot = &app.world().resource::<LatestSurfaceSnapshot>().0;
    let revision = snapshot
        .outbound_ui_preferences
        .data
        .as_ref()
        .expect("accepted typed preference snapshot")
        .revision();
    let inventory_revision = snapshot
        .outbound_inventory
        .data
        .as_ref()
        .expect("accepted typed inventory snapshot")
        .revision();
    let request = OutboundUiPreferenceRequest::new(revision, intent).unwrap();
    if let OutboundUiPreferenceIntent::ReorderGroups {
        inventory_revision: expected,
        ..
    }
    | OutboundUiPreferenceIntent::ResetGroupOrder {
        inventory_revision: expected,
    } = request.intent()
    {
        assert_eq!(*expected, inventory_revision);
    }
    let request_id = sink.submit_tracked(UiCommand::ApplyOutboundUiPreference { request });
    assert!(request_id.is_some(), "typed preference command is tracked");
    app.update();
}

fn inventory_parts(
    snapshot: &SurfaceSnapshot,
) -> (Vec<OutboundSnapshot>, Vec<OutboundSelectionSet>) {
    let inventory = snapshot
        .outbound_inventory
        .data
        .as_ref()
        .expect("demo has a typed outbound inventory");
    match inventory.inventory() {
        OutboundInventoryState::Ready {
            entries,
            collections,
        } => (entries.clone(), collections.clone()),
        _ => panic!("demo outbound inventory is ready"),
    }
}

fn update_inventory(
    snapshot: &mut SurfaceSnapshot,
    update: impl FnOnce(&mut Vec<OutboundSnapshot>, &mut Vec<OutboundSelectionSet>),
) {
    let current = snapshot
        .outbound_inventory
        .data
        .as_ref()
        .expect("typed inventory is observed");
    let scope = current.scope().clone();
    let revision = current.revision().value() + 1;
    let (mut entries, mut collections) = inventory_parts(snapshot);
    update(&mut entries, &mut collections);
    let next = OutboundInventorySnapshot::new(
        scope,
        SnapshotRevision::new(revision),
        OutboundInventoryState::Ready {
            entries,
            collections,
        },
    )
    .expect("updated typed inventory is valid");
    snapshot.outbound_inventory = PageData::ready(next);
}

fn collection_ids(snapshot: &SurfaceSnapshot) -> Vec<OutboundId> {
    inventory_parts(snapshot)
        .1
        .into_iter()
        .map(|collection| collection.id)
        .collect()
}

fn outbound_id_for_name(snapshot: &SurfaceSnapshot, name: &str) -> OutboundId {
    inventory_parts(snapshot)
        .0
        .into_iter()
        .find(|entry| entry.display_name.as_deref() == Some(name))
        .expect("fixture contains the named outbound")
        .id
}

fn nodes(app: &mut App) -> HashMap<(OutboundId, OutboundId), Entity> {
    app.world_mut()
        .query::<(Entity, &ProxyNodeButton)>()
        .iter(app.world())
        .filter_map(|(entity, button)| {
            Some((
                (button.collection_id.clone()?, button.outbound_id.clone()?),
                entity,
            ))
        })
        .collect()
}

fn visible(app: &mut App, group: &OutboundId) -> bool {
    app.world_mut()
        .query::<(&Node, &GroupNodesContainer, &ProxyGroupIdentity)>()
        .iter(app.world())
        .find(|(_, _, identity)| identity.0 == group.value())
        .unwrap()
        .0
        .display
        == Display::Flex
}

fn find_group(app: &App, entity: Entity) -> Option<String> {
    if let Some(identity) = app.world().get::<ProxyGroupIdentity>(entity) {
        return Some(identity.0.clone());
    }
    app.world()
        .get::<Children>(entity)?
        .iter()
        .find_map(|child| find_group(app, *child))
}

#[test]
fn native_fold_round_trip_executes_the_typed_preference_and_preserves_node_entities() {
    let (mut app, sink) = app_with_preferences();
    let base = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    let group = collection_ids(&base)[0].clone();
    let originals = nodes(&mut app);
    let button = app
        .world_mut()
        .query::<(Entity, &ProxyGroupFoldButton)>()
        .iter(app.world())
        .find(|(_, button)| button.collection_id.as_ref() == Some(&group))
        .unwrap()
        .0;
    for expected in [false, true] {
        app.world_mut()
            .commands()
            .trigger(Activate { entity: button });
        app.update();
        let deadline = Instant::now() + Duration::from_secs(2);
        while sink.preferences().collapsed_group_ids.contains(&group) != !expected {
            assert!(
                Instant::now() < deadline,
                "typed fold command must complete"
            );
            app.update();
            yield_now();
        }
        publish(&mut app, &sink, base.clone());
        assert_eq!(visible(&mut app, &group), expected);
        assert_eq!(nodes(&mut app), originals);
    }
    assert!(sink.commands().iter().all(|command| matches!(
        command,
        UiCommand::ApplyOutboundUiPreference { request }
            if matches!(request.intent(), OutboundUiPreferenceIntent::SetGroupExpansion { group_id, .. } if group_id == &group)
    )));
}

#[test]
fn native_health_and_layout_controls_replay_the_observed_typed_preferences() {
    let (mut app, sink) = app_with_preferences();
    let filter = app
        .world_mut()
        .query::<(Entity, &FilterAliveToggle)>()
        .single(app.world())
        .unwrap()
        .0;
    let compact = app
        .world_mut()
        .query::<(Entity, &ToggleViewModeButton)>()
        .single(app.world())
        .unwrap()
        .0;
    let indicator = app
        .world_mut()
        .query::<(Entity, &FilterAliveIndicator)>()
        .single(app.world())
        .unwrap()
        .0;
    let label = app
        .world_mut()
        .query::<(Entity, &ProxyViewLabel)>()
        .single(app.world())
        .unwrap()
        .0;
    for expected in [true, false] {
        app.world_mut()
            .commands()
            .trigger(Activate { entity: filter });
        app.update();
        let deadline = Instant::now() + Duration::from_secs(2);
        while (sink.preferences().health_filter == OutboundHealthFilter::HealthyOnly) != expected {
            assert!(
                Instant::now() < deadline,
                "typed health-filter command must complete"
            );
            app.update();
            yield_now();
        }
        let observed = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
        publish(&mut app, &sink, observed);
        app.world_mut()
            .commands()
            .trigger(Activate { entity: compact });
        app.update();
        let deadline = Instant::now() + Duration::from_secs(2);
        while (sink.preferences().layout == OutboundLayout::CompactList) != expected {
            assert!(
                Instant::now() < deadline,
                "typed layout command must complete"
            );
            app.update();
            yield_now();
        }
        let observed = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
        publish(&mut app, &sink, observed);
        let palette = app.world().resource::<UiPalette>();
        let color = if expected {
            palette.accent_container
        } else {
            palette.surface_elevated
        };
        assert_eq!(
            app.world().get::<BackgroundColor>(indicator).unwrap().0,
            color
        );
        assert_eq!(
            app.world().get::<Text>(label).unwrap().0,
            if expected {
                "紧凑列表"
            } else {
                "网格视图"
            }
        );
        assert!(app.world().get::<FilterAliveToggle>(filter).is_some());
        assert!(app.world().get::<ToggleViewModeButton>(compact).is_some());
    }
}

#[test]
fn reordered_changed_and_removed_collections_keep_typed_node_identity_and_reject_stale_controls() {
    let (mut app, sink) = app_with_preferences();
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    let original_ids = collection_ids(&snapshot);
    let removed_group = original_ids[0].clone();
    let original_nodes = nodes(&mut app);
    let old_button = app
        .world_mut()
        .query::<(Entity, &ProxyGroupFoldButton)>()
        .iter(app.world())
        .find(|(_, button)| button.collection_id.as_ref() == Some(&removed_group))
        .unwrap()
        .0;

    let reversed: Vec<_> = original_ids.iter().rev().cloned().collect();
    apply_preference(
        &mut app,
        &sink,
        OutboundUiPreferenceIntent::ReorderGroups {
            group_ids: reversed.clone(),
            inventory_revision: snapshot
                .outbound_inventory
                .data
                .as_ref()
                .expect("typed inventory")
                .revision(),
        },
    );
    publish(&mut app, &sink, snapshot.clone());
    assert_eq!(nodes(&mut app), original_nodes);
    let container = app
        .world_mut()
        .query::<(Entity, &ProxyCardsContainer)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0;
    let order = app
        .world()
        .get::<Children>(container)
        .unwrap()
        .iter()
        .filter_map(|entity| find_group(&app, *entity))
        .collect::<Vec<_>>();
    assert_eq!(
        order,
        reversed
            .iter()
            .map(|group| group.value().to_owned())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        app.world()
            .get::<ProxyGroupFoldButton>(old_button)
            .unwrap()
            .collection_id,
        Some(removed_group.clone())
    );

    let renamed_id = outbound_id_for_name(&snapshot, "HK-01");
    let engine = snapshot.core.scope().unwrap().identity.engine.clone();
    let added_node = OutboundId::new(engine.clone(), "new-live-node-id").unwrap();
    update_inventory(&mut snapshot, |entries, collections| {
        entries
            .iter_mut()
            .find(|entry| entry.id == renamed_id)
            .unwrap()
            .display_name = Some("renamed-live-node".into());
        entries.push(OutboundSnapshot {
            id: added_node.clone(),
            display_name: Some("new-live-node".into()),
        });
        collections
            .iter_mut()
            .find(|collection| collection.id == removed_group)
            .unwrap()
            .member_ids
            .push(added_node.clone());
    });
    publish(&mut app, &sink, snapshot.clone());
    let updated_nodes = nodes(&mut app);
    for (key, entity) in &original_nodes {
        assert_eq!(updated_nodes[key], *entity);
    }
    assert!(updated_nodes.contains_key(&(removed_group.clone(), added_node.clone())));
    assert!(
        app.world()
            .resource::<LastProxiesProjection>()
            .0
            .as_ref()
            .unwrap()
            .groups
            .iter()
            .flat_map(|group| &group.proxies)
            .any(|node| node.outbound_id.as_ref() == Some(&renamed_id)
                && node.name == "renamed-live-node")
    );

    let added_group = OutboundId::new(engine, "new-live-collection").unwrap();
    let added_group_node =
        OutboundId::new(added_group.engine_id().clone(), "new-live-collection-node").unwrap();
    update_inventory(&mut snapshot, |entries, collections| {
        entries.push(OutboundSnapshot {
            id: added_group_node.clone(),
            display_name: Some("new-live-collection-node".into()),
        });
        collections.push(OutboundSelectionSet {
            id: added_group.clone(),
            member_ids: vec![added_group_node.clone()],
            availability: SelectionAvailability::Selectable,
            selection: OutboundSelectionState::Ready {
                selected: added_group_node.clone(),
            },
        });
    });
    apply_preference(
        &mut app,
        &sink,
        OutboundUiPreferenceIntent::SetGroupExpansion {
            group_id: removed_group.clone(),
            expansion: OutboundGroupExpansion::Collapsed,
        },
    );
    publish(&mut app, &sink, snapshot.clone());
    assert!(!visible(&mut app, &removed_group));
    assert!(visible(&mut app, &added_group));
    assert!(nodes(&mut app).contains_key(&(added_group.clone(), added_group_node.clone())));

    let removed_members = inventory_parts(&snapshot)
        .1
        .into_iter()
        .find(|collection| collection.id == removed_group)
        .unwrap()
        .member_ids;
    update_inventory(&mut snapshot, |_, collections| {
        collections.retain(|collection| collection.id != removed_group);
    });
    publish(&mut app, &sink, snapshot);
    assert!(
        !nodes(&mut app)
            .keys()
            .any(|(collection, _)| collection == &removed_group)
    );
    assert!(
        removed_members
            .iter()
            .all(|member| !nodes(&mut app).contains_key(&(removed_group.clone(), member.clone())))
    );
    assert!(
        app.world()
            .get::<ProxyGroupFoldButton>(old_button)
            .is_none()
    );
    let command_count = sink.commands().len();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: old_button });
    app.update();
    assert_eq!(sink.commands().len(), command_count);
    assert!(
        sink.preferences()
            .collapsed_group_ids
            .contains(&removed_group)
    );
}

fn pointer() -> Pointer {
    Pointer::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::None {
                width: 1180,
                height: 780,
            },
            position: Vec2::ZERO,
        },
    )
}
fn press(app: &mut App, entity: Entity) {
    app.world_mut().trigger(PointerPress {
        entity,
        pointer: pointer(),
        button: PointerButton::Primary,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        count: 1,
    });
    app.world_mut().flush();
}
fn favorite_star(app: &App, entity: Entity) -> Option<Entity> {
    if app.world().get::<ProxyFavoriteStar>(entity).is_some() {
        return Some(entity);
    }
    app.world()
        .get::<Children>(entity)?
        .iter()
        .copied()
        .find_map(|child| favorite_star(app, child))
}

#[test]
fn real_pointer_pin_click_and_cancel_never_selects_and_favorite_ink_replays_by_outbound_id() {
    let (mut app, sink) = app_with_preferences();
    app.add_plugins(ButtonPlugin);
    let snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    let (pin, outbound_id) = app
        .world_mut()
        .query::<(Entity, &NodePinButton, &TextColor)>()
        .iter(app.world())
        .find_map(|(entity, pin, _)| Some((entity, pin.outbound_id.clone()?)))
        .unwrap();
    assert!(app.world().get::<Button>(pin).is_some());
    let original_nodes = nodes(&mut app);
    let node_entity = *original_nodes
        .iter()
        .find(|((_, id), _)| id == &outbound_id)
        .unwrap()
        .1;
    let star = favorite_star(&app, node_entity).unwrap();
    assert!(
        !sink
            .preferences()
            .favorite_outbound_ids
            .contains(&outbound_id)
    );
    press(&mut app, pin);
    assert!(app.world().get::<Pressed>(pin).is_some());
    assert!(
        original_nodes
            .values()
            .all(|node| app.world().get::<Pressed>(*node).is_none())
    );
    app.world_mut().trigger(PointerCancel {
        entity: pin,
        pointer: pointer(),
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
    });
    app.world_mut().flush();
    assert!(app.world().get::<Pressed>(pin).is_none());
    assert!(
        !sink
            .preferences()
            .favorite_outbound_ids
            .contains(&outbound_id)
    );

    for favorite in [true, false] {
        press(&mut app, pin);
        app.world_mut().trigger(PointerClick {
            entity: pin,
            pointer: pointer(),
            button: PointerButton::Primary,
            hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
            duration: Duration::from_millis(120),
            count: 1,
        });
        app.world_mut().flush();
        app.world_mut().trigger(PointerCancel {
            entity: pin,
            pointer: pointer(),
            hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        while sink
            .preferences()
            .favorite_outbound_ids
            .contains(&outbound_id)
            != favorite
        {
            assert!(
                Instant::now() < deadline,
                "typed favorite command must complete"
            );
            app.update();
            yield_now();
        }
        publish(&mut app, &sink, snapshot.clone());
        assert_eq!(nodes(&mut app), original_nodes);
        assert_eq!(
            app.world().get::<Text>(star).unwrap().0,
            if favorite { "★" } else { "☆" }
        );
        let palette = app.world().resource::<UiPalette>();
        assert_eq!(
            app.world().get::<TextColor>(pin).unwrap().0,
            if favorite {
                palette.warning
            } else {
                palette.ink_dim
            }
        );
        let projection = app
            .world()
            .resource::<LastProxiesProjection>()
            .0
            .as_ref()
            .unwrap();
        assert!(projection.groups.iter().all(|group| {
            collection_ids(&snapshot).contains(group.collection_id.as_ref().unwrap())
        }));
        assert!(sink.commands().iter().all(|command| matches!(
            command,
            UiCommand::ApplyOutboundUiPreference { request }
                if matches!(request.intent(), OutboundUiPreferenceIntent::SetFavorite { outbound_id: id, .. } if id == &outbound_id)
        )), "pin clicks only submit typed favorite preferences");
        assert!(
            sink.commands()
                .iter()
                .all(|command| !matches!(command, UiCommand::SelectOutbound { .. })),
            "pin clicks cannot submit an outbound selection"
        );
    }
}
