//! test-intent: behavior
use super::*;
use crate::apply_workspace::apply_confirmed_workspace;
use infiltrator_domain::profile_options::{FilterSpec, ProfileOptions, options_path};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspacePurpose, ProfileWorkspaceUpdate};
use tokio::fs::{create_dir_all, write};
use tokio::time::timeout;

fn proposal() -> ProfileWorkspaceUpdate {
    ProfileWorkspaceUpdate {
        purpose: ProfileWorkspacePurpose::Derived,
        content: NEW.into(),
        options: ProfileOptions {
            filter: Some(FilterSpec {
                include_keywords: vec!["keep".into()],
                ..Default::default()
            }),
            ..Default::default()
        },
    }
}
async fn ready(f: &Fixture) {
    let generation = f.session.start().await.unwrap();
    f.session
        .wait_for_ready(generation, Duration::from_secs(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn confirmed_workspace_commits_both_documents_and_reports_actual_stored_source() {
    let f = fixture(0, false).await;
    ready(&f).await;
    let before = f.config.load_active_workspace().await.unwrap();
    let committed = apply_confirmed_workspace(
        &f.session,
        &f.config,
        &f.reloader,
        &before.source,
        &proposal(),
        params(ApplyStrategy::PreferReload),
    )
    .await
    .unwrap();
    assert_eq!(committed.content, NEW);
    assert_eq!(committed.options, proposal().options);
    assert_eq!(f.config.load_active_workspace().await.unwrap(), committed);
    assert_ne!(committed.source, before.source);
    assert!(committed.options_document.is_some());
    assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 1);
    assert_eq!(f.session.lifecycle(), CoreLifecycle::Ready);
    assert_eq!(
        f.config.apply_transaction("main").unwrap().stage,
        ApplyTransactionStage::Committed
    );
}

#[tokio::test]
async fn changed_options_or_active_profile_rejects_without_applying_or_writing() {
    let f = fixture(0, false).await;
    let before = f.config.load_active_workspace().await.unwrap();
    f.config
        .save_options("main", &proposal().options)
        .await
        .unwrap();
    let changed = f.config.load_active_workspace().await.unwrap();
    assert_eq!(
        apply_confirmed_workspace(
            &f.session,
            &f.config,
            &f.reloader,
            &before.source,
            &proposal(),
            params(ApplyStrategy::PreferReload)
        )
        .await
        .unwrap_err(),
        ApplyError::SourceChanged
    );
    assert_eq!(f.config.load_active_workspace().await.unwrap(), changed);
    f.config.save("other", OLD).await.unwrap();
    f.config.set_current("other").await.unwrap();
    assert_eq!(
        apply_confirmed_workspace(
            &f.session,
            &f.config,
            &f.reloader,
            &changed.source,
            &proposal(),
            params(ApplyStrategy::PreferReload)
        )
        .await
        .unwrap_err(),
        ApplyError::SourceChanged
    );
    assert_eq!(f.config.load("main").await.unwrap(), OLD);
    assert_eq!(f.config.load("other").await.unwrap(), OLD);
    assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.session.lifecycle(), CoreLifecycle::Stopped);
}

#[tokio::test]
async fn apply_failure_restores_exact_sidecar_or_absence_and_recovers_the_old_core() {
    for sidecar in [None, Some("# operator annotation\nmixin:\n  mode: rule\n")] {
        let f = fixture(0, true).await;
        if let Some(text) = sidecar {
            let path = options_path(f.config.config_dir(), "main");
            create_dir_all(path.parent().unwrap()).await.unwrap();
            write(path, text).await.unwrap();
        }
        ready(&f).await;
        let before = f.config.load_active_workspace().await.unwrap();
        f.controller.fail_starts_left.store(1, Ordering::SeqCst);
        let failure = apply_confirmed_workspace(
            &f.session,
            &f.config,
            &f.reloader,
            &before.source,
            &proposal(),
            params(ApplyStrategy::PreferReload),
        )
        .await
        .unwrap_err();
        assert!(matches!(failure, ApplyError::RolledBack { .. }));
        assert_eq!(f.config.load_active_workspace().await.unwrap(), before);
        assert_eq!(before.options_document.as_deref(), sidecar);
        assert_eq!(f.session.lifecycle(), CoreLifecycle::Ready);
        assert!(f.controller.running.load(Ordering::SeqCst));
        assert_eq!(
            f.config.apply_transaction("main").unwrap().stage,
            ApplyTransactionStage::RolledBack
        );
    }
}

struct LaterOptions<'a> {
    config: &'a ConfigManager<MockStore>,
}
#[async_trait]
impl ConfigReloader for LaterOptions<'_> {
    async fn reload(&self, _: &Path) -> Result<(), String> {
        let options = ProfileOptions {
            filter: Some(FilterSpec {
                include_keywords: vec!["later edit".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        self.config
            .save_options("main", &options)
            .await
            .map_err(|error| error.to_string())?;
        Err("reload failed after a later option edit".into())
    }
}
#[tokio::test]
async fn later_option_edit_is_preserved_and_recovery_refuses_to_overwrite_it() {
    let f = fixture(0, false).await;
    ready(&f).await;
    let before = f.config.load_active_workspace().await.unwrap();
    f.controller.fail_starts_left.store(1, Ordering::SeqCst);
    let reloader = LaterOptions { config: &f.config };
    let failure = timeout(
        Duration::from_secs(2),
        apply_confirmed_workspace(
            &f.session,
            &f.config,
            &reloader,
            &before.source,
            &proposal(),
            params(ApplyStrategy::PreferReload),
        ),
    )
    .await
    .expect("lifecycle callback can write through the shared boundary")
    .unwrap_err();
    assert!(matches!(failure, ApplyError::RollbackFailed { .. }));
    assert_eq!(f.config.load("main").await.unwrap(), NEW);
    assert_eq!(
        f.config
            .load_options("main")
            .await
            .unwrap()
            .filter
            .unwrap()
            .include_keywords,
        vec!["later edit"]
    );
    assert_eq!(
        f.config.apply_transaction("main").unwrap().stage,
        ApplyTransactionStage::RollbackFailed
    );
}

struct SwitchActive<'a> {
    config: &'a ConfigManager<MockStore>,
}
#[async_trait]
impl ConfigReloader for SwitchActive<'_> {
    async fn reload(&self, _: &Path) -> Result<(), String> {
        self.config
            .set_current("other")
            .await
            .map_err(|error| error.to_string())
    }
}
#[tokio::test]
async fn successful_reload_cannot_report_applied_after_the_active_profile_changes() {
    let f = fixture(0, false).await;
    f.config.save("other", OLD).await.unwrap();
    ready(&f).await;
    let before = f.config.load_active_workspace().await.unwrap();
    let reloader = SwitchActive { config: &f.config };
    assert_eq!(
        apply_confirmed_workspace(
            &f.session,
            &f.config,
            &reloader,
            &before.source,
            &proposal(),
            params(ApplyStrategy::PreferReload)
        )
        .await
        .unwrap_err(),
        ApplyError::SourceChanged
    );
    assert_eq!(f.config.get_current().await.unwrap(), "other");
    assert_eq!(f.config.load("other").await.unwrap(), OLD);
    assert_eq!(f.config.load("main").await.unwrap(), NEW);
    assert_eq!(
        f.config.apply_transaction("main").unwrap().stage,
        ApplyTransactionStage::RollbackFailed
    );
}
