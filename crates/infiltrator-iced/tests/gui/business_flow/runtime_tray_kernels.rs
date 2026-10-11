//! Journeys 8/9/10/11 — runtime lifecycle degradation, typed routing choices,
//! kernel management with the throttled tray refresh, and the tray event
//! chains (resolve → intent → forwarded message).
//!
//! test-intent: behavior

use super::support::{FakeTray, TempHome, block_on, feed, fresh_state, subscribed_profile};
use crate::state::core_operations::{CoreDownloadRunId, CoreDownloadStart};
use crate::tray::spec::{
    TRAY_ACTION_ACTIVATE_PROFILE, TRAY_ACTION_ROUTING_ALL_THROUGH_PROXY, TRAY_ACTION_SELECT_PROXY,
    TRAY_ACTION_SET_DEFAULT_KERNEL, TRAY_ACTION_SET_PROFILE_AUTO_UPDATE,
    TRAY_ACTION_UPDATE_ALL_PROFILES, TrayEvent, TrayEventContext, TrayIntent, TrayMenuItem,
    encode_pair_payload, resolve_tray_event_in,
};
use crate::types::app::CoreDownloadProgress;
use crate::types::message::Message;
use crate::types::runtime::RuntimeStreamState;
use crate::version_application::application;
use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::port_conflict::{PortBinding, PortConflict, PortConflictSnapshot};
use infiltrator_contract::routing_intent::RoutingIntentSnapshot;
use infiltrator_contract::routing_intent::{
    EngineDefinedRoutingIntent, RoutingIntent, RoutingIntentExtensionId,
};
use infiltrator_contract::version::{CoreRollbackSnapshot, InstalledCoreVersion};
use infiltrator_core::version_port::MihomoVersionPort;
use infiltrator_domain::runtime::TrafficData;
use mihomo_platform::paths::get_home_dir;
use mihomo_version::manager::VersionManager;
#[cfg(unix)]
use std::fs::Permissions;
#[cfg(unix)]
use std::fs::create_dir_all;
#[cfg(unix)]
use std::fs::set_permissions;
#[cfg(unix)]
use std::fs::write;
#[cfg(unix)]
use std::path;

/// Core stop readback clears transient runtime observations.
#[test]
fn core_stopped_clears_transient_runtime_observations() {
    let mut state = fresh_state();
    let tray = FakeTray::install(&mut state);

    // Seed every domain CoreStopped is supposed to clear.
    state.diag.traffic = Some(TrafficData { up: 1, down: 2 });
    state.diag.logs.push_back("stale log".into());
    state.runtime.routing_intent = Some(RoutingIntent::FollowRules);
    state.runtime.routing_intent_state = RoutingIntentSnapshot::demo_fixture();
    state.runtime.tun_enabled = Some(true);
    state.runtime.runtime_selected_group = "GLOBAL".into();

    let units = feed(&mut state, Message::CoreStopped);
    assert_eq!(units, 0);
    assert!(state.diag.traffic.is_none(), "traffic cleared");
    assert!(state.diag.logs.is_empty(), "logs cleared");
    assert!(state.runtime.routing_intent.is_none(), "intent reset");
    assert_eq!(
        state.runtime.routing_intent_state,
        RoutingIntentSnapshot::default()
    );
    assert_eq!(state.runtime.tun_enabled, None);
    assert!(state.runtime.runtime_selected_group.is_empty());
    assert_eq!(state.diag.traffic_stream_state, RuntimeStreamState::Idle);

    // CoreStopped pushes the refreshed (stopped) tray spec without dying.
    assert!(
        tray.count() >= 1,
        "refresh_tray ran against the fake controller"
    );
}

