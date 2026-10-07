//! test-intent: behavior
//! Native SDK clicks stage a full source-bound list, then explicitly commit or discard it.
use crate::command_harness::recording_application;
use crate::native_input::{click_before_frame, click_entity, press, type_text};
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::ui::widget::Text;
use bevy::ui_widgets::ButtonPlugin;
use bevy::window::{Ime, PrimaryWindow, Window};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::rule_list_application::RuleListApplication;
use infiltrator_application::rule_trace_fixtures::{RuleTraceStore, TRACE_DOCUMENT};
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::app::SidebarActiveProfileCard;
use infiltrator_bevy_ui::command::CommandPumpPlugin;
use infiltrator_bevy_ui::pages::rules::LastRulesProjection;
use infiltrator_bevy_ui::pages::rules_builder::{
    AddCustomRuleButton, DiscardRuleFormButton, RuleFormStatus, RulesBuilderState,
};
use infiltrator_bevy_ui::pages::rules_builder_input::{RuleBuilderField, RuleBuilderFieldKind};
use infiltrator_bevy_ui::pages::rules_draft::{RuleListControl, RulesDraftState};
use infiltrator_bevy_ui::pages::rules_edit::{RuleMoveUpButton, RuleToggleButton};
use infiltrator_bevy_ui::pages::rules_projection::{RuleHitText, RulePayloadText};
use infiltrator_bevy_ui::pages::rules_tabs::{RulesTabChip, RulesTabState};
use infiltrator_bevy_ui::route::{ActiveRoute, PageRoot, PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::shell_readout::ShellReadoutText;
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::rules_workspace::RulesTab;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::PageId;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::{Arc, atomic::Ordering};
use std::thread::yield_now;
use std::time::{Duration, Instant};
use tokio::runtime::Builder;

fn publish(app: &mut App, reader: &ApplicationSurfaceReader) {
    let mut snapshot = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(reader.read())
        .unwrap();
    snapshot.revision = app.world().resource::<LatestSurfaceSnapshot>().0.revision + 1;
    app.world_mut().trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    app.update();
}

#[test]
fn native_rule_hits_replay_shared_counts_and_localize_without_replacing_the_row_text() {
    let (mut app, reader, store) = setup();
    publish(&mut app, &reader);
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.revision += 1;
    snapshot.pages.rules.data.as_mut().unwrap().rules[0].hit_count = Some(5);
    app.world_mut().trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    app.update();
    let toggle = first_toggle(&mut app);
    click_entity(&mut app, toggle);
    app.update();
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    let read = |app: &mut App| {
        let (entity, _, text) = app
            .world_mut()
            .query::<(Entity, &RuleHitText, &Text)>()
            .iter(app.world())
            .find(|(_, marker, _)| marker.0 == 0)
            .unwrap();
        (entity, text.0.clone())
    };
    let (entity, value) = read(&mut app);
    assert_eq!(value, "5 local trace hits · Disabled");
    app.insert_resource(UiLocale::new("zh-CN"));
    app.update();
    assert_eq!(read(&mut app), (entity, "5 次本地追踪命中 · 已停用".into()));
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}
fn action(app: &mut App, wanted: RuleListControl) -> Entity {
    app.world_mut()
        .query::<(Entity, &RuleListControl)>()
        .iter(app.world())
        .find(|(_, action)| **action == wanted)
        .unwrap()
        .0
}
fn first_toggle(app: &mut App) -> Entity {
    let id = app
        .world()
        .resource::<RulesDraftState>()
        .model
        .row_id(0)
        .unwrap();
    app.world_mut()
        .query_filtered::<(Entity, &RuleToggleButton), With<RuleToggleButton>>()
        .iter(app.world())
        .find(|(_, row)| row.0 == Some(id))
        .unwrap()
        .0
}
fn settle(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app
        .world()
        .resource::<RulesDraftState>()
        .model
        .pending
        .is_some()
    {
        assert!(Instant::now() < deadline, "real terminal event");
        app.update();
        yield_now();
    }
    app.update();
}
pub(crate) fn setup() -> (App, ApplicationSurfaceReader, Arc<RuleTraceStore>) {
    let store = Arc::new(RuleTraceStore::default());
    let (core, _) = recording_application();
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_rule_list(RuleListApplication::new(store.clone())),
    ));
    let core = Arc::new(core);
    let reader =
        ApplicationSurfaceReader::new(core.clone(), SurfaceKind::BevyDesktop, HostKind::Desktop)
            .with_profiles(ProfileApplication::new(store.clone()))
            .with_configuration(ConfigurationApplication::new(store.clone()));
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ButtonPlugin,
        ShellPlugin::default(),
        PagesPlugin::default(),
        CommandPumpPlugin::for_application(core),
    ));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Rules));
    publish(&mut app, &reader);
    (app, reader, store)
}

