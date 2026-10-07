//! test-intent: behavior
//! Real projection fields replace invented differences and replay locale in place.
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ui::widget::Text;
use bevy::ui::{Display, InteractionDisabled, Node};
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::pages::sync::{
    ConflictCardContainer, ConflictingKey, SyncConflictInfo, SyncLine, SyncLineKind,
    SyncProjection, SyncProjectionUpdated,
};
use infiltrator_bevy_ui::pages::sync_merge::{SyncMergeChoice, SyncMergeRow};
use infiltrator_bevy_ui::projection::DemoOverviewSource;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::sync::SyncStatus;

fn rows(app: &mut App) -> Vec<(Entity, String)> {
    let mut rows = app
        .world_mut()
        .query::<(Entity, &SyncMergeRow, &Text)>()
        .iter(app.world())
        .map(|(entity, row, text)| (row.0, entity, text.0.clone()))
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.0);
    rows.into_iter()
        .map(|(_, entity, text)| (entity, text))
        .collect()
}
fn line(app: &mut App, kind: SyncLineKind) -> String {
    app.world_mut()
        .query::<(&SyncLine, &Text)>()
        .iter(app.world())
        .find(|(line, _)| line.0 == kind)
        .unwrap()
        .1
        .0
        .clone()
}
#[test]
fn native_sync_uses_actual_fields_preserves_entities_on_locale_and_retires_old_rows() {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ButtonPlugin,
        ShellPlugin::default(),
        PagesPlugin::new(DemoOverviewSource::running()),
    ));
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Sync));
    app.update();
    app.update();
    assert!(rows(&mut app).is_empty());
    let mut cards = app.world_mut().query::<(&ConflictCardContainer, &Node)>();
    assert_eq!(cards.single(app.world()).unwrap().1.display, Display::None);
    let mut choices = app
        .world_mut()
        .query::<(&SyncMergeChoice, &ButtonDisabled)>();
    assert_eq!(choices.iter(app.world()).count(), 3);
    assert!(choices.iter(app.world()).all(|(_, disabled)| disabled.0));
    let mut disabled = app
        .world_mut()
        .query::<(&SyncMergeChoice, &InteractionDisabled)>();
    assert_eq!(disabled.iter(app.world()).count(), 3);
    let mut observed = SyncProjection::demo();
    observed.status = SyncStatus::Configured;
    observed.username = "account {server}/中文🙂".into();
    observed.server_url = "https://dav.example/{username}".into();
    observed.last_sync = None;
    observed.conflict = Some(SyncConflictInfo {
        remote_device: "device {count}/中文🙂".into(),
        conflict_time: "time {device}".into(),
        conflicting_keys: vec![
            ConflictingKey {
                key: "mode {local}".into(),
                local_value: "rule 中文🙂".into(),
                remote_value: "global {key}".into(),
            },
            ConflictingKey {
                key: "rules".into(),
                local_value: "[one]\nnull".into(),
                remote_value: "[two]\nfalse".into(),
            },
        ],
    });
    app.world_mut()
        .trigger(SyncProjectionUpdated(observed.clone()));
    app.update();
    app.update();
    let mut cards = app
        .world_mut()
        .query::<(Entity, &ConflictCardContainer, &Node)>();
    let (card, _, node) = cards.single(app.world()).unwrap();
    assert_eq!(node.display, Display::Flex);
    let inner = *app.world().get::<Children>(card).unwrap().first().unwrap();
    assert_eq!(
        app.world().get::<Node>(inner).unwrap().display,
        Display::Flex
    );
    let before = rows(&mut app);
    assert_eq!(before.len(), 2);
    assert_eq!(
        before[0].1,
        "mode {local}\n本地: rule 中文🙂\n远端: global {key}"
    );
    assert_eq!(
        line(&mut app, SyncLineKind::Summary),
        "数据同步 · 已配置 · 连接未验证"
    );
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    let after = rows(&mut app);
    assert_eq!(
        before.iter().map(|row| row.0).collect::<Vec<_>>(),
        after.iter().map(|row| row.0).collect::<Vec<_>>()
    );
    assert_eq!(
        after[0].1,
        "mode {local}\nLocal: rule 中文🙂\nRemote: global {key}"
    );
    assert_eq!(
        after[1].1,
        "rules\nLocal: [one]\nnull\nRemote: [two]\nfalse"
    );
    assert_eq!(
        line(&mut app, SyncLineKind::Summary),
        "Sync · Configured; connection not verified"
    );
    assert_eq!(
        line(&mut app, SyncLineKind::Username),
        "Account: account {server}/中文🙂"
    );
    assert_eq!(
        line(&mut app, SyncLineKind::ServerUrl),
        "Server: https://dav.example/{username}"
    );
    assert_eq!(
        line(&mut app, SyncLineKind::LastSync),
        "Last sync: not observed"
    );
    observed
        .conflict
        .as_mut()
        .unwrap()
        .conflicting_keys
        .remove(0);
    app.world_mut()
        .trigger(SyncProjectionUpdated(observed.clone()));
    app.update();
    app.update();
    assert_eq!(rows(&mut app).len(), 1);
    for (entity, _) in after {
        assert!(app.world().get_entity(entity).is_err());
    }
    observed.conflict = None;
    observed.status = SyncStatus::Unknown;
    app.world_mut().trigger(SyncProjectionUpdated(observed));
    app.update();
    app.update();
    assert!(rows(&mut app).is_empty());
    assert_eq!(line(&mut app, SyncLineKind::Summary), "Sync · Not observed");
}
