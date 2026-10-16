use super::*;

#[tokio::test]
async fn hot_reload_success_keeps_generation_and_updates_file() {
    let f = fixture(0, false).await;
    let generation = f.session.start().await.expect("start");
    f.session
        .wait_for_ready(generation, Duration::from_secs(5))
        .await
        .expect("ready");
    let session_token = f.session.session_token().expect("session token");

    let outcome = apply_to_selected_reference(
        &f.session,
        &f.config,
        &f.reloader,
        NEW,
        params(ApplyStrategy::PreferReload),
    )
    .await
    .expect("apply");

    assert_eq!(outcome.method, ApplyMethod::HotReload);
    assert_eq!(outcome.generation, generation);
    assert_eq!(outcome.session_token, session_token);
    assert_eq!(file_content(&f.config).await, NEW);
    assert_eq!(f.session.status(), CoreLifecycle::Ready);
    assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn active_reference_apply_uses_the_exact_selected_profile_under_its_lease() {
    let f = fixture(0, false).await;
    start_ready(&f).await;
    f.config
        .save_inactive_profile("other", NEW)
        .await
        .expect("seed another profile");
    let profile_store = f.config.engine_profile_store();
    let ActiveProfileReferenceState::Selected(expected) =
        ActiveProfileReferenceStore::read(profile_store)
            .await
            .expect("active profile reference")
    else {
        panic!("fixture has a selected profile");
    };

    let outcome = apply_active_profile_reference(
        &f.session,
        &f.config,
        &f.reloader,
        &expected,
        params(ApplyStrategy::AlwaysRestart),
    )
    .await
    .expect("apply the exact selected profile");

    assert_eq!(outcome.method, ApplyMethod::Restart);
    assert_eq!(
        f.config.get_current().await.expect("selected profile"),
        "main"
    );
    assert_eq!(f.config.load("main").await.expect("target file"), OLD);
    assert_eq!(f.config.load("other").await.expect("other file"), NEW);
    assert_eq!(f.session.lifecycle(), CoreLifecycle::Ready);
}

#[tokio::test]
async fn active_reference_content_apply_writes_only_the_exact_selected_profile() {
    let f = fixture(0, false).await;
    start_ready(&f).await;
    f.config
        .save_inactive_profile("other", "port: 7888\n")
        .await
        .expect("seed a second profile");
    let profile_store = f.config.engine_profile_store();
    let ActiveProfileReferenceState::Selected(expected) =
        ActiveProfileReferenceStore::read(profile_store)
            .await
            .expect("active profile reference")
    else {
        panic!("fixture has a selected profile");
    };

    apply_workspace_content_for_reference(
        &f.session,
        &f.config,
        &f.reloader,
        &expected,
        NEW,
        params(ApplyStrategy::AlwaysRestart),
    )
    .await
    .expect("apply content to the exact selected profile");

    assert_eq!(f.config.load("main").await.expect("main file"), NEW);
    assert_eq!(
        f.config.load("other").await.expect("other file"),
        "port: 7888\n"
    );
    let profile_store = f.config.engine_profile_store();
    assert_eq!(
        ActiveProfileReferenceStore::read(profile_store)
            .await
            .expect("reference remains selected"),
        ActiveProfileReferenceState::Selected(expected)
    );
    assert_eq!(f.session.lifecycle(), CoreLifecycle::Ready);
}

#[tokio::test]
async fn stale_active_reference_content_apply_has_zero_profile_or_runtime_effects() {
    let f = fixture(0, false).await;
    let profile_store = f.config.engine_profile_store();
    let ActiveProfileReferenceState::Selected(expected) =
        ActiveProfileReferenceStore::read(profile_store)
            .await
            .expect("active profile reference")
    else {
        panic!("fixture has a selected profile");
    };
    f.config
        .save_inactive_profile("other", "port: 7888\n")
        .await
        .unwrap();
    select_profile(&f.config, "other").await;
    start_ready(&f).await;
    let generation = f.session.generation();
    let start_calls = f.session.start_calls();
    let stop_calls = f.session.stop_calls();

    assert_eq!(
        apply_workspace_content_for_reference(
            &f.session,
            &f.config,
            &f.reloader,
            &expected,
            NEW,
            params(ApplyStrategy::AlwaysRestart),
        )
        .await
        .expect_err("stale content reference"),
        ApplyError::SourceChanged
    );
    assert_eq!(f.config.load("main").await.expect("main file"), OLD);
    assert_eq!(
        f.config.load("other").await.expect("other file"),
        "port: 7888\n"
    );
    assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.session.lifecycle(), CoreLifecycle::Ready);
    assert!(f.controller.running.load(Ordering::SeqCst));
    assert_eq!(f.session.generation(), generation);
    assert_eq!(f.session.start_calls(), start_calls);
    assert_eq!(f.session.stop_calls(), stop_calls);
}

