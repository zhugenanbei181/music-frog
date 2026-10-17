//! Application-specific deterministic capture assembly.
use infiltrator_application::connection_rate_application::connections_page_snapshot;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::log_application::{
    LogBindingIntent, LogBindingOutcome, LogIngestOutcome,
};
use infiltrator_application::rule_list_application::document_snapshot;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::core_identity::CoreCommandRequest;
use infiltrator_contract::logs::LogSession;
use infiltrator_contract::rule_document::RuleDocumentSnapshot;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface_snapshot::{PageData, SurfaceOrigin, SurfaceSnapshot};
use infiltrator_domain::rules::RuleEntry;
use infiltrator_ports::runtime_stream::RuntimeStreamEvent;
use infiltrator_presentation::shell_readout_fold::ShellReadoutProjection;
use infiltrator_test_support::connection_grouping_fixtures::grouping_snapshot;
use infiltrator_test_support::log_capture_fixtures::LOG_RECORDS;
use infiltrator_test_support::rule_list_fixtures::list_workspace;
use lurker_core_contract::error::{ErrorCode, Failure};

pub fn grouping_surface(mut surface: SurfaceSnapshot) -> SurfaceSnapshot {
    let connections = grouping_snapshot();
    let page = connections_page_snapshot(&connections, &Default::default());
    surface.origin = SurfaceOrigin::Demo;
    surface.failure = None;
    surface.core.failure = None;
    surface.core.lifecycle = CoreLifecycle::Running;
    surface.revision += 1;
    surface.core.revision += 1;
    surface.core.active_connections = page.total_connections as u32;
    if let Some(overview) = surface.pages.overview.data.as_mut() {
        overview.active_connections = surface.core.active_connections;
    }
    surface.pages.connections = PageData::ready(page);
    surface.shell_readout = ShellReadoutProjection::default().project(&surface);
    surface
}

pub fn list_document(rules: Vec<RuleEntry>) -> RuleDocumentSnapshot {
    let workspace = list_workspace(rules);
    document_snapshot(&workspace)
}

pub async fn populate_logs(core: &CoreApplication) -> Result<(), Failure> {
    let scope = core.snapshot().scope().ok_or_else(|| {
        Failure::new(
            ErrorCode::InvalidState,
            "isolated log controller has no owner scope",
            false,
        )
    })?;
    core.execute_scoped(CoreCommandRequest {
        scope,
        intent: CommandIntent::StartCore,
    })
    .await
    .result
    .into_unit()?;
    let snapshot = core.snapshot();
    let scope = snapshot
        .scope()
        .and_then(LogSession::from_core_scope)
        .ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidState,
                "isolated log controller has no identity-bound session",
                false,
            )
        })?;
    let logs = core.log_application();
    if logs.bind(LogBindingIntent::Bind(scope.clone())) == LogBindingOutcome::RejectedByCoreOwner {
        return Err(Failure::new(
            ErrorCode::InvalidState,
            "isolated log owner rejected fixture binding",
            false,
        ));
    }
    if logs.ingest(scope.clone(), RuntimeStreamEvent::Connected)
        == LogIngestOutcome::RejectedForeignSession
    {
        return Err(Failure::new(
            ErrorCode::InvalidState,
            "isolated log owner rejected fixture observation",
            false,
        ));
    }
    for record in LOG_RECORDS {
        if logs.ingest(scope.clone(), RuntimeStreamEvent::Item(record.into()))
            == LogIngestOutcome::RejectedForeignSession
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "isolated log session changed",
                false,
            ));
        }
    }
    Ok(())
}

pub fn append_follow_records(core: &CoreApplication) -> Result<(), Failure> {
    let snapshot = core.snapshot();
    let scope = LogSession::from_core_scope(
        snapshot
            .scope()
            .ok_or_else(|| Failure::new(ErrorCode::NotReady, "no isolated log session", false))?,
    )
    .ok_or_else(|| Failure::new(ErrorCode::NotReady, "no isolated log session", false))?;
    for index in 4..=35 {
        if core.log_application().ingest(
            scope.clone(),
            RuntimeStreamEvent::Item(format!(
                "INFO[12:01:{index:02}] continued controller record {index}"
            )),
        ) == LogIngestOutcome::RejectedForeignSession
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "isolated log session changed",
                false,
            ));
        }
    }
    Ok(())
}
