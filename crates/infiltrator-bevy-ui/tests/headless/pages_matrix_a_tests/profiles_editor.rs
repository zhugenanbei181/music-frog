//! Behavior cases for profiles editor.
//! test-intent: behavior

use super::*;
use crate::native_input::{click_entity, type_text};
use bevy::ui::prelude::BackgroundColor;
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::profiles_editor_transactions::EditorDiscardButton;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_options::ProfileOptionsSnapshot;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::subscription_filter_result::SubscriptionFilterApplied;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_contract::yaml_snippets::YAML_SNIPPETS;
use infiltrator_shared::locales::{Lang, Localizer};

#[test]
fn test_profiles_editor_runs_the_shared_preflight_and_formatter() {
    use infiltrator_bevy_ui::pages::profiles_editor::{
        ProfileEditorFocusButton, ProfileEditorFormatButton,
    };
    use infiltrator_bevy_ui::pages::profiles_editor_state::ProfileEditorState;

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(editor_page_projection(
            ProfileWriteProtection::Editable,
            "# 手写注释\nmode: rule\n",
        )));
    app.update();
    assert!(
        subtree_has_text(app.world(), root, "语法正确（共享预检实时通过）"),
        "the loaded document passes the shared preflight"
    );

    let focus = marker_entity::<ProfileEditorFocusButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: focus });
    app.update();
    assert!(
        app.world().resource::<ProfileEditorState>().focused,
        "the explicit keyboard seam is armed by the focus button"
    );

    // Type an invalid document: the shared preflight reports the line live.
    app.world_mut()
        .write_message(keyboard_press(Key::Character("x".into()), Some("x")));
    app.world_mut()
        .write_message(keyboard_press(Key::Character(":".into()), Some(": ")));
    app.world_mut()
        .write_message(keyboard_press(Key::Character("[".into()), Some("[")));
    app.update();
    {
        let state = app.world().resource::<ProfileEditorState>();
        assert!(
            state.diagnostic.is_some(),
            "the shared preflight flags the incomplete flow mapping"
        );
        assert!(state.dirty, "an edit marks the buffer dirty");
    }
    assert!(
        subtree_has_text(app.world(), root, "语法错误"),
        "the banner and pill render the live verdict"
    );

    // Formatting an invalid buffer is refused with the shared error, and the
    // user's bytes survive.
    let before = app
        .world()
        .resource::<ProfileEditorState>()
        .buffer
        .full_text();
    let format = marker_entity::<ProfileEditorFormatButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: format });
    app.update();
    let state = app.world().resource::<ProfileEditorState>();
    assert_eq!(
        state.buffer.full_text(),
        before,
        "a formatter refusal never rewrites the user's bytes"
    );
    assert!(
        state
            .notice
            .as_deref()
            .is_some_and(|notice| notice.contains("格式化")),
        "the refusal reason is surfaced"
    );
}

#[test]
fn test_profiles_editor_formats_with_the_shared_engine_and_saves_through_the_guard() {
    use infiltrator_bevy_ui::pages::profiles_editor::{
        ProfileEditorFormatButton, ProfileEditorProtectionToggle, ProfileEditorSaveButton,
    };
    use infiltrator_bevy_ui::pages::profiles_editor_state::ProfileEditorState;

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Profiles);

    // 4-space indentation with a comment: the shared formatter normalizes the
    // layout and keeps the comment, unlike a serde re-serialize.
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(editor_page_projection(
            ProfileWriteProtection::RemoteSubscription,
            "# 手写注释\nmode: rule\nrules:\n    - MATCH,DIRECT\n",
        )));
    app.update();

    let format = marker_entity::<ProfileEditorFormatButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: format });
    app.update();
    let text = app
        .world()
        .resource::<ProfileEditorState>()
        .buffer
        .full_text();
    assert!(
        text.contains("# 手写注释"),
        "comments survive the formatter"
    );
    assert!(
        text.contains("\n  - MATCH,DIRECT"),
        "the shared formatter normalizes the indentation: {text}"
    );

    // A protected subscription refuses to save until explicitly unlocked.
    let save = marker_entity::<ProfileEditorSaveButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();
    assert!(
        !sink
            .submitted()
            .iter()
            .any(|command| matches!(command, UiCommand::SaveProfileDocument { .. })),
        "a protected profile without the explicit unlock submits nothing"
    );

    let unlock = marker_entity::<ProfileEditorProtectionToggle>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: unlock });
    app.update();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();
    match sink.submitted().last() {
        Some(UiCommand::SaveProfileDocument {
            source,
            content,
            allow_protected,
        }) => {
            assert_eq!(source.profile, "main");
            assert!(content.contains("# 手写注释"));
            assert!(
                *allow_protected,
                "the unlock travels with the save so the application guard re-checks it"
            );
        }
        other => panic!("expected a SaveProfileDocument command, got {other:?}"),
    }
}