#[tokio::test]
async fn active_reference_apply_rejects_a_stale_revision_before_core_or_file_effects() {
    let f = fixture(0, false).await;
    let profile_store = f.config.engine_profile_store();
    let ActiveProfileReferenceState::Selected(expected) =
        ActiveProfileReferenceStore::read(profile_store)
            .await
            .expect("active profile reference")
    else {
        panic!("fixture has a selected profile");
    };
    f.config
        .save_inactive_profile("other", NEW)
        .await
        .expect("seed other profile");
    select_profile(&f.config, "other").await;

    assert_eq!(
        apply_active_profile_reference(
            &f.session,
            &f.config,
            &f.reloader,
            &expected,
            params(ApplyStrategy::AlwaysRestart),
        )
        .await
        .expect_err("stale reference"),
        ApplyError::ActiveProfileReferenceChanged {
            expected_revision: expected.revision.value(),
            observed_revision: expected.revision.value() + 1,
        }
    );
    assert_eq!(
        f.config.get_current().await.expect("winner selection"),
        "other"
    );
    assert_eq!(f.config.load("main").await.expect("main file"), OLD);
    assert_eq!(f.config.load("other").await.expect("other file"), NEW);
    assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.session.lifecycle(), CoreLifecycle::Stopped);
    assert!(!f.controller.running.load(Ordering::SeqCst));
}

#[tokio::test]
async fn reload_failure_falls_back_to_restart() {
    let f = fixture(0, true).await;
    let generation = f.session.start().await.expect("start");
    f.session
        .wait_for_ready(generation, Duration::from_secs(5))
        .await
        .expect("ready");
    let before = f.session.generation();
    let old_session = f.session.session_token().expect("old session token");

    let outcome = apply_to_selected_reference(
        &f.session,
        &f.config,
        &f.reloader,
        NEW,
        params(ApplyStrategy::PreferReload),
    )
    .await
    .expect("apply via restart");

    assert_eq!(outcome.method, ApplyMethod::Restart);
    assert!(outcome.generation > before);
    assert_ne!(outcome.session_token, old_session);
    assert_eq!(file_content(&f.config).await, NEW);
    assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn reload_and_restart_failure_preserve_both_causes_through_rollback() {
    let f = fixture(0, true).await;
    start_ready(&f).await;
    f.controller.fail_starts_left.store(1, Ordering::SeqCst);

    let error = apply_to_selected_reference(
        &f.session,
        &f.config,
        &f.reloader,
        NEW,
        params(ApplyStrategy::PreferReload),
    )
    .await
    .expect_err("the injected reload and restart failures must roll back");

    let ApplyError::RolledBack { cause } = error else {
        panic!("the previous profile should be restored, got {error:?}")
    };
    assert!(cause.contains("reload rejected"), "{cause}");
    assert!(cause.contains("start rejected"), "{cause}");
    assert_eq!(file_content(&f.config).await, OLD);
    assert_eq!(f.session.lifecycle(), CoreLifecycle::Ready);
}

#[tokio::test]
async fn restart_failure_rolls_back_and_recovers() {
    let f = fixture(0, true).await;
    let generation = f.session.start().await.expect("start");
    f.session
        .wait_for_ready(generation, Duration::from_secs(5))
        .await
        .expect("ready");
    f.controller.fail_starts_left.store(1, Ordering::SeqCst);

    let err = apply_to_selected_reference(
        &f.session,
        &f.config,
        &f.reloader,
        NEW,
        params(ApplyStrategy::AlwaysRestart),
    )
    .await
    .expect_err("restart was sabotaged");

    assert!(matches!(err, ApplyError::RolledBack { .. }));
    assert_eq!(file_content(&f.config).await, OLD);
    assert_eq!(f.session.status(), CoreLifecycle::Ready);

    // DUAL-09-11: the surfaces read this typed record, not a string guess.
    let record = f
        .config
        .apply_transaction("main")
        .expect("the failed transaction is published");
    assert_eq!(record.profile, "main");
    assert_eq!(record.stage, ApplyTransactionStage::RolledBack);
    assert!(record.is_failure());
    assert!(!record.detail.is_empty());
}

#[tokio::test]
async fn stopped_core_exact_apply_fails_without_starting_or_writing() {
    let f = fixture(0, false).await;

    let error = apply_to_selected_reference(
        &f.session,
        &f.config,
        &f.reloader,
        NEW,
        params(ApplyStrategy::PreferReload),
    )
    .await
    .expect_err("paused Core must not be started by a profile apply");

    assert_eq!(
        error,
        ApplyError::NotRunning {
            status: CoreLifecycle::Stopped
        }
    );
    assert_eq!(file_content(&f.config).await, OLD);
    assert_eq!(f.session.status(), CoreLifecycle::Stopped);
    assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 0);
    assert!(!f.controller.running.load(Ordering::SeqCst));
}