/// Journey 9 — engine-defined policies stay outside the shared selectable choices.
#[test]
fn engine_defined_routing_intent_is_rejected_by_the_shared_ui_action() {
    let mut state = fresh_state();
    state.runtime.routing_intent = Some(RoutingIntent::FollowRules);
    state.runtime.routing_intent_state = RoutingIntentSnapshot::demo_fixture();
    state.runtime.routing_intent_actions.observe(
        state.runtime.runtime_generation,
        1,
        state.runtime.routing_intent_state.clone(),
    );
    let engine_defined = RoutingIntent::EngineDefined(EngineDefinedRoutingIntent::new(
        EngineId::new("mihomo").unwrap(),
        RoutingIntentExtensionId::new("script").unwrap(),
    ));

    // The typed extension never enters the common three-choice UI path.
    let units = feed(&mut state, Message::SetRoutingIntent(engine_defined));
    assert_eq!(
        units, 0,
        "refusal stays in the persistent operation surface"
    );
    assert!(state.runtime.routing_intent_actions.failure.is_some());
    assert_eq!(
        state.runtime.routing_intent.as_ref(),
        Some(&RoutingIntent::FollowRules),
        "observed intent remains unchanged by the refusal"
    );
    assert!(
        state.runtime.pending_runtime_patch.is_none(),
        "no patch armed"
    );
    assert!(
        state.shell.error_msg.is_none(),
        "routing refusal does not use the generic error banner"
    );
}

/// Journey 10 — 内核管理：真实 VersionManager（temp HOME）列表→设默认→
/// 回流；CoreDownloadProgress 节流（1s 内多次进度只刷一次托盘）+ 假 token
/// 丢弃。
#[test]
fn kernel_management_round_trip_and_download_progress_tray_throttle() {
    let home = TempHome::acquire("kernels");
    let mut state = fresh_state();
    let tray = FakeTray::install(&mut state);

    // LoadKernels task body for real on an empty store.
    let versions = block_on(async {
        application()
            .unwrap()
            .list_installed()
            .await
            .unwrap_or_else(|failure| panic!("list kernels: {}", failure.message))
    });
    assert!(versions.is_empty());
    let units = feed(&mut state, Message::KernelsLoaded(Ok(versions)));
    assert_eq!(units, 0);
    assert_eq!(tray.count(), 1, "KernelsLoaded refreshes the tray");

    // Install (directory plant with a runnable stand-in binary), set default
    // through the real manager, feed the operation result, reload.
    plant_runnable_fake_binary(&home, "v1.19.18");

    let units = feed(&mut state, Message::SetDefaultKernel("v1.19.18".into()));
    assert_eq!(units, 1);
    block_on(async {
        VersionManager::for_engine(
            get_home_dir().unwrap(),
            MihomoVersionPort::engine_identifier(),
        )
        .unwrap()
        .set_default("v1.19.18")
        .await
        .unwrap()
    });
    let units = feed(&mut state, Message::KernelOperationFinished(Ok(())));
    assert_eq!(units, 1, "result chains LoadKernels");

    let versions = block_on(async {
        application()
            .unwrap()
            .list_installed()
            .await
            .unwrap_or_else(|failure| panic!("list kernels: {}", failure.message))
    });
    assert_eq!(versions.len(), 1);
    assert!(
        versions[0].is_default,
        "default pointer persisted to config.toml"
    );
    feed(&mut state, Message::KernelsLoaded(Ok(versions.clone())));
    assert_eq!(state.runtime.installed_kernels.len(), 1);
    assert_eq!(state.runtime.installed_kernels[0].version, "v1.19.18");

    // KernelOperationFinished(Err) → banner + error toast + tray refresh.
    let units = feed(
        &mut state,
        Message::KernelOperationFinished(Err(InfiltratorError::Internal("删除失败".into()))),
    );
    assert!(state.shell.error_msg.is_some());
    assert!(units >= 1, "error toast armed");
    assert!(tray.count() >= 2);

    // Download progress: throttled tray refresh — many ticks inside the
    // 1s window update the stats each time but push at most one spec.
    let run_id = match state.runtime.core_download.start() {
        CoreDownloadStart::Started { run_id, .. } => run_id,
        CoreDownloadStart::AlreadyDownloading => panic!("first download must start"),
    };
    let count_before = tray.count();
    for downloaded in [100u64, 300, 500, 700] {
        let units = feed(
            &mut state,
            Message::CoreDownloadProgress {
                progress: CoreDownloadProgress {
                    downloaded,
                    total: Some(1000),
                    speed_bytes: 10,
                },
                run_id,
            },
        );
        assert_eq!(units, 0);
        assert_eq!(
            state.runtime.core_download.progress().unwrap().downloaded,
            downloaded,
            "stats update on every tick"
        );
    }
    assert_eq!(state.runtime.core_download.progress_fraction(), 0.7);
    assert_eq!(
        tray.count() - count_before,
        1,
        "tray spec pushed at most once per second (throttled)"
    );

    // A stale token is dropped before touching anything.
    let units = feed(
        &mut state,
        Message::CoreDownloadProgress {
            progress: CoreDownloadProgress {
                downloaded: 999,
                total: Some(1000),
                speed_bytes: 10,
            },
            run_id: CoreDownloadRunId::default(),
        },
    );
    assert_eq!(units, 0);
    assert_eq!(
        state.runtime.core_download.progress().unwrap().downloaded,
        700
    );
}

