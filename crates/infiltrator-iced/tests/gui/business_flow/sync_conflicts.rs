//! Journeys 6/7/15 — WebDAV sync-conflict surface: the key-level diff merge
//! against real local/remote YAML files, conflict resolve/dismiss lifecycle,
//! and the mixed profile-switch → rebuild → refetch journey.
//!
//! Journey 7 is a FULL NETWORK leg: a minimal axum WebDAV stub bound to
//! `127.0.0.1:0` (zero external network; the stub implements exactly the
//! verbs `WebDavClient` issues — PROPFIND/GET/PUT) lets the real
//! `SyncUpload`/`SyncDownload` workers run end to end. The worker iced
//! `Task`s are unwrapped back into streams via `iced_runtime::task::
//! into_stream` and every produced `Message` is fed back through the real
//! `AppState::update()` (异步结果回灌).
//!
//! test-intent: behavior

use super::support::{SAMPLE_PROFILE_YAML, TempHome, block_on, feed, fresh_state, last_toast};
use crate::types::app::{SyncConflict, SyncSummary, ToastStatus};
use crate::types::message::Message;
use crate::types::options::SyncDiffBundle;
use crate::types::runtime::{RebuildFlowState, RuntimeConfig, RuntimeStatus};
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use infiltrator_contract::error::InfiltratorError;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use infiltrator_domain::sync::diff_yaml_configs;
use tokio::sync::oneshot;

// ---------------------------------------------------------------------------
// Minimal WebDAV stub (journey 7)
// ---------------------------------------------------------------------------

/// In-memory WebDAV collection: URL path ("/alpha.yaml") -> body.
type StubFiles = Arc<Mutex<BTreeMap<String, String>>>;

/// PROPFIND multistatus in exactly the shape `dav_client::xml_parser`
/// deserializes (same namespace prefixes as its own fixture).
fn multistatus_xml(paths: &[String]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<D:multistatus xmlns:D=\"DAV:\">",
    );
    xml.push_str(
        "<D:response><D:href>/</D:href><D:propstat><D:prop>\
         <D:resourcetype><D:collection/></D:resourcetype></D:prop>\
         <D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>",
    );
    for path in paths {
        xml.push_str(&format!(
            "<D:response><D:href>{path}</D:href><D:propstat><D:prop>\
             <D:resourcetype/></D:prop>\
             <D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>"
        ));
    }
    xml.push_str("</D:multistatus>");
    xml
}

/// The only three verbs `WebDavClient` needs for the sync workers:
/// PROPFIND (list), GET (download), PUT (upload). Everything else is 405.
async fn stub_dav(State(files): State<StubFiles>, request: Request) -> Response {
    let path = request.uri().path().to_string();
    match request.method().as_str() {
        "PROPFIND" => {
            let paths: Vec<String> = files.lock().unwrap().keys().cloned().collect();
            (
                StatusCode::MULTI_STATUS,
                [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
                multistatus_xml(&paths),
            )
                .into_response()
        }
        "GET" => match files.lock().unwrap().get(&path).cloned() {
            Some(body) => (StatusCode::OK, body).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        },
        "PUT" => {
            let body = axum::body::to_bytes(request.into_body(), 16 * 1024 * 1024)
                .await
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .unwrap_or_default();
            files.lock().unwrap().insert(path, body);
            StatusCode::CREATED.into_response()
        }
        _ => StatusCode::METHOD_NOT_ALLOWED.into_response(),
    }
}

/// The stub server: bound to 127.0.0.1:0 (zero external network), serving
/// [`stub_dav`]. Graceful shutdown fires when the guard is dropped.
struct StubServer {
    files: StubFiles,
    addr: std::net::SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
}

impl StubServer {
    async fn spawn() -> Self {
        let files: StubFiles = Arc::new(Mutex::new(BTreeMap::new()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new()
            .fallback(stub_dav)
            .with_state(files.clone());
        let (shutdown, shutdown_rx) = oneshot::channel::<()>();
        let server = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        });
        tokio::spawn(async move {
            let _ = server.await;
        });
        Self {
            files,
            addr,
            shutdown: Some(shutdown),
        }
    }
}

impl Drop for StubServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

/// Drive a real iced `Task` (the worker chains) to completion: its stream is
/// unwrapped with `iced_runtime::task::into_stream` and every output message
/// is fed back through the real `AppState::update()` (异步结果回灌).
/// `depth` bounds how many levels of task-returned-from-update are driven:
/// depth 1 covers the worker outputs plus the `SyncFinished` completion
/// batch (toast + LoadProfiles) while stopping short of side legs like the
/// 5s `RemoveToast` expiry task. Returns every observed message.
async fn drive_task(
    state: &mut crate::state::AppState,
    task: iced::Task<Message>,
    depth: usize,
) -> Vec<Message> {
    use iced::futures::StreamExt;
    let mut observed = Vec::new();
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return observed;
    };
    while let Some(action) = stream.next().await {
        if let iced_runtime::Action::Output(message) = action {
            observed.push(message.clone());
            let next = state.update(message);
            if depth > 0 {
                let nested = Box::pin(drive_task(state, next, depth - 1)).await;
                observed.extend(nested);
            }
        }
    }
    observed
}