#[test]
fn native_queued_clicks_keep_the_original_row_after_reorder_and_retire_on_discard() {
    let (mut app, _, store) = setup();
    let original = store.content();
    let model = &app.world().resource::<RulesDraftState>().model;
    let first = model.row_id(0).unwrap();
    let next = model.row_id(1).unwrap();
    let first_rule = model.draft[0].rule.clone();
    let next_rule = model.draft[1].rule.clone();
    let toggle = first_toggle(&mut app);
    let up = app
        .world_mut()
        .query::<(Entity, &RuleMoveUpButton)>()
        .iter(app.world())
        .find(|(_, button)| button.0 == Some(next))
        .unwrap()
        .0;
    // Both SDK clicks arrive before the next frame rebuilds the row entities.
    click_before_frame(&mut app, up);
    click_before_frame(&mut app, toggle);
    let model = &app.world().resource::<RulesDraftState>().model;
    assert_eq!(model.row_index(first), Some(1));
    assert_eq!(model.draft[0].rule, next_rule);
    assert!(model.draft[0].enabled);
    assert_eq!(model.draft[1].rule, first_rule);
    assert!(!model.draft[1].enabled);
    app.update();
    let toggle = app
        .world_mut()
        .query::<(Entity, &RuleToggleButton)>()
        .iter(app.world())
        .find(|(_, button)| button.0 == Some(first))
        .unwrap()
        .0;
    let discard = action(&mut app, RuleListControl::Discard);
    click_before_frame(&mut app, discard);
    click_before_frame(&mut app, toggle);
    let model = &app.world().resource::<RulesDraftState>().model;
    assert!(
        !model.dirty(),
        "retired row cannot change the discarded draft"
    );
    assert_eq!(store.content(), original);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    app.update();
}

fn shell_rule_count(app: &mut App) -> String {
    app.world_mut()
        .query::<(&ShellReadoutText, &Text)>()
        .iter(app.world())
        .find_map(|(part, text)| {
            matches!(part, ShellReadoutText::Count(PageId::Rules)).then(|| text.0.clone())
        })
        .unwrap()
}

#[test]
fn native_shell_counts_follow_the_real_reader_and_locale_without_replacing_the_entity() {
    let (mut app, reader, store) = setup();
    assert_eq!(shell_rule_count(&mut app), "4");
    let entity = app
        .world_mut()
        .query::<(Entity, &ShellReadoutText)>()
        .iter(app.world())
        .find_map(|(entity, part)| {
            matches!(part, ShellReadoutText::Count(PageId::Rules)).then_some(entity)
        })
        .unwrap();
    store.select_profile("second.yaml", "rules:\n  - MATCH,DIRECT\n");
    publish(&mut app, &reader);
    assert_eq!(shell_rule_count(&mut app), "1");
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    assert_eq!(app.world().get::<Text>(entity).unwrap().0, "1");
    store.select_profile("second.yaml", "rules: []\n");
    publish(&mut app, &reader);
    assert_eq!(shell_rule_count(&mut app), "0");
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}