#[test]
fn test_profiles_editor_renders_a_bounded_window_on_a_large_document() {
    use infiltrator_bevy_ui::pages::profiles_editor::ProfileEditorBody;
    use infiltrator_bevy_ui::pages::profiles_editor_state::{
        PROFILE_EDITOR_RENDER_LIMIT, ProfileEditorState,
    };

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Profiles);

    // DUAL-09-13 evidence: a 10,000-line document renders the *same* bounded
    // window as a short one — the count is a real entity count from the
    // spawned scene, not a claimed frame rate.
    let large: String = (0..10_000)
        .map(|index| format!("key-{index}: value-{index}\n"))
        .collect();
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(editor_page_projection(
            ProfileWriteProtection::Editable,
            &large,
        )));
    app.update();

    {
        let state = app.world().resource::<ProfileEditorState>();
        let viewport = state.viewport();
        let total = state.buffer.line_count();
        assert_eq!(
            total, 10_001,
            "the buffer keeps the complete document including its final newline"
        );
        assert_eq!(viewport.rendered_len(), PROFILE_EDITOR_RENDER_LIMIT);
        assert_eq!(viewport.line_numbers().count(), PROFILE_EDITOR_RENDER_LIMIT);
        assert_eq!(viewport.hidden_below(), total - PROFILE_EDITOR_RENDER_LIMIT);
        assert!(!viewport.covers_document());
    }

    let body = marker_entity::<ProfileEditorBody>(&mut app);
    let rendered_texts = subtree_text_count(app.world(), body);
    // Each rendered line contributes its line number plus at least one token;
    // the window is 240 lines, so the count must stay far below the document's
    // 10,000 lines and above the window size.
    assert!(
        rendered_texts >= PROFILE_EDITOR_RENDER_LIMIT,
        "the window renders every line of the window ({rendered_texts})"
    );
    assert!(
        rendered_texts <= (PROFILE_EDITOR_RENDER_LIMIT + 2) * 4,
        "the render is bounded by the window, not the document ({rendered_texts} text nodes)"
    );
    let hidden_below = 10_001 - PROFILE_EDITOR_RENDER_LIMIT;
    assert!(
        subtree_has_text(
            app.world(),
            body,
            &format!("下方还有 {hidden_below} 行未渲染")
        ),
        "the window states its bounds instead of pretending to show everything"
    );

    // The window follows the caret: moving to the end of the document renders
    // the last window, and nothing beyond it.
    {
        let mut state = app.world_mut().resource_mut::<ProfileEditorState>();
        state.buffer.cursor_row = state.buffer.line_count() - 1;
        state.generation = state.generation.wrapping_add(1);
    }
    app.update();
    let state = app.world().resource::<ProfileEditorState>();
    let viewport = state.viewport();
    assert_eq!(viewport.last_line(), 10_000);
    assert_eq!(viewport.hidden_below(), 0);
    assert_eq!(
        viewport.hidden_above(),
        10_001 - PROFILE_EDITOR_RENDER_LIMIT
    );
    let body = marker_entity::<ProfileEditorBody>(&mut app);
    let rendered_texts = subtree_text_count(app.world(), body);
    assert!(
        rendered_texts <= (PROFILE_EDITOR_RENDER_LIMIT + 2) * 4,
        "the window stays bounded while scrolled to the end ({rendered_texts})"
    );
}

