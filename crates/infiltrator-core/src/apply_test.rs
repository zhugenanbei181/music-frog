//! Integration and unit tests for config apply transaction and YAML fidelity.

use super::*;
use crate::session::{CoreSession, CoreStatus, ReadinessProbe, SessionError};
use infiltrator_ports::core_process::CoreProcess;
use infiltrator_ports::endpoint::{ControllerEndpoint, EndpointSource};

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const OLD: &str = "port: 7890\n";
const NEW: &str = "port: 7891\n";

struct MockController {
    running: AtomicBool,
    fail_starts_left: AtomicU64,
}

#[async_trait]
impl CoreProcess for MockController {
    async fn start(&self) -> std::result::Result<(), infiltrator_ports::error::PortError> {
        if self.fail_starts_left.load(Ordering::SeqCst) > 0 {
            self.fail_starts_left.fetch_sub(1, Ordering::SeqCst);
            return Err(infiltrator_ports::error::PortError::Failed(
                "start rejected".into(),
            ));
        }
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn stop(&self) -> std::result::Result<(), infiltrator_ports::error::PortError> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn status(
        &self,
    ) -> std::result::Result<
        infiltrator_contract::snapshot::CoreLifecycle,
        infiltrator_ports::error::PortError,
    > {
        Ok(if self.running.load(Ordering::SeqCst) {
            infiltrator_contract::snapshot::CoreLifecycle::Running
        } else {
            infiltrator_contract::snapshot::CoreLifecycle::Stopped
        })
    }

    fn controller_endpoint(&self) -> Option<String> {
        None
    }
}

struct MockProbe {
    failures_left: AtomicU64,
}

#[async_trait]
impl ReadinessProbe for MockProbe {
    async fn probe(&self) -> Result<(), SessionError> {
        if self.failures_left.load(Ordering::SeqCst) > 0 {
            self.failures_left.fetch_sub(1, Ordering::SeqCst);
            return Err(SessionError::Probe("not listening yet".into()));
        }
        Ok(())
    }
}

struct StaticEndpoints;

#[async_trait]
impl EndpointSource for StaticEndpoints {
    async fn resolve(&self) -> Result<ControllerEndpoint, infiltrator_ports::error::PortError> {
        Ok(ControllerEndpoint {
            url: "http://127.0.0.1:9090".into(),
            secret: None,
        })
    }
}

struct MockReloader {
    fail: bool,
    calls: AtomicU64,
}

#[async_trait]
impl ConfigReloader for MockReloader {
    async fn reload(&self, _path: &Path) -> Result<(), String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err("reload rejected".into())
        } else {
            Ok(())
        }
    }
}

struct MockStore {
    entries: Mutex<HashMap<String, String>>,
}

#[async_trait]
impl SecureStore for MockStore {
    async fn get(
        &self,
        service: &str,
        key: &str,
    ) -> std::result::Result<Option<String>, infiltrator_ports::error::PortError> {
        Ok(self
            .entries
            .lock()
            .expect("store lock")
            .get(&format!("{service}/{key}"))
            .cloned())
    }

    async fn set(
        &self,
        service: &str,
        key: &str,
        value: &str,
    ) -> std::result::Result<(), infiltrator_ports::error::PortError> {
        self.entries
            .lock()
            .expect("store lock")
            .insert(format!("{service}/{key}"), value.to_string());
        Ok(())
    }

    async fn delete(
        &self,
        service: &str,
        key: &str,
    ) -> std::result::Result<(), infiltrator_ports::error::PortError> {
        self.entries
            .lock()
            .expect("store lock")
            .remove(&format!("{service}/{key}"));
        Ok(())
    }
}

struct Fixture {
    _dir: tempfile::TempDir,
    session: CoreSession,
    config: ConfigManager<MockStore>,
    controller: Arc<MockController>,
    reloader: MockReloader,
}

async fn fixture(probe_failures: u64, reload_fails: bool) -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = ConfigManager::with_home_and_store(
        dir.path().to_path_buf(),
        MockStore {
            entries: Mutex::new(HashMap::new()),
        },
    )
    .expect("config manager");
    config
        .ensure_default_config()
        .await
        .expect("default config");
    config.save("main", OLD).await.expect("seed profile");
    config.set_current("main").await.expect("set current");

    let controller = Arc::new(MockController {
        running: AtomicBool::new(false),
        fail_starts_left: AtomicU64::new(0),
    });
    let session = CoreSession::new(
        controller.clone(),
        Arc::new(StaticEndpoints),
        Arc::new(MockProbe {
            failures_left: AtomicU64::new(probe_failures),
        }),
    );
    Fixture {
        _dir: dir,
        session,
        config,
        controller,
        reloader: MockReloader {
            fail: reload_fails,
            calls: AtomicU64::new(0),
        },
    }
}

fn params(strategy: ApplyStrategy) -> ApplyParams {
    ApplyParams {
        strategy,
        health_timeout: Duration::from_secs(5),
        restart_timeout: Duration::from_secs(5),
        snapshot_history: false,
    }
}