#[test]
fn native_profile_card_click_mounts_profiles_from_the_accepted_live_snapshot_and_preserves_its_identity()
 {
    let (mut app, _, store) = setup();
    let card = app
        .world_mut()
        .query_filtered::<Entity, With<SidebarActiveProfileCard>>()
        .single(app.world())
        .unwrap();
    click_entity(&mut app, card);
    assert_eq!(
        app.world().resource::<ActiveRoute>().0,
        Some(Route::Profiles)
    );
    assert_eq!(
        app.world_mut()
            .query::<&PageRoot>()
            .single(app.world())
            .unwrap()
            .0,
        Route::Profiles
    );
    let last = app.world().resource::<LatestSurfaceSnapshot>();
    assert!(
        last.0
            .pages
            .profiles
            .data
            .as_ref()
            .unwrap()
            .profiles
            .iter()
            .any(|profile| profile.is_active && profile.name == "trace.yaml")
    );
    let native_text = app
        .world_mut()
        .query::<&Text>()
        .iter(app.world())
        .any(|text| text.0 == "trace.yaml");
    assert!(
        native_text,
        "the mounted profile page must replay the accepted live catalogue"
    );
    assert!(app.world().get::<SidebarActiveProfileCard>(card).is_some());
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}

#[test]
fn native_list_toggle_discard_permission_retry_and_changed_profile_keep_full_draft_and_actual_bytes()
 {
    let (mut app, reader, store) = setup();
    let toggle = first_toggle(&mut app);
    assert!(!app.world().get::<ButtonDisabled>(toggle).unwrap().0);
    click_entity(&mut app, toggle);
    assert!(app.world().resource::<RulesDraftState>().model.dirty());
    assert_eq!(store.content(), TRACE_DOCUMENT);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    let mut discard = action(&mut app, RuleListControl::Discard);
    click_entity(&mut app, discard);
    assert!(!app.world().resource::<RulesDraftState>().model.dirty());
    assert_eq!(store.content(), TRACE_DOCUMENT);
    let toggle = first_toggle(&mut app);
    click_entity(&mut app, toggle);
    let mut save = action(&mut app, RuleListControl::Save);
    assert!(
        app.world().resource::<RulesDraftState>().model.can_save(),
        "{:?}",
        app.world().resource::<RulesDraftState>().model
    );
    assert!(
        !app.world().get::<ButtonDisabled>(save).unwrap().0,
        "actual save control must be enabled for the staged draft"
    );
    store.deny_write.store(true, Ordering::SeqCst);
    click_entity(&mut app, save);
    settle(&mut app);
    assert_eq!(
        app.world()
            .resource::<RulesDraftState>()
            .model
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Permission
    );
    assert!(app.world().resource::<RulesDraftState>().model.dirty());
    assert_eq!(store.content(), TRACE_DOCUMENT);
    let retained = app
        .world()
        .resource::<RulesDraftState>()
        .model
        .draft
        .clone();
    let guide = action(&mut app, RuleListControl::Settings);
    click_entity(&mut app, guide);
    assert_eq!(
        app.world().resource::<ActiveRoute>().0,
        Some(Route::Settings)
    );
    assert_eq!(
        app.world().resource::<RulesDraftState>().model.draft,
        retained
    );
    app.world_mut().trigger(RouteChanged(Route::Rules));
    app.update();
    app.update();
    save = action(&mut app, RuleListControl::Save);
    discard = action(&mut app, RuleListControl::Discard);
    store.deny_write.store(false, Ordering::SeqCst);
    click_entity(&mut app, save);
    settle(&mut app);
    publish(&mut app, &reader);
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert!(!app.world().resource::<RulesDraftState>().model.dirty());
    let updated = store.content();
    assert!(updated.contains("# SRC-IP-CIDR,10.0.0.0/8,DIRECT"));
    let toggle = first_toggle(&mut app);
    click_entity(&mut app, toggle);
    let draft = app
        .world()
        .resource::<RulesDraftState>()
        .model
        .draft
        .clone();
    store.select_profile("second.yaml", "rules:\n  - MATCH,REJECT\n");
    publish(&mut app, &reader);
    assert!(app.world().get::<ButtonDisabled>(save).unwrap().0);
    assert_eq!(app.world().resource::<RulesDraftState>().model.draft, draft);
    click_entity(&mut app, save);
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert_eq!(store.content(), updated);
    click_entity(&mut app, discard);
    assert_eq!(
        app.world()
            .resource::<RulesDraftState>()
            .model
            .base
            .as_ref()
            .unwrap()
            .source
            .profile,
        "second.yaml"
    );
    assert_eq!(
        app.world().resource::<RulesDraftState>().model.draft[0].rule,
        "MATCH,REJECT"
    );
}