#[test]
fn test_profiles_editor_inserts_the_shared_snippet_catalogue() {
    use infiltrator_bevy_ui::pages::profiles_editor_state::ProfileEditorState;

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    app.add_plugins(ButtonPlugin);
    navigate_to(&mut app, Route::Profiles);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(editor_page_projection(
            ProfileWriteProtection::Editable,
            "proxies:\n  - name: keep\n",
        )));
    app.update();

    // The first button is the first catalogue entry; its body comes from the
    // contract, and the splice runs through the shared application use-case.
    // The caret is parked at the end of the last list item first, so the
    // snippet joins the list instead of splitting a scalar.
    {
        let mut state = app.world_mut().resource_mut::<ProfileEditorState>();
        state.buffer.cursor_row = 1;
        state.buffer.cursor_col = state.buffer.lines[1].len();
    }
    let first = YAML_SNIPPETS.first().expect("catalogue");
    let button = snippet_button_entity(&mut app, 0);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    let state = app.world().resource::<ProfileEditorState>();
    let text = state.buffer.full_text();
    assert!(
        text.contains(first.body.trim_end()),
        "the catalogue body is spliced into the Bevy buffer: {text}"
    );
    assert!(
        text.starts_with("proxies:\n  - name: keep"),
        "the untouched bytes stay in place: {text}"
    );
    assert!(state.dirty, "the insertion marks the buffer dirty");

    // A snippet that would break the document is refused with the shared
    // diagnostic and never rewrites the buffer.
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(editor_page_projection(
            ProfileWriteProtection::Editable,
            "mode: rule\n",
        )));
    app.update();
    let discard = app
        .world_mut()
        .query::<(Entity, &EditorDiscardButton)>()
        .iter(app.world())
        .find(|(_, button)| !button.mixin)
        .unwrap()
        .0;
    click_entity(&mut app, discard);
    assert_eq!(
        app.world()
            .resource::<ProfileEditorState>()
            .buffer
            .full_text(),
        "mode: rule\n",
        "the native discard adopts the newly observed source without submitting a write"
    );
    {
        // Splitting the scalar to open a block sequence is not valid YAML.
        let mut state = app.world_mut().resource_mut::<ProfileEditorState>();
        state.buffer.cursor_col = 6;
    }
    let before = app
        .world()
        .resource::<ProfileEditorState>()
        .buffer
        .full_text();
    let group = snippet_button_entity(&mut app, 4);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: group });
    app.update();
    let state = app.world().resource::<ProfileEditorState>();
    assert_eq!(
        state.buffer.full_text(),
        before,
        "a refused snippet keeps the user's bytes"
    );
    assert!(
        state
            .notice
            .as_deref()
            .is_some_and(|notice| notice.contains("片段")),
        "the refusal is surfaced: {:?}",
        state.notice
    );
}