/// DUAL-01-06 — 本地版本历史只做默认指针切换：回滚不发网络请求，先由
/// VersionManager 校验候选 binary，再回灌 Iced 的统一内核操作结果。
#[cfg(unix)]
#[test]
fn kernel_rollback_round_trip_uses_the_shared_operation_path() {
    let _home = TempHome::acquire("kernel-rollback");
    let mut state = fresh_state();
    let manager = VersionManager::for_engine(
        get_home_dir().unwrap(),
        MihomoVersionPort::engine_identifier(),
    )
    .unwrap();
    plant_runnable_fake_binary(&_home, "v1.19.28");
    plant_runnable_fake_binary(&_home, "v1.19.29");

    block_on(manager.set_default("v1.19.28")).unwrap();
    block_on(manager.set_default("v1.19.29")).unwrap();
    let before = block_on(manager.rollback_info()).unwrap();
    assert_eq!(before.current.as_deref(), Some("v1.19.29"));
    assert_eq!(before.target.as_deref(), Some("v1.19.28"));
    state.runtime.core_versions.rollback = CoreRollbackSnapshot {
        engine_id: EngineId::new("mihomo").unwrap(),
        current: before.current,
        target: before.target,
        history: before.history,
    };

    assert_eq!(feed(&mut state, Message::RollbackCore), 1);
    block_on(manager.rollback()).unwrap();
    assert_eq!(block_on(manager.get_default()).unwrap(), "v1.19.28");
    assert_eq!(
        feed(&mut state, Message::KernelOperationFinished(Ok(()))),
        1
    );

    let after = block_on(manager.rollback_info()).unwrap();
    assert_eq!(after.current.as_deref(), Some("v1.19.28"));
    assert_eq!(after.target, None, "rollback consumes the historical stack");
}

#[test]
fn core_log_level_requires_owner_and_does_not_optimistically_update() {
    let mut state = fresh_state();
    assert_eq!(state.diag.log_level, "info");

    assert_eq!(
        feed(&mut state, Message::SetCoreLogLevel(CoreLogLevel::Debug)),
        0
    );
    assert_eq!(state.diag.log_level, "info");
    assert!(
        state
            .shell
            .error_msg
            .as_deref()
            .unwrap_or_default()
            .contains("core is not running")
    );
}

#[test]
fn port_conflict_repair_replaces_the_shared_snapshot_without_killing_unknown_pids() {
    let mut state = fresh_state();
    state.runtime.port_conflicts = PortConflictSnapshot {
        revision: 1,
        conflicts: vec![PortConflict {
            binding: PortBinding::Controller,
            port: 9090,
            available: false,
            owner_pid: Some(4242),
            owner_name: Some("unrelated-app".to_owned()),
            can_release: false,
        }],
    };
    assert_eq!(feed(&mut state, Message::RepairPortConflicts), 1);

    let repaired = PortConflictSnapshot {
        revision: 2,
        conflicts: vec![PortConflict {
            binding: PortBinding::Controller,
            port: 9091,
            available: true,
            owner_pid: None,
            owner_name: None,
            can_release: false,
        }],
    };
    assert_eq!(
        feed(&mut state, Message::PortConflictsRepaired(Ok(repaired))),
        1
    );
    assert!(!state.runtime.port_conflicts.has_conflicts());
    assert!(state.shell.error_msg.is_none());
}

#[cfg(unix)]
fn plant_runnable_fake_binary(home: &path::Path, version: &str) {
    use std::os::unix::fs::PermissionsExt;
    let dir = home.join("engines/mihomo/versions").join(version);
    create_dir_all(&dir).unwrap();
    let bin = dir.join("mihomo");
    write(&bin, "#!/bin/sh\necho \"Mihomo Meta v1.19.18 test\"\n").unwrap();
    set_permissions(&bin, Permissions::from_mode(0o755)).unwrap();
}

