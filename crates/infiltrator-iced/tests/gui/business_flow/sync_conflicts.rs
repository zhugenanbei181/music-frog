//! Journeys 6/7/15 — WebDAV sync-conflict surface: the key-level diff merge
//! against real local/remote YAML files, conflict resolve/dismiss lifecycle,
//! and the mixed profile-switch → rebuild → refetch journey.
//!
//! The WebDAV HTTP client itself has no injection seam (`WebDavClient::new`
//! is constructed inside the `SyncUpload`/`SyncDownload` workers), so the
//! *download worker* is out of scope; the conflict *state machine* is fed at
//! its real boundary (`Message::SyncFinished` with the worker's summary) and
//! the file-touching resolve/merge legs run for real.
//!
//! test-intent: behavior

use super::support::{TempHome, block_on, feed, fresh_state};
use crate::types::app::{SyncConflict, SyncSummary};
use crate::types::message::Message;
use crate::types::options::SyncDiffBundle;
use crate::types::runtime::{RebuildFlowState, RuntimeConfig, RuntimeStatus};
use infiltrator_core::error::InfiltratorError;
use sync_engine::conflict_resolution::diff_yaml_configs;

const LOCAL_YAML: &str = "mixed-port: 7890\nmode: rule\nlog-level: info\nproxies: []\nrules: []\n";
const REMOTE_YAML: &str = "mixed-port: 7890\nmode: global\ntun:\n  enable: true\nproxies: []\nrules: []\n";

/// Journey 6 — SyncDiff 逐键合并：真实本地/远端 YAML → diff 回灌 → 逐键
/// pick → 全量 pick → apply → 合并文件落盘 + conflict 清除。
#[test]
fn sync_diff_journey_merges_key_picks_into_the_local_file() {
    let home = TempHome::acquire("sync-diff");
    home.seed_profile("alpha", LOCAL_YAML);
    let remote_path = home.join("alpha.remote-conflict.yaml");
    std::fs::write(&remote_path, REMOTE_YAML).unwrap();

    let mut state = fresh_state();
    state.profile.sync_conflicts = vec![SyncConflict {
        profile: "alpha".into(),
        remote_path: remote_path.clone(),
    }];

    // Sync page → open the merge session (guard arm runs, loader task arms).
    let units = feed(&mut state, Message::LoadSyncDiff("alpha".into()));
    assert!(state.profile.is_loading_sync_diff);
    assert!(units >= 1);

    // Loader task body for real: local via manager + remote file + diff.
    let bundle: SyncDiffBundle = block_on(async {
        let manager = crate::configs_dir::config_manager().await.unwrap();
        let local = manager.load("alpha").await.unwrap();
        let remote = tokio::fs::read_to_string(&remote_path).await.unwrap();
        let summary = diff_yaml_configs(&local, &remote).unwrap();
        SyncDiffBundle {
            profile: "alpha".into(),
            remote_path: remote_path.clone(),
            added: summary.added_keys,
            removed: summary.removed_keys,
            modified: summary.modified_keys,
        }
    });
    assert_eq!(bundle.added, vec!["tun".to_string()]);
    assert_eq!(bundle.removed, vec!["log-level".to_string()]);
    assert_eq!(
        bundle.modified,
        vec![("mode".to_string(), "rule".to_string(), "global".to_string())]
    );

    let units = feed(&mut state, Message::SyncDiffLoaded(Ok(bundle)));
    assert_eq!(units, 0);
    let diff = state.profile.sync_diff.as_ref().unwrap();
    assert_eq!(diff.picks.len(), 3);
    assert!(diff.picks.values().all(|pick| !pick), "picks start keep-local");

    // Per-key pick flips only its own key.
    feed(&mut state, Message::PickSyncDiffKey("mode".into(), true));
    assert!(state.profile.sync_diff.as_ref().unwrap().picks["mode"]);
    assert!(!state.profile.sync_diff.as_ref().unwrap().picks["tun"]);

    // Bulk pick overrides everything (including the removal).
    feed(&mut state, Message::SetSyncDiffPicks(true));
    assert!(state
        .profile
        .sync_diff
        .as_ref()
        .unwrap()
        .picks
        .values()
        .all(|pick| *pick));

    // Apply arms the merge task (guards on an open session + the conflict).
    let units = feed(&mut state, Message::ApplySyncDiffMerge);
    assert!(state.profile.is_applying_sync_diff);
    assert!(units >= 1);

    // Task body for real — merge by picks, validate, commit, drop the
    // remote conflict file.
    let picks = state.profile.sync_diff.clone().unwrap();
    block_on(async {
        let removed_keys: std::collections::HashSet<String> =
            picks.bundle.removed.iter().cloned().collect();
        let mut take_remote = Vec::new();
        let mut accept_removals = Vec::new();
        for (key, pick) in &picks.picks {
            if !pick {
                continue;
            }
            if removed_keys.contains(key) {
                accept_removals.push(key.clone());
            } else {
                take_remote.push(key.clone());
            }
        }
        let manager = crate::configs_dir::config_manager().await.unwrap();
        let local = manager.load("alpha").await.unwrap();
        let remote = tokio::fs::read_to_string(&remote_path).await.unwrap();
        let merged = infiltrator_core::mixin::merge_yaml_key_picks(
            &local,
            &remote,
            &take_remote,
            &accept_removals,
        )
        .unwrap();
        infiltrator_core::config::validate_yaml(&merged).unwrap();
        crate::update::core::profile_apply::save_profile_content(
            None,
            "alpha".into(),
            merged,
            infiltrator_core::apply::ApplyStrategy::PreferReload,
        )
        .await
        .unwrap();
        tokio::fs::remove_file(&remote_path).await.unwrap();
    });

    let units = feed(&mut state, Message::SyncDiffMerged(Ok("alpha".into())));
    assert!(!state.profile.is_applying_sync_diff);
    assert!(state.profile.sync_diff.is_none(), "session closed");
    assert!(state.profile.sync_conflicts.is_empty(), "conflict cleared");
    assert!(units >= 2, "LoadProfiles + success-toast legs");

    // Disk truth: mode came from remote, tun was adopted, log-level removed.
    let merged = std::fs::read_to_string(home.configs().join("alpha.yaml")).unwrap();
    assert!(merged.contains("mode: global"), "remote mode adopted: {merged}");
    assert!(merged.contains("enable: true"), "added tun adopted");
    assert!(!merged.contains("log-level"), "accepted removal dropped");
    assert!(!remote_path.exists(), "conflict file consumed");
}

