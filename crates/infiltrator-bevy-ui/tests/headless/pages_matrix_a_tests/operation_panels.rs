//! Behavior cases for operation panels.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use infiltrator_bevy_ui::pages::proxies_custom::ImportUriButton;
use infiltrator_bevy_widgets::text_input::TextField;

#[test]
fn operation_panels_preserve_editable_controls_and_cancel_without_commands() {
    use infiltrator_bevy_ui::pages::business_panel::{
        BusinessPanelRoot, BusinessPanelState, CloseBusinessPanel, OpenBusinessPanel, PanelKind,
    };
    for (route, kind) in [
        (Route::Proxies, PanelKind::CustomNode),
        (Route::Profiles, PanelKind::Aggregator),
        (Route::Profiles, PanelKind::SnapshotDiff),
    ] {
        let sink = Arc::new(DemoCommandSink::accepting());
        let mut app = setup_matrix_a_app(Arc::clone(&sink));
        navigate_to(&mut app, route);
        let root = app
            .world_mut()
            .query::<(Entity, &BusinessPanelRoot)>()
            .iter(app.world())
            .find(|(_, marker)| marker.0 == kind)
            .map(|(entity, _)| entity)
            .unwrap();
        let before_fields: Vec<_> = app
            .world_mut()
            .query::<(Entity, &TextField)>()
            .iter(app.world())
            .map(|(entity, field)| (entity, field.0.text().to_owned()))
            .collect();
        assert_eq!(
            app.world().get::<Node>(root).unwrap().display,
            Display::None
        );
        let opener = app
            .world_mut()
            .query::<(Entity, &OpenBusinessPanel)>()
            .iter(app.world())
            .find(|(_, marker)| marker.0 == kind)
            .map(|(entity, _)| entity)
            .unwrap();
        app.world_mut()
            .commands()
            .trigger(Activate { entity: opener });
        app.update();
        assert_eq!(
            app.world().get::<Node>(root).unwrap().display,
            Display::Flex
        );
        assert_eq!(app.world().resource::<BusinessPanelState>().0, Some(kind));
        // The independent panel must contain the production controls, not an empty overlay.
        let control = match kind {
            PanelKind::CustomNode => app
                .world_mut()
                .query_filtered::<Entity, With<ImportUriButton>>()
                .single(app.world())
                .unwrap(),
            PanelKind::Aggregator => app
                .world_mut()
                .query_filtered::<Entity, With<PreviewAggregationButton>>()
                .single(app.world())
                .unwrap(),
            PanelKind::SnapshotDiff => app
                .world_mut()
                .query::<(Entity, &SnapshotDiffModeButton)>()
                .iter(app.world())
                .find(|(_, button)| button.split)
                .map(|(entity, _)| entity)
                .unwrap(),
        };
        let mut ancestor = control;
        while ancestor != root {
            ancestor = app
                .world()
                .get::<ChildOf>(ancestor)
                .expect("operation control belongs to its own panel")
                .0;
        }
        let cancel = app
            .world_mut()
            .query::<(Entity, &CloseBusinessPanel)>()
            .iter(app.world())
            .find(|(_, marker)| marker.0 == kind)
            .map(|(entity, _)| entity)
            .unwrap();
        app.world_mut()
            .commands()
            .trigger(Activate { entity: cancel });
        app.update();
        assert_eq!(
            app.world().get::<Node>(root).unwrap().display,
            Display::None
        );
        assert_eq!(app.world().resource::<BusinessPanelState>().0, None);
        let after_fields: Vec<_> = app
            .world_mut()
            .query::<(Entity, &TextField)>()
            .iter(app.world())
            .map(|(entity, field)| (entity, field.0.text().to_owned()))
            .collect();
        assert_eq!(after_fields, before_fields);
        assert_eq!(sink.submitted(), Vec::<UiCommand>::new());
    }
}
