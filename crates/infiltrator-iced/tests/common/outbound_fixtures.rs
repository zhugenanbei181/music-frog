//! Typed owner snapshots used by product-path outbound UI tests.
use infiltrator_application::core_application::CoreApplication;
use infiltrator_contract::core_capabilities::CoreCapabilitySnapshot;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::message;
use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::outbound::preferences::OutboundLayout;
use lurker_core_contract::outbound::{
    details::{
        OutboundCollectionKind, OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState,
        OutboundEndpoint, OutboundHealthObservation, OutboundLatencySample, OutboundObservation,
        OutboundProbeAvailability, OutboundProtocolObservation, OutboundRttStatistics,
        OutboundSnapshot, OutboundUdpSupport,
    },
    identity::{OutboundCipherId, OutboundId, OutboundProtocolId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionSet,
        OutboundSelectionState, SelectionAvailability,
    },
    preferences::{
        OutboundHealthFilter, OutboundSortOrder, OutboundUiPreferencesSnapshot,
        OutboundUiPreferencesValues,
    },
};
use lurker_core_contract::page::PageData;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct OutboundMemberFixture {
    pub id: &'static str,
    pub display_name: &'static str,
    pub health: OutboundHealthObservation,
    pub latency_ms: Option<u32>,
    pub probe_availability: OutboundProbeAvailability,
}

pub struct OutboundFixtureInput<'a> {
    pub scope: &'a CoreScope,
    pub revision: u64,
    pub members: &'a [OutboundMemberFixture],
    pub selected: Option<&'a str>,
    pub favorites: &'a [&'a str],
    pub health_filter: OutboundHealthFilter,
    pub sort_order: OutboundSortOrder,
    pub layout: OutboundLayout,
}

#[derive(Clone, Debug)]
pub struct OutboundFixture {
    pub scope: CoreScope,
    #[allow(dead_code)]
    pub group_id: OutboundId,
    pub ids: HashMap<&'static str, OutboundId>,
    pub inventory: OutboundInventorySnapshot,
    pub details: OutboundDetailsSnapshot,
    pub preferences: OutboundUiPreferencesSnapshot,
}

impl OutboundFixture {
    pub fn new(input: OutboundFixtureInput<'_>) -> Self {
        let OutboundFixtureInput {
            scope,
            revision,
            members,
            selected,
            favorites,
            health_filter,
            sort_order,
            layout,
        } = input;
        let engine = scope.identity.engine.clone();
        let group_id = OutboundId::new(engine.clone(), "GLOBAL").unwrap();
        let ids = members
            .iter()
            .map(|member| {
                (
                    member.id,
                    OutboundId::new(engine.clone(), member.id).unwrap(),
                )
            })
            .collect::<HashMap<_, _>>();
        let entries = members
            .iter()
            .map(|member| OutboundSnapshot {
                id: ids[member.id].clone(),
                display_name: Some(member.display_name.to_owned()),
            })
            .collect::<Vec<_>>();
        let member_ids = members
            .iter()
            .map(|member| ids[member.id].clone())
            .collect::<Vec<_>>();
        let selection = selected
            .map(|selected| OutboundSelectionState::Ready {
                selected: ids[selected].clone(),
            })
            .unwrap_or(OutboundSelectionState::Unknown);
        let inventory = OutboundInventorySnapshot::new(
            scope.clone(),
            SnapshotRevision::new(revision),
            OutboundInventoryState::Ready {
                entries,
                collections: vec![OutboundSelectionSet {
                    id: group_id.clone(),
                    member_ids,
                    availability: SelectionAvailability::Selectable,
                    selection,
                }],
            },
        )
        .unwrap();
        let details = OutboundDetailsSnapshot::new(
            scope.clone(),
            SnapshotRevision::new(revision),
            OutboundDetailsState::Ready {
                details: members
                    .iter()
                    .map(|member| OutboundDetail {
                        id: ids[member.id].clone(),
                        protocol: OutboundProtocolObservation::Reported(
                            OutboundProtocolId::new(engine.clone(), "shadowsocks").unwrap(),
                        ),
                        protocol_display_name: Some("Shadowsocks".into()),
                        cipher: OutboundObservation::Observed(
                            OutboundCipherId::new(engine.clone(), "chacha20-ietf-poly1305")
                                .unwrap(),
                        ),
                        cipher_display_name: Some("ChaCha20-Poly1305".into()),
                        collection_kind: OutboundObservation::Observed(
                            OutboundCollectionKind::ManualSelection,
                        ),
                        features: OutboundObservation::Observed(Vec::new()),
                        endpoint: OutboundObservation::Observed(OutboundEndpoint {
                            host: "192.0.2.1".into(),
                            port: OutboundObservation::Observed(443),
                        }),
                        udp: OutboundUdpSupport::Supported,
                        health: member.health,
                        current_latency_ms: member
                            .latency_ms
                            .map(OutboundObservation::Observed)
                            .unwrap_or(OutboundObservation::Unknown),
                        latency_history: OutboundObservation::Observed(
                            member
                                .latency_ms
                                .map(|delay_ms| {
                                    vec![OutboundLatencySample {
                                        observed_at: "2026-10-10T00:00:00Z".into(),
                                        delay_ms,
                                    }]
                                })
                                .unwrap_or_default(),
                        ),
                        rtt: OutboundObservation::Observed(OutboundRttStatistics::default()),
                        historical_egress: OutboundObservation::Observed(Vec::new()),
                        probe_availability: member.probe_availability,
                    })
                    .collect(),
            },
        )
        .unwrap();
        let favorite_outbound_ids = favorites
            .iter()
            .map(|favorite| ids[favorite].clone())
            .collect();
        let preferences = OutboundUiPreferencesSnapshot::new(
            scope.clone(),
            SnapshotRevision::new(revision),
            OutboundUiPreferencesValues {
                search_query: String::new(),
                favorite_outbound_ids,
                group_order: Vec::new(),
                collapsed_group_ids: Vec::new(),
                sort_order,
                health_filter,
                layout,
            },
        )
        .unwrap();
        Self {
            scope: scope.clone(),
            group_id,
            ids,
            inventory,
            details,
            preferences,
        }
    }

