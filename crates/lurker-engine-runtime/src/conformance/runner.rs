//! Neutral adapter conformance runner.
//!
//! The runner only uses the engine-neutral ports carried by
//! [`EngineBackendPorts`]. It starts and stops the Core, derives the active
//! [`CoreScope`] from the adapter-reported identity and readiness version, and
//! asserts each group independently. Every failure is a
//! [`ConformanceFailure`]; nothing is reported as a bare string.

mod errors;
mod outbound;

use self::errors::{verify_routing, verify_typed_errors};
use self::outbound::verify_outbound;
use super::case::{AdapterConformanceCase, LifecycleExpectation};
use super::failure::{ConformanceCheck, ConformanceFailure};
use super::group::ConformanceGroup;
use super::readiness::wait_for_readiness;
use async_trait::async_trait;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::identity::{CoreGeneration, CoreScope};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_ports::engine_backend::EngineBackendPorts;
use lurker_core_ports::error::PortError;

/// A host-owned check that must run while the Core is ready.
///
/// The product harness uses this to assert product-only behavior (for example
/// an unsupported live profile save) without giving the neutral kit a product
/// port. The runner still owns start, stop and terminal verification, so a
/// failing hook cannot leak a running process.
#[async_trait]
pub trait AdapterConformanceRunningCheck: Send + Sync {
    async fn check(&self) -> Result<(), ConformanceFailure>;
}

/// Run every selected group against one composed backend.
pub async fn run_adapter_conformance(
    case: &AdapterConformanceCase,
    backend: &EngineBackendPorts,
    groups: &[ConformanceGroup],
) -> Result<(), ConformanceFailure> {
    run_adapter_conformance_with_running_check(case, backend, groups, None).await
}

/// Run every selected group and invoke `while_running` after readiness.
pub async fn run_adapter_conformance_with_running_check(
    case: &AdapterConformanceCase,
    backend: &EngineBackendPorts,
    groups: &[ConformanceGroup],
    while_running: Option<&dyn AdapterConformanceRunningCheck>,
) -> Result<(), ConformanceFailure> {
    case.validate()?;
    verify_engine_binding(case, backend)?;
    if groups.contains(&ConformanceGroup::Document) {
        verify_documents(case, backend)?;
    }
    if case.lifecycle == LifecycleExpectation::ObserveStoppedOnly {
        if groups.iter().any(|group| {
            matches!(
                group,
                ConformanceGroup::Outbound | ConformanceGroup::Routing | ConformanceGroup::Errors
            )
        }) {
            return Err(ConformanceFailure::state(
                ConformanceCheck::LifecycleExpectation,
                "an externally managed stopped Core",
                "ObserveStoppedOnly",
                "outbound, routing and error groups require a started Core",
            ));
        }
        if groups.contains(&ConformanceGroup::Lifecycle) {
            verify_initial_stopped(backend).await?;
        }
        return Ok(());
    }
    if !requires_running(groups) {
        return Ok(());
    }

    verify_initial_stopped(backend).await?;
    let process = backend.process();
    let identity = process.core_identity().ok_or_else(|| {
        ConformanceFailure::state(
            ConformanceCheck::EngineBinding,
            "a stable process identity",
            "None",
            "the process adapter must report a CoreIdentity before start",
        )
    })?;
    process
        .start(CoreProcessStartContext::standalone(identity))
        .await
        .map_err(|error| {
            ConformanceFailure::port_error(
                ConformanceCheck::LifecycleStart,
                "the Core process starts",
                "start rejected by the adapter",
                &error,
            )
        })?;

    let run_result = run_while_running(case, backend, groups, while_running).await;
    let stop_result = process.stop().await;
    let terminal_result = verify_terminal(backend).await;
    combine_run_and_cleanup(case, run_result, stop_result, terminal_result)
}

fn requires_running(groups: &[ConformanceGroup]) -> bool {
    groups.iter().any(|group| {
        matches!(
            group,
            ConformanceGroup::Lifecycle
                | ConformanceGroup::Outbound
                | ConformanceGroup::Routing
                | ConformanceGroup::Errors
        )
    })
}

