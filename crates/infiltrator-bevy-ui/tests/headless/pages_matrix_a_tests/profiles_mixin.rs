//! Behavior cases for profiles mixin.
//! test-intent: behavior

use super::*;
use infiltrator_bevy_widgets::editor::state::CodeEditorState;
use infiltrator_contract::profile_options::ProfileOptionsSnapshot;
use infiltrator_domain::mixin_studio;

/// DUAL-10-08/10/11: the Bevy Mixin pane renders the shared preflight verdict,
/// the shared preset-toggle chips and the real cascade pipeline strip, flips a
/// toggle through the shared codec, and refuses an overlay the shared
/// preflight blocks (before any command is submitted).
#[test]
fn test_profiles_mixin_studio_renders_shared_preflight_toggles_and_cascade() {
    use infiltrator_bevy_ui::pages::profiles_editor_mixin_studio::MixinToggleButton;
    use infiltrator_bevy_ui::pages::profiles_editor_panes::{
        MixinEditorSaveButton, ProfileEditorOptionsState, ProfileEditorPane,
    };

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    // Open a profile document + its stored overlay.
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(editor_options_page_projection(
            "mode: rule\nport: 7890\n",
            Some(ProfileOptionsSnapshot::new(
                observation("main", "mode: rule\nport: 7890\n", Default::default())
                    .unwrap()
                    .source,
                "{}\n",
                Default::default(),
            )),
        )));
    app.update();
    let switch = pane_button_entity(&mut app, ProfileEditorPane::Mixin);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: switch });
    app.update();

    // The shared studio rows are mounted and restamped from the shared facts.
    assert!(
        subtree_has_text(app.world(), root, "常用覆写开关"),
        "the shared toggle catalogue renders"
    );
    assert!(
        subtree_has_text(app.world(), root, "预检通过"),
        "the shared preflight verdict renders"
    );
    assert!(
        subtree_has_text(app.world(), root, "覆写流水线"),
        "the cascade pipeline strip renders"
    );

    // DUAL-10-09: the three-column workspace renders the real base document,
    // the editable overlay column and the real composed output.
    assert!(
        subtree_has_text(app.world(), root, "Base 配置"),
        "the left Base column renders"
    );
    assert!(
        subtree_has_text(app.world(), root, "Mixin 覆写块"),
        "the middle overlay column renders"
    );
    assert!(
        subtree_has_text(app.world(), root, "合成后最终配置"),
        "the right composed column renders"
    );
    assert!(
        subtree_has_text(app.world(), root, "mode: rule"),
        "the composed column carries the real base document"
    );

    // DUAL-10-11: the first chip flips IPv6 through the real MixinConfig codec.
    let toggle = {
        let mut query = app.world_mut().query::<(Entity, &MixinToggleButton)>();
        query
            .iter(app.world())
            .find(|(_, button)| button.index == 0)
            .map(|(entity, _)| entity)
            .expect("ipv6 toggle chip")
    };
    app.world_mut()
        .commands()
        .trigger(Activate { entity: toggle });
    app.update();
    assert!(
        app.world()
            .resource::<ProfileEditorOptionsState>()
            .mixin
            .buffer
            .full_text()
            .contains("ipv6: true"),
        "the toggle wrote the real overlay field"
    );

    // DUAL-10-10: a YAML-valid but semantically-invalid overlay (a bad
    // `mixed-port` type) is blocked by the shared preflight, not just by the
    // YAML syntax check, and never reaches the command pump.
    app.world_mut()
        .resource_mut::<ProfileEditorOptionsState>()
        .mixin
        .buffer = CodeEditorState::new("mixed-port: not-a-number\n");
    let before = sink.submitted().len();
    let save = marker_entity::<MixinEditorSaveButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();
    assert_eq!(
        sink.submitted().len(),
        before,
        "a shared-preflight refusal is not submitted"
    );
    let reason =
        mixin_studio::preflight_mixin("mode: rule\nport: 7890\n", "mixed-port: not-a-number\n")
            .error
            .expect("actual schema failure");
    assert_eq!(
        app.world()
            .resource::<ProfileEditorOptionsState>()
            .mixin
            .notice
            .as_deref(),
        Some(reason.as_str())
    );
    assert!(
        subtree_has_text(app.world(), root, &reason),
        "the actual shared failure must be visible in the pane"
    );
}