    pub fn attach_to(&self, state: &mut AppState, application: &CoreApplication) {
        let core = application.snapshot();
        state.attach_product_application(application.clone());
        state.shell.demo = false;
        let mut surface = SurfaceSnapshot::unavailable(
            SurfaceKind::IcedDesktop,
            HostKind::Desktop,
            Failure::unsupported("unrelated test surface page"),
        );
        surface.core = core;
        surface.generation = surface.core.generation;
        surface.revision = 1;
        surface.core_capability_snapshot = application
            .core_capability_application()
            .matrix_for(
                &self.scope.identity.engine,
                self.scope.identity.version.as_ref(),
            )
            .map(|matrix| CoreCapabilitySnapshot::from_matrix(&matrix))
            .map(PageData::ready)
            .unwrap_or_else(PageData::unavailable);
        surface.outbound_inventory = PageData::ready(self.inventory.clone());
        surface.outbound_details = PageData::ready(self.details.clone());
        surface.outbound_ui_preferences = PageData::ready(self.preferences.clone());
        assert!(state.apply_shared_surface_snapshot(surface));
    }
}

pub fn standard_members() -> Vec<OutboundMemberFixture> {
    vec![
        OutboundMemberFixture {
            id: "proxy-a",
            display_name: "Proxy-A",
            health: OutboundHealthObservation::Healthy,
            latency_ms: Some(100),
            probe_availability: OutboundProbeAvailability::Supported,
        },
        OutboundMemberFixture {
            id: "proxy-b",
            display_name: "Proxy-B",
            health: OutboundHealthObservation::Healthy,
            latency_ms: Some(50),
            probe_availability: OutboundProbeAvailability::Supported,
        },
        OutboundMemberFixture {
            id: "hong-kong",
            display_name: "香港 IEPL-01",
            health: OutboundHealthObservation::Healthy,
            latency_ms: Some(120),
            probe_availability: OutboundProbeAvailability::Supported,
        },
        OutboundMemberFixture {
            id: "japan",
            display_name: "日本 Tokyo-01",
            health: OutboundHealthObservation::Healthy,
            latency_ms: Some(80),
            probe_availability: OutboundProbeAvailability::Supported,
        },
        OutboundMemberFixture {
            id: "node-alive-1",
            display_name: "Node-Alive-1",
            health: OutboundHealthObservation::Healthy,
            latency_ms: Some(120),
            probe_availability: OutboundProbeAvailability::Supported,
        },
        OutboundMemberFixture {
            id: "node-dead",
            display_name: "Node-Dead",
            health: OutboundHealthObservation::Unhealthy,
            latency_ms: None,
            probe_availability: OutboundProbeAvailability::Supported,
        },
        OutboundMemberFixture {
            id: "node-alive-2",
            display_name: "Node-Alive-2",
            health: OutboundHealthObservation::Healthy,
            latency_ms: Some(50),
            probe_availability: OutboundProbeAvailability::Supported,
        },
    ]
}

#[allow(dead_code)]
pub fn terminal(task: iced::Task<message::Message>) -> message::Message {
    use futures_util::StreamExt;
    use iced_runtime::Action;
    use iced_runtime::task::into_stream;
    use tokio::runtime::Builder;

    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("typed outbound command task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("typed outbound command must deliver a terminal message")
            };
            assert!(stream.next().await.is_none());
            message
        })
}
