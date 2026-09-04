//! Headless tests for Select / Combobox: pure search filter, keyboard navigation,
//! action outcomes, and in-place trigger label updates.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::scene::ScenePlugin;
use bevy::ui::prelude::BorderColor;
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::combobox::{
    ComboboxAction, ComboboxLabel, ComboboxNavEvent, ComboboxOption, ComboboxOutcome, ComboboxRoot,
    ComboboxState, ComboboxStateComp,
};
use infiltrator_bevy_widgets::theme::Theme;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn combobox_state_machine_search_and_keyboard_nav() {
    let options = vec![
        ComboboxOption::new("hk", "Hong Kong VIP 01"),
        ComboboxOption::new("jp", "Tokyo Direct 02"),
        ComboboxOption::new("sg", "Singapore Standard 03"),
        ComboboxOption::disabled("us", "US West Maintenance"),
    ];

    let mut state = ComboboxState::new(options);

    // Initial state
    assert!(!state.is_open());
    assert_eq!(state.selected_id(), None);

    // Open
    let outcome = state.apply_action(ComboboxAction::Open);
    assert_eq!(outcome, Some(ComboboxOutcome::Opened));
    assert!(state.is_open());
    assert_eq!(state.highlighted_index(), Some(0));

    // Next item
    state.apply_action(ComboboxAction::Next);
    assert_eq!(state.highlighted_index(), Some(1));

    // Search filter
    state.apply_action(ComboboxAction::SetFilter("singapore".to_owned()));
    assert_eq!(state.filtered_options().len(), 1);
    assert_eq!(state.filtered_options()[0].1.id, "sg");
    assert_eq!(state.highlighted_index(), Some(0));

    // Select highlighted
    let select_outcome = state.apply_action(ComboboxAction::SelectHighlighted);
    assert_eq!(
        select_outcome,
        Some(ComboboxOutcome::Selected {
            id: "sg".to_owned(),
            label: "Singapore Standard 03".to_owned(),
        })
    );
    assert!(!state.is_open());
    assert_eq!(state.selected_id(), Some("sg"));
    assert_eq!(state.selected_label(), Some("Singapore Standard 03"));
}

#[test]
fn combobox_ecs_event_advances_state_and_restamps_label() {
    let mut app = headless_app();
    let options = vec![
        ComboboxOption::new("node-1", "Proxy Node 1"),
        ComboboxOption::new("node-2", "Proxy Node 2"),
    ];

    let combo_entity = app
        .world_mut()
        .spawn((
            ComboboxRoot,
            ComboboxStateComp(ComboboxState::new(options)),
            BorderColor::all(bevy::color::Color::NONE),
        ))
        .id();

    let label_entity = app
        .world_mut()
        .spawn((
            Text("Select...".to_owned()),
            ComboboxLabel,
            bevy::text::TextColor::default(),
        ))
        .id();

    app.world_mut()
        .entity_mut(combo_entity)
        .add_child(label_entity);

    app.update();

    // Send select action via message
    app.world_mut().write_message(ComboboxNavEvent {
        combobox: combo_entity,
        action: ComboboxAction::SelectId("node-2".to_owned()),
    });
    app.update();

    let world = app.world();
    let text = world.get::<Text>(label_entity).expect("label exists");
    assert_eq!(text.0, "Proxy Node 2");
}
