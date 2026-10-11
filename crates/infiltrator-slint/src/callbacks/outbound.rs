//! Slint outbound callbacks routed to the shared product owner.

use super::super::*;
use crate::lumen_projection::collections::project_connection_detail;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub(super) fn register(
    window: &AppWindow,
    requests: Sender<WorkerRequest>,
    latest: Arc<Mutex<Option<SurfaceSnapshot>>>,
) {
    let weak = window.as_weak();
    let weak_for_group = weak.clone();
    window.on_select_proxy_group(move |group| {
        let _ = weak_for_group.clone().upgrade_in_event_loop(move |window| {
            window.set_selected_group(group);
        });
    });

    let tx = requests.clone();
    let latest_for_select = Arc::clone(&latest);
    let weak_for_select = weak.clone();
    window.on_select_proxy(move |group, node| {
        let action = selection_action(&latest_for_select, group.as_str(), node.as_str());
        queue_action(&tx, &weak_for_select, action);
    });

    let tx = requests.clone();
    let latest_for_search_proxy = Arc::clone(&latest);
    let weak_for_search_proxy = weak.clone();
    window.on_search_proxy_nodes(move |query| {
        let action = outbound_preference_action(
            &latest_for_search_proxy,
            OutboundUiPreferenceIntent::SetSearchQuery(query.to_string()),
        );
        queue_action(&tx, &weak_for_search_proxy, action);
    });
    let tx = requests.clone();
    let latest_for_proxy_sort = Arc::clone(&latest);
    let weak_for_proxy_sort = weak.clone();
    window.on_set_proxy_sort(move |sort| {
        let order = match sort.as_str() {
            "latency_asc" => Some(OutboundSortOrder::LatencyAscending),
            "latency_desc" => Some(OutboundSortOrder::LatencyDescending),
            "name_asc" => Some(OutboundSortOrder::NameAscending),
            "name_desc" => Some(OutboundSortOrder::NameDescending),
            _ => None,
        };
        match order {
            Some(order) => {
                let action = outbound_preference_action(
                    &latest_for_proxy_sort,
                    OutboundUiPreferenceIntent::SetSortOrder(order),
                );
                queue_action(&tx, &weak_for_proxy_sort, action);
            }
            None => show_failure(
                &weak_for_proxy_sort,
                Failure::new(ErrorCode::InvalidInput, "Unknown proxy sort order", false),
            ),
        }
    });

    let tx = requests.clone();
    let latest_for_probe = Arc::clone(&latest);
    let weak_for_probe = weak.clone();
    window.on_test_proxy(move |group, node| {
        let group_name = group.to_string();
        let node_id = node.to_string();
        let weak = weak_for_probe.clone();
        let group_for_ui = group_name.clone();
        let node_for_ui = node_id.clone();
        let _ = weak.clone().upgrade_in_event_loop(move |window| {
            window.set_proxy_testing_group(group_for_ui.into());
            window.set_proxy_testing_node(node_for_ui.into());
        });
        let action = probe_action(&latest_for_probe, &group_name, &node_id);
        queue_action(&tx, &weak_for_probe, action);
    });

    let tx = requests.clone();
    let weak_for_egress = weak.clone();
    window.on_refresh_home_egress(move || {
        queue_intent(&tx, &weak_for_egress, CommandIntent::RefreshPublicIpProbe);
    });

    let tx = requests.clone();
    let weak_for_close_connection = weak.clone();
    window.on_close_connection(move |id| {
        let id = id.to_string();
        let weak = weak_for_close_connection.clone();
        let pending_id = id.clone();
        let _ = weak.clone().upgrade_in_event_loop(move |window| {
            window.set_connection_breaking_id(pending_id.into());
        });
        queue_intent(
            &tx,
            &weak_for_close_connection,
            CommandIntent::CloseConnection { id },
        );
    });
    let latest_for_open_connection = Arc::clone(&latest);
    let weak_for_open_connection = weak.clone();
    window.on_open_connection(move |id| {
        if let Some(snapshot) = clone_latest(&latest_for_open_connection) {
            let id = id.to_string();
            let _ = weak_for_open_connection
                .clone()
                .upgrade_in_event_loop(move |window| {
                    project_connection_detail(&window, &snapshot, &id);
                });
        }
    });

    let weak_for_connection_search = weak.clone();
    let latest_for_connection_search = Arc::clone(&latest);
    window.on_search_connections(move |_| {
        refresh_projection(&weak_for_connection_search, &latest_for_connection_search);
    });
    let weak_for_closed = weak.clone();
    window.on_set_connection_view(move |show_closed| {
        if show_closed {
            let _ = weak_for_closed.clone().upgrade_in_event_loop(|window| {
                window.set_connection_show_closed(false);
                window.set_notice(
                    "Closed connection history is not published by the shared owner".into(),
                );
            });
        }
    });
    let weak_for_connection_sort = weak.clone();
    let latest_for_connection_sort = Arc::clone(&latest);
    window.on_set_connection_sort(move |_| {
        refresh_projection(&weak_for_connection_sort, &latest_for_connection_sort);
    });

    let weak_for_log_search = weak.clone();
    let latest_for_log_search = Arc::clone(&latest);
    window.on_search_logs(move |_| {
        refresh_projection(&weak_for_log_search, &latest_for_log_search);
    });
    let tx = requests.clone();
    let weak_for_log_level = weak.clone();
    window.on_set_log_level(move |level| {
        let level = level.to_string();
        queue_intent(
            &tx,
            &weak_for_log_level,
            CommandIntent::SetLogLevelFilter {
                level: if level == "all" { None } else { Some(level) },
            },
        );
    });

    let weak_for_rule_search = weak.clone();
    let latest_for_rule_search = Arc::clone(&latest);
    window.on_search_rules(move |_| {
        refresh_projection(&weak_for_rule_search, &latest_for_rule_search);
    });
    let tx = requests.clone();
    let latest_for_rule = Arc::clone(&latest);
    let weak_for_rule = weak.clone();
    window.on_set_rule_enabled(move |index, enabled| {
        let action = require_snapshot(&latest_for_rule).and_then(|snapshot| {
            let index = index.to_string().parse::<usize>().map_err(|_| {
                Failure::new(ErrorCode::InvalidInput, "Invalid rule identity", false)
            })?;
            let rule = snapshot
                .pages
                .rules
                .data
                .as_ref()
                .and_then(|page| page.rules.iter().find(|rule| rule.id == index))
                .ok_or_else(|| Failure::new(ErrorCode::NotReady, "Rule was not observed", true))?;
            if rule.is_enabled == enabled {
                return Ok(None);
            }
            Ok(Some(SlintAction::Command(
                CommandIntent::ToggleRuleEnabled { index },
            )))
        });
        match action {
            Ok(Some(action)) => {
                let weak = weak_for_rule.clone();
                let pending_index = index.to_string();
                let _ = weak.clone().upgrade_in_event_loop(move |window| {
                    window.set_rule_pending_index(pending_index.into());
                });
                queue_action(&tx, &weak_for_rule, Ok(action));
            }
            Ok(None) => {}
            Err(failure) => show_failure(&weak_for_rule, failure),
        }
    });
    let tx = requests.clone();
    let weak_for_provider = weak.clone();
    window.on_update_provider(move |name| {
        // RefreshRuleProviders is the shared batch operation; the UI carries
        // one row id but the kernel owns the provider set and update ordering.
        let _ = name;
        queue_intent(&tx, &weak_for_provider, CommandIntent::RefreshRuleProviders);
    });

    let weak_for_export_logs = weak.clone();
    window.on_export_logs(move || {
        show_unsupported(
            &weak_for_export_logs,
            "Log export needs the shared prepared-export review and a platform file-output adapter",
        );
    });
    let tx = requests.clone();
    let weak_for_observations = weak.clone();
    window.on_refresh_observations(move || {
        queue_intent(
            &tx,
            &weak_for_observations,
            CommandIntent::ReadOutboundInventory,
        );
    });
}
