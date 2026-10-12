use super::*;

pub(super) async fn run_while_process_is_running(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
) -> Result<(), String> {
    wait_until_ready(ports.readiness).await?;
    let observed_version = ports
        .readiness
        .active_core_version()
        .await
        .map_err(|error| format!("live version observation failed: {error}"))?
        .ok_or_else(|| "ready core did not report an active engine version".to_owned())?;
    if observed_version != case.expected_version {
        return Err(format!(
            "live version {} differs from locked expected version {}",
            observed_version.raw(),
            case.expected_version.raw()
        ));
    }

    let mut identity = ports
        .process
        .core_identity()
        .ok_or_else(|| "running process lost its CoreIdentity".to_owned())?;
    if identity.engine != case.engine {
        return Err("running process engine identity changed".to_owned());
    }
    if let Some(process_version) = &identity.version
        && process_version != &observed_version
    {
        return Err(format!(
            "process identity version {} differs from live readiness version {}",
            process_version.raw(),
            observed_version.raw()
        ));
    }
    identity.version = Some(observed_version);
    let scope = CoreScope {
        identity,
        generation: CoreGeneration::new(1),
        session_token: None,
    };

    match ports.process.status().await {
        Ok(CoreLifecycle::Running) => {
            ports
                .transaction_owner
                .update_observed_lifecycle(CoreLifecycle::Running);
        }
        Ok(status) => {
            return Err(format!(
                "ready controller is not backed by a Running process: {status:?}"
            ));
        }
        Err(error) => return Err(format!("process state read failed: {error}")),
    }

    match case.running_profile_transaction {
        RunningProfileTransactionBehavior::Unsupported => {
            let before = ports
                .profile_store
                .load_active_workspace()
                .await
                .map_err(|error| {
                    format!("active workspace read before live-save check failed: {error}")
                })?;
            let expected_reference = match ports
                .profile_store
                .read_active_profile_reference()
                .await
                .map_err(|error| {
                    format!("active reference read before live-save check failed: {error}")
                })? {
                ActiveProfileReferenceState::Selected(reference) => reference,
                ActiveProfileReferenceState::Unselected { .. } => {
                    return Err("live-save check has no selected profile reference".to_owned());
                }
            };
            let result = ports
                .configuration_transaction
                .transact(ProfileConfigurationTransactionRequest {
                    expected_reference,
                    expected: before.identity(),
                    update: ProfileWorkspaceUpdate {
                        purpose: ProfileWorkspacePurpose::Derived,
                        document_bytes: case.valid_profile.clone(),
                        sidecar_bytes: before.sidecar_bytes.clone(),
                    },
                    authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
                    strategy: infiltrator_domain::apply::ApplyStrategy::PreferReload,
                })
                .await;
            match result {
                Err(error) if error.error_code() == ErrorCode::Unsupported => {}
                Err(error) => {
                    return Err(format!(
                        "{} live profile save returned the wrong typed error: {error}",
                        case.engine.as_str()
                    ));
                }
                Ok(_) => {
                    return Err(format!(
                        "{} live profile save was accepted despite the provider contract",
                        case.engine.as_str()
                    ));
                }
            }
            let after = ports
                .profile_store
                .load_active_workspace()
                .await
                .map_err(|error| {
                    format!("active workspace read after live-save check failed: {error}")
                })?;
            if after != before {
                return Err(format!(
                    "{} unsupported live profile save changed the workspace",
                    case.engine.as_str()
                ));
            }
        }
        RunningProfileTransactionBehavior::NotExercised => {}
    }

    let inventory = ports
        .outbound
        .inventory(scope.clone())
        .await
        .map_err(|error| format!("outbound inventory failed: {error}"))?;
    if inventory.scope() != &scope {
        return Err("outbound inventory escaped the active CoreScope".to_owned());
    }
    let initial_selection = selection_from_inventory(
        &inventory.inventory(),
        &case.collection_id,
        &case.initial_member_id,
    )?;
    if initial_selection != case.initial_member_id {
        return Err(format!(
            "selector {} started on {}, expected {}",
            case.collection_id.value(),
            initial_selection.value(),
            case.initial_member_id.value()
        ));
    }

    verify_scope_fences_have_no_selection_side_effect(case, ports, &scope, &inventory).await?;
    verify_stale_revision_has_no_selection_side_effect(case, ports, &scope, &inventory).await?;
    let selected = apply_selection(case, ports, &scope, &inventory).await?;
    verify_details(case, ports, &scope, selected.revision()).await?;
    verify_probe(case, ports, &scope, selected.revision()).await?;
    verify_selection_restore(case, ports, &scope, &initial_selection).await?;
    verify_routing(case, ports, &scope).await?;
    Ok(())
}