fn verify_engine_binding(
    case: &AdapterConformanceCase,
    backend: &EngineBackendPorts,
) -> Result<(), ConformanceFailure> {
    if backend.engine_id() != &case.engine {
        return Err(ConformanceFailure::mismatch(
            ConformanceCheck::EngineBinding,
            case.engine.as_str(),
            backend.engine_id().as_str(),
            "the backend bundle is registered under another engine",
        ));
    }
    let identity = backend.process().core_identity().ok_or_else(|| {
        ConformanceFailure::state(
            ConformanceCheck::EngineBinding,
            "a stable process identity",
            "None",
            "the process adapter must report a CoreIdentity",
        )
    })?;
    if identity.engine != case.engine {
        return Err(ConformanceFailure::mismatch(
            ConformanceCheck::EngineBinding,
            case.engine.as_str(),
            identity.engine.as_str(),
            "the process adapter belongs to another engine",
        ));
    }
    let codec_scope = backend.configuration().scope();
    if codec_scope.engine_id != case.engine {
        return Err(ConformanceFailure::mismatch(
            ConformanceCheck::CodecScope,
            case.engine.as_str(),
            codec_scope.engine_id.as_str(),
            "the document codec belongs to another engine",
        ));
    }
    for id in [
        &case.collection_id,
        &case.initial_member_id,
        &case.alternate_member_id,
        &case.probe_outbound_id,
    ] {
        if id.engine_id() != &case.engine {
            return Err(ConformanceFailure::state(
                ConformanceCheck::OutboundScope,
                format!("outbound IDs bound to {}", case.engine.as_str()),
                format!("{} is bound to {}", id.value(), id.engine_id().as_str()),
                "every outbound ID must belong to the conformance engine",
            ));
        }
    }
    Ok(())
}

fn verify_documents(
    case: &AdapterConformanceCase,
    backend: &EngineBackendPorts,
) -> Result<(), ConformanceFailure> {
    let codec = backend.configuration();
    let scope = codec.scope();
    if scope.format_id != case.profile_format || scope.schema_version != case.profile_schema {
        return Err(ConformanceFailure::mismatch(
            ConformanceCheck::CodecScope,
            format!(
                "{} schema {}",
                case.profile_format.as_str(),
                case.profile_schema.value()
            ),
            format!(
                "{} schema {}",
                scope.format_id.as_str(),
                scope.schema_version.value()
            ),
            "the codec scope differs from the conformance case",
        ));
    }

    match codec.inspect(&case.valid_document) {
        Ok(DocumentValidation::Valid) => {}
        Ok(other) => {
            return Err(ConformanceFailure::mismatch(
                ConformanceCheck::DocumentValid,
                "Valid",
                format!("{other:?}"),
                "the valid fixture was not accepted",
            ));
        }
        Err(error) => {
            return Err(ConformanceFailure::port_error(
                ConformanceCheck::DocumentValid,
                "Valid",
                "the valid fixture inspection failed",
                &error,
            ));
        }
    }
    match codec.inspect(&case.invalid_document) {
        Ok(DocumentValidation::Invalid { .. }) => {}
        Ok(other) => {
            return Err(ConformanceFailure::mismatch(
                ConformanceCheck::DocumentInvalid,
                "Invalid",
                format!("{other:?}"),
                "the syntactically invalid fixture was not rejected",
            ));
        }
        Err(error) => {
            return Err(ConformanceFailure::port_error(
                ConformanceCheck::DocumentInvalid,
                "Invalid",
                "the invalid fixture inspection failed unexpectedly",
                &error,
            ));
        }
    }
    match codec.inspect(&case.native_invalid_document) {
        Ok(DocumentValidation::Valid) => Ok(()),
        Ok(other) => Err(ConformanceFailure::mismatch(
            ConformanceCheck::DocumentNativeInvalid,
            "Valid",
            format!("{other:?}"),
            "the native-invalid fixture must pass neutral syntax inspection",
        )),
        Err(error) => Err(ConformanceFailure::port_error(
            ConformanceCheck::DocumentNativeInvalid,
            "Valid",
            "the native-invalid fixture failed neutral syntax inspection",
            &error,
        )),
    }
}

async fn verify_initial_stopped(backend: &EngineBackendPorts) -> Result<(), ConformanceFailure> {
    let process = backend.process();
    match process.status().await {
        Ok(CoreLifecycle::Stopped) => {}
        Ok(other) => {
            return Err(ConformanceFailure::state(
                ConformanceCheck::LifecycleInitial,
                "Stopped",
                format!("{other:?}"),
                "conformance requires an initially stopped process",
            ));
        }
        Err(error) => {
            return Err(ConformanceFailure::port_error(
                ConformanceCheck::LifecycleInitial,
                "Stopped",
                "the initial process state is unreadable",
                &error,
            ));
        }
    }
    match process.pid().await {
        None => Ok(()),
        Some(pid) => Err(ConformanceFailure::state(
            ConformanceCheck::LifecycleInitial,
            "no owned process id",
            format!("pid {pid}"),
            "a stopped process must not report an owned pid",
        )),
    }
}

