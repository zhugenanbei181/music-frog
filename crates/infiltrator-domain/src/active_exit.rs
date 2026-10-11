//! Pure derivation of the selected exit from the engine-neutral inventory.

use crate::filter::extract_country_code;
use infiltrator_contract::active_exit::{ActiveExitSnapshot, ActiveExitStatus};
use infiltrator_contract::outbound::details::{
    OutboundHealthObservation, OutboundObservation, OutboundProtocolObservation,
};
use infiltrator_contract::outbound::inventory::{
    OutboundInventoryState, OutboundObservationSnapshot, OutboundSelectionState,
};

/// Derive an active exit only when the accepted inventory reports exactly one
/// selected collection. Ambiguous selections stay Unknown rather than being
/// guessed from display names or map ordering.
pub fn derive(
    generation: u64,
    revision: u64,
    observation: &OutboundObservationSnapshot,
) -> ActiveExitSnapshot {
    let (entries, collections) = match observation.inventory().inventory() {
        OutboundInventoryState::Ready {
            entries,
            collections,
        } => (entries, collections),
        OutboundInventoryState::Empty => {
            return ActiveExitSnapshot {
                generation,
                revision,
                status: ActiveExitStatus::Empty,
                ..Default::default()
            };
        }
        OutboundInventoryState::Unsupported => {
            return ActiveExitSnapshot::unsupported(
                generation,
                revision,
                "outbound inventory is unsupported",
            );
        }
        OutboundInventoryState::Unknown => {
            return ActiveExitSnapshot::unavailable(
                generation,
                revision,
                "outbound inventory is unknown",
            );
        }
        OutboundInventoryState::Failed { failure } => {
            return ActiveExitSnapshot::failed(generation, revision, failure.message.clone());
        }
    };

    let mut selected = collections.iter().filter_map(|collection| {
        let OutboundSelectionState::Ready { selected } = &collection.selection else {
            return None;
        };
        Some((&collection.id, selected))
    });
    let Some((group_id, outbound_id)) = selected.next() else {
        let unresolved = collections.iter().any(|collection| {
            matches!(
                &collection.selection,
                OutboundSelectionState::Unknown
                    | OutboundSelectionState::Unsupported
                    | OutboundSelectionState::Failed { .. }
            )
        });
        return if unresolved {
            ActiveExitSnapshot::unavailable(
                generation,
                revision,
                "no selected outbound has been reported",
            )
        } else {
            ActiveExitSnapshot {
                generation,
                revision,
                status: ActiveExitStatus::Empty,
                ..Default::default()
            }
        };
    };
    if selected.next().is_some() {
        return ActiveExitSnapshot::unavailable(
            generation,
            revision,
            "multiple outbound collections report a selected member",
        );
    }

    let Some(entry) = entries.iter().find(|entry| &entry.id == outbound_id) else {
        return ActiveExitSnapshot::unavailable(
            generation,
            revision,
            "selected outbound is absent from the accepted inventory",
        );
    };
    let display_name = entry
        .display_name
        .clone()
        .unwrap_or_else(|| outbound_id.value().to_owned());
    let detail = observation.details().detail(outbound_id);
    let protocol = detail.and_then(|detail| match &detail.protocol {
        OutboundProtocolObservation::Reported(protocol) => detail
            .protocol_display_name
            .clone()
            .or_else(|| Some(protocol.value().to_owned())),
        OutboundProtocolObservation::Unknown | OutboundProtocolObservation::Unsupported => None,
    });
    let delay_ms = detail.and_then(|detail| match &detail.current_latency_ms {
        OutboundObservation::Observed(delay) => Some(*delay),
        OutboundObservation::Unknown | OutboundObservation::Unsupported => None,
    });
    let alive = detail.and_then(|detail| match detail.health {
        OutboundHealthObservation::Healthy => Some(true),
        OutboundHealthObservation::Unhealthy => Some(false),
        OutboundHealthObservation::Unknown | OutboundHealthObservation::Unsupported => None,
    });
    ActiveExitSnapshot {
        generation,
        revision,
        status: ActiveExitStatus::Ready {
            group_id: group_id.clone(),
            outbound_id: outbound_id.clone(),
        },
        failure: None,
        group: Some(group_id.value().to_owned()),
        name: Some(display_name.clone()),
        country_code: extract_country_code(&display_name).map(str::to_owned),
        protocol,
        delay_ms,
        alive,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::core_identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
    };
    use infiltrator_contract::outbound::details::{
        OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState, OutboundHealthObservation,
        OutboundObservation, OutboundProbeAvailability, OutboundProtocolObservation,
        OutboundRttStatistics, OutboundUdpSupport,
    };
    use infiltrator_contract::outbound::identity::{
        OutboundId, OutboundProtocolId, SnapshotRevision,
    };
    use infiltrator_contract::outbound::inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundObservationSnapshot,
        OutboundSelectionSet, OutboundSelectionState, SelectionAvailability,
    };

    fn observation() -> OutboundObservationSnapshot {
        let engine = EngineId::new("mihomo").unwrap();
        let scope = CoreScope {
            identity: CoreIdentity::new(
                engine.clone(),
                CoreInstanceId::new("active").unwrap(),
                None,
            ),
            generation: CoreGeneration::new(3),
            session_token: None,
        };
        let group_id = OutboundId::new(engine.clone(), "GLOBAL").unwrap();
        let outbound_id = OutboundId::new(engine.clone(), "out-1").unwrap();
        let inventory = OutboundInventorySnapshot::new(
            scope.clone(),
            SnapshotRevision::new(4),
            OutboundInventoryState::Ready {
                entries: vec![infiltrator_contract::outbound::details::OutboundSnapshot {
                    id: outbound_id.clone(),
                    display_name: Some("香港 01".to_owned()),
                }],
                collections: vec![OutboundSelectionSet {
                    id: group_id,
                    member_ids: vec![outbound_id.clone()],
                    availability: SelectionAvailability::Selectable,
                    selection: OutboundSelectionState::Ready {
                        selected: outbound_id.clone(),
                    },
                }],
            },
        )
        .unwrap();
        let details = OutboundDetailsSnapshot::new(
            scope,
            SnapshotRevision::new(5),
            OutboundDetailsState::Ready {
                details: vec![OutboundDetail {
                    id: outbound_id,
                    protocol: OutboundProtocolObservation::Reported(
                        OutboundProtocolId::new(engine, "shadowsocks").unwrap(),
                    ),
                    protocol_display_name: Some("Shadowsocks".to_owned()),
                    cipher: OutboundObservation::Unknown,
                    cipher_display_name: None,
                    collection_kind: OutboundObservation::Unknown,
                    features: OutboundObservation::Unknown,
                    endpoint: OutboundObservation::Unknown,
                    udp: OutboundUdpSupport::Unknown,
                    health: OutboundHealthObservation::Healthy,
                    current_latency_ms: OutboundObservation::Observed(42),
                    latency_history: OutboundObservation::Unknown,
                    rtt: OutboundObservation::Observed(OutboundRttStatistics {
                        min_ms: Some(42),
                        max_ms: Some(42),
                        avg_ms: Some(42),
                        valid_count: 1,
                    }),
                    historical_egress: OutboundObservation::Unsupported,
                    probe_availability: OutboundProbeAvailability::Supported,
                }],
            },
        )
        .unwrap();
        OutboundObservationSnapshot::new(inventory, details).unwrap()
    }

    #[test]
    fn derives_selected_group_node_protocol_delay_and_region() {
        let snapshot = derive(3, 4, &observation());
        let ActiveExitStatus::Ready {
            group_id,
            outbound_id,
        } = &snapshot.status
        else {
            panic!("selected outbound should have typed identity")
        };
        assert_eq!(group_id.value(), "GLOBAL");
        assert_eq!(outbound_id.value(), "out-1");
        assert_eq!(snapshot.group.as_deref(), Some("GLOBAL"));
        assert_eq!(snapshot.name.as_deref(), Some("香港 01"));
        assert_eq!(snapshot.country_code.as_deref(), Some("HK"));
        assert_eq!(snapshot.protocol.as_deref(), Some("Shadowsocks"));
        assert_eq!(snapshot.delay_ms, Some(42));
        assert_eq!(snapshot.alive, Some(true));
    }

    #[test]
    fn empty_proxy_map_is_typed_empty() {
        let mut observation = observation();
        let inventory = OutboundInventorySnapshot::new(
            observation.inventory().scope().clone(),
            observation.inventory().revision(),
            OutboundInventoryState::Empty,
        )
        .unwrap();
        let details = OutboundDetailsSnapshot::new(
            observation.details().scope().clone(),
            observation.details().detail_revision(),
            OutboundDetailsState::Empty,
        )
        .unwrap();
        observation = OutboundObservationSnapshot::new(inventory, details).unwrap();
        let snapshot = derive(1, 1, &observation);
        assert!(matches!(snapshot.status, ActiveExitStatus::Empty));
        assert!(!snapshot.is_drawable());
    }
}