/// DUAL-09-14: the Mixin pane loads the stored sidecar through the shared
/// `LoadProfileOptions` command, edits a real buffer with the same keyboard
/// seam as the profile document, and commits through `SaveMixinOverlay` (the
/// shared `ProfileOptionsApplication::save_mixin` use-case).
#[test]
fn test_profiles_editor_mixin_pane_uses_the_shared_sidecar_use_case() {
    use infiltrator_bevy_ui::pages::profiles_editor_panes::{
        MixinEditorFocusButton, MixinEditorReloadButton, MixinEditorSaveButton,
        ProfileEditorOptionsState, ProfileEditorPane,
    };
    use infiltrator_bevy_ui::pages::profiles_editor_state::ProfileEditorState;

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    // The stored document must be open before the pane knows which sidecar to
    // ask for ("main" is the active profile in this fixture).
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(editor_options_page_projection(
            "mode: rule\n",
            None,
        )));
    app.update();

    let switch = pane_button_entity(&mut app, ProfileEditorPane::Mixin);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: switch });
    app.update();
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::LoadProfileOptions {
            profile: Some("main".to_owned()),
        }),
        "opening the Mixin pane asks the shared application for the sidecar"
    );
    assert_eq!(
        app.world().resource::<ProfileEditorOptionsState>().pane,
        ProfileEditorPane::Mixin
    );
    assert_eq!(
        pane_area_display(&mut app, ProfileEditorPane::Mixin),
        Display::Flex,
        "the Mixin area is shown"
    );
    assert_eq!(
        pane_area_display(&mut app, ProfileEditorPane::Profile),
        Display::None,
        "the profile document rows are hidden while the Mixin pane is active"
    );
    {
        use infiltrator_bevy_ui::pages::profiles_editor_panes::ProfileEditorPaneButton;
        let mut query = app
            .world_mut()
            .query::<(&ProfileEditorPaneButton, &BackgroundColor)>();
        let active = query
            .iter(app.world())
            .find(|(button, _)| button.pane == ProfileEditorPane::Mixin)
            .map(|(_, background)| background.0)
            .expect("mixin switcher chip");
        let inactive = query
            .iter(app.world())
            .find(|(button, _)| button.pane == ProfileEditorPane::Profile)
            .map(|(_, background)| background.0)
            .expect("profile switcher chip");
        assert_ne!(
            active, inactive,
            "the active pane chip is restamped from the shared palette"
        );
    }

    // The shared snapshot fills the Mixin buffer + the shared filter draft.
    let draft = SubscriptionFilterDraft {
        include: "香港".to_owned(),
        ..Default::default()
    };
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(editor_options_page_projection(
            "mode: rule\n",
            Some(ProfileOptionsSnapshot::new(
                observation("main", "mode: rule\n", Default::default())
                    .unwrap()
                    .source,
                "# 覆盖注释\nmode: global\n",
                draft.clone(),
            )),
        )));
    app.update();
    {
        let options = app.world().resource::<ProfileEditorOptionsState>();
        assert!(
            options.mixin.buffer.full_text().contains("# 覆盖注释"),
            "the stored Mixin YAML is the buffer: {}",
            options.mixin.buffer.full_text()
        );
        assert_eq!(
            options.filter.draft.include, "香港",
            "the same snapshot carries the stored filter draft"
        );
    }
    assert!(
        subtree_has_text(
            app.world(),
            root,
            Lang("zh-CN").tr("profiles_mixin_save_hint").as_ref()
        ),
        "the pane explains replacement and preservation when saving"
    );

    // Save submits the shared use-case command with the buffer bytes.
    let save = marker_entity::<MixinEditorSaveButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();
    match sink.submitted().last() {
        Some(UiCommand::SaveMixinOverlay { source, mixin_yaml }) => {
            assert_eq!(source.profile, "main");
            assert!(mixin_yaml.contains("# 覆盖注释"));
        }
        other => panic!("expected SaveMixinOverlay, got {other:?}"),
    }

    let (request_id, _) = app
        .world()
        .resource::<ProfileEditorOptionsState>()
        .mixin
        .save_request
        .unwrap();
    let command = sink.submitted().last().unwrap().clone();
    app.world_mut().trigger(CommandExecutedEvent {
        command,
        request_id,
        result: Err(Failure::new(
            ErrorCode::Permission,
            "fixture denied save",
            true,
        )),
    });
    app.update();
    assert_eq!(
        app.world()
            .resource::<ProfileEditorOptionsState>()
            .mixin
            .session
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Permission
    );
    // The keyboard seam routes to the Mixin buffer, not the profile document.
    let focus = marker_entity::<MixinEditorFocusButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: focus });
    app.update();
    let profile_before = app
        .world()
        .resource::<ProfileEditorState>()
        .buffer
        .full_text();
    app.world_mut()
        .write_message(keyboard_press(Key::Character("x".into()), Some("x")));
    app.update();
    {
        let options = app.world().resource::<ProfileEditorOptionsState>();
        assert!(options.mixin.focused, "the explicit seam armed the buffer");
        assert!(
            options.mixin.buffer.full_text().starts_with('x'),
            "the key landed in the Mixin buffer: {}",
            options.mixin.buffer.full_text()
        );
        assert!(options.mixin.dirty);
        assert!(
            options.mixin.diagnostic.is_some(),
            "the broken Mixin buffer fails the shared preflight"
        );
    }
    assert_eq!(
        app.world()
            .resource::<ProfileEditorState>()
            .buffer
            .full_text(),
        profile_before,
        "the profile document buffer is untouched"
    );

    // A buffer that no longer passes the shared preflight is refused before
    // the command pump sees it (the application would re-check anyway).
    let before = sink.submitted().len();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();
    assert_eq!(
        sink.submitted().len(),
        before,
        "an invalid Mixin overlay is not submitted"
    );
    let mixin = &app.world().resource::<ProfileEditorOptionsState>().mixin;
    let reason = &mixin
        .diagnostic
        .as_ref()
        .expect("actual syntax failure")
        .message;
    assert_eq!(
        mixin.notice.as_deref(),
        Some(reason.as_str()),
        "the parser's actual cause is retained"
    );
    assert!(
        subtree_has_text(app.world(), root, reason),
        "the actual failure must be visible in the pane"
    );

    // Reload re-requests the shared sidecar instead of answering locally.
    let reload = marker_entity::<MixinEditorReloadButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: reload });
    app.update();
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::LoadProfileOptions {
            profile: Some("main".to_owned()),
        }),
        "reload goes back through the shared application"
    );
}

