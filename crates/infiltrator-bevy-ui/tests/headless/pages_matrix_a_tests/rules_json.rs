//! Behavior cases for rules json.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use infiltrator_contract::rules_workspace::RulesJsonDocumentSnapshot;

#[test]
fn test_rules_json_partition_edits_and_submits_shared_intent() {
    use infiltrator_bevy_ui::pages::rules_json::{
        RulesJsonEditButton, RulesJsonSaveButton, RulesJsonState, RulesJsonStatusText,
    };
    use infiltrator_bevy_ui::pages::rules_tabs::{RulesTabChip, RulesTabState};
    use infiltrator_contract::command::CommandIntent;
    use infiltrator_contract::profile_configuration::ProfileConfigurationUpdate;
    use infiltrator_contract::rules_workspace::{RulesJsonSection, RulesTab};

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Rules);

    // The demo projection publishes the three workspace documents; the
    // partition adopts them verbatim.
    {
        let state = app.world().resource::<RulesJsonState>();
        assert_eq!(state.published.len(), RulesJsonSection::ALL.len());
        assert_eq!(
            state.published_document().map(str::to_owned),
            Some(state.buffer().full_text()),
            "the partition adopts the published document verbatim"
        );
        assert!(state.status_label("zh-CN").contains("已与读模型一致"));
    }

    // Switch to the JSON partition and focus the buffer through the explicit
    // seam, exactly like the profiles editor.
    let json_chip = {
        let mut query = app.world_mut().query::<(Entity, &RulesTabChip)>();
        query
            .iter(app.world())
            .find(|(_, chip)| chip.0 == RulesTab::JsonEditors.index())
            .map(|(entity, _)| entity)
            .expect("json editors chip")
    };
    select_rules_tab(&mut app, json_chip);
    assert_eq!(
        app.world().resource::<RulesTabState>().tab,
        RulesTab::JsonEditors
    );

    let edit = app
        .world_mut()
        .query_filtered::<Entity, With<RulesJsonEditButton>>()
        .single(app.world())
        .expect("edit button");
    activate(&mut app, edit);
    {
        let state = app.world().resource::<RulesJsonState>();
        assert!(state.focused, "the explicit seam armed the buffer");
        assert!(state.status_label("zh-CN").contains("编辑中"));
    }

    // Typed keystrokes reach the buffer and mark it dirty.
    let last_row = app
        .world()
        .resource::<RulesJsonState>()
        .buffer()
        .line_count()
        .saturating_sub(1);
    for _ in 0..last_row {
        app.world_mut()
            .write_message(keyboard_press(Key::ArrowDown, None));
        app.update();
    }
    app.world_mut()
        .write_message(keyboard_press(Key::End, None));
    app.update();
    app.world_mut()
        .write_message(keyboard_press(Key::Character(" ".into()), Some(" ")));
    app.update();
    {
        let state = app.world().resource::<RulesJsonState>();
        assert!(state.buffer().full_text().ends_with(' '));
        assert!(state.status_label("zh-CN").contains("有未提交改动"));
    }

    // Save submits the decoded typed value with the exact observed source
    // identity, and the status is honest about the fire-and-forget command bus.
    let save = app
        .world_mut()
        .query_filtered::<Entity, With<RulesJsonSaveButton>>()
        .single(app.world())
        .expect("save button");
    activate(&mut app, save);
    let submitted = sink.submitted();
    assert_eq!(submitted.len(), 1);
    let (request, canonical_json) = match &submitted[0] {
        UiCommand::ApplyProfileConfiguration(request) => {
            assert_eq!(request.update, ProfileConfigurationUpdate::RuleProviders(
                serde_json::from_str("{\n  \"geosite-geolocation-!cn\": {\n    \"type\": \"http\",\n    \"behavior\": \"domain\",\n    \"url\": \"https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/geolocation-!cn.mrs\",\n    \"interval\": 86400\n  }\n}").unwrap(),
            ));
            (
                request.clone(),
                serde_json::to_string_pretty(match &request.update {
                    ProfileConfigurationUpdate::RuleProviders(value) => value,
                    _ => unreachable!(),
                })
                .unwrap(),
            )
        }
        other => panic!("expected a typed profile update, got {other:?}"),
    };
    {
        let state = app.world().resource::<RulesJsonState>();
        assert!(state.status_label("zh-CN").contains("已提交共享应用保存"));
    }
    assert_eq!(
        UiCommand::ApplyProfileConfiguration(request.clone()).to_intent(),
        Some(CommandIntent::ApplyProfileConfiguration {
            request: request.clone(),
        })
    );

    // The read model confirms the submitted bytes: the pending flag clears and
    // the status stops claiming unsubmitted work.
    let mut confirmed = RulesProjection::demo();
    confirmed.json_documents = vec![RulesJsonDocumentSnapshot {
        section: RulesJsonSection::RuleProviders,
        identity: request.identity.clone(),
        json: canonical_json,
    }];
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(confirmed));
    app.update();
    {
        let state = app.world().resource::<RulesJsonState>();
        assert!(!state.status_label("zh-CN").contains("有未提交改动"));
        assert!(state.status_label("zh-CN").contains("已与读模型一致"));
    }

    // The status line is rendered from the partition state.
    let status = app
        .world_mut()
        .query::<(&Text, &RulesJsonStatusText)>()
        .iter(app.world())
        .map(|(text, _)| text.0.clone())
        .next()
        .expect("json status line");
    assert!(status.contains("已提交共享应用保存"), "{status}");
}
