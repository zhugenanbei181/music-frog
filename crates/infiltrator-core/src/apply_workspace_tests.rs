//! test-intent: behavior
use super::*;
use crate::apply_workspace::apply_confirmed_workspace;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceState, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_domain::profile_options::{FilterSpec, ProfileOptions};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspacePurpose, ProfileWorkspaceUpdate};
use std::fmt::Debug;
use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::time::timeout;

async fn selected_reference(fixture: &Fixture) -> ActiveProfileReference {
    match ProfileStore::read_active_profile_reference(&fixture.config)
        .await
        .unwrap()
    {
        ActiveProfileReferenceState::Selected(reference) => reference,
        ActiveProfileReferenceState::Unselected { .. } => {
            panic!("workspace apply fixture must have a selected profile")
        }
    }
}

async fn apply_workspace(
    fixture: &Fixture,
    reloader: &dyn ConfigReloader,
    reference: &ActiveProfileReference,
    expected: &ProfileWorkspaceIdentity,
    update: &ProfileWorkspaceUpdate,
    params: ApplyParams,
) -> ApplyResult<ProfileWorkspace> {
    apply_confirmed_workspace(
        &fixture.session,
        &fixture.config,
        reloader,
        reference,
        expected,
        update,
        ActiveProfileReferenceWriteAuthorization::Unowned,
        params,
    )
    .await
}

fn proposal() -> ProfileWorkspaceUpdate {
    let options = ProfileOptions {
        filter: Some(FilterSpec {
            include_keywords: vec!["keep".into()],
            ..Default::default()
        }),
        ..Default::default()
    };
    ProfileWorkspaceUpdate {
        purpose: ProfileWorkspacePurpose::Derived,
        document_bytes: NEW.as_bytes().to_vec(),
        sidecar_bytes: Some(serde_yaml_ng::to_string(&options).unwrap().into_bytes()),
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
    let reference = selected_reference(&f).await;
    let committed = apply_workspace(
        &f,
        &f.reloader,
        &reference,
        &before.identity(),
        &proposal(),
        params(ApplyStrategy::PreferReload),
    )
    .await
    .unwrap();
    assert_eq!(committed.document.utf8_text().unwrap(), NEW);
    assert_eq!(committed.sidecar_bytes, proposal().sidecar_bytes);
    assert_eq!(f.config.load_active_workspace().await.unwrap(), committed);
    assert_ne!(committed.identity(), before.identity());
    assert!(committed.sidecar_bytes.is_some());
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
    let reference = selected_reference(&f).await;
    let mut changed_sidecar = proposal();
    changed_sidecar.document_bytes = before.document.raw_bytes.clone();
    let changed = publish_active_workspace_for_test(
        &f.config,
        &reference,
        &before.identity(),
        changed_sidecar,
    )
    .await;
    ready(&f).await;
    assert_eq!(
        apply_workspace(
            &f,
            &f.reloader,
            &reference,
            &before.identity(),
            &proposal(),
            params(ApplyStrategy::PreferReload),
        )
        .await
        .unwrap_err(),
        ApplyError::SourceChanged
    );
    assert_eq!(f.config.load_active_workspace().await.unwrap(), changed);
    f.config.save_inactive_profile("other", OLD).await.unwrap();
    super::select_profile(&f.config, "other").await;
    assert_eq!(
        apply_workspace(
            &f,
            &f.reloader,
            &reference,
            &changed.identity(),
            &proposal(),
            params(ApplyStrategy::PreferReload),
        )
        .await
        .unwrap_err(),
        ApplyError::SourceChanged
    );
    assert_eq!(f.config.load("main").await.unwrap(), OLD);
    assert_eq!(f.config.load("other").await.unwrap(), OLD);
    assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.session.lifecycle(), CoreLifecycle::Ready);
    assert!(f.controller.running.load(Ordering::SeqCst));
}

