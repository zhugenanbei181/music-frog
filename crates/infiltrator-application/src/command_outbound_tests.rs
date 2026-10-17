use crate::command_application::CommandApplication;
use crate::core_capability_test_support::{
    core_scope, execute_core_output_command, with_test_capabilities,
};
use crate::outbound_application::OutboundApplication;
use crate::outbound_application::preferences::OutboundUiPreferencesApplication;
use async_trait::async_trait;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::{CoreScope, EngineId};
use lurker_core_contract::outbound::{
    details::{
        OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState, OutboundHealthObservation,
        OutboundObservation, OutboundProbeAvailability, OutboundProtocolObservation,
        OutboundSnapshot, OutboundUdpSupport,
    },
    identity::{OutboundId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundProbeSnapshot,
        OutboundProbeState, OutboundSelectionSet, OutboundSelectionState, SelectionAvailability,
    },
    preferences::{
        OutboundFavoriteState, OutboundGroupExpansion, OutboundHealthFilter, OutboundLayout,
        OutboundSortOrder, OutboundUiPreferenceIntent, OutboundUiPreferenceRequest,
    },
    probe::{
        OutboundProbeBatchOutcome, OutboundProbeBatchScope, OutboundProbeConcurrency,
        OutboundSelectionRequest, ProbeOutboundBatchRequest, ProbeOutboundRequest,
    },
};
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::{OutboundPort, OutboundProbeRequest};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Calls {
    inventory: usize,
    selection: usize,
    probes: usize,
}

struct TestOutboundPort {
    current: Mutex<OutboundInventorySnapshot>,
    readback: Option<OutboundInventorySnapshot>,
    probe_result: Option<OutboundProbeSnapshot>,
    probe_availability: OutboundProbeAvailability,
    calls: Mutex<Calls>,
}

impl TestOutboundPort {
    fn new(
        current: OutboundInventorySnapshot,
        readback: Option<OutboundInventorySnapshot>,
    ) -> Self {
        Self {
            current: Mutex::new(current),
            readback,
            probe_result: None,
            probe_availability: OutboundProbeAvailability::Supported,
            calls: Mutex::new(Calls::default()),
        }
    }

    fn with_probe(mut self, result: OutboundProbeSnapshot) -> Self {
        self.probe_result = Some(result);
        self
    }

    fn with_probe_availability(mut self, availability: OutboundProbeAvailability) -> Self {
        self.probe_availability = availability;
        self
    }

    fn call_counts(&self) -> (usize, usize) {
        let calls = self.calls.lock().expect("outbound calls lock");
        (calls.inventory, calls.selection)
    }
}

#[async_trait]
impl OutboundPort for TestOutboundPort {
    async fn inventory(&self, _scope: CoreScope) -> Result<OutboundInventorySnapshot, PortError> {
        self.calls.lock().expect("outbound calls lock").inventory += 1;
        Ok(self.current.lock().expect("current inventory lock").clone())
    }