/// DUAL-09-14: the Filter pane renders the shared `SubscriptionFilterDraft`,
/// mirrors typing into it and submits the same `SaveSubscriptionFilter`
/// command the Iced filter pane runs — a malformed draft is refused with the
/// shared parser before anything is submitted.
#[test]
fn test_profiles_editor_filter_pane_mirrors_the_shared_draft_and_gates_submits() {
    use infiltrator_bevy_ui::pages::profiles_editor_panes::{
        EditorFilterDedupButton, EditorFilterSaveButton, ProfileEditorOptionsState,
        ProfileEditorPane,
    };

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    if !app.is_plugin_added::<ButtonPlugin>() {
        app.add_plugins(ButtonPlugin);
    }
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(editor_options_page_projection(
            "mode: rule\n",
            None,
        )));
    app.update();

    let switch = pane_button_entity(&mut app, ProfileEditorPane::Filter);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: switch });
    app.update();
    assert_eq!(
        sink.submitted().last(),
        Some(&UiCommand::LoadProfileOptions {
            profile: Some("main".to_owned()),
        }),
        "opening the Filter pane loads the shared sidecar too"
    );

    let draft = SubscriptionFilterDraft {
        include: "香港".to_owned(),
        dedup_index: 1,
        ..Default::default()
    };
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(editor_options_page_projection(
            "mode: rule\n",
            Some(ProfileOptionsSnapshot::new(
                observation("main", "mode: rule\n", draft.clone())
                    .unwrap()
                    .source,
                "{}\n",
                draft,
            )),
        )));
    app.update();
    assert_eq!(
        filter_field_text(&mut app, FilterField::Include),
        "香港",
        "the controlled field renders the shared draft"
    );
    assert!(
        subtree_has_text(
            app.world(),
            root,
            Lang("zh-CN").tr("profiles_filter_save_hint").as_ref()
        ),
        "the pane explains applying and retaining filter choices"
    );

    // Typing mirrors into the shared draft through the same text field the
    // restamp system owns.
    let include = filter_field_entity(&mut app, FilterField::Include);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: include });
    app.update();
    app.world_mut()
        .write_message(keyboard_press(Key::Character("日本".into()), Some("日本")));
    app.update();
    assert_eq!(
        app.world()
            .resource::<ProfileEditorOptionsState>()
            .filter
            .draft
            .include,
        "香港日本",
        "the field and the draft stay in step"
    );

    // A dedup strategy chip updates the shared draft, not a surface copy.
    let chip = {
        let mut query = app
            .world_mut()
            .query::<(Entity, &EditorFilterDedupButton)>();
        query
            .iter(app.world())
            .find(|(_, chip)| chip.index == 3)
            .map(|(entity, _)| entity)
            .expect("dedup chip")
    };
    app.world_mut()
        .commands()
        .trigger(Activate { entity: chip });
    app.update();
    assert_eq!(
        app.world()
            .resource::<ProfileEditorOptionsState>()
            .filter
            .draft
            .dedup_index,
        3
    );

    // Submit through the shared command with the edited draft.
    let save = marker_entity::<EditorFilterSaveButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();
    match sink.submitted().last() {
        Some(UiCommand::SaveSubscriptionFilter { source, filter }) => {
            assert_eq!(source.profile, "main");
            assert_eq!(filter.include, "香港日本");
            assert_eq!(filter.dedup_index, 3);
        }
        other => panic!("expected SaveSubscriptionFilter, got {other:?}"),
    }

    let (id, token) = app
        .world()
        .resource::<ProfileEditorOptionsState>()
        .filter_request
        .unwrap();
    let command = sink.submitted().last().unwrap().clone();
    let UiCommand::SaveSubscriptionFilter { source, filter } = &command else {
        panic!("filter command");
    };
    let applied = SubscriptionFilterApplied {
        source: observation(&source.profile, FIXTURE_DOCUMENT, filter.clone())
            .unwrap()
            .source,
        report: Default::default(),
    };
    app.world_mut().trigger(CommandExecutedEvent {
        request_id: id,
        command,
        result: Ok(CommandOutput::SubscriptionFilterApplied(applied)),
    });
    app.update();
    assert!(
        app.world()
            .resource::<ProfileEditorOptionsState>()
            .filter
            .pending
            .is_none()
    );
    assert!(token > 0);
    // Actual native input produces an invalid rename and is refused inline.
    let root = filter_field_entity(&mut app, FilterField::Renames);
    let input = app
        .world()
        .get::<Children>(root)
        .unwrap()
        .iter()
        .copied()
        .find(|entity| app.world().get::<TextField>(*entity).is_some())
        .unwrap();
    click_entity(&mut app, input);
    type_text(&mut app, "没有箭头的规则");
    let before = sink.submitted().len();
    let save = marker_entity::<EditorFilterSaveButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();
    assert_eq!(
        sink.submitted().len(),
        before,
        "a malformed draft is refused before the submit"
    );
    let notice = app
        .world()
        .resource::<ProfileEditorOptionsState>()
        .filter
        .failure
        .as_ref()
        .unwrap()
        .message
        .clone();
    assert!(
        notice.contains("=>"),
        "the refusal names the shared parser rule: {notice}"
    );
}
