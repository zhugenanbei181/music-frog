//! Behavior cases for rules tab.
//! test-intent: behavior

use super::*;
use bevy::a11y::AccessibilityNode;
use bevy::ecs::query::With;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_shared::locales::{Lang, Localizer};

#[test]
fn test_rules_tab_partition_matches_the_shared_capability_set() {
    use infiltrator_bevy_ui::pages::rules_json::{
        RulesJsonEditorBody, RulesJsonSaveButton, RulesJsonSectionChip, RulesJsonState,
    };
    use infiltrator_bevy_ui::pages::rules_tabs::{RulesTabBody, RulesTabChip, RulesTabState};
    use infiltrator_contract::rules_workspace::{RulesJsonSection, RulesTab};

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Rules);

    // One chip and one body per shared partition, labelled from the shared
    // vocabulary in its shared order.
    let chips: Vec<usize> = {
        let mut query = app.world_mut().query::<&RulesTabChip>();
        let mut values: Vec<usize> = query.iter(app.world()).map(|chip| chip.0).collect();
        values.sort_unstable();
        values
    };
    assert_eq!(chips, vec![0, 1, 2, 3]);
    let mut bodies: Vec<usize> = {
        let mut query = app.world_mut().query::<&RulesTabBody>();
        query.iter(app.world()).map(|body| body.0.index()).collect()
    };
    bodies.sort_unstable();
    assert_eq!(bodies, vec![0, 1, 2, 3]);
    for tab in RulesTab::ALL {
        assert!(subtree_has_text(
            app.world(),
            root,
            Lang("zh-CN").tr(tab.i18n_key()).as_ref()
        ));
    }
    assert_eq!(RulesTabState::default().tab, RulesTab::List);

    // Only the active partition is displayed; switching tabs flips exactly one.
    let displays = |app: &mut App| -> Vec<(usize, Display)> {
        let mut query = app.world_mut().query::<(&Node, &RulesTabBody)>();
        let mut values: Vec<(usize, Display)> = query
            .iter(app.world())
            .map(|(node, body)| (body.0.index(), node.display))
            .collect();
        values.sort_by_key(|(index, _)| *index);
        values
    };
    assert_eq!(
        displays(&mut app),
        vec![
            (0, Display::Flex),
            (1, Display::None),
            (2, Display::None),
            (3, Display::None)
        ]
    );

    let json_chip = {
        let mut query = app.world_mut().query::<(Entity, &RulesTabChip)>();
        query
            .iter(app.world())
            .find(|(_, chip)| chip.0 == RulesTab::JsonEditors.index())
            .map(|(entity, _)| entity)
            .expect("json editors chip")
    };
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    assert_eq!(
        app.world()
            .get::<AccessibilityNode>(json_chip)
            .unwrap()
            .label(),
        Some("JSON Editors")
    );
    for tab in RulesTab::ALL {
        assert!(subtree_has_text(
            app.world(),
            root,
            Lang("en-US").tr(tab.i18n_key()).as_ref()
        ));
    }
    assert_eq!(app.world().resource::<RulesTabState>().tab, RulesTab::List);
    activate(&mut app, json_chip);
    assert_eq!(
        app.world().resource::<RulesTabState>().tab,
        RulesTab::JsonEditors
    );
    assert_eq!(
        displays(&mut app),
        vec![
            (0, Display::None),
            (1, Display::None),
            (2, Display::Flex),
            (3, Display::None)
        ]
    );

    // The JSON partition exposes every shared section and the editor body.
    let sections: Vec<usize> = {
        let mut query = app.world_mut().query::<&RulesJsonSectionChip>();
        let mut values: Vec<usize> = query.iter(app.world()).map(|chip| chip.0).collect();
        values.sort_unstable();
        values
    };
    assert_eq!(sections, vec![0, 1, 2]);
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<RulesJsonEditorBody>>()
            .iter(app.world())
            .count(),
        1
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<RulesJsonSaveButton>>()
            .iter(app.world())
            .next()
            .is_some()
    );
    assert_eq!(
        app.world().resource::<RulesJsonState>().buffers.len(),
        RulesJsonSection::ALL.len()
    );
}