/// Journey 7 — WebDAV 冲突生命周期：SyncFinished(冲突 summary) → 入列 →
/// Resolve（真实文件合并路径）→ 清除；Dismiss 直接删除冲突文件。
#[test]
fn sync_conflict_resolution_and_dismissal_clean_up_conflict_files() {
    let home = TempHome::acquire("sync-conflict");
    home.seed_profile("alpha", LOCAL_YAML);
    let conflict_path = home.join("alpha.remote-conflict-1.yaml");
    std::fs::write(&conflict_path, REMOTE_YAML).unwrap();

    let mut state = fresh_state();
    state.shell.lang = "zh-CN".into();

    // The download worker's completion boundary: one conflict found.
    let summary = SyncSummary {
        uploaded: 0,
        downloaded: 2,
        conflicts: 1,
        active_profile_changed: false,
        conflict_files: vec![SyncConflict {
            profile: "alpha".into(),
            remote_path: conflict_path.clone(),
        }],
    };
    let units = feed(&mut state, Message::SyncFinished(Ok(summary)));
    assert!(!state.profile.is_syncing);
    assert_eq!(state.profile.sync_conflicts.len(), 1);
    // The completion toast (Warning because conflicts > 0) rides the
    // ShowToast leg counted below.
    assert!(units >= 2, "LoadProfiles + toast legs");

    // ResolveSyncConflict arms the merge task; run it for real.
    let units = feed(&mut state, Message::ResolveSyncConflict("alpha".into()));
    assert_eq!(units, 1);
    block_on(async {
        let content = tokio::fs::read_to_string(&conflict_path).await.unwrap();
        infiltrator_core::config::validate_yaml(&content).unwrap();
        crate::update::core::profile_apply::save_profile_content(
            None,
            "alpha".into(),
            content,
            infiltrator_core::apply::ApplyStrategy::PreferReload,
        )
        .await
        .unwrap();
        tokio::fs::remove_file(&conflict_path).await.unwrap();
    });
    let units = feed(&mut state, Message::SyncConflictResolved(Ok("alpha".into())));
    assert!(state.profile.sync_conflicts.is_empty());
    assert!(units >= 2, "LoadProfiles + success-toast legs");

    // A second conflict is dismissed instead: the dismissal task deletes the
    // backup file (run for real), then the result clears the queue entry.
    std::fs::write(&conflict_path, REMOTE_YAML).unwrap();
    state.profile.sync_conflicts = vec![SyncConflict {
        profile: "alpha".into(),
        remote_path: conflict_path.clone(),
    }];
    feed(&mut state, Message::DismissSyncConflict("alpha".into()));
    block_on(tokio::fs::remove_file(&conflict_path)).unwrap();
    let units = feed(&mut state, Message::SyncConflictDismissed(Ok("alpha".into())));
    assert!(state.profile.sync_conflicts.is_empty());
    assert!(!conflict_path.exists(), "dismissal deletes the backup file");
    assert_eq!(units, 1, "info-toast leg");

    // Resolving an unknown profile is an inert no-op.
    let units = feed(&mut state, Message::ResolveSyncConflict("ghost".into()));
    assert_eq!(units, 0);
}

