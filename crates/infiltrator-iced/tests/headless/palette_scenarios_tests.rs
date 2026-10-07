//! test-intent: behavior
use crate::test_support::demo_env;
use infiltrator_contract::command_catalogue::CommandTarget;
use infiltrator_contract::shortcuts::KeyModifiers;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;

#[test]
fn palette_enter_executes_selected_row_and_rejects_stale_submit() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Overview));
    assert_eq!(state.update(Message::OpenCommandPalette).units(), 0);
    let expected = state.shell.command_catalogue.entries().iter().enumerate().find(|(_, entry)| matches!(entry.target, CommandTarget::Navigate(page) if Route::from_shell_page(page) == Route::Profiles)).map(|(index, _)| index).unwrap();
    while state.shell.command_selected_index != expected {
        assert_eq!(state.update(Message::SelectNextCommand).units(), 0);
    }
    drop(state.view());
    assert_eq!(state.update(Message::ExecuteSelectedCommand).units(), 0);
    assert_eq!(state.shell.current_route, Route::Profiles);
    assert!(!state.shell.command_palette_open);
    state.shell.current_route = Route::Overview;
    assert_eq!(state.update(Message::ExecuteSelectedCommand).units(), 0);
    assert_eq!(state.shell.current_route, Route::Overview);
}

#[test]
fn palette_empty_enter_and_escape_schedule_no_commands() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Overview));
    let before = state.diag.connections.clone();
    let _ = state.update(Message::OpenCommandPalette);
    let _ = state.update(Message::SetCommandQuery("no-catalogue-entry-8371".into()));
    assert!(state.filtered_command_indices().is_empty());
    assert_eq!(state.update(Message::ExecuteSelectedCommand).units(), 0);
    assert!(state.shell.command_palette_open);
    assert_eq!(state.shell.current_route, Route::Overview);
    assert_eq!(
        state
            .update(Message::KeyboardChord {
                key: "Escape".into(),
                modifiers: KeyModifiers::default()
            })
            .units(),
        0
    );
    assert!(!state.shell.command_palette_open);
    assert_eq!(state.diag.connections, before);
}

#[test]
fn palette_locale_category_search_keeps_all_navigation_targets() {
    let mut env = demo_env(Route::Overview);
    env.lang = "en-US".into();
    let (mut state, _) = AppState::demo(&env);
    let _ = state.update(Message::OpenCommandPalette);
    let _ = state.update(Message::SetCommandQuery("Navigation".into()));
    let indices = state.filtered_command_indices();
    assert_eq!(indices.len(), 11);
    assert!(indices.iter().all(|index| matches!(
        state.shell.command_catalogue.entry(*index).unwrap().target,
        CommandTarget::Navigate(_)
    )));
}

#[test]
fn palette_native_widget_tree_keeps_the_full_catalogue_tail() {
    use iced::advanced::widget::Tree;
    fn has_complete_column(tree: &Tree, rows: usize) -> bool {
        tree.children.len() == rows
            || tree
                .children
                .iter()
                .any(|child| has_complete_column(child, rows))
    }
    let (mut state, _) = AppState::demo(&demo_env(Route::Overview));
    let _ = state.update(Message::OpenCommandPalette);
    let count = state.shell.command_catalogue.len();
    assert!(count > 8);
    let tree = Tree::new(state.view().as_widget());
    assert!(has_complete_column(&tree, count));
    for _ in 1..count {
        let _ = state.update(Message::SelectNextCommand);
    }
    assert_eq!(state.shell.command_selected_index, count - 1);
    let tree = Tree::new(state.view().as_widget());
    assert!(has_complete_column(&tree, count));
    let _ = state.update(Message::SetCommandQuery("no-catalogue-entry-8371".into()));
    let tree = Tree::new(state.view().as_widget());
    assert!(!has_complete_column(&tree, count));
}

#[test]
fn palette_profile_refresh_replaces_stale_titles_without_changing_row_count() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Overview));
    let _ = state.update(Message::OpenCommandPalette);
    let count = state.shell.command_catalogue.len();
    let mut profiles = state.profile.profiles.clone();
    let old_name = profiles[0].name.clone();
    profiles[0].name = "Refreshed profile".into();
    assert_eq!(
        state.update(Message::ProfilesLoaded(Ok(profiles))).units(),
        0
    );
    assert_eq!(state.shell.command_catalogue.len(), count);
    assert!(
        state
            .shell
            .command_catalogue
            .entries()
            .iter()
            .any(|entry| entry.profile_name() == Some("Refreshed profile"))
    );
    assert!(
        !state
            .shell
            .command_catalogue
            .entries()
            .iter()
            .any(|entry| entry.profile_name() == Some(old_name.as_str()))
    );
    let _ = state.update(Message::SetCommandQuery("Refreshed profile".into()));
    assert_eq!(state.filtered_command_indices().len(), 1);
    drop(state.view());
}
