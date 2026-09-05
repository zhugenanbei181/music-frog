//! Journeys 8/9/10/11 — runtime lifecycle degradation, script-mode gating,
//! kernel management with the throttled tray refresh, and the tray event
//! chains (resolve → intent → forwarded message).
//!
//! test-intent: behavior

use super::support::{TempHome, block_on, feed, fresh_state, subscribed_profile};
use crate::tray::spec::{
    TRAY_ACTION_ACTIVATE_PROFILE, TRAY_ACTION_MODE_GLOBAL, TRAY_ACTION_SELECT_PROXY,
    TRAY_ACTION_SET_DEFAULT_KERNEL, TRAY_ACTION_SET_PROFILE_AUTO_UPDATE,
    TRAY_ACTION_UPDATE_ALL_PROFILES, TrayEvent, TrayEventContext, encode_pair_payload,
    resolve_tray_event_in,
};
use crate::types::app::CoreDownloadProgress;
use crate::types::message::Message;
use crate::types::runtime::RuntimeStatus;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_domain::proxy::{Proxy, ProxyBase, ProxyGroup};
use mihomo_version::manager::VersionManager;

/// Journey 8 — 生命周期：StartProxy（无内核二进制）→ ProxyStarted(Err) →
/// 错误横幅 + Critical 系统通知任务武装（不炸）→ StopProxy → ProxyStopped
/// 全域清理。
#[test]
fn core_lifecycle_degrades_cleanly_from_boot_failure_to_full_stop_cleanup() {
    let mut state = fresh_state();
    let tray = super::support::FakeTray::install(&mut state);

    // Seed every domain ProxyStopped is supposed to clear.
    state.diag.traffic = Some(infiltrator_domain::runtime::TrafficData { up: 1, down: 2 });
    state.diag.logs.push_back("stale log".into());
    state.runtime.proxy_mode = Some("rule".into());
    state.runtime.script_block_present = true;
    state.runtime.tun_enabled = Some(true);
    state.runtime.runtime_selected_group = "GLOBAL".into();

    let token = state.runtime.lifecycle_token;
    let units = feed(&mut state, Message::StartProxy);
    assert_eq!(state.runtime.status, RuntimeStatus::Starting);
    assert_eq!(state.runtime.lifecycle_token, token + 1);
    assert!(state.shell.error_msg.is_none(), "start clears the banner");
    assert!(units >= 1, "boot task armed (would fail: no kernel)");

    // The boot fails (no kernel binary / no controller) → result 回灌.
    let units = feed(
        &mut state,
        Message::ProxyStarted(
            Err(InfiltratorError::Mihomo(
                "启动失败（已尝试控制端口 [9090]）".into(),
            )),
            token + 1,
        ),
    );
    assert!(matches!(state.runtime.status, RuntimeStatus::Error(_)));
    assert!(
        state
            .shell
            .error_msg
            .as_deref()
            .unwrap_or("")
            .contains("启动失败"),
        "error banner carries the boot failure"
    );
    assert!(
        units >= 1,
        "critical system notification armed; lazy, so no D-Bus here"
    );

    // User gives up: StopProxy → ProxyStopped cleanup across all domains.
    let units = feed(&mut state, Message::StopProxy);
    assert_eq!(state.runtime.status, RuntimeStatus::Stopped);
    assert!(units >= 1, "shutdown task armed (no runtime → no-op body)");

    let units = feed(&mut state, Message::ProxyStopped);
    assert_eq!(units, 0);
    assert!(state.diag.traffic.is_none(), "traffic cleared");
    assert!(state.diag.logs.is_empty(), "logs cleared");
    assert!(state.runtime.proxy_mode.is_none(), "mode reset");
    assert!(!state.runtime.script_block_present, "script gate closed");
    assert_eq!(state.runtime.tun_enabled, None);
    assert!(state.runtime.runtime_selected_group.is_empty());
    assert_eq!(
        state.diag.traffic_stream_state,
        crate::types::runtime::RuntimeStreamState::Idle
    );

    // ProxyStopped pushes the refreshed (stopped) tray spec without dying.
    assert!(
        tray.count() >= 1,
        "refresh_tray ran against the fake controller"
    );
}