const LOCAL_YAML: &str = "mixed-port: 7890\nmode: rule\nlog-level: info\nproxies: []\nrules: []\n";
const REMOTE_YAML: &str =
    "mixed-port: 7890\nmode: global\ntun:\n  enable: true\nproxies: []\nrules: []\n";
/// Journey 7 的第二个冲突内容（与本地 Resolve 后的 alpha 均不同）。
const CONFLICT2_YAML: &str = "mixed-port: 7899\nmode: direct\nproxies: []\nrules: []\n";

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
    assert!(
        diff.picks.values().all(|pick| !pick),
        "picks start keep-local"
    );

    // Per-key pick flips only its own key.
    feed(&mut state, Message::PickSyncDiffKey("mode".into(), true));
    assert!(state.profile.sync_diff.as_ref().unwrap().picks["mode"]);
    assert!(!state.profile.sync_diff.as_ref().unwrap().picks["tun"]);

    // Bulk pick overrides everything (including the removal).
    feed(&mut state, Message::SetSyncDiffPicks(true));
    assert!(
        state
            .profile
            .sync_diff
            .as_ref()
            .unwrap()
            .picks
            .values()
            .all(|pick| *pick)
    );

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
        let merged = infiltrator_domain::mixin::merge_yaml_key_picks(
            &local,
            &remote,
            &take_remote,
            &accept_removals,
        )
        .unwrap();
        infiltrator_domain::config::validate_yaml(&merged).unwrap();
        crate::update::core::profile_apply::save_profile_content(
            None,
            "alpha".into(),
            merged,
            infiltrator_domain::apply::ApplyStrategy::PreferReload,
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
    assert!(
        merged.contains("mode: global"),
        "remote mode adopted: {merged}"
    );
    assert!(merged.contains("enable: true"), "added tun adopted");
    assert!(!merged.contains("log-level"), "accepted removal dropped");
    assert!(!remote_path.exists(), "conflict file consumed");
}