async fn file_content(config: &ConfigManager<MockStore>) -> String {
    let _current = config.get_current().await.expect("current");
    let path = config.get_current_path().await.expect("path");
    tokio::fs::read_to_string(path).await.expect("profile file")
}

#[tokio::test]
async fn hot_reload_success_keeps_generation_and_updates_file() {
    let f = fixture(0, false).await;
    let generation = f.session.start().await.expect("start");
    f.session
        .wait_for_ready(generation, Duration::from_secs(5))
        .await
        .expect("ready");

    let outcome = apply_current_profile(
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
    assert_eq!(file_content(&f.config).await, NEW);
    assert_eq!(f.session.status(), CoreStatus::Ready);
    assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 1);
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

    let outcome = apply_current_profile(
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
    assert_eq!(file_content(&f.config).await, NEW);
    assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 1);
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

    let err = apply_current_profile(
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
    assert_eq!(f.session.status(), CoreStatus::Ready);
}

#[tokio::test]
async fn stopped_core_starts_with_new_config_without_reload() {
    let f = fixture(0, false).await;

    let outcome = apply_current_profile(
        &f.session,
        &f.config,
        &f.reloader,
        NEW,
        params(ApplyStrategy::PreferReload),
    )
    .await
    .expect("apply");

    assert_eq!(outcome.method, ApplyMethod::Restart);
    assert_eq!(file_content(&f.config).await, NEW);
    assert_eq!(f.session.status(), CoreStatus::Ready);
    assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 0);
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

    apply_current_profile(&f.session, &f.config, &f.reloader, NEW, p)
        .await
        .expect("apply");

    let config_dir = f._dir.path().join("configs");
    let snapshots = crate::history::list_snapshots(&config_dir, "main")
        .await
        .expect("list");
    assert_eq!(snapshots.len(), 1);
    assert_eq!(
        crate::history::read_snapshot(&snapshots[0].path)
            .await
            .unwrap(),
        NEW
    );
}

#[tokio::test]
async fn invalid_content_aborts_before_any_write() {
    let f = fixture(0, false).await;

    let err = apply_current_profile(
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

    let err = apply_current_profile(
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
            status: infiltrator_contract::snapshot::CoreLifecycle::Starting
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
async fn fidelity_apply_current_profile_doc_preserves_100_percent() {
    let f = fixture(0, false).await;
    f.config
        .save("main", COMPLEX_YAML_WITH_COMMENTS_AND_ANCHORS)
        .await
        .unwrap();

    let doc = SourceDoc::parse(COMPLEX_YAML_WITH_COMMENTS_AND_ANCHORS).unwrap();
    let outcome = apply_current_profile_doc(
        &f.session,
        &f.config,
        &f.reloader,
        &doc,
        params(ApplyStrategy::PreferReload),
    )
    .await
    .expect("apply doc");

    assert_eq!(outcome.method, ApplyMethod::Restart);
    let disk = file_content(&f.config).await;
    assert_eq!(
        disk, COMPLEX_YAML_WITH_COMMENTS_AND_ANCHORS,
        "byte-exact fidelity preservation"
    );
}

#[tokio::test]
async fn fidelity_scalar_override_preserves_all_comments_and_anchors() {
    let f = fixture(0, false).await;
    f.config
        .save("main", COMPLEX_YAML_WITH_COMMENTS_AND_ANCHORS)
        .await
        .unwrap();

    let outcome = apply_profile_set_scalar(
        &f.session,
        &f.config,
        &f.reloader,
        "mode",
        "global",
        params(ApplyStrategy::PreferReload),
    )
    .await
    .expect("apply scalar override");

    assert_eq!(outcome.method, ApplyMethod::Restart);
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
    f.config
        .save("main", COMPLEX_YAML_WITH_COMMENTS_AND_ANCHORS)
        .await
        .unwrap();

    // 1. Append rule
    apply_profile_append_rule(
        &f.session,
        &f.config,
        &f.reloader,
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
    apply_profile_remove_rule(
        &f.session,
        &f.config,
        &f.reloader,
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
    f.config
        .save("main", COMPLEX_YAML_WITH_COMMENTS_AND_ANCHORS)
        .await
        .unwrap();

    apply_profile_rewrite_anchors(
        &f.session,
        &f.config,
        &f.reloader,
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
    f.config
        .save("main", COMPLEX_YAML_WITH_COMMENTS_AND_ANCHORS)
        .await
        .unwrap();

    let mixin = crate::mixin::MixinConfig {
        mode: Some("direct".to_string()),
        mixed_port: Some(9999),
        rules: Some(crate::mixin::RuleMixin {
            append: vec!["DOMAIN,custom-mixin.com,DIRECT".to_string()],
            ..Default::default()
        }),
        ..Default::default()
    };

    apply_profile_mixin_fidelity(
        &f.session,
        &f.config,
        &f.reloader,
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