#[tokio::test]
async fn foreign_engine_is_rejected_before_document_validation_or_runtime_work() {
    let f = fixture(0, false).await;
    let before = f.config.load_active_workspace().await.unwrap();
    let reference = selected_reference(&f).await;
    let mut foreign = before.identity();
    foreign.document.engine_id = EngineId::new("sing-box").unwrap();
    let update = ProfileWorkspaceUpdate {
        document_bytes: vec![0xFF],
        ..proposal()
    };

    let failure = apply_workspace(
        &f,
        &f.reloader,
        &reference,
        &foreign,
        &update,
        params(ApplyStrategy::PreferReload),
    )
    .await
    .unwrap_err();

    assert_eq!(failure, ApplyError::SourceChanged);
    assert_eq!(f.config.load_active_workspace().await.unwrap(), before);
    assert_eq!(f.reloader.calls.load(Ordering::SeqCst), 0);
    assert_eq!(f.session.lifecycle(), CoreLifecycle::Stopped);
    assert!(!f.controller.running.load(Ordering::SeqCst));
}

#[tokio::test]
async fn apply_failure_restores_exact_sidecar_or_absence_and_recovers_the_old_core() {
    for sidecar in [None, Some("# operator annotation\nmixin:\n  mode: rule\n")] {
        let f = fixture(0, true).await;
        if let Some(text) = sidecar {
            let reference = selected_reference(&f).await;
            let previous = f.config.load_active_workspace().await.unwrap();
            let update = ProfileWorkspaceUpdate {
                purpose: ProfileWorkspacePurpose::Derived,
                document_bytes: previous.document.raw_bytes.clone(),
                sidecar_bytes: Some(text.as_bytes().to_vec()),
            };
            publish_active_workspace_for_test(&f.config, &reference, &previous.identity(), update)
                .await;
        }
        ready(&f).await;
        let before = f.config.load_active_workspace().await.unwrap();
        let reference = selected_reference(&f).await;
        f.controller.fail_starts_left.store(1, Ordering::SeqCst);
        let failure = apply_workspace(
            &f,
            &f.reloader,
            &reference,
            &before.identity(),
            &proposal(),
            params(ApplyStrategy::PreferReload),
        )
        .await
        .unwrap_err();
        assert!(matches!(failure, ApplyError::RolledBack { .. }));
        assert_eq!(f.config.load_active_workspace().await.unwrap(), before);
        assert_eq!(before.sidecar_bytes.as_deref(), sidecar.map(str::as_bytes));
        assert_eq!(f.session.lifecycle(), CoreLifecycle::Ready);
        assert!(f.controller.running.load(Ordering::SeqCst));
        assert_eq!(
            f.config.apply_transaction("main").unwrap().stage,
            ApplyTransactionStage::RolledBack
        );
    }
}

async fn assert_pending<F>(mut future: Pin<&mut F>, description: &str)
where
    F: Future,
    F::Output: Debug,
{
    poll_fn(|context| match future.as_mut().poll(context) {
        Poll::Pending => Poll::Ready(()),
        Poll::Ready(output) => panic!("{description}: {output:?}"),
    })
    .await;
}

#[derive(Default)]
struct ReloadGate {
    entered: Notify,
    resume: Notify,
}

#[derive(Clone, Copy)]
enum GatedReloadOutcome {
    Succeed,
    Fail,
}

struct GatedReloader<'a> {
    gate: &'a ReloadGate,
    outcome: GatedReloadOutcome,
}

#[async_trait]
impl PreparedConfigReload for GatedReloader<'_> {
    async fn reload(&self, _: &Path) -> Result<(), String> {
        self.gate.entered.notify_one();
        self.gate.resume.notified().await;
        match self.outcome {
            GatedReloadOutcome::Succeed => Ok(()),
            GatedReloadOutcome::Fail => Err("gated reload rejected".into()),
        }
    }
}