/// Journey 9 — script 门控：`script_block_present=false` 时 SetProxyMode
/// ("script") 被同步拒绝（toast，无 PATCH、无快照）；门开后无内核走
/// runtime_unavailable 分支。
#[test]
fn script_mode_gate_refuses_without_script_block_then_hits_runtime_guard() {
    let mut state = fresh_state();
    state.runtime.proxy_mode = Some("rule".into());
    state.runtime.script_block_present = false;

    // Gate rejects synchronously: no optimistic flip, no patch snapshot.
    let units = feed(&mut state, Message::SetProxyMode("script".into()));
    assert_eq!(units, 1, "refusal arms only the error toast");
    assert_eq!(
        state.runtime.proxy_mode.as_deref(),
        Some("rule"),
        "mode untouched by the refusal"
    );
    assert!(
        state.runtime.pending_runtime_patch.is_none(),
        "no patch armed"
    );
    assert!(
        state.shell.error_msg.is_none(),
        "gate refusal is toast-only"
    );

    // script: block present → the gate opens, but with no runtime the
    // request lands in the runtime_unavailable branch (banner + toast).
    state.runtime.script_block_present = true;
    let units = feed(&mut state, Message::SetProxyMode("script".into()));
    assert!(state.shell.error_msg.is_some(), "runtime guard surfaces");
    assert_eq!(units, 1, "guard toast armed");
    assert_eq!(
        state.runtime.proxy_mode.as_deref(),
        Some("rule"),
        "no runtime → no optimistic switch"
    );
    assert!(state.runtime.pending_runtime_patch.is_none());
}

/// Journey 10 — 内核管理：真实 VersionManager（temp HOME）列表→设默认→
/// 回流；CoreDownloadProgress 节流（1s 内多次进度只刷一次托盘）+ 假 token
/// 丢弃。
#[test]
fn kernel_management_round_trip_and_download_progress_tray_throttle() {
    let home = TempHome::acquire("kernels");
    let mut state = fresh_state();
    let tray = super::support::FakeTray::install(&mut state);

    // LoadKernels task body for real on an empty store.
    let versions = block_on(async {
        crate::version_application::application()
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
        VersionManager::new()
            .unwrap()
            .set_default("v1.19.18")
            .await
            .unwrap()
    });
    let units = feed(&mut state, Message::KernelOperationFinished(Ok(())));
    assert_eq!(units, 1, "result chains LoadKernels");

    let versions = block_on(async {
        crate::version_application::application()
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
    state.runtime.core_download_token = 42;
    state.runtime.is_downloading_core = true;
    let count_before = tray.count();
    for downloaded in [100u64, 300, 500, 700] {
        let units = feed(
            &mut state,
            Message::CoreDownloadProgress(
                CoreDownloadProgress {
                    downloaded,
                    total: Some(1000),
                    speed_bytes: 10,
                },
                42,
            ),
        );
        assert_eq!(units, 0);
        assert_eq!(
            state.runtime.download_stats.as_ref().unwrap().downloaded,
            downloaded,
            "stats update on every tick"
        );
    }
    assert_eq!(state.runtime.download_progress, 0.7);
    assert_eq!(
        tray.count() - count_before,
        1,
        "tray spec pushed at most once per second (throttled)"
    );

    // A stale token is dropped before touching anything.
    let units = feed(
        &mut state,
        Message::CoreDownloadProgress(
            CoreDownloadProgress {
                downloaded: 999,
                total: Some(1000),
                speed_bytes: 10,
            },
            41,
        ),
    );
    assert_eq!(units, 0);
    assert_eq!(
        state.runtime.download_stats.as_ref().unwrap().downloaded,
        700
    );
}

#[cfg(unix)]
fn plant_runnable_fake_binary(home: &std::path::Path, version: &str) {
    use std::os::unix::fs::PermissionsExt;
    let dir = home.join("versions").join(version);
    std::fs::create_dir_all(&dir).unwrap();
    let bin = dir.join("mihomo");
    std::fs::write(&bin, "#!/bin/sh\necho \"Mihomo Meta v1.19.18 test\"\n").unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
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

    // 节点切换: pair payload decodes group/node; resolve produces SelectProxy
    // (with proxies present, the forwarded message is the real request).
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
            Some(crate::tray::spec::TrayIntent::SelectProxy { group, node })
            if group == "PROXY" && node == "HK-1"
        ),
        "pair payload resolves to the group/node pair"
    );
    state.runtime.proxies.insert(
        "PROXY".into(),
        Proxy::Selector(ProxyGroup {
            name: "PROXY".into(),
            now: "JP-1".into(),
            all: vec!["HK-1".into(), "JP-1".into()],
            history: Vec::new(),
        }),
    );
    let _ = state
        .runtime
        .proxies
        .insert("HK-1".into(), Proxy::Shadowsocks(shadowsocks_proxy()));
    // No runtime → the forwarded request degrades to a no-op task.
    let units = feed(
        &mut state,
        Message::SelectProxy("PROXY".into(), "HK-1".into()),
    );
    assert_eq!(units, 0, "no runtime → switch request is inert");
    assert_eq!(
        state.runtime.proxies["PROXY"].now(),
        Some("JP-1"),
        "unswitched"
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
            Some(crate::tray::spec::TrayIntent::SetProfileAutoUpdate { name, enabled })
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
        Some(crate::tray::spec::TrayIntent::SetProfileAutoUpdate { enabled: false, .. })
    ));
    assert!(matches!(
        resolve_tray_event_in(&free_event, &ctx),
        Some(crate::tray::spec::TrayIntent::SetProfileAutoUpdate { enabled: true, .. })
    ));

    // Mode switch from the tray: no optimistic flip without a runtime (the
    // tray must never show a mode that never took effect); SetProxyMode
    // lands in the runtime-unavailable branch.
    let units = feed(
        &mut state,
        Message::TrayEvent(TrayEvent::MenuActivated {
            id: TRAY_ACTION_MODE_GLOBAL,
            payload: None,
        }),
    );
    assert_eq!(units, 1);
    feed(&mut state, Message::SetProxyMode("global".into()));
    assert!(
        state.shell.error_msg.is_some(),
        "no runtime → unavailable banner"
    );
    assert_eq!(
        state.runtime.proxy_mode, None,
        "no optimistic flip without a runtime: the tray never lies"
    );

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