    async fn details(
        &self,
        scope: CoreScope,
        _expected_revision: SnapshotRevision,
    ) -> Result<OutboundDetailsSnapshot, PortError> {
        let current = self.current.lock().expect("current inventory").clone();
        let OutboundInventoryState::Ready { entries, .. } = current.inventory() else {
            return Err(PortError::Rejected(Failure::unsupported(
                "test inventory has no details",
            )));
        };
        let details = entries
            .iter()
            .map(|entry| OutboundDetail {
                id: entry.id.clone(),
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
                probe_availability: self.probe_availability,
            })
            .collect();
        OutboundDetailsSnapshot::new(
            scope,
            SnapshotRevision::new(1),
            OutboundDetailsState::Ready { details },
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
        self.calls.lock().expect("outbound calls lock").selection += 1;
        let readback = self
            .readback
            .clone()
            .ok_or_else(|| PortError::Failed("selection readback unavailable".to_owned()))?;
        *self.current.lock().expect("current inventory lock") = readback.clone();
        Ok(readback)
    }

    async fn probe(
        &self,
        _scope: CoreScope,
        _expected_revision: SnapshotRevision,
        _outbound_id: OutboundId,
        _request: OutboundProbeRequest,
    ) -> Result<OutboundProbeSnapshot, PortError> {
        self.calls.lock().expect("outbound calls lock").probes += 1;
        self.probe_result
            .clone()
            .ok_or_else(|| PortError::Failed("probe result missing".to_owned()))
    }
}

fn id(value: &str) -> OutboundId {
    OutboundId::new(EngineId::new("mihomo").expect("test engine"), value)
        .expect("valid outbound ID")
}

fn inventory(
    scope: CoreScope,
    revision: u64,
    selected: &str,
    availability: SelectionAvailability,
) -> OutboundInventorySnapshot {
    OutboundInventorySnapshot::new(
        scope,
        SnapshotRevision::new(revision),
        OutboundInventoryState::Ready {
            entries: vec![
                OutboundSnapshot {
                    id: id("proxy-a"),
                    display_name: Some("Proxy A".to_owned()),
                },
                OutboundSnapshot {
                    id: id("proxy-b"),
                    display_name: Some("Proxy B".to_owned()),
                },
            ],
            collections: vec![OutboundSelectionSet {
                id: id("selector/main"),
                member_ids: vec![id("proxy-a"), id("proxy-b")],
                availability,
                selection: OutboundSelectionState::Ready {
                    selected: id(selected),
                },
            }],
        },
    )
    .expect("valid outbound inventory")
}

fn request(revision: u64) -> OutboundSelectionRequest {
    OutboundSelectionRequest::new(
        id("selector/main"),
        id("proxy-b"),
        SnapshotRevision::new(revision),
    )
    .expect("valid selection request")
}

#[tokio::test]
async fn inventory_command_returns_the_adapter_authored_typed_observation() {
    let scope = core_scope();
    let port = Arc::new(TestOutboundPort::new(
        inventory(scope, 12, "proxy-a", SelectionAvailability::Selectable),
        None,
    ));
    let application = with_test_capabilities(
        CommandApplication::new().with_outbound(OutboundApplication::new(port.clone())),
    );

    let output = execute_core_output_command(&application, CommandIntent::ReadOutboundInventory)
        .await
        .expect("inventory command succeeds");

    assert!(matches!(
        output,
        CommandOutput::OutboundInventory(snapshot)
            if snapshot.revision() == SnapshotRevision::new(12)
    ));
    assert_eq!(port.call_counts(), (1, 0));
}

#[tokio::test]
async fn select_command_requires_typed_revision_and_returns_confirmed_readback() {
    let scope = core_scope();
    let port = Arc::new(TestOutboundPort::new(
        inventory(
            scope.clone(),
            12,
            "proxy-a",
            SelectionAvailability::Selectable,
        ),
        Some(inventory(
            scope,
            13,
            "proxy-b",
            SelectionAvailability::Selectable,
        )),
    ));
    let application = with_test_capabilities(
        CommandApplication::new().with_outbound(OutboundApplication::new(port.clone())),
    );
    let intent = CommandIntent::SelectOutbound {
        request: request(12),
    };

    let output = execute_core_output_command(&application, intent.clone())
        .await
        .expect("selection succeeds after readback");

    assert!(output.validate_for(&intent).is_ok());
    assert!(matches!(
        output,
        CommandOutput::OutboundInventory(snapshot)
            if snapshot.revision() == SnapshotRevision::new(13)
    ));
    assert_eq!(port.call_counts(), (1, 1));
}

#[tokio::test]
async fn stale_revision_and_unsupported_collection_fail_before_selection_port() {
    let scope = core_scope();
    let stale_port = Arc::new(TestOutboundPort::new(
        inventory(
            scope.clone(),
            12,
            "proxy-a",
            SelectionAvailability::Selectable,
        ),
        None,
    ));
    let stale_application = with_test_capabilities(
        CommandApplication::new().with_outbound(OutboundApplication::new(stale_port.clone())),
    );
    let stale = execute_core_output_command(
        &stale_application,
        CommandIntent::SelectOutbound {
            request: request(11),
        },
    )
    .await
    .expect_err("old observation is rejected");
    assert_eq!(stale.code, ErrorCode::InvalidState);
    assert_eq!(stale_port.call_counts(), (1, 0));

    let unsupported_port = Arc::new(TestOutboundPort::new(
        inventory(scope, 12, "proxy-a", SelectionAvailability::Automatic),
        None,
    ));
    let unsupported_application = with_test_capabilities(
        CommandApplication::new().with_outbound(OutboundApplication::new(unsupported_port.clone())),
    );
    let unsupported = execute_core_output_command(
        &unsupported_application,
        CommandIntent::SelectOutbound {
            request: request(12),
        },
    )
    .await
    .expect_err("automatic collection cannot be manually selected");
    assert_eq!(unsupported.code, ErrorCode::InvalidState);
    assert_eq!(unsupported_port.call_counts(), (1, 0));
}

#[tokio::test]
async fn missing_adapter_and_missing_capability_gate_fail_closed_without_reads() {
    let missing_adapter = with_test_capabilities(CommandApplication::new());
    let failure =
        execute_core_output_command(&missing_adapter, CommandIntent::ReadOutboundInventory)
            .await
            .expect_err("missing adapter remains explicit");
    assert_eq!(failure.code, ErrorCode::Unsupported);

    let port = Arc::new(TestOutboundPort::new(
        inventory(
            core_scope(),
            12,
            "proxy-a",
            SelectionAvailability::Selectable,
        ),
        None,
    ));
    let missing_gate =
        CommandApplication::new().with_outbound(OutboundApplication::new(port.clone()));
    let failure = execute_core_output_command(&missing_gate, CommandIntent::ReadOutboundInventory)
        .await
        .expect_err("missing capability provider denies before the port");
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(port.call_counts(), (0, 0));
}

#[tokio::test]
async fn selection_request_for_another_engine_is_rejected_before_inventory_read() {
    let port = Arc::new(TestOutboundPort::new(
        inventory(
            core_scope(),
            12,
            "proxy-a",
            SelectionAvailability::Selectable,
        ),
        None,
    ));
    let application = with_test_capabilities(
        CommandApplication::new().with_outbound(OutboundApplication::new(port.clone())),
    );
    let request = OutboundSelectionRequest::new(
        OutboundId::new(
            EngineId::new("sing-box").expect("test engine"),
            "selector/main",
        )
        .expect("valid collection ID"),
        OutboundId::new(EngineId::new("sing-box").expect("test engine"), "proxy-b")
            .expect("valid outbound ID"),
        SnapshotRevision::new(12),
    )
    .expect("same-engine request");

    let failure =
        execute_core_output_command(&application, CommandIntent::SelectOutbound { request })
            .await
            .expect_err("scope engine mismatch is rejected");

    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert_eq!(port.call_counts(), (0, 0));
}

fn probe_request(revision: u64, outbound: &str) -> ProbeOutboundRequest {
    ProbeOutboundRequest::new(
        id(outbound),
        SnapshotRevision::new(revision),
        "https://health.example.test/check",
        5_000,
    )
    .expect("valid typed probe request")
}

#[tokio::test]
async fn probe_outbound_command_returns_inventory_and_detail_readback() {
    let scope = core_scope();
    let probe = OutboundProbeSnapshot::new(
        scope.clone(),
        SnapshotRevision::new(12),
        SnapshotRevision::new(2),
        id("proxy-b"),
        OutboundProbeState::Measured { delay_ms: 0 },
    )
    .expect("typed probe result");
    let port = Arc::new(
        TestOutboundPort::new(
            inventory(
                scope.clone(),
                12,
                "proxy-a",
                SelectionAvailability::Selectable,
            ),
            None,
        )
        .with_probe(probe),
    );
    let application = with_test_capabilities(
        CommandApplication::new().with_outbound(OutboundApplication::new(port.clone())),
    );
    let intent = CommandIntent::ProbeOutbound {
        request: probe_request(12, "proxy-b"),
    };

    let output = execute_core_output_command(&application, intent.clone())
        .await
        .expect("probe command returns a typed observation");

    assert!(output.validate_for(&intent).is_ok());
    assert!(matches!(
        output,
        CommandOutput::OutboundProbe(snapshot)
            if snapshot.observed_inventory_revision() == SnapshotRevision::new(12)
                && snapshot.detail_revision() == SnapshotRevision::new(2)
                && snapshot.state() == &OutboundProbeState::Measured { delay_ms: 0 }
    ));
    assert_eq!(port.calls.lock().expect("calls lock").probes, 1);
}

#[tokio::test]
async fn typed_batch_probe_command_returns_a_correlated_result_snapshot() {
    let scope = core_scope();
    let probe = OutboundProbeSnapshot::new(
        scope.clone(),
        SnapshotRevision::new(12),
        SnapshotRevision::new(2),
        id("proxy-b"),
        OutboundProbeState::Measured { delay_ms: 17 },
    )
    .expect("typed probe result");
    let port = Arc::new(
        TestOutboundPort::new(
            inventory(
                scope.clone(),
                12,
                "proxy-a",
                SelectionAvailability::Selectable,
            ),
            None,
        )
        .with_probe(probe),
    );
    let application = with_test_capabilities(
        CommandApplication::new().with_outbound(OutboundApplication::new(port.clone())),
    );
    let request = ProbeOutboundBatchRequest::new(
        OutboundProbeBatchScope::One(id("proxy-b")),
        SnapshotRevision::new(12),
        "https://health.example.test/check",
        5_000,
        OutboundProbeConcurrency::new(1).expect("bounded concurrency"),
    )
    .expect("valid batch request");
    let intent = CommandIntent::ProbeOutboundBatch {
        request: request.clone(),
    };

    let output = execute_core_output_command(&application, intent.clone())
        .await
        .expect("batch command returns a typed result snapshot");

    assert!(output.validate_for(&intent).is_ok());
    assert!(matches!(
        output,
        CommandOutput::OutboundProbeBatch(snapshot)
            if snapshot.request() == &request
                && snapshot.items().len() == 1
                && snapshot.items()[0].outbound_id == id("proxy-b")
                && snapshot.items()[0].outcome == OutboundProbeBatchOutcome::Measured { delay_ms: 17 }
    ));
    assert_eq!(port.calls.lock().expect("calls lock").probes, 1);
}

#[tokio::test]
async fn stale_nonmember_and_unsupported_probe_requests_have_no_probe_side_effect() {
    let active = core_scope();
    let stale_port = Arc::new(TestOutboundPort::new(
        inventory(
            active.clone(),
            12,
            "proxy-a",
            SelectionAvailability::Selectable,
        ),
        None,
    ));
    let application = with_test_capabilities(
        CommandApplication::new().with_outbound(OutboundApplication::new(stale_port.clone())),
    );
    let stale = execute_core_output_command(
        &application,
        CommandIntent::ProbeOutbound {
            request: probe_request(11, "proxy-b"),
        },
    )
    .await
    .expect_err("stale inventory is rejected before probe");
    assert_eq!(stale.code, ErrorCode::InvalidState);
    assert_eq!(stale_port.calls.lock().expect("calls lock").probes, 0);

    let nonmember_port = Arc::new(TestOutboundPort::new(
        inventory(
            active.clone(),
            12,
            "proxy-a",
            SelectionAvailability::Selectable,
        ),
        None,
    ));
    let application = with_test_capabilities(
        CommandApplication::new().with_outbound(OutboundApplication::new(nonmember_port.clone())),
    );
    let nonmember = execute_core_output_command(
        &application,
        CommandIntent::ProbeOutbound {
            request: probe_request(12, "proxy-c"),
        },
    )
    .await
    .expect_err("nonmember is rejected before probe");
    assert_eq!(nonmember.code, ErrorCode::InvalidInput);
    assert_eq!(nonmember_port.calls.lock().expect("calls lock").probes, 0);

    let unsupported_port = Arc::new(
        TestOutboundPort::new(
            inventory(active, 12, "proxy-a", SelectionAvailability::Selectable),
            None,
        )
        .with_probe_availability(OutboundProbeAvailability::Unsupported),
    );
    let application = with_test_capabilities(
        CommandApplication::new().with_outbound(OutboundApplication::new(unsupported_port.clone())),
    );
    let unsupported = execute_core_output_command(
        &application,
        CommandIntent::ProbeOutbound {
            request: probe_request(12, "proxy-b"),
        },
    )
    .await
    .expect_err("unsupported probe is rejected before port side effect");
    assert_eq!(unsupported.code, ErrorCode::Unsupported);
    assert_eq!(unsupported_port.calls.lock().expect("calls lock").probes, 0);
}

#[tokio::test]
async fn outbound_preference_commands_apply_typed_values_with_scope_and_revision_readback() {
    let scope = core_scope();
    let preferences = OutboundUiPreferencesApplication::new(scope.clone());
    let port = Arc::new(TestOutboundPort::new(
        inventory(
            scope.clone(),
            12,
            "proxy-a",
            SelectionAvailability::Selectable,
        ),
        None,
    ));
    let application = with_test_capabilities(
        CommandApplication::new()
            .with_outbound(OutboundApplication::new(port.clone()))
            .with_outbound_ui_preferences(preferences.clone()),
    );
    let intent = CommandIntent::ApplyOutboundUiPreference {
        request: OutboundUiPreferenceRequest::new(
            SnapshotRevision::new(1),
            OutboundUiPreferenceIntent::SetFavorite {
                outbound_id: id("proxy-a"),
                state: OutboundFavoriteState::Pinned,
            },
        )
        .expect("typed preference request"),
    };

    let output = execute_core_output_command(&application, intent.clone())
        .await
        .expect("preference applies and returns readback");
    assert!(output.validate_for(&intent).is_ok());
    let CommandOutput::OutboundUiPreferencesApplied(readback) = output else {
        panic!("preference command returns its typed snapshot");
    };
    assert_eq!(readback.revision(), SnapshotRevision::new(2));
    assert!(readback.favorite_outbound_ids.contains(&id("proxy-a")));

    for preference in [
        OutboundUiPreferenceIntent::SetSearchQuery("proxy".to_owned()),
        OutboundUiPreferenceIntent::SetHealthFilter(OutboundHealthFilter::HealthyOnly),
        OutboundUiPreferenceIntent::SetLayout(OutboundLayout::Grid),
        OutboundUiPreferenceIntent::SetSortOrder(OutboundSortOrder::NameAscending),
    ] {
        let current = preferences.snapshot(&scope).expect("current preferences");
        let intent = CommandIntent::ApplyOutboundUiPreference {
            request: OutboundUiPreferenceRequest::new(current.revision(), preference)
                .expect("typed preference request"),
        };
        let output = execute_core_output_command(&application, intent.clone())
            .await
            .expect("typed preference update succeeds");
        assert!(output.validate_for(&intent).is_ok());
    }
    let readback = preferences.snapshot(&scope).expect("updated preferences");
    assert_eq!(readback.revision(), SnapshotRevision::new(6));
    assert_eq!(readback.search_query, "proxy");
    assert_eq!(readback.health_filter, OutboundHealthFilter::HealthyOnly);
    assert_eq!(readback.layout, OutboundLayout::Grid);
    assert_eq!(readback.sort_order, OutboundSortOrder::NameAscending);

    let stale = execute_core_output_command(&application, intent)
        .await
        .expect_err("preference revision is fenced");
    assert_eq!(stale.code, ErrorCode::InvalidState);
    assert_eq!(
        preferences.snapshot(&scope).expect("readback").revision(),
        SnapshotRevision::new(6)
    );

    let wrong_engine = OutboundUiPreferenceRequest::new(
        SnapshotRevision::new(6),
        OutboundUiPreferenceIntent::SetGroupExpansion {
            group_id: OutboundId::new(EngineId::new("sing-box").expect("engine"), "group-a")
                .expect("group ID"),
            expansion: OutboundGroupExpansion::Expanded,
        },
    )
    .expect("well-formed request");
    let mismatch = preferences
        .apply(
            &scope,
            &wrong_engine,
            &OutboundApplication::new(port)
                .observation(scope.clone())
                .await
                .expect("current inventory and details"),
        )
        .expect_err("preference IDs are fenced to the full owner scope");
    assert_eq!(mismatch.code, ErrorCode::InvalidState);
}

#[tokio::test]
async fn outbound_preference_targets_must_be_current_members_of_the_right_kind() {
    let scope = core_scope();
    let preferences = OutboundUiPreferencesApplication::new(scope.clone());
    let port = Arc::new(TestOutboundPort::new(
        inventory(
            scope.clone(),
            12,
            "proxy-a",
            SelectionAvailability::Selectable,
        ),
        None,
    ));
    let application = with_test_capabilities(
        CommandApplication::new()
            .with_outbound(OutboundApplication::new(port))
            .with_outbound_ui_preferences(preferences.clone()),
    );
    let invalid_targets = [
        OutboundUiPreferenceIntent::SetFavorite {
            outbound_id: id("proxy-c"),
            state: OutboundFavoriteState::Pinned,
        },
        OutboundUiPreferenceIntent::SetGroupExpansion {
            group_id: id("proxy-a"),
            expansion: OutboundGroupExpansion::Collapsed,
        },
        OutboundUiPreferenceIntent::ReorderGroups {
            group_ids: vec![id("proxy-c")],
            inventory_revision: SnapshotRevision::new(12),
        },
    ];

    for preference in invalid_targets {
        let request = OutboundUiPreferenceRequest::new(SnapshotRevision::new(1), preference)
            .expect("well-formed but unobserved target");
        let failure = execute_core_output_command(
            &application,
            CommandIntent::ApplyOutboundUiPreference { request },
        )
        .await
        .expect_err("unknown target is rejected before preference mutation");
        assert_eq!(failure.code, ErrorCode::InvalidInput);
    }
    assert_eq!(
        preferences
            .snapshot(&scope)
            .expect("unchanged state")
            .revision(),
        SnapshotRevision::new(1)
    );

    let stale_inventory = OutboundUiPreferenceRequest::new(
        SnapshotRevision::new(1),
        OutboundUiPreferenceIntent::ResetGroupOrder {
            inventory_revision: SnapshotRevision::new(11),
        },
    )
    .expect("well-formed stale inventory fence");
    let failure = execute_core_output_command(
        &application,
        CommandIntent::ApplyOutboundUiPreference {
            request: stale_inventory,
        },
    )
    .await
    .expect_err("stale inventory revision is rejected before preference mutation");
    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert_eq!(
        preferences
            .snapshot(&scope)
            .expect("unchanged state")
            .revision(),
        SnapshotRevision::new(1)
    );
}

#[tokio::test]
async fn group_order_preference_uses_outbound_ids_inventory_revision_and_typed_readback() {
    let scope = core_scope();
    let preferences = OutboundUiPreferencesApplication::new(scope.clone());
    let port = Arc::new(TestOutboundPort::new(
        inventory(
            scope.clone(),
            12,
            "proxy-a",
            SelectionAvailability::Selectable,
        ),
        None,
    ));
    let application = with_test_capabilities(
        CommandApplication::new()
            .with_outbound(OutboundApplication::new(port.clone()))
            .with_outbound_ui_preferences(preferences.clone()),
    );
    let request = OutboundUiPreferenceRequest::new(
        SnapshotRevision::new(1),
        OutboundUiPreferenceIntent::ReorderGroups {
            group_ids: vec![id("selector/main")],
            inventory_revision: SnapshotRevision::new(12),
        },
    )
    .expect("group order carries typed identity and inventory fence");
    let intent = CommandIntent::ApplyOutboundUiPreference {
        request: request.clone(),
    };
    let output = execute_core_output_command(&application, intent.clone())
        .await
        .expect("group order applies through the shared preference route");
    assert!(output.validate_for(&intent).is_ok());
    let CommandOutput::OutboundUiPreferencesApplied(readback) = output else {
        panic!("group order returns typed preference readback");
    };
    assert_eq!(readback.revision(), SnapshotRevision::new(2));
    assert_eq!(readback.group_order, vec![id("selector/main")]);

    let stale_inventory = OutboundUiPreferenceRequest::new(
        SnapshotRevision::new(2),
        OutboundUiPreferenceIntent::ResetGroupOrder {
            inventory_revision: SnapshotRevision::new(11),
        },
    )
    .expect("stale revision is structurally valid");
    let failure = execute_core_output_command(
        &application,
        CommandIntent::ApplyOutboundUiPreference {
            request: stale_inventory,
        },
    )
    .await
    .expect_err("old inventory revision is refused");
    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert_eq!(preferences.snapshot(&scope).unwrap(), readback);
    assert_eq!(port.call_counts().0, 2);
}
