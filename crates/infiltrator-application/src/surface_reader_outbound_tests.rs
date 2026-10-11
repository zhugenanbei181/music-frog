use super::*;
use crate::core_application::test_execute_result_at_current_scope;

struct SurfaceOutboundPort {
    calls: AtomicUsize,
}

#[derive(Clone, Copy)]
enum SurfaceOutboundCapabilitySet {
    NoList,
    ListOnly,
    ListAndSelect,
}

struct SurfaceOutboundCapabilityProvider {
    support: SurfaceOutboundCapabilitySet,
}

impl CoreCapabilityProvider for SurfaceOutboundCapabilityProvider {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        let scope = CoreCapabilityScope::new(
            engine_id.clone(),
            version.cloned(),
            HostKind::Desktop,
            AdapterRevision::new(1),
        );
        let supported = |operation| {
            CoreCapabilityEntry::new(
                operation,
                EngineCapabilityDecision::Supported,
                HostCapabilityDecision::Supported,
                AdapterCapabilityDecision::Supported,
            )
        };
        let unsupported = |operation, reason: &str| {
            CoreCapabilityEntry::new(
                operation,
                EngineCapabilityDecision::Unsupported {
                    reason: reason.to_owned(),
                },
                HostCapabilityDecision::Supported,
                AdapterCapabilityDecision::Supported,
            )
        };
        let mut entries = vec![
            supported(CoreOperation::Lifecycle(CoreLifecycleOperation::Start)),
            supported(CoreOperation::Lifecycle(CoreLifecycleOperation::Stop)),
            supported(CoreOperation::Lifecycle(CoreLifecycleOperation::Restart)),
        ];
        match self.support {
            SurfaceOutboundCapabilitySet::NoList => entries.push(unsupported(
                CoreOperation::Outbound(OutboundOperation::List),
                "outbound list is not registered for this test engine version",
            )),
            SurfaceOutboundCapabilitySet::ListOnly
            | SurfaceOutboundCapabilitySet::ListAndSelect => {
                entries.push(supported(CoreOperation::Outbound(OutboundOperation::List)))
            }
        }
        match self.support {
            SurfaceOutboundCapabilitySet::ListAndSelect => entries.push(supported(
                CoreOperation::Outbound(OutboundOperation::Select),
            )),
            SurfaceOutboundCapabilitySet::NoList | SurfaceOutboundCapabilitySet::ListOnly => {
                entries.push(unsupported(
                    CoreOperation::Outbound(OutboundOperation::Select),
                    "outbound selection is not registered for this test engine version",
                ))
            }
        }
        CoreCapabilityMatrix::new(scope, entries).map_err(|error| {
            CoreCapabilityProviderError::MatrixUnavailable {
                reason: error.to_string(),
            }
        })
    }
}

fn install_surface_outbound_capabilities(
    core: &CoreApplication,
    support: SurfaceOutboundCapabilitySet,
) {
    core.install_core_capability_application(CoreCapabilityApplication::new(Arc::new(
        SurfaceOutboundCapabilityProvider { support },
    )))
    .expect("surface outbound capabilities install before use");
}