#[async_trait]
impl ConfigReloader for GatedReloader<'_> {
    async fn prepare(&self) -> Result<Box<dyn PreparedConfigReload + '_>, String> {
        Ok(Box::new(self))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EndpointPreparationObservation {
    Unchecked,
    ActiveReferenceReadable,
    ActiveReferenceLeaseWasAlreadyHeld,
    ReadFailed,
}

struct EndpointPreparationProbe<'a> {
    config: &'a ConfigManager<MockStore>,
    observation: Mutex<EndpointPreparationObservation>,
}

impl EndpointPreparationProbe<'_> {
    fn observation(&self) -> EndpointPreparationObservation {
        *self
            .observation
            .lock()
            .expect("endpoint preparation observation")
    }
}

struct PreparedProbeReload;

#[async_trait]
impl PreparedConfigReload for PreparedProbeReload {
    async fn reload(&self, _: &Path) -> Result<(), String> {
        Ok(())
    }
}

#[async_trait]
impl ConfigReloader for EndpointPreparationProbe<'_> {
    async fn prepare(&self) -> Result<Box<dyn PreparedConfigReload + '_>, String> {
        match timeout(
            Duration::from_secs(1),
            ProfileStore::read_active_profile_reference(self.config),
        )
        .await
        {
            Ok(Ok(_)) => {
                *self
                    .observation
                    .lock()
                    .expect("endpoint preparation observation") =
                    EndpointPreparationObservation::ActiveReferenceReadable;
                Ok(Box::new(PreparedProbeReload))
            }
            Ok(Err(_)) => {
                *self
                    .observation
                    .lock()
                    .expect("endpoint preparation observation") =
                    EndpointPreparationObservation::ReadFailed;
                Err("active reference could not be read during endpoint preparation".into())
            }
            Err(_) => {
                *self
                    .observation
                    .lock()
                    .expect("endpoint preparation observation") =
                    EndpointPreparationObservation::ActiveReferenceLeaseWasAlreadyHeld;
                Err("endpoint preparation ran while the active reference lease was held".into())
            }
        }
    }
}

#[tokio::test]
async fn workspace_reload_endpoint_is_prepared_before_active_workspace_lease() {
    let f = fixture(0, false).await;
    ready(&f).await;
    let before = f.config.load_active_workspace().await.unwrap();
    let reference = selected_reference(&f).await;
    let reloader = EndpointPreparationProbe {
        config: &f.config,
        observation: Mutex::new(EndpointPreparationObservation::Unchecked),
    };

    let committed = apply_workspace(
        &f,
        &reloader,
        &reference,
        &before.identity(),
        &proposal(),
        params(ApplyStrategy::PreferReload),
    )
    .await
    .expect("workspace apply prepares its controller endpoint before locking the profile");

    assert_eq!(committed.document.utf8_text().unwrap(), NEW);
    assert_eq!(
        reloader.observation(),
        EndpointPreparationObservation::ActiveReferenceReadable
    );
}

#[tokio::test]
async fn selected_profile_reload_endpoint_is_prepared_before_selection_lease() {
    let f = fixture(0, false).await;
    ready(&f).await;
    let reference = selected_reference(&f).await;
    let reloader = EndpointPreparationProbe {
        config: &f.config,
        observation: Mutex::new(EndpointPreparationObservation::Unchecked),
    };

    apply_active_profile_reference(
        &f.session,
        &f.config,
        &reloader,
        &reference,
        params(ApplyStrategy::PreferReload),
    )
    .await
    .expect("active profile apply prepares its endpoint before acquiring selection lease");

    assert_eq!(
        reloader.observation(),
        EndpointPreparationObservation::ActiveReferenceReadable
    );
}

