//! test-intent: behavior
//! Locale replay preserves live editor state and document entities without submitting commands.
use super::*;
use infiltrator_bevy_ui::pages::profiles_editor::{
    ProfileEditorProtectionText, ProfileEditorStatusText, ProfileEditorTitle,
};
use infiltrator_bevy_ui::pages::profiles_editor_mixin_studio::{
    MixinCascadeText, MixinColumnCaption,
};
use infiltrator_bevy_ui::pages::profiles_editor_panes::{
    MixinEditorStatusText, ProfileEditorOptionsState,
};
use infiltrator_bevy_ui::pages::profiles_editor_state::ProfileEditorState;
use infiltrator_bevy_widgets::editor::state::CodeEditorState;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_domain::mixin_studio::MixinColumnRole;
use infiltrator_domain::yaml_edit::format::FormatSkipReason;

#[test]
fn editor_locale_replays_shared_facts_and_keeps_dirty_buffers_cursor_focus_and_entities() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Profiles);
    app.world_mut()
        .trigger(ProfilesProjectionUpdated(editor_page_projection(
            ProfileWriteProtection::RemoteSubscription,
            "mode: rule\nproxies: []\n",
        )));
    app.update();
    {
        let mut state = app.world_mut().resource_mut::<ProfileEditorState>();
        state.buffer = CodeEditorState::new("# 中文🙂\nmode: global\nproxies: []\n");
        state.buffer.cursor_row = 0;
        state.buffer.cursor_col = "# 中文".len();
        state.dirty = true;
        state.focused = true;
        state.protection_override = true;
        state.format_note = Some(FormatSkipReason::AnchorsPresent);
        state.generation += 1;
    }
    {
        let mut options = app.world_mut().resource_mut::<ProfileEditorOptionsState>();
        options.pane = ProfileEditorPane::Mixin;
        options.mixin.buffer = CodeEditorState::new("ipv6: true\n");
        options.mixin.dirty = true;
        options.mixin.generation += 1;
    }
    app.update();
    let before = app.world().resource::<ProfileEditorState>().clone();
    let mixin_before = app
        .world()
        .resource::<ProfileEditorOptionsState>()
        .mixin
        .clone();
    let title = marker_entity::<ProfileEditorTitle>(&mut app);
    let protection = marker_entity::<ProfileEditorProtectionText>(&mut app);
    let status = marker_entity::<ProfileEditorStatusText>(&mut app);
    let mixin_status = marker_entity::<MixinEditorStatusText>(&mut app);
    let cascade = marker_entity::<MixinCascadeText>(&mut app);
    let caption = app
        .world_mut()
        .query::<(Entity, &MixinColumnCaption)>()
        .iter(app.world())
        .find(|(_, caption)| caption.0 == MixinColumnRole::Overlay)
        .unwrap()
        .0;
    sink.clear();
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    assert_eq!(*app.world().resource::<ProfileEditorState>(), before);
    assert_eq!(
        app.world().resource::<ProfileEditorOptionsState>().mixin,
        mixin_before
    );
    assert!(
        sink.submitted().is_empty(),
        "locale replay must not submit a business command"
    );
    assert_eq!(
        app.world().get::<Text>(title).unwrap().0,
        "Profile document editor · YAML (4 lines)"
    );
    let banner = &app.world().get::<Text>(protection).unwrap().0;
    assert!(banner.contains("read-only"));
    assert!(banner.contains("Anchors/aliases present: key order kept"));
    let copy = &app.world().get::<Text>(status).unwrap().0;
    assert!(
        copy.contains("Cursor 1:5"),
        "cursor column is Unicode characters: {copy}"
    );
    assert!(copy.contains("Unsaved changes"));
    assert!(
        app.world()
            .get::<Text>(mixin_status)
            .unwrap()
            .0
            .contains("Unsaved changes")
    );
    assert!(
        app.world()
            .get::<Text>(cascade)
            .unwrap()
            .0
            .contains("Subscription (not declared)")
    );
    assert_eq!(
        app.world().get::<Text>(caption).unwrap().0,
        "Mixin overlay · 1 lines · editable"
    );
}