async fn verify_scope_fences_have_no_selection_side_effect(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
    active_scope: &CoreScope,
    inventory: &infiltrator_contract::outbound::inventory::OutboundInventorySnapshot,
) -> Result<(), String> {
    let next_generation = active_scope
        .generation
        .value()
        .checked_add(1)
        .ok_or_else(|| "core generation cannot advance for scope-fence check".to_owned())?;
    let mut stale_scope = active_scope.clone();
    stale_scope.generation = CoreGeneration::new(next_generation);
    verify_scope_selection_rejection(
        case,
        ports,
        active_scope,
        inventory,
        stale_scope,
        "stale generation",
    )
    .await?;

    let mut stale_version = active_scope.clone();
    stale_version.identity.version =
        Some(CoreVersion::new("v0.0.0").expect("static stale Core version is valid"));
    verify_scope_selection_rejection(
        case,
        ports,
        active_scope,
        inventory,
        stale_version,
        "stale version",
    )
    .await?;

    let mut stale_session = active_scope.clone();
    stale_session.session_token = Some(SessionToken::new(2));
    verify_scope_selection_rejection(
        case,
        ports,
        active_scope,
        inventory,
        stale_session,
        "stale session",
    )
    .await?;

    let mut foreign_scope = active_scope.clone();
    foreign_scope.identity.engine =
        EngineId::new("conformance-foreign-engine").expect("static foreign engine ID is valid");
    verify_scope_selection_rejection(
        case,
        ports,
        active_scope,
        inventory,
        foreign_scope,
        "foreign engine",
    )
    .await
}

async fn verify_scope_selection_rejection(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
    active_scope: &CoreScope,
    inventory: &infiltrator_contract::outbound::inventory::OutboundInventorySnapshot,
    rejected_scope: CoreScope,
    label: &str,
) -> Result<(), String> {
    match ports
        .outbound
        .select(
            rejected_scope,
            inventory.revision(),
            case.collection_id.clone(),
            case.alternate_member_id.clone(),
        )
        .await
    {
        Err(error) if error.error_code() == ErrorCode::InvalidState => {}
        Err(error) => {
            return Err(format!(
                "{label} selector scope returned the wrong typed error: {error}"
            ));
        }
        Ok(_) => return Err(format!("{label} selector scope was accepted")),
    }

    let after_rejection = ports
        .outbound
        .inventory(active_scope.clone())
        .await
        .map_err(|error| format!("inventory read after {label} rejection failed: {error}"))?;
    if after_rejection.revision() != inventory.revision() {
        return Err(format!(
            "{label} selector scope changed the active inventory revision"
        ));
    }
    let selected = selection_from_inventory(
        &after_rejection.inventory(),
        &case.collection_id,
        &case.initial_member_id,
    )?;
    if selected != case.initial_member_id {
        return Err(format!(
            "{label} selector scope changed the active outbound selection"
        ));
    }
    Ok(())
}

async fn wait_until_ready(readiness: &dyn CoreReadiness) -> Result<(), String> {
    let result = timeout(REAL_READINESS_TIMEOUT, async {
        loop {
            match readiness.probe().await {
                Ok(_) => break,
                Err(_) => sleep(READINESS_RETRY_INTERVAL).await,
            }
        }
    })
    .await;
    match result {
        Ok(()) => Ok(()),
        Err(_) => Err(format!(
            "controller did not become ready within {REAL_READINESS_TIMEOUT:?}"
        )),
    }
}

async fn verify_stale_revision_has_no_selection_side_effect(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
    scope: &CoreScope,
    inventory: &infiltrator_contract::outbound::inventory::OutboundInventorySnapshot,
) -> Result<(), String> {
    let stale_value =
        inventory.revision().value().checked_add(1).ok_or_else(|| {
            "inventory revision cannot be advanced for stale-write check".to_owned()
        })?;
    let stale_revision = SnapshotRevision::new(stale_value);
    let stale_write = ports
        .outbound
        .select(
            scope.clone(),
            stale_revision,
            case.collection_id.clone(),
            case.alternate_member_id.clone(),
        )
        .await;
    match stale_write {
        Err(error) if error.error_code() == ErrorCode::InvalidState => {}
        Err(error) => {
            return Err(format!(
                "stale selector revision returned the wrong typed error: {error}"
            ));
        }
        Ok(_) => return Err("stale selector revision was accepted".to_owned()),
    }

    let after_rejection = ports
        .outbound
        .inventory(scope.clone())
        .await
        .map_err(|error| format!("inventory read after stale write failed: {error}"))?;
    if after_rejection.revision() != inventory.revision() {
        return Err("stale selector revision changed the active inventory revision".to_owned());
    }
    let selected = selection_from_inventory(
        &after_rejection.inventory(),
        &case.collection_id,
        &case.initial_member_id,
    )?;
    if selected != case.initial_member_id {
        return Err("stale selector revision caused an outbound selection side effect".to_owned());
    }
    Ok(())
}