#[tokio::test]
async fn competing_option_edit_waits_for_the_apply_lease_and_is_fenced_after_recovery() {
    let f = fixture(0, false).await;
    ready(&f).await;
    let before = f.config.load_active_workspace().await.unwrap();
    let reference = selected_reference(&f).await;
    f.controller.fail_starts_left.store(1, Ordering::SeqCst);
    let options = ProfileOptions {
        filter: Some(FilterSpec {
            include_keywords: vec!["later edit".into()],
            ..Default::default()
        }),
        ..Default::default()
    };
    let competing_update = ProfileWorkspaceUpdate {
        purpose: ProfileWorkspacePurpose::Derived,
        document_bytes: before.document.raw_bytes.clone(),
        sidecar_bytes: Some(serde_yaml_ng::to_string(&options).unwrap().into_bytes()),
    };
    let gate = ReloadGate::default();
    let reloader = GatedReloader {
        gate: &gate,
        outcome: GatedReloadOutcome::Fail,
    };
    let proposed = proposal();
    let expected_identity = before.identity();
    let (failure, competing_write) = timeout(Duration::from_secs(2), async {
        let apply = apply_workspace(
            &f,
            &reloader,
            &reference,
            &expected_identity,
            &proposed,
            params(ApplyStrategy::PreferReload),
        );
        tokio::pin!(apply);
        tokio::select! {
            result = &mut apply => panic!("apply completed before its reload gate: {result:?}"),
            _ = gate.entered.notified() => {}
        }

        let competing_write = f
            .config
            .compare_and_save_inactive_workspace(&expected_identity, &competing_update);
        tokio::pin!(competing_write);
        assert_pending(
            competing_write.as_mut(),
            "competing workspace write completed while the apply lease was held",
        )
        .await;

        gate.resume.notify_one();
        tokio::join!(apply, competing_write)
    })
    .await
    .expect("apply recovery releases the competing workspace write");
    let failure = failure.unwrap_err();
    assert!(matches!(failure, ApplyError::RolledBack { .. }));
    assert_eq!(
        competing_write.unwrap_err().error_code(),
        ErrorCode::NotReady
    );
    assert_eq!(f.config.load_active_workspace().await.unwrap(), before);
    assert_eq!(f.config.load("main").await.unwrap(), OLD);
    assert_eq!(
        f.config.apply_transaction("main").unwrap().stage,
        ApplyTransactionStage::RolledBack
    );
}

#[tokio::test]
async fn active_profile_switch_waits_for_apply_lease_and_runs_after_commit() {
    let f = fixture(0, false).await;
    f.config.save_inactive_profile("other", OLD).await.unwrap();
    ready(&f).await;
    let before = f.config.load_active_workspace().await.unwrap();
    let reference = selected_reference(&f).await;
    let gate = ReloadGate::default();
    let reloader = GatedReloader {
        gate: &gate,
        outcome: GatedReloadOutcome::Succeed,
    };
    let expected_identity = before.identity();
    let proposed = proposal();
    let applied = timeout(Duration::from_secs(2), async {
        let apply = apply_workspace(
            &f,
            &reloader,
            &reference,
            &expected_identity,
            &proposed,
            params(ApplyStrategy::PreferReload),
        );
        tokio::pin!(apply);
        tokio::select! {
            result = &mut apply => panic!("apply completed before its reload gate: {result:?}"),
            _ = gate.entered.notified() => {}
        }

        let switch = super::select_profile(&f.config, "other");
        tokio::pin!(switch);
        assert_pending(
            switch.as_mut(),
            "active profile switch completed while the apply lease was held",
        )
        .await;

        gate.resume.notify_one();
        let (applied, ()) = tokio::join!(apply, switch);
        applied
    })
    .await
    .expect("apply releases the competing profile switch");
    let applied = applied.unwrap();
    assert_eq!(applied.document.utf8_text().unwrap(), NEW);
    assert_eq!(applied.sidecar_bytes, proposal().sidecar_bytes);
    assert_ne!(applied.identity(), before.identity());
    assert_eq!(f.config.get_current().await.unwrap(), "other");
    assert_eq!(f.config.load("other").await.unwrap(), OLD);
    assert_eq!(f.config.load("main").await.unwrap(), NEW);
    assert_eq!(
        f.config.apply_transaction("main").unwrap().stage,
        ApplyTransactionStage::Committed
    );
}
