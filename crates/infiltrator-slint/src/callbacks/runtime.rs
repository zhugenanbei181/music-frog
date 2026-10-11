//! Slint runtime callbacks routed to the shared product owner.

use super::super::*;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub(super) fn register(
    window: &AppWindow,
    requests: Sender<WorkerRequest>,
    latest: Arc<Mutex<Option<SurfaceSnapshot>>>,
) {
    let weak = window.as_weak();
    let tx = requests.clone();
    let weak_for_start = weak.clone();
    window.on_start_runtime(move || {
        queue_intent(
            &tx,
            &weak_for_start,
            if cfg!(target_os = "android") {
                CommandIntent::StartVpn
            } else {
                CommandIntent::StartProxy
            },
        );
    });
    let tx = requests.clone();
    let weak_for_stop = weak.clone();
    window.on_stop_runtime(move || {
        queue_intent(
            &tx,
            &weak_for_stop,
            if cfg!(target_os = "android") {
                CommandIntent::StopVpn
            } else {
                CommandIntent::StopProxy
            },
        );
    });

    let tx = requests.clone();
    let latest_for_route = Arc::clone(&latest);
    let weak_for_route = weak.clone();
    window.on_set_route_mode(move |mode| {
        let intent = match mode.as_str() {
            "Rule" => Ok(RoutingIntent::FollowRules),
            "Global" => Ok(RoutingIntent::RouteAllThroughProxy),
            "Direct" => Ok(RoutingIntent::BypassProxy),
            _ => Err(Failure::new(
                ErrorCode::InvalidInput,
                "Unknown routing mode",
                false,
            )),
        };
        let action = require_snapshot(&latest_for_route).and_then(|_| {
            intent.map(|intent| SlintAction::Command(CommandIntent::SetRoutingIntent { intent }))
        });
        queue_action(&tx, &weak_for_route, action);
    });

    let tx = requests.clone();
    let latest_for_network = Arc::clone(&latest);
    let weak_for_network = weak.clone();
    window.on_set_network_mode(move |mode| {
        let action = require_snapshot(&latest_for_network)
            .and_then(|snapshot| network_mode_action(&snapshot, mode.as_str()));
        queue_action(&tx, &weak_for_network, action);
    });

    let tx = requests.clone();
    let weak_for_check = weak.clone();
    window.on_check_core(move || {
        queue_intent(&tx, &weak_for_check, CommandIntent::CheckUpdates);
    });
    let weak_for_restore = weak.clone();
    window.on_retry_proxy_restore(move || {
        show_unsupported(&weak_for_restore, "The shared network owner does not expose system-proxy journal recovery as a user command");
    });

    let weak_for_override_actions = weak.clone();
    window.on_add_override(move |_, _| {
        show_unsupported(
            &weak_for_override_actions,
            "Named YAML overrides are not represented by the shared profile model",
        );
    });
    let weak_for_override_import = weak.clone();
    window.on_import_override_url(move |_| {
        show_unsupported(
            &weak_for_override_import,
            "Named override imports have no shared owner",
        );
    });
    let weak_for_override_file = weak.clone();
    window.on_choose_override_file(move || {
        show_unsupported(
            &weak_for_override_file,
            "Override file selection needs a platform file-picker adapter",
        );
    });
    let weak_for_override_save = weak.clone();
    window.on_save_override(move |_, _| {
        show_unsupported(
            &weak_for_override_save,
            "Named YAML overrides are not represented by the shared profile model",
        );
    });
    let weak_for_override_delete = weak.clone();
    window.on_delete_override(move |_| {
        show_unsupported(
            &weak_for_override_delete,
            "Named YAML overrides are not represented by the shared profile model",
        );
    });
    let weak_for_override_export = weak.clone();
    window.on_export_override(move |_| {
        show_unsupported(
            &weak_for_override_export,
            "Override export needs a platform file-output adapter",
        );
    });
    let weak_for_override_edit = weak.clone();
    window.on_edit_override(move |_| {
        show_unsupported(
            &weak_for_override_edit,
            "Named YAML overrides are not represented by the shared profile model",
        );
    });
    let weak_for_override_apply = weak.clone();
    window.on_apply_override(move |_| {
        show_unsupported(
            &weak_for_override_apply,
            "Named YAML overrides are not represented by the shared profile model",
        );
    });
}