/// Journey 11 — 托盘全链：MenuActivated → resolve（真实当前状态）→ intent →
/// 转发消息 → 域状态迁移；同 id 不同 payload 的 checkmark 解析互不串扰。
#[test]
fn tray_event_chains_resolve_intents_and_drive_state_domains() {
    let mut state = fresh_state();
    state.runtime.tun_enabled = Some(false);
    state.runtime.autostart_enabled = false;
    state.profile.profiles = vec![
        subscribed_profile("Paid", true, Some("https://paid.example.com")),
        subscribed_profile("Free", false, None),
    ];

    // Node selection is a typed request only when an accepted owner inventory
    // has supplied the current scope and member revision.
    let event = TrayEvent::MenuActivated {
        id: TRAY_ACTION_SELECT_PROXY,
        payload: Some(encode_pair_payload("PROXY", "HK-1")),
    };
    // Snapshot for the pure resolver so state can keep mutating below.
    let profile_snapshot = state.profile.profiles.clone();
    let ctx = TrayEventContext {
        system_proxy: false,
        tun: false,
        autostart: false,
        profiles: &profile_snapshot,
    };
    assert!(
        matches!(
            resolve_tray_event_in(&event, &ctx),
            Some(TrayIntent::SelectOutbound { collection, outbound })
            if collection == "PROXY" && outbound == "HK-1"
        ),
        "pair payload resolves to the group/node pair"
    );
    assert!(state.outbound_selection_message("PROXY", "HK-1").is_none());
    assert!(
        state.runtime.proxy_groups.is_empty(),
        "no owner inventory was observed"
    );

    // ActivateProfile → SetActiveProfile: banner cleared, activation armed.
    let event = TrayEvent::MenuActivated {
        id: TRAY_ACTION_ACTIVATE_PROFILE,
        payload: Some("Free".into()),
    };
    state.shell.error_msg = Some("stale".into());
    let units = feed(&mut state, Message::TrayEvent(event));
    assert_eq!(units, 1, "resolve → forwarded SetActiveProfile task");
    feed(&mut state, Message::SetActiveProfile("Free".into()));
    assert!(state.shell.error_msg.is_none());

    // SetProfileAutoUpdate for the profile WITHOUT a URL → rejection toast,
    // Paid's own auto-update state untouched (no crosstalk).
    let event = TrayEvent::MenuActivated {
        id: TRAY_ACTION_SET_PROFILE_AUTO_UPDATE,
        payload: Some("Free".into()),
    };
    let intent = resolve_tray_event_in(&event, &ctx);
    assert!(
        matches!(
            &intent,
            Some(TrayIntent::SetProfileAutoUpdate { name, enabled })
            if name == "Free" && *enabled
        ),
        "resolver flips against the current per-profile flag"
    );
    // (Delivering the rejection toast is the router's job — and that is
    // exactly the broken link this documents; nothing observable happens.)
    feed(
        &mut state,
        Message::SetProfileAutoUpdate {
            name: "Free".into(),
            enabled: true,
        },
    );
    assert!(
        state.profile.profiles[0].auto_update_enabled,
        "Paid untouched"
    );
    assert!(
        !state.profile.profiles[1].auto_update_enabled,
        "Free stays off"
    );

    // Composite checkmark keys: the same action id resolves independently
    // per payload (Paid flips off, Free flips on) — no state bleed.
    let paid_event = TrayEvent::MenuActivated {
        id: TRAY_ACTION_SET_PROFILE_AUTO_UPDATE,
        payload: Some("Paid".into()),
    };
    let free_event = TrayEvent::MenuActivated {
        id: TRAY_ACTION_SET_PROFILE_AUTO_UPDATE,
        payload: Some("Free".into()),
    };
    assert!(matches!(
        resolve_tray_event_in(&paid_event, &ctx),
        Some(TrayIntent::SetProfileAutoUpdate { enabled: false, .. })
    ));
    assert!(matches!(
        resolve_tray_event_in(&free_event, &ctx),
        Some(TrayIntent::SetProfileAutoUpdate { enabled: true, .. })
    ));

    // Routing choice from the tray: no optimistic flip without a runtime;
    // SetRoutingIntent
    // lands in the runtime-unavailable branch.
    let units = feed(
        &mut state,
        Message::TrayEvent(TrayEvent::MenuActivated {
            id: TRAY_ACTION_ROUTING_ALL_THROUGH_PROXY,
            payload: None,
        }),
    );
    assert_eq!(units, 1);
    feed(
        &mut state,
        Message::SetRoutingIntent(RoutingIntent::RouteAllThroughProxy),
    );
    assert!(
        state.runtime.routing_intent_actions.failure.is_some(),
        "no runtime → persistent routing intent issue"
    );
    assert_eq!(
        state.runtime.routing_intent, None,
        "no optimistic flip without a runtime: the tray never lies"
    );
    assert!(state.runtime.pending_runtime_patch.is_none());
    assert_eq!(state.runtime.routing_intent_state.current, None);
    assert!(state.runtime.routing_intent_actions.failure.is_some());

    // Kernel default switch forwards the version payload.
    let units = feed(
        &mut state,
        Message::TrayEvent(TrayEvent::MenuActivated {
            id: TRAY_ACTION_SET_DEFAULT_KERNEL,
            payload: Some("v1.19.18".into()),
        }),
    );
    assert_eq!(units, 1, "resolve → forwarded SetDefaultKernel task");

    // The tray bulk "update all" entry resolves and its forwarded message
    // drives the real handler through the router (see profile_lifecycle).
    let units = feed(
        &mut state,
        Message::TrayEvent(TrayEvent::MenuActivated {
            id: TRAY_ACTION_UPDATE_ALL_PROFILES,
            payload: None,
        }),
    );
    assert_eq!(units, 1, "intent resolves and forwards…");
    let units = feed(&mut state, Message::UpdateAllSubscriptionsNow);
    assert!(units >= 1, "…into the live update-all handler");
    assert!(state.profile.is_updating_subscription_now);
}