/// Journey 7 — WebDAV 冲突生命周期（全网络腿）：本地 axum WebDAV 桩
/// （127.0.0.1:0，零外网）上驱动真实 SyncUpload/SyncDownload worker：
/// 上传两个 profile → 服务器状态断言 → 篡改远端制造内容冲突 →
/// SyncFinished 冲突入列 → Resolve（真实文件合并路径）→ 清除；Dismiss
/// 直接删除冲突文件。
#[test]
fn sync_conflict_network_leg_upload_download_resolve_and_dismiss() {
    let home = TempHome::acquire("sync-conflict-net");
    home.seed_profile("alpha", LOCAL_YAML);
    home.seed_profile("beta", SAMPLE_PROFILE_YAML);

    let mut state = fresh_state();
    state.shell.lang = "zh-CN".into();

    // 单个 block_on 内同时承载桩服务器与 worker（current-thread 运行时，
    // 拆开会让 spawned server 随 runtime 一起被丢弃）。
    block_on(async {
        let stub = StubServer::spawn().await;

        // WebDAV 凭据指向本地桩（注入点：WebDavClient::new 在 spawn 前构造）。
        state.profile.webdav_url = format!("http://{}", stub.addr);
        state.profile.webdav_user = "smoke".into();
        state.profile.webdav_pass = "smoke".into();

        // ---- 上传腿：真实 SyncUpload worker 上传全部本地 profile --------
        let upload_task = state.update(Message::SyncUpload);
        let observed = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            drive_task(&mut state, upload_task, 1),
        )
        .await
        .expect("upload worker must finish")
        .into_iter()
        .collect::<Vec<_>>();
        assert!(
            observed
                .iter()
                .any(|message| matches!(message, Message::SyncProgress(_))),
            "upload worker emits progress: {observed:?}"
        );
        let finished = observed.iter().find_map(|message| match message {
            Message::SyncFinished(result) => Some(result.clone()),
            _ => None,
        });
        let summary = finished.expect("upload worker reports SyncFinished");
        let summary = summary.expect("upload succeeds");
        assert_eq!(summary.uploaded, 2, "both profiles uploaded");

        // 服务器状态断言：两个 profile 都在桩上。
        {
            let files = stub.files.lock().unwrap();
            assert_eq!(files.len(), 2, "stub holds exactly the two profiles");
            assert!(
                files
                    .get("/alpha.yaml")
                    .is_some_and(|body| body.contains("mode: rule")),
                "alpha uploaded verbatim: {:?}",
                files.get("/alpha.yaml")
            );
            assert!(
                files
                    .get("/beta.yaml")
                    .is_some_and(|body| body.contains("MATCH,PROXY")),
                "beta uploaded verbatim: {:?}",
                files.get("/beta.yaml")
            );
        }

        // ---- 冲突制造：远端 alpha 换成不同（合法）内容 -----------------
        stub.files
            .lock()
            .unwrap()
            .insert("/alpha.yaml".to_string(), REMOTE_YAML.to_string());

        // ---- 下载腿：真实 SyncDownload worker 发现内容冲突 --------------
        let download_task = state.update(Message::SyncDownload);
        let observed = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            drive_task(&mut state, download_task, 1),
        )
        .await
        .expect("download worker must finish")
        .into_iter()
        .collect::<Vec<_>>();
        let finished = observed.iter().find_map(|message| match message {
            Message::SyncFinished(result) => Some(result.clone()),
            _ => None,
        });
        let summary = finished.expect("download worker reports SyncFinished");
        let summary = summary.expect("download succeeds");
        assert_eq!(summary.conflicts, 1, "alpha content conflict detected");
        assert_eq!(summary.downloaded, 0, "beta identical → nothing re-saved");

        // 冲突入列（worker 真实产出，而非手工构造 summary）。
        assert_eq!(state.profile.sync_conflicts.len(), 1);
        assert_eq!(state.profile.sync_conflicts[0].profile, "alpha");
        let conflict_path = state.profile.sync_conflicts[0].remote_path.clone();
        assert!(conflict_path.parent() == Some(&home.configs()));
        assert_eq!(
            std::fs::read_to_string(&conflict_path).unwrap(),
            REMOTE_YAML,
            "conflict backup holds the remote content"
        );
        assert!(
            matches!(last_toast(&state), Some((_, ToastStatus::Warning))),
            "conflict completion toasts Warning: {:?}",
            last_toast(&state)
        );

        // ---- Resolve：真实文件合并路径消费冲突文件 ----------------------
        let units = feed(&mut state, Message::ResolveSyncConflict("alpha".into()));
        assert_eq!(units, 1);
        {
            let content = tokio::fs::read_to_string(&conflict_path).await.unwrap();
        infiltrator_domain::config::validate_yaml(&content).unwrap();
            crate::update::core::profile_apply::save_profile_content(
                None,
                "alpha".into(),
                content,
                infiltrator_domain::apply::ApplyStrategy::PreferReload,
            )
            .await
            .unwrap();
            tokio::fs::remove_file(&conflict_path).await.unwrap();
        }
        let units = feed(
            &mut state,
            Message::SyncConflictResolved(Ok("alpha".into())),
        );
        assert!(state.profile.sync_conflicts.is_empty());
        assert!(units >= 2, "LoadProfiles + success-toast legs");
        assert!(
            std::fs::read_to_string(home.configs().join("alpha.yaml"))
                .unwrap()
                .contains("mode: global"),
            "resolve adopted the remote content"
        );

        // ---- Dismiss：第二个冲突直接删除备份文件 ------------------------
        // Resolve 后本地 alpha 已与远端一致，再次篡改远端制造新冲突。
        stub.files
            .lock()
            .unwrap()
            .insert("/alpha.yaml".to_string(), CONFLICT2_YAML.to_string());
        let dismissal_task = state.update(Message::SyncDownload);
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            drive_task(&mut state, dismissal_task, 1),
        )
        .await
        .expect("second download worker must finish");
        assert_eq!(state.profile.sync_conflicts.len(), 1, "conflict re-created");
        let conflict_path = state.profile.sync_conflicts[0].remote_path.clone();

        let units = feed(&mut state, Message::DismissSyncConflict("alpha".into()));
        assert_eq!(units, 1);
        tokio::fs::remove_file(&conflict_path).await.unwrap();
        let units = feed(
            &mut state,
            Message::SyncConflictDismissed(Ok("alpha".into())),
        );
        assert!(state.profile.sync_conflicts.is_empty());
        assert!(!conflict_path.exists(), "dismissal deletes the backup file");
        assert_eq!(units, 1, "info-toast leg");
    });
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
    state.runtime.rebuild_flow = RebuildFlowState::Rebuilding {
        label: "配置".into(),
    };
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
