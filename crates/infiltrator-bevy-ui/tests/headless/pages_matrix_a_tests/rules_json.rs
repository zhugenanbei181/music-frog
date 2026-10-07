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
    activate(&mut app, json_chip);
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
    app.world_mut()
        .write_message(keyboard_press(Key::End, None));
    app.world_mut()
        .write_message(keyboard_press(Key::Character("x".into()), Some("x")));
    app.update();
    {
        let state = app.world().resource::<RulesJsonState>();
        assert!(state.buffer().full_text().contains('x'));
        assert!(state.status_label("zh-CN").contains("有未提交改动"));
    }

    // Save submits the edited text through the shared intent, and the status
    // is honest about the fire-and-forget command bus.
    let save = app
        .world_mut()
        .query_filtered::<Entity, With<RulesJsonSaveButton>>()
        .single(app.world())
        .expect("save button");
    activate(&mut app, save);
    let submitted = sink.submitted();
    assert_eq!(submitted.len(), 1);
    match &submitted[0] {
        UiCommand::ApplyRulesJsonDocument { section, json } => {
            assert_eq!(*section, RulesJsonSection::RuleProviders);
            assert!(json.contains('x'));
        }
        other => panic!("expected the shared JSON intent, got {other:?}"),
    }
    {
        let state = app.world().resource::<RulesJsonState>();
        assert!(state.status_label("zh-CN").contains("已提交共享应用保存"));
    }
    assert_eq!(
        UiCommand::ApplyRulesJsonDocument {
            section: RulesJsonSection::RuleProviders,
            json: "{}".to_owned(),
        }
        .to_intent(),
        Some(CommandIntent::ApplyRulesJsonDocument {
            section: RulesJsonSection::RuleProviders,
            json: "{}".to_owned(),
        })
    );

    // The read model confirms the submitted bytes: the pending flag clears and
    // the status stops claiming unsubmitted work.
    let edited_json = app
        .world()
        .resource::<RulesJsonState>()
        .buffer()
        .full_text();
    let mut confirmed = RulesProjection::demo();
    confirmed.json_documents = vec![RulesJsonDocumentSnapshot {
        section: RulesJsonSection::RuleProviders,
        json: edited_json,
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