#[async_trait]
impl OutboundPort for SurfaceOutboundPort {
    async fn inventory(&self, scope: CoreScope) -> Result<OutboundInventorySnapshot, PortError> {
        let revision = self.calls.fetch_add(1, Ordering::SeqCst) as u64 + 1;
        let engine_id = scope.identity.engine.clone();
        let id = OutboundId::new(engine_id.clone(), "proxy-a")
            .map_err(|error| PortError::Failed(error.to_string()))?;
        OutboundInventorySnapshot::new(
            scope,
            SnapshotRevision::new(revision),
            OutboundInventoryState::Ready {
                entries: vec![OutboundSnapshot {
                    id: id.clone(),
                    display_name: Some("Proxy A".to_owned()),
                }],
                collections: vec![OutboundSelectionSet {
                    id: OutboundId::new(engine_id, "selector")
                        .map_err(|error| PortError::Failed(error.to_string()))?,
                    member_ids: vec![id.clone()],
                    availability: SelectionAvailability::Selectable,
                    selection: OutboundSelectionState::Ready {
                        selected: id.clone(),
                    },
                }],
            },
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn details(
        &self,
        scope: CoreScope,
        _expected_revision: SnapshotRevision,
    ) -> Result<OutboundDetailsSnapshot, PortError> {
        let id = OutboundId::new(scope.identity.engine.clone(), "proxy-a")
            .map_err(|error| PortError::Failed(error.to_string()))?;
        OutboundDetailsSnapshot::new(
            scope,
            SnapshotRevision::new(1),
            OutboundDetailsState::Ready {
                details: vec![OutboundDetail {
                    id,
                    protocol: OutboundProtocolObservation::Unknown,
                    protocol_display_name: None,
                    cipher: OutboundObservation::Unsupported,
                    cipher_display_name: None,
                    collection_kind: OutboundObservation::Unsupported,
                    features: OutboundObservation::Unknown,
                    endpoint: OutboundObservation::Unknown,
                    udp: OutboundUdpSupport::Unknown,
                    health: OutboundHealthObservation::Unknown,
                    current_latency_ms: OutboundObservation::Unknown,
                    latency_history: OutboundObservation::Unknown,
                    rtt: OutboundObservation::Unknown,
                    historical_egress: OutboundObservation::Unsupported,
                    probe_availability: OutboundProbeAvailability::Supported,
                }],
            },
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn select(
        &self,
        _scope: CoreScope,
        _expected_revision: SnapshotRevision,
        _collection_id: OutboundId,
        _outbound_id: OutboundId,
    ) -> Result<OutboundInventorySnapshot, PortError> {
        Err(PortError::Failed(
            "selection is unused in this test".to_owned(),
        ))
    }

    async fn probe(
        &self,
        _scope: CoreScope,
        _expected_revision: SnapshotRevision,
        _outbound_id: OutboundId,
        _request: OutboundProbeRequest,
    ) -> Result<OutboundProbeSnapshot, PortError> {
        Err(PortError::Failed(
            "probing is unused in this test".to_owned(),
        ))
    }
}

#[tokio::test]
async fn surface_reader_publishes_the_typed_outbound_inventory_for_its_core_scope() {
    let core = Arc::new(CoreApplication::new(
        Arc::new(ScopeProcess {
            running: AtomicBool::new(false),
        }),
        Arc::new(OutboundReadiness),
        Arc::new(TestRuntime),
    ));
    install_surface_outbound_capabilities(&core, SurfaceOutboundCapabilitySet::ListAndSelect);
    assert!(matches!(
        test_execute_result_at_current_scope(&core, CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));
    let port = Arc::new(SurfaceOutboundPort {
        calls: AtomicUsize::new(0),
    });
    let scope = core.snapshot().scope().expect("active scope");
    let reader =
        ApplicationSurfaceReader::new(core.clone(), SurfaceKind::BevyDesktop, HostKind::Desktop)
            .with_outbound(OutboundApplication::new(port.clone()));

    let snapshot = reader.read().await.expect("surface read");

    assert_eq!(port.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        snapshot.outbound_inventory.status,
        surface_snapshot::PageStatus::Ready
    );
    let inventory = snapshot
        .outbound_inventory
        .data
        .expect("typed outbound inventory is present");
    assert_eq!(inventory.scope(), &core.snapshot().scope().unwrap());
    assert!(matches!(
        inventory.inventory(),
        OutboundInventoryState::Ready { entries, .. }
            if entries[0].display_name.as_deref() == Some("Proxy A")
    ));
    assert!(matches!(
        inventory.inventory(),
        OutboundInventoryState::Ready { collections, .. }
            if collections[0].availability == SelectionAvailability::Selectable
    ));
    let details = snapshot
        .outbound_details
        .data
        .expect("details from the same read are present");
    assert_eq!(details.scope(), &scope);
    assert_eq!(details.detail_revision(), SnapshotRevision::new(1));
    let preferences = snapshot
        .outbound_ui_preferences
        .data
        .expect("typed preference owner readback is present");
    assert_eq!(preferences.scope(), &scope);
    assert_eq!(preferences.revision(), SnapshotRevision::new(1));
}

#[tokio::test]
async fn surface_reader_denies_unregistered_outbound_list_before_adapter_io() {
    let core = Arc::new(CoreApplication::new(
        Arc::new(ScopeProcess {
            running: AtomicBool::new(false),
        }),
        Arc::new(OutboundReadiness),
        Arc::new(TestRuntime),
    ));
    install_surface_outbound_capabilities(&core, SurfaceOutboundCapabilitySet::NoList);
    assert!(matches!(
        test_execute_result_at_current_scope(&core, CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));
    let port = Arc::new(SurfaceOutboundPort {
        calls: AtomicUsize::new(0),
    });
    let reader = ApplicationSurfaceReader::new(core, SurfaceKind::BevyDesktop, HostKind::Desktop)
        .with_outbound(OutboundApplication::new(port.clone()));

    let snapshot = reader.read().await.expect("surface read");

    assert_eq!(port.calls.load(Ordering::SeqCst), 0);
    let surface_snapshot::PageStatus::Failed { failure } = snapshot.outbound_inventory.status
    else {
        panic!("unsupported outbound list must be a failed page");
    };
    assert_eq!(failure.code, ErrorCode::Unsupported);
}

#[tokio::test]
async fn surface_reader_disables_selection_when_select_capability_is_not_registered() {
    let core = Arc::new(CoreApplication::new(
        Arc::new(ScopeProcess {
            running: AtomicBool::new(false),
        }),
        Arc::new(OutboundReadiness),
        Arc::new(TestRuntime),
    ));
    install_surface_outbound_capabilities(&core, SurfaceOutboundCapabilitySet::ListOnly);
    assert!(matches!(
        test_execute_result_at_current_scope(&core, CommandIntent::StartCore).await,
        CommandResult::Completed { .. }
    ));
    let port = Arc::new(SurfaceOutboundPort {
        calls: AtomicUsize::new(0),
    });
    let reader = ApplicationSurfaceReader::new(core, SurfaceKind::BevyDesktop, HostKind::Desktop)
        .with_outbound(OutboundApplication::new(port.clone()));

    let snapshot = reader.read().await.expect("surface read");

    assert_eq!(port.calls.load(Ordering::SeqCst), 1);
    let inventory = snapshot
        .outbound_inventory
        .data
        .expect("list capability allows observation");
    assert!(matches!(
        inventory.inventory(),
        OutboundInventoryState::Ready { collections, .. }
            if collections[0].availability == SelectionAvailability::Unsupported
    ));
}