async fn apply_selection(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
    scope: &CoreScope,
    inventory: &infiltrator_contract::outbound::inventory::OutboundInventorySnapshot,
) -> Result<infiltrator_contract::outbound::inventory::OutboundInventorySnapshot, String> {
    let result = ports
        .outbound
        .select(
            scope.clone(),
            inventory.revision(),
            case.collection_id.clone(),
            case.alternate_member_id.clone(),
        )
        .await
        .map_err(|error| format!("selector write/readback failed: {error}"))?;
    let selected = selection_from_inventory(
        &result.inventory(),
        &case.collection_id,
        &case.alternate_member_id,
    )?;
    if selected != case.alternate_member_id {
        return Err("selector write did not read back the requested member".to_owned());
    }
    Ok(result)
}

async fn verify_details(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
    scope: &CoreScope,
    revision: SnapshotRevision,
) -> Result<(), String> {
    let details = ports
        .outbound
        .details(scope.clone(), revision)
        .await
        .map_err(|error| format!("outbound details observation failed: {error}"))?;
    if details.scope() != scope {
        return Err("outbound details escaped the active CoreScope".to_owned());
    }
    match (&case.details, details.state()) {
        (DetailsBehavior::Ready, OutboundDetailsState::Ready { details })
            if details
                .iter()
                .any(|detail| detail.id == case.probe_outbound_id) =>
        {
            Ok(())
        }
        (DetailsBehavior::Unsupported, OutboundDetailsState::Unsupported) => Ok(()),
        (expected, actual) => Err(format!(
            "outbound details state {actual:?} does not match expected {expected:?}"
        )),
    }
}

async fn verify_probe(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
    scope: &CoreScope,
    revision: SnapshotRevision,
) -> Result<(), String> {
    let probe = ports
        .outbound
        .probe(
            scope.clone(),
            revision,
            case.probe_outbound_id.clone(),
            case.probe_request.clone(),
        )
        .await
        .map_err(|error| format!("outbound probe failed: {error}"))?;
    if probe.scope() != scope || probe.outbound_id() != &case.probe_outbound_id {
        return Err("outbound probe result escaped its requested scope or outbound".to_owned());
    }
    match (&case.probe, probe.state()) {
        (ProbeBehavior::Measured, OutboundProbeState::Measured { .. }) => Ok(()),
        (ProbeBehavior::Unsupported, OutboundProbeState::Unsupported) => Ok(()),
        (expected, actual) => Err(format!(
            "outbound probe state {actual:?} does not match expected {expected:?}"
        )),
    }?;
    let after_probe = ports
        .outbound
        .inventory(scope.clone())
        .await
        .map_err(|error| format!("inventory read after outbound probe failed: {error}"))?;
    if after_probe.scope() != scope {
        return Err("outbound probe changed the observed CoreScope".to_owned());
    }
    let selected = selection_from_inventory(
        &after_probe.inventory(),
        &case.collection_id,
        &case.alternate_member_id,
    )?;
    if selected != case.alternate_member_id {
        return Err("outbound probe changed the current selector member".to_owned());
    }
    if case.probe == ProbeBehavior::Unsupported && after_probe.revision() != revision {
        return Err("unsupported outbound probe changed the inventory revision".to_owned());
    }
    Ok(())
}

async fn verify_selection_restore(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
    scope: &CoreScope,
    original_member: &OutboundId,
) -> Result<(), String> {
    let current = ports
        .outbound
        .inventory(scope.clone())
        .await
        .map_err(|error| format!("inventory read before selector restore failed: {error}"))?;
    let restored = ports
        .outbound
        .select(
            scope.clone(),
            current.revision(),
            case.collection_id.clone(),
            original_member.clone(),
        )
        .await
        .map_err(|error| format!("selector restore/readback failed: {error}"))?;
    let selected =
        selection_from_inventory(&restored.inventory(), &case.collection_id, original_member)?;
    if &selected != original_member {
        return Err("selector restore did not read back the original member".to_owned());
    }
    Ok(())
}

