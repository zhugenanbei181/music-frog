//! Behavior cases for profiles editor large document window.
//! test-intent: behavior

use super::*;
use bevy::ui::prelude::{Display, Node};
use bevy::ui::widget::Text;

#[test]
fn test_profiles_editor_opens_a_bounded_large_document_window() {
    use infiltrator_bevy_ui::pages::profiles_editor_large::{
        LargeWindowScroll, ProfileEditorLargeWindowButton, ProfileEditorLargeWindowPanel,
        ProfileEditorLargeWindowScrollButton, ProfileEditorLargeWindowStatusText,
    };
    use infiltrator_bevy_ui::pages::profiles_editor_state::{
        LARGE_DOCUMENT_WINDOW_LINES, ProfileEditorState,
    };

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    // A long document: more lines than the large-document window renders.
    let document: String = (0..(LARGE_DOCUMENT_WINDOW_LINES * 3))
        .map(|index| format!("key-{index}: value-{index}\n"))
        .collect();
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(editor_page_projection(
            ProfileWriteProtection::Editable,
            &document,
        )));
    app.update();

    let panel = marker_entity::<ProfileEditorLargeWindowPanel>(&mut app);
    assert_eq!(
        app.world()
            .get::<Node>(panel)
            .expect("the panel node")
            .display,
        Display::None,
        "the large-document window starts closed"
    );

    let open = marker_entity::<ProfileEditorLargeWindowButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: open });
    app.update();

    assert_eq!(
        app.world()
            .get::<Node>(panel)
            .expect("the panel node")
            .display,
        Display::Flex,
        "the control opens the large-document window"
    );
    let state = app.world().resource::<ProfileEditorState>();
    assert!(state.large_window_is_open());

    let status = marker_entity::<ProfileEditorLargeWindowStatusText>(&mut app);
    let rendered = app
        .world()
        .get::<Text>(status)
        .expect("the window status line")
        .0
        .clone();
    assert!(
        rendered.contains("lines 1.."),
        "the window reports where it stands, got {rendered}"
    );

    // The scroll control moves the window with the shared arithmetic.
    let down = {
        let world = app.world_mut();
        let mut buttons = world.query::<(Entity, &ProfileEditorLargeWindowScrollButton)>();
        buttons
            .iter(world)
            .find(|(_, marker)| marker.0 == LargeWindowScroll::Down)
            .map(|(entity, _)| entity)
            .expect("the window's scroll-down control")
    };
    app.world_mut()
        .commands()
        .trigger(Activate { entity: down });
    app.update();

    let moved = app
        .world()
        .get::<Text>(status)
        .expect("the window status line")
        .0
        .clone();
    assert!(
        moved.contains(&format!("lines {}..", LARGE_DOCUMENT_WINDOW_LINES + 1)),
        "the window scrolled by one page, got {moved}"
    );
    let _ = root;
}