fn builder_field(app: &mut App, kind: RuleBuilderFieldKind) -> Entity {
    app.world_mut()
        .query::<(Entity, &RuleBuilderField)>()
        .iter(app.world())
        .find(|(_, field)| field.kind == kind)
        .unwrap()
        .0
}

#[test]
fn native_unsent_form_keeps_source_and_fields_until_discard_then_stages_only_the_new_document() {
    let (mut app, reader, store) = setup();
    let payload = builder_field(&mut app, RuleBuilderFieldKind::Payload);
    press(&mut app, payload);
    type_text(&mut app, "old-form.test");
    store.select_profile("new.yaml", "rules:\n  - MATCH,DIRECT\n");
    publish(&mut app, &reader);
    assert_eq!(
        app.world().get::<TextField>(payload).unwrap().0.text(),
        "old-form.test"
    );
    let add = app
        .world_mut()
        .query_filtered::<Entity, With<AddCustomRuleButton>>()
        .single(app.world())
        .unwrap();
    assert!(app.world().get::<ButtonDisabled>(add).unwrap().0);
    let warning = app
        .world_mut()
        .query_filtered::<&Text, With<RuleFormStatus>>()
        .single(app.world())
        .unwrap()
        .0
        .clone();
    assert!(
        warning.contains("trace.yaml"),
        "visible source warning identifies the retained form"
    );
    click_entity(&mut app, add);
    assert_eq!(
        app.world().resource::<RulesDraftState>().model.draft.len(),
        1
    );
    assert!(!app.world().resource::<RulesDraftState>().model.dirty());
    let reset = app
        .world_mut()
        .query_filtered::<Entity, With<DiscardRuleFormButton>>()
        .single(app.world())
        .unwrap();
    click_entity(&mut app, reset);
    assert_eq!(app.world().get::<TextField>(payload).unwrap().0.text(), "");
    assert!(!app.world().get::<ButtonDisabled>(add).unwrap().0);
    press(&mut app, payload);
    type_text(&mut app, "new-form.test");
    click_entity(&mut app, add);
    let model = &app.world().resource::<RulesDraftState>().model;
    assert_eq!(model.base.as_ref().unwrap().source.profile, "new.yaml");
    assert_eq!(model.draft[0].rule, "DOMAIN-SUFFIX,new-form.test,DIRECT");
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    assert_eq!(
        store.content_for("new.yaml").as_deref(),
        Some("rules:\n  - MATCH,DIRECT\n")
    );
}
fn send_key(app: &mut App, logical_key: Key, key_code: KeyCode) {
    app.world_mut().write_message(KeyboardInput {
        key_code,
        logical_key,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    app.update();
}
#[test]
fn native_builder_keyboard_ime_hidden_tab_route_restore_and_language_refresh_preserve_unsaved_fields()
 {
    let (mut app, _, store) = setup();
    let payload = builder_field(&mut app, RuleBuilderFieldKind::Payload);
    let target = builder_field(&mut app, RuleBuilderFieldKind::Target);
    assert!(
        app.world().get::<TextField>(payload).is_some(),
        "native marker must identify the actual field"
    );
    press(&mut app, payload);
    type_text(&mut app, "new.example");
    press(&mut app, target);
    let old = app
        .world()
        .get::<TextField>(target)
        .unwrap()
        .0
        .text()
        .chars()
        .count();
    send_key(&mut app, Key::End, KeyCode::End);
    for _ in 0..old {
        send_key(&mut app, Key::Backspace, KeyCode::Backspace);
    }
    type_text(&mut app, "DIRECT");
    let add = app
        .world_mut()
        .query_filtered::<Entity, With<AddCustomRuleButton>>()
        .single(app.world())
        .unwrap();
    click_entity(&mut app, add);
    assert_eq!(
        app.world().resource::<RulesDraftState>().model.draft[0].rule,
        "DOMAIN-SUFFIX,new.example,DIRECT"
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    press(&mut app, payload);
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut().write_message(Ime::Enabled { window });
    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "ni".into(),
        cursor: Some((0, 2)),
    });
    app.update();
    let count = app.world().resource::<RulesDraftState>().model.draft.len();
    assert!(app.world().get::<ButtonDisabled>(add).unwrap().0);
    let save = action(&mut app, RuleListControl::Save);
    assert!(app.world().get::<ButtonDisabled>(save).unwrap().0);
    click_entity(&mut app, add);
    click_entity(&mut app, save);
    assert_eq!(
        app.world().resource::<RulesDraftState>().model.draft.len(),
        count
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    app.world_mut().write_message(Ime::Commit {
        window,
        value: "你".into(),
    });
    app.update();
    let retained = app
        .world()
        .get::<TextField>(payload)
        .unwrap()
        .0
        .text()
        .to_owned();
    assert!(retained.contains('你'));
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    assert_eq!(
        builder_field(&mut app, RuleBuilderFieldKind::Payload),
        payload
    );
    assert_eq!(
        app.world().get::<TextField>(payload).unwrap().0.text(),
        retained
    );
    let tracer_tab = app
        .world_mut()
        .query::<(Entity, &RulesTabChip)>()
        .iter(app.world())
        .find(|(_, tab)| tab.0 == 3)
        .unwrap()
        .0;
    click_entity(&mut app, tracer_tab);
    assert!(!app.world().get::<TextFieldFocused>(payload).unwrap().0);
    type_text(&mut app, "hidden");
    assert_eq!(
        app.world().get::<TextField>(payload).unwrap().0.text(),
        retained
    );
    app.world_mut().trigger(RouteChanged(Route::Overview));
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Rules));
    app.update();
    app.update();
    let restored = builder_field(&mut app, RuleBuilderFieldKind::Payload);
    assert_eq!(
        app.world().resource::<RulesTabState>().tab,
        RulesTab::Tracer
    );
    assert!(!app.world().get::<TextFieldFocused>(restored).unwrap().0);
    assert_eq!(
        app.world().get::<TextField>(restored).unwrap().0.text(),
        retained
    );
    assert_eq!(
        app.world().resource::<RulesBuilderState>().payload,
        retained
    );
    assert_eq!(
        app.world().resource::<RulesDraftState>().model.draft.len(),
        count
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}

#[test]
fn native_rule_rows_replay_exact_raw_parameters_and_parse_failures_across_locale_changes() {
    let (mut app, reader, store) = setup();
    store.replace("rules:\n  - IP-CIDR,192.0.2.0/24,DIRECT,src,no-resolve\n  - 'AND,((DOMAIN,example.com)),DIRECT,no-resolve'\n  - MATCH,REJECT\n");
    publish(&mut app, &reader);
    let projection = app
        .world()
        .resource::<LastRulesProjection>()
        .0
        .as_ref()
        .unwrap();
    assert_eq!(
        projection.rules[0].raw,
        "IP-CIDR,192.0.2.0/24,DIRECT,src,no-resolve"
    );
    assert!(projection.rules[0].source_ip && projection.rules[0].no_resolve);
    assert_eq!(
        projection.rules[1].failure.as_ref().unwrap().code,
        ErrorCode::Configuration
    );
    assert!(projection.rules[1].proxy.is_empty());
    let copy = |app: &mut App| {
        app.world_mut()
            .query::<(&Text, &RulePayloadText)>()
            .iter(app.world())
            .map(|(text, _)| text.0.clone())
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert!(copy(&mut app).contains("匹配来源 IP"));
    assert!(copy(&mut app).contains("规则无效"));
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    assert!(copy(&mut app).contains("Match source IP"));
    assert!(copy(&mut app).contains("Invalid rule"));
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}