/// Kernel entries surface in the tray spec: one nested submenu per version,
/// default-version actions disabled, info line naming the default.
#[test]
fn kernel_versions_flow_into_the_tray_spec_submenu() {
    let mut state = fresh_state();
    state.shell.lang = "zh-CN".into();
    state.runtime.installed_kernels = vec![
        InstalledCoreVersion {
            engine_id: EngineId::new("mihomo").unwrap(),
            version: "v1.19.18".into(),
            path: "/engines/mihomo/versions/v1.19.18".into(),
            is_default: true,
        },
        InstalledCoreVersion {
            engine_id: EngineId::new("mihomo").unwrap(),
            version: "v1.18.0".into(),
            path: "/engines/mihomo/versions/v1.18.0".into(),
            is_default: false,
        },
    ];

    let spec = state.current_tray_spec();
    let TrayMenuItem::Submenu {
        label,
        items: kernel_items,
        ..
    } = spec
        .menu
        .items
        .iter()
        .find(|item| matches!(item, TrayMenuItem::Submenu { label, .. } if label.contains("内核")))
        .expect("kernels submenu expected")
    else {
        panic!("kernels entry must be a submenu");
    };
    assert_eq!(label, "内核");

    // Info line names the default version.
    let info = &kernel_items[0];
    assert!(
        info.action_label().unwrap_or_default().contains("v1.19.18"),
        "default kernel version is displayed: {info:?}"
    );

    // One nested submenu per version with per-version default/ uninstall
    // actions: the current default's actions are disabled, the other's are
    // enabled and payload-tagged.
    let version_submenus: Vec<&TrayMenuItem> = kernel_items
        .iter()
        .filter(|item| matches!(item, TrayMenuItem::Submenu { .. }))
        .collect();
    assert_eq!(version_submenus.len(), 2);
    for entry in version_submenus {
        let TrayMenuItem::Submenu {
            label: version,
            items: actions,
            ..
        } = entry
        else {
            unreachable!()
        };
        let set_default = actions
            .iter()
            .find(|item| matches!(item, TrayMenuItem::Action { id, .. } if *id == TRAY_ACTION_SET_DEFAULT_KERNEL))
            .expect("set-default action present");
        let TrayMenuItem::Action {
            enabled, payload, ..
        } = set_default
        else {
            unreachable!()
        };
        assert_eq!(
            payload.as_deref(),
            Some(version.as_str()),
            "payload carries the version"
        );
        assert_eq!(
            *enabled,
            version != "v1.19.18",
            "default version's own switch action is disabled"
        );
    }
}