fn shadowsocks_proxy() -> infiltrator_domain::proxy::Shadowsocks {
    infiltrator_domain::proxy::Shadowsocks {
        base: ProxyBase::default(),
        ..Default::default()
    }
}

/// Kernel entries surface in the tray spec: one nested submenu per version,
/// default-version actions disabled, info line naming the default.
#[test]
fn kernel_versions_flow_into_the_tray_spec_submenu() {
    let mut state = fresh_state();
    state.shell.lang = "zh-CN".into();
    state.runtime.installed_kernels = vec![
        infiltrator_contract::version::InstalledCoreVersion {
            version: "v1.19.18".into(),
            path: "/versions/v1.19.18".into(),
            is_default: true,
        },
        infiltrator_contract::version::InstalledCoreVersion {
            version: "v1.18.0".into(),
            path: "/versions/v1.18.0".into(),
            is_default: false,
        },
    ];

    let spec = state.current_tray_spec();
    let crate::tray::spec::TrayMenuItem::Submenu { label, items: kernel_items, .. } = spec
        .menu
        .items
        .iter()
        .find(|item| {
            matches!(item, crate::tray::spec::TrayMenuItem::Submenu { label, .. } if label.contains("内核"))
        })
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
    let version_submenus: Vec<&crate::tray::spec::TrayMenuItem> = kernel_items
        .iter()
        .filter(|item| matches!(item, crate::tray::spec::TrayMenuItem::Submenu { .. }))
        .collect();
    assert_eq!(version_submenus.len(), 2);
    for entry in version_submenus {
        let crate::tray::spec::TrayMenuItem::Submenu {
            label: version,
            items: actions,
            ..
        } = entry
        else {
            unreachable!()
        };
        let set_default = actions
            .iter()
            .find(|item| matches!(item, crate::tray::spec::TrayMenuItem::Action { id, .. } if *id == TRAY_ACTION_SET_DEFAULT_KERNEL))
            .expect("set-default action present");
        let crate::tray::spec::TrayMenuItem::Action {
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