async fn run_while_running(
    case: &AdapterConformanceCase,
    backend: &EngineBackendPorts,
    groups: &[ConformanceGroup],
    while_running: Option<&dyn AdapterConformanceRunningCheck>,
) -> Result<(), ConformanceFailure> {
    let observed_version = wait_for_readiness(backend.readiness().as_ref(), case.readiness_timeout)
        .await
        .map_err(|error| {
            ConformanceFailure::port_error(
                ConformanceCheck::LifecycleReadiness,
                "the Core becomes ready",
                "the readiness probe did not succeed",
                &error,
            )
        })?
        .ok_or_else(|| {
            ConformanceFailure::state(
                ConformanceCheck::LifecycleReadiness,
                "an active engine version",
                "None",
                "a ready Core must report its active version for scoped operations",
            )
        })?;

    if groups.contains(&ConformanceGroup::Lifecycle) && observed_version != case.expected_version {
        return Err(ConformanceFailure::mismatch(
            ConformanceCheck::LifecycleVersion,
            case.expected_version.raw(),
            observed_version.raw(),
            "the live version differs from the case version",
        ));
    }

    let process = backend.process();
    let mut identity = process.core_identity().ok_or_else(|| {
        ConformanceFailure::state(
            ConformanceCheck::EngineBinding,
            "a stable process identity",
            "None",
            "the running process lost its CoreIdentity",
        )
    })?;
    if identity.engine != case.engine {
        return Err(ConformanceFailure::mismatch(
            ConformanceCheck::EngineBinding,
            case.engine.as_str(),
            identity.engine.as_str(),
            "the running process engine identity changed",
        ));
    }
    if let Some(process_version) = &identity.version
        && process_version != &observed_version
    {
        return Err(ConformanceFailure::mismatch(
            ConformanceCheck::LifecycleVersion,
            observed_version.raw(),
            process_version.raw(),
            "the process identity version differs from the live readiness version",
        ));
    }
    identity.version = Some(observed_version);
    let scope = CoreScope {
        identity,
        generation: CoreGeneration::new(1),
        session_token: None,
    };

    match process.status().await {
        Ok(CoreLifecycle::Running) => {}
        Ok(other) => {
            return Err(ConformanceFailure::state(
                ConformanceCheck::LifecycleRunning,
                "Running",
                format!("{other:?}"),
                "a ready controller must be backed by a running process",
            ));
        }
        Err(error) => {
            return Err(ConformanceFailure::port_error(
                ConformanceCheck::LifecycleRunning,
                "Running",
                "the running state is unreadable",
                &error,
            ));
        }
    }

    if let Some(check) = while_running {
        check.check().await?;
    }
    if groups.contains(&ConformanceGroup::Outbound) {
        verify_outbound(case, backend, &scope).await?;
    }
    if groups.contains(&ConformanceGroup::Routing) {
        verify_routing(case, backend, &scope).await?;
    }
    if groups.contains(&ConformanceGroup::Errors) {
        verify_typed_errors(case, backend, &scope).await?;
    }
    Ok(())
}

async fn verify_terminal(backend: &EngineBackendPorts) -> Result<(), ConformanceFailure> {
    let process = backend.process();
    match process.status().await {
        Ok(CoreLifecycle::Stopped) => {}
        Ok(other) => {
            return Err(ConformanceFailure::state(
                ConformanceCheck::LifecycleTerminal,
                "Stopped",
                format!("{other:?}"),
                "process cleanup left a non-terminal lifecycle",
            ));
        }
        Err(error) => {
            return Err(ConformanceFailure::port_error(
                ConformanceCheck::LifecycleTerminal,
                "Stopped",
                "the terminal process state is unreadable",
                &error,
            ));
        }
    }
    match process.pid().await {
        None => Ok(()),
        Some(pid) => Err(ConformanceFailure::state(
            ConformanceCheck::LifecycleTerminal,
            "no owned process id",
            format!("pid {pid}"),
            "the stopped process still reports an owned pid",
        )),
    }
}

fn combine_run_and_cleanup(
    case: &AdapterConformanceCase,
    run_result: Result<(), ConformanceFailure>,
    stop_result: Result<(), PortError>,
    terminal_result: Result<(), ConformanceFailure>,
) -> Result<(), ConformanceFailure> {
    match (run_result, stop_result, terminal_result) {
        (Ok(()), Ok(()), Ok(())) => Ok(()),
        (Err(run_failure), Ok(()), Ok(())) => Err(run_failure),
        (run_result, stop_result, terminal_result) => {
            let mut cleanup = match stop_result {
                Err(error) => {
                    let failure = ConformanceFailure::port_error(
                        ConformanceCheck::LifecycleStop,
                        "the process stops",
                        "stop failed during conformance cleanup",
                        &error,
                    );
                    format!(
                        "the {} conformance run did not finish cleanly: {failure}",
                        case.engine.as_str()
                    )
                }
                Ok(()) => match terminal_result {
                    Err(failure) => format!(
                        "the {} conformance run did not finish cleanly: {failure}",
                        case.engine.as_str()
                    ),
                    Ok(()) => "the conformance run did not finish cleanly".to_owned(),
                },
            };
            if let Err(run_failure) = run_result {
                cleanup.push_str(&format!("; primary failure: {run_failure}"));
            }
            Err(ConformanceFailure::state(
                ConformanceCheck::LifecycleStop,
                "a clean stop and exact terminal after the run",
                "cleanup did not reach the exact terminal",
                cleanup,
            ))
        }
    }
}