#[tokio::test]
async fn stopped_core_selected_reference_apply_does_not_dispatch_or_start() {
    let f = fixture(0, false).await;
    let expected = selected_reference(&f.config).await;

    let error = apply_active_profile_reference(
        &f.session,
        &f.config,
        &f.reloader,
        &expected,
        params(ApplyStrategy::AlwaysRestart),
    )
    .await
    .expect_err("exact-reference apply requires an already running Core");

    assert_eq!(
        error,
        ApplyError::NotRunning {
            status: CoreLifecycle::Stopped
        }
    );
    assert_eq!(f.config.load("main").await.unwrap(), OLD);
    assert_eq!(f.session.status(), CoreLifecycle::Stopped);
    assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 0);
    assert!(!f.controller.running.load(Ordering::SeqCst));
}

#[tokio::test]
async fn successful_apply_stores_snapshot_history() {
    let f = fixture(0, false).await;
    let generation = f.session.start().await.expect("start");
    f.session
        .wait_for_ready(generation, Duration::from_secs(5))
        .await
        .expect("ready");
    let mut p = params(ApplyStrategy::PreferReload);
    p.snapshot_history = true;

    apply_to_selected_reference(&f.session, &f.config, &f.reloader, NEW, p)
        .await
        .expect("apply");

    // DUAL-09-11: a committed transaction is published with its reload method.
    let record = f
        .config
        .apply_transaction("main")
        .expect("the committed transaction is published");
    assert_eq!(record.stage, ApplyTransactionStage::Committed);
    assert_eq!(record.method.as_deref(), Some("hot_reload"));
    assert!(!record.is_failure());

    let config_dir = f._dir.path().join("configs");
    let snapshots = list_snapshots(&EngineId::new("mihomo").unwrap(), &config_dir, "main")
        .await
        .expect("list");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(read_snapshot(&snapshots[0].path).await.unwrap(), NEW);
}

