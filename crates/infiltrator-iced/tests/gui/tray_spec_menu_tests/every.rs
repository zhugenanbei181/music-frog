//! Behavior cases for every.
//! test-intent: behavior

use super::super::spec::TrayActionId;
use super::*;

#[test]
fn every_spec_action_id_resolves_to_an_intent() {
    // Fully populated snapshot so every fixed action appears; disabled
    // informational lines are the expected misses.
    let groups = vec![
        proxy_group("GLOBAL", "A", &[("A", None), ("B", Some(80))]),
        proxy_group("HK", "HK-1", &[("HK-1", None)]),
    ];
    let profiles = vec![
        test_profile("Paid", true, false),
        test_profile("Free", false, true),
    ];
    let kernels = vec![test_kernel("v1.18.0", true), test_kernel("v1.19.0", false)];
    let mut ctx = base_ctx();
    ctx.groups = &groups;
    ctx.profiles = &profiles;
    ctx.kernels = &kernels;
    ctx.webdav_enabled = true;
    ctx.admin_enabled = true;
    let spec = build_tray_spec(&ctx);
    let event_ctx = TrayEventContext {
        system_proxy: false,
        tun: false,
        autostart: false,
        profiles: &profiles,
    };

    const EXPECTED_MISSES: &[TrayActionId] = &[
        TRAY_ACTION_NO_PROXIES,
        TRAY_ACTION_NO_PROFILES,
        TRAY_ACTION_INFO_MODE,
        TRAY_ACTION_INFO_STATUS,
        TRAY_ACTION_INFO_CONTROLLER,
        TRAY_ACTION_INFO_ADMIN,
        TRAY_ACTION_INFO_KERNEL_VERSION,
        TRAY_ACTION_INFO_SYNC,
        TRAY_ACTION_INFO_KERNEL_DEFAULT,
        TRAY_ACTION_INFO_KERNEL_STATUS,
        TRAY_ACTION_INFO_DOWNLOAD,
    ];

    fn walk(
        items: &[TrayMenuItem],
        ctx: &TrayEventContext<'_>,
        hits: &mut usize,
        misses: &mut Vec<TrayActionId>,
    ) {
        for item in items {
            match item {
                TrayMenuItem::Action { id, payload, .. } => {
                    let resolved = resolve_tray_event_in(
                        &TrayEvent::MenuActivated {
                            id: *id,
                            payload: payload.clone(),
                        },
                        ctx,
                    );
                    if EXPECTED_MISSES.contains(id) {
                        assert!(
                            resolved.is_none(),
                            "informational entry {id} must not resolve"
                        );
                        misses.push(*id);
                    } else {
                        assert!(resolved.is_some(), "action {id} must resolve");
                        *hits += 1;
                    }
                }
                TrayMenuItem::Checkmark { id, payload, .. } => {
                    assert!(
                        resolve_tray_event_in(
                            &TrayEvent::MenuActivated {
                                id: *id,
                                payload: payload.clone(),
                            },
                            ctx,
                        )
                        .is_some(),
                        "checkmark {id} must resolve"
                    );
                    *hits += 1;
                }
                TrayMenuItem::Submenu { items, .. } => walk(items, ctx, hits, misses),
                TrayMenuItem::Separator => {}
            }
        }
    }

    let (mut hits, mut misses) = (0, Vec::new());
    walk(&spec.menu.items, &event_ctx, &mut hits, &mut misses);
    // 28 clickable entries: show, 4 modes, 3 nodes, sys/tun/theme, 2 profile
    // activations, update-all, 2 auto-update checkmarks, 2×(set-default +
    // uninstall), check-update, flush-fakeip, upload, download, sync
    // settings, autostart, factory reset, quit.
    assert_eq!(hits, 28);
    // 8 read-only lines rendered by this snapshot: the two kernel info
    // entries, the sync status line and the five-entry info submenu. (The
    // two placeholders and the download-progress line only appear in their
    // degraded snapshots, covered by the dedicated tests above.)
    misses.sort_unstable();
    assert_eq!(misses, vec![80, 81, 82, 83, 84, 85, 86, 87]);
    assert!(misses.iter().all(|id| EXPECTED_MISSES.contains(id)));
}
