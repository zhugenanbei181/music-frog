//! test-intent: behavior
use crate::test_support::demo_env;
use iced::advanced::widget::Tree;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;

fn mounted_nodes(tree: &Tree) -> usize {
    1 + tree.children.iter().map(mounted_nodes).sum::<usize>()
}

#[test]
fn group_fold_replays_a_real_smaller_widget_tree_and_reopens_without_side_effects() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Proxies));
    let group = state.runtime.filtered_groups.first().unwrap().0.clone();
    assert!(
        state
            .runtime
            .proxy_ui_preferences
            .collapsed_groups
            .is_empty()
    );
    let original = mounted_nodes(&Tree::new(state.view().as_widget()));
    assert_eq!(
        state
            .update(Message::ToggleProxyGroupExpanded(group.clone()))
            .units(),
        0
    );
    assert!(!state.runtime.proxy_ui_preferences.is_group_expanded(&group));
    let collapsed = mounted_nodes(&Tree::new(state.view().as_widget()));
    assert!(
        collapsed < original,
        "closing the group must remove its visible native node cards"
    );
    assert_eq!(
        state
            .update(Message::ToggleProxyGroupExpanded(group.clone()))
            .units(),
        0
    );
    assert!(state.runtime.proxy_ui_preferences.is_group_expanded(&group));
    assert_eq!(
        mounted_nodes(&Tree::new(state.view().as_widget())),
        original
    );
}

#[test]
fn group_fold_rejects_a_removed_identity_and_keeps_other_group_choices() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Proxies));
    let groups: Vec<_> = state
        .runtime
        .filtered_groups
        .iter()
        .map(|(name, _)| name.clone())
        .collect();
    assert!(groups.len() > 1);
    assert_eq!(
        state
            .update(Message::ToggleProxyGroupExpanded(groups[0].clone()))
            .units(),
        0
    );
    let expected = state.runtime.proxy_ui_preferences.clone();
    assert!(expected.is_group_expanded(&groups[1]));
    let mounted = mounted_nodes(&Tree::new(state.view().as_widget()));
    state.runtime.filtered_groups.reverse();
    assert_eq!(mounted_nodes(&Tree::new(state.view().as_widget())), mounted);
    assert_eq!(
        state
            .update(Message::ToggleProxyGroupExpanded(groups[1].clone()))
            .units(),
        0
    );
    assert!(
        !state
            .runtime
            .proxy_ui_preferences
            .is_group_expanded(&groups[0])
    );
    assert!(
        !state
            .runtime
            .proxy_ui_preferences
            .is_group_expanded(&groups[1])
    );
    assert_eq!(
        state
            .update(Message::ToggleProxyGroupExpanded(groups[1].clone()))
            .units(),
        0
    );
    assert_eq!(state.runtime.proxy_ui_preferences, expected);

    state.runtime.proxies.remove(&groups[0]);
    assert_eq!(
        state
            .update(Message::ToggleProxyGroupExpanded(groups[0].clone()))
            .units(),
        0
    );
    assert_eq!(state.runtime.proxy_ui_preferences, expected);
    assert_eq!(
        state
            .update(Message::ToggleProxyGroupExpanded("unknown-group".into()))
            .units(),
        0
    );
    assert_eq!(state.runtime.proxy_ui_preferences, expected);
}

#[test]
fn newly_published_groups_expand_without_reopening_the_users_collapsed_group() {
    use infiltrator_domain::proxy::{Proxy, ProxyGroup};

    let (mut state, _) = AppState::demo(&demo_env(Route::Proxies));
    let (closed, members) = state.runtime.filtered_groups[0].clone();
    assert_eq!(
        state
            .update(Message::ToggleProxyGroupExpanded(closed.clone()))
            .units(),
        0
    );
    let collapsed = mounted_nodes(&Tree::new(state.view().as_widget()));
    let name = "newly-published-group".to_owned();
    state.runtime.proxies.insert(
        name.clone(),
        Proxy::Selector(ProxyGroup {
            name: name.clone(),
            all: members.clone(),
            now: members[0].clone(),
            ..Default::default()
        }),
    );
    state.recompute_filtered_groups();
    assert!(
        !state
            .runtime
            .proxy_ui_preferences
            .is_group_expanded(&closed)
    );
    assert!(state.runtime.proxy_ui_preferences.is_group_expanded(&name));
    let expanded = mounted_nodes(&Tree::new(state.view().as_widget()));
    assert!(
        expanded > collapsed,
        "new group must render native node controls"
    );
    assert_eq!(
        state
            .update(Message::ToggleProxyGroupExpanded(name.clone()))
            .units(),
        0
    );
    let folded = mounted_nodes(&Tree::new(state.view().as_widget()));
    assert!(folded < expanded);
    assert!(
        !state
            .runtime
            .proxy_ui_preferences
            .is_group_expanded(&closed)
    );
    assert_eq!(
        state
            .update(Message::ToggleProxyGroupExpanded(name))
            .units(),
        0
    );
    assert_eq!(
        mounted_nodes(&Tree::new(state.view().as_widget())),
        expanded
    );
}

#[test]
fn nonanimated_proxy_capture_requests_real_frames_until_its_receipt_is_written() {
    use infiltrator_contract::parity::FeatureId;
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;
    use std::time::Instant;

    let (mut state, _) = AppState::demo(&demo_env(Route::Proxies));
    let baseline = state.subscription().units();
    state.shell.capture_marker = Some(PathBuf::from("unused-native-layout-receipt"));
    state.shell.capture_scenario = Some(FeatureId::ProxiesGroupExpanded);
    assert_eq!(state.subscription().units(), baseline + 1);
    assert_eq!(state.update(Message::TickFrame(Instant::now())).units(), 1);
    state
        .shell
        .capture_marker_written
        .store(true, Ordering::SeqCst);
    assert_eq!(state.subscription().units(), baseline);
    assert_eq!(state.update(Message::TickFrame(Instant::now())).units(), 0);
}

#[test]
fn stable_node_widget_ids_keep_names_with_delimiters_distinct() {
    use infiltrator_iced::view::proxies::node_region_id;
    assert_ne!(node_region_id("a:b", "c"), node_region_id("a", "b:c"));
    assert_eq!(
        node_region_id("香港", "node:one"),
        node_region_id("香港", "node:one")
    );
}