/// Journey 15 — 混合旅程：SyncFinished（active profile 变更）→ 无内核时清
/// 备份不重建；Rebuilding 中途失败的 RebuildFlow 状态机；FetchRuntimeConfig
/// 回灌刷新 proxy_mode 并推送托盘。
#[test]
fn mixed_profile_switch_rebuild_flow_and_runtime_refetch_chain() {
    let home = TempHome::acquire("mixed-rebuild");
    home.seed_profile("alpha", LOCAL_YAML);
    let mut state = fresh_state();
    state.shell.lang = "zh-CN".into();
    let tray = super::support::FakeTray::install(&mut state);

    // Download worker reports the ACTIVE profile changed + one conflict.
    let conflict_path = home.join("alpha.remote-conflict-2.yaml");
    std::fs::write(&conflict_path, REMOTE_YAML).unwrap();
    let summary = SyncSummary {
        uploaded: 0,
        downloaded: 1,
        conflicts: 1,
        active_profile_changed: true,
        conflict_files: vec![SyncConflict {
            profile: "alpha".into(),
            remote_path: conflict_path.clone(),
        }],
    };
    let units = feed(&mut state, Message::SyncFinished(Ok(summary)));
    // No runtime → no rebuild trigger; the transient backup clear task is
    // armed instead, conflicts still queue.
    assert_eq!(state.profile.sync_conflicts.len(), 1);
    assert!(units >= 3, "clear-backup + LoadProfiles + toast");
    assert!(matches!(state.runtime.rebuild_flow, RebuildFlowState::Idle));

    // With a live core this is where trigger_runtime_rebuild() puts the
    // state; drive the rebuild state machine through its failure leg.
    state.runtime.rebuild_flow = RebuildFlowState::Rebuilding { label: "配置".into() };
    state.runtime.status = RuntimeStatus::Running;
    let units = feed(
        &mut state,
        Message::RuntimeRebuildFinished(Err(InfiltratorError::Mihomo("内核启动失败".into()))),
    );
    assert!(matches!(
        state.runtime.rebuild_flow,
        RebuildFlowState::Failed { .. }
    ));
    assert!(matches!(state.runtime.status, RuntimeStatus::Error(_)));
    assert!(state.shell.error_msg.is_some());
    assert!(units >= 1);

    // The banner reset clears the flow.
    let units = feed(&mut state, Message::ClearRebuildFlow);
    assert_eq!(units, 0);
    assert!(matches!(state.runtime.rebuild_flow, RebuildFlowState::Idle));

    // FetchRuntimeConfig 链: with no runtime the fetch is an inert no-op;
    // the fetched config 回灌 still refreshes every runtime domain + tray.
    let generation = state.runtime.runtime_generation;
    let units = feed(&mut state, Message::FetchRuntimeConfig);
    assert_eq!(units, 0, "no runtime → no fetch");
    let units = feed(
        &mut state,
        Message::RuntimeConfigFetched(
            Ok(RuntimeConfig {
                mode: "global".into(),
                script_block_present: true,
                tun_enabled: false,
                dns_nameservers: vec!["1.1.1.1".into()],
                dns_fallback: vec![],
                dns_enhanced_mode: "fake-ip".into(),
                tun_stack: "gvisor".into(),
                tun_auto_route: true,
                tun_strict_route: false,
                sniffer_enabled: true,
            }),
            generation,
        ),
    );
    assert_eq!(units, 0);
    assert_eq!(state.runtime.proxy_mode.as_deref(), Some("global"));
    assert!(state.runtime.script_block_present, "script gate reopened");
    assert_eq!(state.editor.dns_nameservers, vec!["1.1.1.1".to_string()]);
    assert!(
        tray.count() >= 1,
        "runtime config refresh pushes a tray spec update"
    );

    // A stale generation is dropped (guard against rebuild races).
    let units = feed(
        &mut state,
        Message::RuntimeConfigFetched(
            Ok(RuntimeConfig {
                mode: "direct".into(),
                script_block_present: false,
                tun_enabled: false,
                dns_nameservers: vec![],
                dns_fallback: vec![],
                dns_enhanced_mode: String::new(),
                tun_stack: String::new(),
                tun_auto_route: false,
                tun_strict_route: false,
                sniffer_enabled: false,
            }),
            generation + 5,
        ),
    );
    assert_eq!(units, 0);
    assert_eq!(
        state.runtime.proxy_mode.as_deref(),
        Some("global"),
        "stale generation must not clobber the live mode"
    );
}
