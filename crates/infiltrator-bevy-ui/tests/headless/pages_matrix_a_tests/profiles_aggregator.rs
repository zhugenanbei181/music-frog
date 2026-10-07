//! Behavior cases for profiles aggregator.
//! test-intent: behavior

use super::*;
use bevy::camera::NormalizedRenderTarget;
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerClick};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::ui_widgets::{Checkbox, CheckboxPlugin};
use infiltrator_bevy_ui::pages::business_panel::{
    BusinessPanelState, OpenBusinessPanel, PanelKind,
};
use infiltrator_bevy_ui::route::PageRoot;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use std::time::Duration;

fn click_checkbox(app: &mut App, entity: Entity) {
    app.world_mut().trigger(PointerClick {
        entity,
        pointer: Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::None {
                    width: 1180,
                    height: 780,
                },
                position: Vec2::ZERO,
            },
        ),
        button: PointerButton::Primary,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        duration: Duration::from_millis(120),
        count: 1,
    });
    app.update();
}

fn open_aggregator(app: &mut App) {
    let button = app
        .world_mut()
        .query::<(Entity, &OpenBusinessPanel)>()
        .iter(app.world())
        .find(|(_, open)| open.0 == PanelKind::Aggregator)
        .unwrap()
        .0;
    app.world_mut().trigger(Activate { entity: button });
    app.update();
    assert_eq!(
        app.world().resource::<BusinessPanelState>().0,
        Some(PanelKind::Aggregator)
    );
}

#[test]
fn native_aggregation_checkbox_clicks_change_only_the_draft_and_the_next_submit_reads_the_new_choices()
 {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    app.add_plugins(CheckboxPlugin);
    navigate_to(&mut app, Route::Profiles);
    open_aggregator(&mut app);
    let projection = aggregation_page_projection();
    app.world_mut()
        .trigger(ProfilesProjectionUpdated(projection));
    app.update();
    let wrapper = app
        .world_mut()
        .query::<(Entity, &AggregatorSwitch)>()
        .iter(app.world())
        .find(|(_, switch)| switch.0 == AggregatorSwitchKind::Deduplicate)
        .unwrap()
        .0;
    let checkbox = app
        .world()
        .get::<Children>(wrapper)
        .unwrap()
        .iter()
        .copied()
        .find(|entity| app.world().get::<Checkbox>(*entity).is_some())
        .unwrap();
    assert!(app.world().get::<Checked>(checkbox).is_some());
    click_checkbox(&mut app, checkbox);
    assert!(app.world().get::<Checked>(checkbox).is_none());
    assert!(sink.submitted().is_empty());
    let preview = marker_entity::<PreviewAggregationButton>(&mut app);
    app.world_mut().trigger(Activate { entity: preview });
    app.update();
    let Some(UiCommand::PreviewProfileAggregation { draft }) = sink.submitted().last().cloned()
    else {
        panic!("actual submit reads the native draft")
    };
    assert!(!draft.deduplicate);
    sink.clear();
    click_checkbox(&mut app, checkbox);
    assert!(app.world().get::<Checked>(checkbox).is_some());
    assert!(sink.submitted().is_empty());
    app.world_mut().trigger(Activate { entity: preview });
    app.update();
    let Some(UiCommand::PreviewProfileAggregation { draft }) = sink.submitted().last().cloned()
    else {
        panic!("actual submit reads the restored choice")
    };
    assert!(draft.deduplicate);
}
use infiltrator_contract::aggregator::{
    AggregationCustomGroup, AggregationDraft, AggregationRenameRule,
};

#[test]
fn aggregation_copy_replays_real_report_without_replacing_the_live_draft_or_clearing_validation() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);
    open_aggregator(&mut app);
    app.world_mut()
        .trigger(ProfilesProjectionUpdated(aggregation_page_projection()));
    app.update();
    let wrapper = marker_entity::<AggregatorNameField>(&mut app);
    let field = app
        .world()
        .get::<Children>(wrapper)
        .unwrap()
        .iter()
        .copied()
        .find(|entity| app.world().get::<TextField>(*entity).is_some())
        .unwrap();
    let mut state = app.world_mut().get_mut::<TextField>(field).unwrap();
    state
        .0
        .apply(TextFieldInput::SetText("draft {count}".into()));
    state.0.apply(TextFieldInput::SelectAll);
    state.0.set_preedit("zhong");
    let expected = state.0.clone();
    app.world_mut()
        .get_mut::<TextFieldFocused>(field)
        .unwrap()
        .0 = true;
    set_marker_text::<AggregatorRenamesField>(&mut app, "invalid rule {line}");
    let preview = marker_entity::<PreviewAggregationButton>(&mut app);
    app.world_mut().trigger(Activate { entity: preview });
    app.update();
    assert!(
        sink.submitted().is_empty(),
        "invalid syntax never executes a preview"
    );
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    assert!(app.world().get::<PageRoot>(root).is_some());
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    let actual = &app.world().get::<TextField>(field).unwrap().0;
    assert_eq!(actual.text(), expected.text());
    assert_eq!(actual.selection(), expected.selection());
    assert_eq!(actual.preedit(), expected.preedit());
    assert!(subtree_has_text(app.world(), root, "deduplicated 2"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "Aggregation YAML (4 lines)"
    ));
    assert!(subtree_has_text(app.world(), root, "invalid rule {line}"));
    assert!(!subtree_has_text(app.world(), root, "重命名规则格式错误"));
    assert!(sink.submitted().is_empty());
}