async fn verify_routing(
    case: &BackendConformanceCase,
    ports: &BackendConformancePorts<'_>,
    scope: &CoreScope,
) -> Result<(), String> {
    match &case.routing {
        RoutingBehavior::Writable { initial, requested } => {
            let observed = ports
                .routing
                .observe(scope)
                .await
                .map_err(|error| format!("initial routing observation failed: {error}"))?;
            if &observed != initial {
                return Err(format!(
                    "initial routing intent {observed:?} differs from expected {initial:?}"
                ));
            }
            let changed = ports
                .routing
                .apply(scope, requested.clone())
                .await
                .map_err(|error| format!("routing write/readback failed: {error}"))?;
            if &changed != requested {
                return Err("routing write returned a mismatching readback".to_owned());
            }
            let restored = ports
                .routing
                .apply(scope, initial.clone())
                .await
                .map_err(|error| format!("routing restore/readback failed: {error}"))?;
            if &restored != initial {
                return Err("routing restore returned a mismatching readback".to_owned());
            }
            Ok(())
        }
        RoutingBehavior::ReadOnlyUnsupported { initial, requested } => {
            let observed = ports
                .routing
                .observe(scope)
                .await
                .map_err(|error| format!("initial routing observation failed: {error}"))?;
            if &observed != initial {
                return Err(format!(
                    "initial routing intent {observed:?} differs from expected {initial:?}"
                ));
            }
            match ports.routing.apply(scope, requested.clone()).await {
                Err(error) if is_typed_unsupported(&error) => {}
                Err(error) => {
                    return Err(format!(
                        "unsupported routing write returned a non-Unsupported error: {error}"
                    ));
                }
                Ok(_) => return Err("unsupported routing write was accepted".to_owned()),
            }
            let after_rejection = ports
                .routing
                .observe(scope)
                .await
                .map_err(|error| format!("routing readback after rejection failed: {error}"))?;
            if &after_rejection != initial {
                return Err(
                    "unsupported routing write changed the observed routing intent".to_owned(),
                );
            }
            Ok(())
        }
    }
}

fn selection_from_inventory(
    state: &OutboundInventoryState,
    collection_id: &OutboundId,
    expected_member: &OutboundId,
) -> Result<OutboundId, String> {
    let OutboundInventoryState::Ready { collections, .. } = state else {
        return Err(format!("outbound inventory was not ready: {state:?}"));
    };
    let collection = collections
        .iter()
        .find(|collection| &collection.id == collection_id)
        .ok_or_else(|| format!("selector {} was absent", collection_id.value()))?;
    if !collection.member_ids.contains(expected_member) {
        return Err(format!(
            "selector {} does not contain expected member {}",
            collection_id.value(),
            expected_member.value()
        ));
    }
    match &collection.selection {
        OutboundSelectionState::Ready { selected } => Ok(selected.clone()),
        other => Err(format!(
            "selector {} did not expose a ready selection: {other:?}",
            collection_id.value()
        )),
    }
}

fn is_typed_unsupported(error: &PortError) -> bool {
    match error {
        PortError::Unsupported { .. } => true,
        PortError::Rejected(failure) => failure.code == ErrorCode::Unsupported,
        PortError::PermissionDenied(_)
        | PortError::NotFound(_)
        | PortError::Io(_)
        | PortError::Network(_)
        | PortError::Failed(_) => false,
    }
}

pub(super) async fn verify_stopped(process: &dyn CoreProcess) -> Result<(), String> {
    match process.status().await {
        Ok(CoreLifecycle::Stopped) => Ok(()),
        Ok(status) => Err(format!("process cleanup left lifecycle at {status:?}")),
        Err(error) => Err(format!("process state read after cleanup failed: {error}")),
    }
}

pub(super) fn combine_run_and_cleanup(
    run_result: Result<(), String>,
    stop_result: Result<(), PortError>,
    stopped_result: Result<(), String>,
) -> Result<(), String> {
    match (run_result, stop_result, stopped_result) {
        (Ok(()), Ok(()), Ok(())) => Ok(()),
        (Err(run_error), Ok(()), Ok(())) => Err(run_error),
        (run_result, stop_result, stopped_result) => Err(format!(
            "conformance failure: {}; stop result: {}; stopped verification: {}",
            run_result.err().unwrap_or_else(|| "passed".to_owned()),
            stop_result
                .err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "passed".to_owned()),
            stopped_result.err().unwrap_or_else(|| "passed".to_owned())
        )),
    }
}
