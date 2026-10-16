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
        queue_action(
            &tx,
            &weak_for_start,
            Ok(SlintAction::StartRuntime),
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
    let tx_for_restore = requests.clone();
    let weak_for_restore = weak.clone();
    window.on_retry_proxy_restore(move || {
        queue_intent(&tx_for_restore, &weak_for_restore, CommandIntent::RecoverSystemProxy);
    });

    let tx_for_add_override = requests.clone();
    let weak_for_add_override = weak.clone();
    window.on_add_override(move |name, content| {
        queue_intent(
            &tx_for_add_override,
            &weak_for_add_override,
            CommandIntent::CreateNamedOverride {
                name: name.to_string(),
                content: content.to_string(),
            },
        );
    });
    let weak_for_override_import = weak.clone();
    window.on_import_override_url(move |_| {
        show_unsupported(
            &weak_for_override_import,
            "Named override imports have no shared owner",
        );
    });
    let tx_for_override_file = requests.clone();
    let weak_for_override_file = weak.clone();
    let latest_for_override_file = Arc::clone(&latest);
    window.on_choose_override_file(move || {
        #[cfg(not(target_os = "android"))]
        {
            let picked = rfd::FileDialog::new()
                .add_filter("YAML Overrides", &["yaml", "yml", "txt"])
                .pick_file();
            if let Some(path) = picked {
                let path_str = path.to_string_lossy().to_string();
                queue_action(
                    &tx_for_override_file,
                    &weak_for_override_file,
                    require_snapshot(&latest_for_override_file).map(|_| SlintAction::ImportOverrideFile {
                        path: path_str,
                    }),
                );
            }
        }
        #[cfg(target_os = "android")]
        {
            show_unsupported(&weak_for_override_file, "Android SAF file selection is handled through Intent");
        }
    });
    let tx_for_save_override = requests.clone();
    let weak_for_save_override = weak.clone();
    window.on_save_override(move |id, content| {
        queue_intent(
            &tx_for_save_override,
            &weak_for_save_override,
            CommandIntent::UpdateNamedOverride {
                id: id.to_string(),
                name: None,
                content: Some(content.to_string()),
            },
        );
    });
    let tx_for_delete_override = requests.clone();
    let weak_for_delete_override = weak.clone();
    window.on_delete_override(move |id| {
        queue_intent(
            &tx_for_delete_override,
            &weak_for_delete_override,
            CommandIntent::DeleteNamedOverride {
                id: id.to_string(),
            },
        );
    });
    let weak_for_override_export = weak.clone();
    let latest_for_override_export = Arc::clone(&latest);
    let tx_for_override_export = requests.clone();
    window.on_export_override(move |override_id| {
        let override_id = override_id.to_string();
        #[cfg(not(target_os = "android"))]
        {
            let picked = rfd::FileDialog::new()
                .set_file_name(&format!("{override_id}.yaml"))
                .add_filter("YAML Override", &["yaml", "yml"])
                .save_file();
            if let Some(destination) = picked {
                queue_action(
                    &tx_for_override_export,
                    &weak_for_override_export,
                    require_snapshot(&latest_for_override_export).map(|_| SlintAction::ExportOverride {
                        override_id,
                        destination,
                    }),
                );
            }
        }
        #[cfg(target_os = "android")]
        {
            show_unsupported(&weak_for_override_export, "Android SAF file export is handled through Intent");
        }
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