#[tokio::test]
async fn invalid_content_aborts_before_any_write() {
    let f = fixture(0, false).await;
    start_ready(&f).await;

    let err = apply_to_selected_reference(
        &f.session,
        &f.config,
        &f.reloader,
        "- a\n- b\n",
        params(ApplyStrategy::PreferReload),
    )
    .await
    .expect_err("validation must fail");

    assert!(matches!(err, ApplyError::Validation(_)));
    assert_eq!(file_content(&f.config).await, OLD);
    assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn busy_transition_rejects_apply() {
    let f = fixture(0, false).await;
    f.session.start().await.expect("start");

    let err = apply_to_selected_reference(
        &f.session,
        &f.config,
        &f.reloader,
        NEW,
        params(ApplyStrategy::PreferReload),
    )
    .await
    .expect_err("Starting must reject apply");

    assert_eq!(
        err,
        ApplyError::Busy {
            status: CoreLifecycle::Starting
        }
    );
    assert_eq!(file_content(&f.config).await, OLD);
}

// --- SourceDoc Fidelity Integration Tests -------------------------------

const COMPLEX_YAML_WITH_COMMENTS_AND_ANCHORS: &str = "\
# ==========================================
# 顶层手写注释：生产环境主配置文件
# ==========================================
mixed-port: 7890
mode: rule   # 当前运行模式 (rule / global / direct)
log-level: info # 日志等级

# 规则部分
rules:
  # 自定义去广告规则
  - DOMAIN-SUFFIX,adservice.google.com,REJECT   # 拦截广告
  - &catchall MATCH,DIRECT                      # 兜底规则锚点

# 代理节点定义
proxies:
  - &hk_01 { name: \"HK-01\", type: ss, server: 1.1.1.1, port: 443 }
  - *hk_01

# 策略组
proxy-groups:
  - name: PROXY
    type: select
    proxies:
      - *hk_01
      - DIRECT
";

#[tokio::test]
async fn fidelity_apply_active_profile_doc_preserves_100_percent() {
    let f = fixture(0, false).await;
    start_ready(&f).await;
    replace_profile_file_for_test(&f.config, "main", COMPLEX_YAML_WITH_COMMENTS_AND_ANCHORS).await;

    let doc = SourceDoc::parse(COMPLEX_YAML_WITH_COMMENTS_AND_ANCHORS).unwrap();
    let expected_reference = selected_reference(&f.config).await;
    let outcome = apply_active_profile_doc(
        &f.session,
        &f.config,
        &f.reloader,
        &expected_reference,
        &doc,
        params(ApplyStrategy::PreferReload),
    )
    .await
    .expect("apply doc");

    assert_eq!(outcome.method, ApplyMethod::HotReload);
    let disk = file_content(&f.config).await;
    assert_eq!(
        disk, COMPLEX_YAML_WITH_COMMENTS_AND_ANCHORS,
        "byte-exact fidelity preservation"
    );
}

#[tokio::test]
async fn fidelity_scalar_override_preserves_all_comments_and_anchors() {
    let f = fixture(0, false).await;
    start_ready(&f).await;
    replace_profile_file_for_test(&f.config, "main", COMPLEX_YAML_WITH_COMMENTS_AND_ANCHORS).await;

    let expected_reference = selected_reference(&f.config).await;
    let outcome = apply_active_profile_set_scalar(
        &f.session,
        &f.config,
        &f.reloader,
        &expected_reference,
        "mode",
        "global",
        params(ApplyStrategy::PreferReload),
    )
    .await
    .expect("apply scalar override");

    assert_eq!(outcome.method, ApplyMethod::HotReload);
    let disk = file_content(&f.config).await;

    // Check updated mode
    assert!(disk.contains("mode: global   # 当前运行模式 (rule / global / direct)"));

    // Check comments 100% preserved
    assert!(disk.contains("# =========================================="));
    assert!(disk.contains("# 顶层手写注释：生产环境主配置文件"));
    assert!(disk.contains("# 规则部分"));
    assert!(disk.contains("# 自定义去广告规则"));
    assert!(disk.contains("# 拦截广告"));
    assert!(disk.contains("# 兜底规则锚点"));
    assert!(disk.contains("# 代理节点定义"));
    assert!(disk.contains("# 策略组"));

    // Check anchors 100% preserved
    assert!(disk.contains("&catchall MATCH,DIRECT"));
    assert!(disk.contains("&hk_01 { name: \"HK-01\", type: ss, server: 1.1.1.1, port: 443 }"));
    assert!(disk.contains("- *hk_01"));
}

#[tokio::test]
async fn fidelity_append_and_remove_rules_preserves_comments_and_anchors() {
    let f = fixture(0, false).await;
    start_ready(&f).await;
    replace_profile_file_for_test(&f.config, "main", COMPLEX_YAML_WITH_COMMENTS_AND_ANCHORS).await;

    // 1. Append rule
    let expected_reference = selected_reference(&f.config).await;
    apply_active_profile_append_rule(
        &f.session,
        &f.config,
        &f.reloader,
        &expected_reference,
        "DOMAIN-SUFFIX,netflix.com,PROXY",
        params(ApplyStrategy::PreferReload),
    )
    .await
    .expect("append rule");

    let disk_appended = file_content(&f.config).await;
    assert!(disk_appended.contains("DOMAIN-SUFFIX,netflix.com,PROXY"));
    assert!(disk_appended.contains("# 顶层手写注释：生产环境主配置文件"));
    assert!(disk_appended.contains("&catchall"));
    assert!(disk_appended.contains("&hk_01"));
    assert!(disk_appended.contains("*hk_01"));

    // 2. Remove rule
    let expected_reference = selected_reference(&f.config).await;
    apply_active_profile_remove_rule(
        &f.session,
        &f.config,
        &f.reloader,
        &expected_reference,
        "DOMAIN-SUFFIX,adservice.google.com,REJECT",
        params(ApplyStrategy::PreferReload),
    )
    .await
    .expect("remove rule");

    let disk_removed = file_content(&f.config).await;
    assert!(!disk_removed.contains("adservice.google.com"));
    assert!(disk_removed.contains("DOMAIN-SUFFIX,netflix.com,PROXY"));
    assert!(disk_removed.contains("# 顶层手写注释：生产环境主配置文件"));
    assert!(disk_removed.contains("&catchall"));
    assert!(disk_removed.contains("&hk_01"));
}

#[tokio::test]
async fn fidelity_rewrite_anchors_preserves_comments() {
    let f = fixture(0, false).await;
    start_ready(&f).await;
    replace_profile_file_for_test(&f.config, "main", COMPLEX_YAML_WITH_COMMENTS_AND_ANCHORS).await;

    let expected_reference = selected_reference(&f.config).await;
    apply_active_profile_rewrite_anchors(
        &f.session,
        &f.config,
        &f.reloader,
        &expected_reference,
        "tenant_a",
        params(ApplyStrategy::PreferReload),
    )
    .await
    .expect("rewrite anchors");

    let disk = file_content(&f.config).await;

    // Verify rewritten anchors & aliases
    assert!(disk.contains("&tenant_a_catchall MATCH,DIRECT"));
    assert!(
        disk.contains("&tenant_a_hk_01 { name: \"HK-01\", type: ss, server: 1.1.1.1, port: 443 }")
    );
    assert!(disk.contains("- *tenant_a_hk_01"));

    // Verify 100% comment retention
    assert!(disk.contains("# =========================================="));
    assert!(disk.contains("# 顶层手写注释：生产环境主配置文件"));
    assert!(disk.contains("# 当前运行模式"));
    assert!(disk.contains("# 拦截广告"));
    assert!(disk.contains("# 兜底规则锚点"));
    assert!(disk.contains("# 策略组"));
}

#[tokio::test]
async fn fidelity_mixin_apply_preserves_comments_and_anchors() {
    let f = fixture(0, false).await;
    start_ready(&f).await;
    replace_profile_file_for_test(&f.config, "main", COMPLEX_YAML_WITH_COMMENTS_AND_ANCHORS).await;

    let mixin = MixinConfig {
        mode: Some("direct".to_string()),
        mixed_port: Some(9999),
        rules: Some(RuleMixin {
            append: vec!["DOMAIN,custom-mixin.com,DIRECT".to_string()],
            ..Default::default()
        }),
        ..Default::default()
    };

    let expected_reference = selected_reference(&f.config).await;
    apply_active_profile_mixin_fidelity(
        &f.session,
        &f.config,
        &f.reloader,
        &expected_reference,
        &mixin,
        params(ApplyStrategy::PreferReload),
    )
    .await
    .expect("apply mixin fidelity");

    let disk = file_content(&f.config).await;

    assert!(disk.contains("mode: direct"));
    assert!(disk.contains("mixed-port: 9999"));
    assert!(disk.contains("DOMAIN,custom-mixin.com,DIRECT"));
    assert!(disk.contains("# 顶层手写注释：生产环境主配置文件"));
    assert!(disk.contains("# 兜底规则锚点"));
    assert!(disk.contains("&catchall"));
    assert!(disk.contains("&hk_01"));
    assert!(disk.contains("*hk_01"));
}

#[tokio::test]
async fn confirmed_profile_rejects_switched_profile_and_changed_document_before_io() {
    let f = fixture(0, false).await;
    let expected = identify_rules_document("main".into(), OLD);
    f.config
        .save_inactive_profile("other", OLD)
        .await
        .expect("other profile");
    select_profile(&f.config, "other").await;
    assert_eq!(
        apply_confirmed_to_selected_reference(
            &f.session,
            &f.config,
            &f.reloader,
            NEW,
            params(ApplyStrategy::PreferReload),
            &expected
        )
        .await
        .expect_err("different profile"),
        ApplyError::SourceChanged
    );
    assert_eq!(f.config.load("main").await.unwrap(), OLD);
    assert_eq!(f.config.load("other").await.unwrap(), OLD);
    select_profile(&f.config, "main").await;
    let changed = format!("{OLD}# concurrent document edit\n");
    replace_profile_file_for_test(&f.config, "main", &changed).await;
    assert_eq!(
        apply_confirmed_to_selected_reference(
            &f.session,
            &f.config,
            &f.reloader,
            NEW,
            params(ApplyStrategy::PreferReload),
            &expected
        )
        .await
        .expect_err("different bytes"),
        ApplyError::SourceChanged
    );
    assert_eq!(file_content(&f.config).await, changed);
    assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.session.lifecycle(), CoreLifecycle::Stopped);
    assert!(!f.controller.running.load(Ordering::SeqCst));
}

#[tokio::test]
async fn confirmed_profile_commits_only_exact_observed_source_and_rolls_back_to_it() {
    let f = fixture(0, false).await;
    let generation = f.session.start().await.unwrap();
    f.session
        .wait_for_ready(generation, Duration::from_secs(5))
        .await
        .unwrap();
    let expected = identify_rules_document("main".into(), OLD);
    let outcome = apply_confirmed_to_selected_reference(
        &f.session,
        &f.config,
        &f.reloader,
        NEW,
        params(ApplyStrategy::PreferReload),
        &expected,
    )
    .await
    .expect("confirmed apply");
    assert_eq!(file_content(&f.config).await, NEW);
    assert_eq!(outcome.method, ApplyMethod::HotReload);
    assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 1);
    assert_eq!(f.session.generation(), generation);

    let failing = fixture(0, true).await;
    let observed = "port: 7892\n# confirmed annotation\n";
    replace_profile_file_for_test(&failing.config, "main", observed).await;
    let observed_workspace = failing
        .config
        .engine_profile_store()
        .load_active_workspace()
        .await
        .expect("capture the exact active workspace before apply");
    assert_eq!(observed_workspace.document.utf8_text().unwrap(), observed);
    let generation = failing.session.start().await.unwrap();
    failing
        .session
        .wait_for_ready(generation, Duration::from_secs(5))
        .await
        .unwrap();
    failing
        .controller
        .fail_starts_left
        .store(1, Ordering::SeqCst);
    let expected = identify_rules_document("main".into(), observed);
    let result = apply_confirmed_to_selected_reference(
        &failing.session,
        &failing.config,
        &failing.reloader,
        NEW,
        params(ApplyStrategy::PreferReload),
        &expected,
    )
    .await;
    assert!(matches!(result, Err(ApplyError::RolledBack { .. })));
    assert_eq!(file_content(&failing.config).await, observed);
    assert_eq!(
        failing
            .config
            .engine_profile_store()
            .load_active_workspace()
            .await
            .unwrap(),
        observed_workspace
    );
    assert_eq!(failing.session.lifecycle(), CoreLifecycle::Ready);
}