/// DUAL-08: the aggregator card projects the shared report and submits the
/// complete edited draft through the shared command bus.
#[test]
fn test_profiles_aggregator_previews_and_saves_through_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(aggregation_page_projection()));
    app.update();

    assert!(
        subtree_has_text(app.world(), root, "多订阅节点聚合器 (Profile Aggregator)"),
        "the aggregator card mounts on the profiles page"
    );
    // The preview is the shared report, never a local fabrication.
    assert!(
        subtree_has_text(app.world(), root, "🇭🇰 HK 香港 → 香港自动测速（2 节点）"),
        "the region cluster line restamps from the shared report"
    );
    assert!(
        subtree_has_text(app.world(), root, "🚀 节点选择 [主选择器]"),
        "the master cascade line restamps from the shared report"
    );
    assert!(
        subtree_has_text(app.world(), root, "去重 2"),
        "the dedup counter restamps from the shared report"
    );
    // DUAL-08-09/08-11/08-13: the new preview lines all restamp from the
    // shared report and the projected template library.
    assert!(
        subtree_has_text(app.world(), root, "预检剔除 2"),
        "the precheck counter restamps from the shared report"
    );
    assert!(
        subtree_has_text(app.world(), root, "聚合 YAML 结构（共 4 行）"),
        "the YAML viewport renders the shared document"
    );
    assert!(
        subtree_has_text(app.world(), root, "流媒体专用 [自定义]"),
        "the custom group rides the shared group cascade"
    );
    assert!(
        subtree_has_text(app.world(), root, "已保存模板 → Template-Target"),
        "the template library restamps from the shared projection"
    );

    // The card's switches are user input (the projection supplies the report),
    // so the test states them explicitly: everything on except activation.
    set_switch_checked(&mut app, AggregatorSwitchKind::ActivateAfterCreate, false);
    set_marker_text::<AggregatorRenamesField>(&mut app, "-广告$ => ");
    set_marker_text::<AggregatorNameField>(&mut app, "Merged-New");
    set_source_checked(&mut app, "Backup Anycast", false);
    set_source_checked(&mut app, "LAN Lab", false);

    let preview = marker_entity::<PreviewAggregationButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: preview });
    app.update();

    let expected = AggregationDraft {
        source_profiles: vec!["Primary VIP".to_owned()],
        target_name: "Merged-New".to_owned(),
        deduplicate: true,
        deduplicate_names: true,
        geo_cluster: true,
        generate_groups: true,
        remove_emojis: true,
        rename_rules: vec![AggregationRenameRule {
            pattern: "-广告$".to_owned(),
            replacement: String::new(),
        }],
        custom_groups: Vec::new(),
        availability_precheck: true,
        activate_after_create: false,
    };
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::PreviewProfileAggregation {
            draft: expected.clone(),
        }],
        "the edited draft rides the shared preview command"
    );

    let save = marker_entity::<SaveAggregatedProfileButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();

    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::CreateAggregatedProfile {
            draft: expected.clone(),
        }),
        "the save action rides the shared create command"
    );
    sink.clear();

    // DUAL-08-10: the appended custom group joins the submitted draft.
    set_marker_text::<AggregatorCustomGroupNameField>(&mut app, "游戏专用");
    set_marker_text::<AggregatorCustomGroupKeywordsField>(&mut app, "LAN, 调试");
    let add = marker_entity::<AddAggregatorCustomGroupButton>(&mut app);
    app.world_mut().commands().trigger(Activate { entity: add });
    app.update();
    assert!(
        subtree_has_text(
            app.world(),
            root,
            "自定义策略组：游戏专用 · select · LAN, 调试"
        ),
        "the appended custom group restamps onto the composer line"
    );
    let preview = marker_entity::<PreviewAggregationButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: preview });
    app.update();
    let mut with_group = expected.clone();
    with_group.custom_groups = vec![AggregationCustomGroup {
        name: "游戏专用".to_owned(),
        group_type: "select".to_owned(),
        member_keywords: vec!["LAN".to_owned(), "调试".to_owned()],
    }];
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::PreviewProfileAggregation {
            draft: with_group.clone(),
        }),
        "the custom group rides the shared preview command"
    );
    sink.clear();

    // DUAL-08-12: the activation switch rides the shared draft.
    set_switch_checked(&mut app, AggregatorSwitchKind::ActivateAfterCreate, true);
    let preview = marker_entity::<PreviewAggregationButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: preview });
    app.update();
    let mut activated = with_group.clone();
    activated.activate_after_create = true;
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::PreviewProfileAggregation {
            draft: activated.clone(),
        }),
        "the activation switch rides the shared draft"
    );
    sink.clear();

    // DUAL-08-08: a malformed rename line is refused at the surface, and the
    // previous command is not repeated.
    set_marker_text::<AggregatorRenamesField>(&mut app, "missing arrow");
    let preview = marker_entity::<PreviewAggregationButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: preview });
    app.update();
    assert!(
        sink.submitted().is_empty(),
        "a malformed rename rule never reaches the shared bus"
    );
    assert!(
        subtree_has_text(app.world(), root, "重命名规则格式错误"),
        "the wizard status line points at the malformed rule"
    );
    set_marker_text::<AggregatorRenamesField>(&mut app, "-广告$ => ");

    // DUAL-08-13: "use template" prefills the wizard from the projection.
    set_marker_text::<AggregatorTemplateNameField>(&mut app, "已保存模板");
    let use_template = marker_entity::<UseAggregationTemplateButton>(&mut app);
    app.world_mut().commands().trigger(Activate {
        entity: use_template,
    });
    app.update();
    let preview = marker_entity::<PreviewAggregationButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: preview });
    app.update();
    let from_template = AggregationDraft {
        source_profiles: vec!["Primary VIP".to_owned()],
        target_name: "Template-Target".to_owned(),
        deduplicate: false,
        deduplicate_names: true,
        geo_cluster: false,
        generate_groups: true,
        remove_emojis: false,
        rename_rules: vec![AggregationRenameRule {
            pattern: "-广告$".to_owned(),
            replacement: String::new(),
        }],
        custom_groups: Vec::new(),
        availability_precheck: false,
        activate_after_create: true,
    };
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::PreviewProfileAggregation {
            draft: from_template.clone(),
        }),
        "the reused template drives the wizard's own fields"
    );
    sink.clear();

    // DUAL-08-13: save the live draft as a template under the typed name.
    let save_template = marker_entity::<SaveAggregationTemplateButton>(&mut app);
    app.world_mut().commands().trigger(Activate {
        entity: save_template,
    });
    app.update();
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::SaveAggregationTemplate {
            name: "已保存模板".to_owned(),
            draft: from_template.clone(),
        }),
        "the template save rides the shared command"
    );
    sink.clear();

    // DUAL-08-07: re-aggregate resolves the template by name and submits it.
    let reaggregate = marker_entity::<ReAggregateTemplateButton>(&mut app);
    app.world_mut().commands().trigger(Activate {
        entity: reaggregate,
    });
    app.update();
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::ReAggregateProfile {
            template_name: "已保存模板".to_owned(),
        }),
        "the re-aggregate action rides the shared command"
    );
    sink.clear();

    // DUAL-08-13: deleting the named template rides the shared command.
    let delete = marker_entity::<DeleteAggregationTemplateButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: delete });
    app.update();
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::DeleteAggregationTemplate {
            name: "已保存模板".to_owned(),
        }),
        "the template delete rides the shared command"
    );
    sink.clear();

    // Unknown template names are refused with a status hint, not a command.
    set_marker_text::<AggregatorTemplateNameField>(&mut app, "不存在");
    let delete = marker_entity::<DeleteAggregationTemplateButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: delete });
    app.update();
    assert!(
        sink.submitted().is_empty(),
        "an unknown template never reaches the shared bus"
    );

    // DUAL-08-13: a host without the template sidecar is reported as such, not
    // as an empty template library.
    let mut unavailable = aggregation_page_projection();
    unavailable.aggregation_templates.clear();
    unavailable.aggregation_templates_available = false;
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(unavailable));
    app.update();
    assert!(
        subtree_has_text(
            app.world(),
            root,
            "历史聚合模板：宿主未提供模板存储（不支持）"
        ),
        "the unsupported template store is stated explicitly"
    );
}
