//! Generic page read-model vocabulary shared by every surface.
//!
//! Product page identifiers and product snapshot payloads stay in the product
//! contract. This module owns only the page state machine every surface folds:
//! a status with an explicit unavailability reason, the optional payload, and
//! whether the observation came from a demo fixture or a live host. No UI may
//! infer these facts from empty fields or a second local flag.

use crate::error::Failure;
use crate::outbound::inventory::{OutboundInventorySnapshot, OutboundInventoryState};
use serde::{Deserialize, Serialize};

/// State of one page read model. `Unavailable` is different from an empty
/// result: a surface must tell the user why a host capability is absent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageStatus {
    Loading,
    Ready,
    Empty,
    Unavailable { failure: Failure },
    Failed { failure: Failure },
}

/// Whether a surface snapshot is an explicit fixture or a host/application
/// observation. This is data, not something a UI may infer from empty fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceOrigin {
    Demo,
    Live,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageData<T> {
    pub status: PageStatus,
    pub data: Option<T>,
}

impl<T> PageData<T> {
    pub fn loading() -> Self {
        Self {
            status: PageStatus::Loading,
            data: None,
        }
    }

    pub fn ready(data: T) -> Self {
        Self {
            status: PageStatus::Ready,
            data: Some(data),
        }
    }

    pub fn empty(data: T) -> Self {
        Self {
            status: PageStatus::Empty,
            data: Some(data),
        }
    }

    pub fn unavailable(failure: Failure) -> Self {
        Self {
            status: PageStatus::Unavailable { failure },
            data: None,
        }
    }

    pub fn failed(failure: Failure) -> Self {
        Self {
            status: PageStatus::Failed { failure },
            data: None,
        }
    }
}

/// Fold a neutral outbound-inventory observation into the page status a surface
/// renders. An inventory that exists but is still unknown stays `Loading`
/// rather than pretending the page is ready or empty.
pub fn outbound_page_status(page: &PageData<OutboundInventorySnapshot>) -> PageStatus {
    match &page.status {
        PageStatus::Loading => PageStatus::Loading,
        PageStatus::Unavailable { failure } => PageStatus::Unavailable {
            failure: failure.clone(),
        },
        PageStatus::Failed { failure } => PageStatus::Failed {
            failure: failure.clone(),
        },
        PageStatus::Ready | PageStatus::Empty => {
            let Some(inventory) = page.data.as_ref() else {
                return PageStatus::Loading;
            };
            match inventory.inventory() {
                OutboundInventoryState::Unknown => PageStatus::Loading,
                OutboundInventoryState::Ready { .. } => PageStatus::Ready,
                OutboundInventoryState::Empty => PageStatus::Empty,
                OutboundInventoryState::Unsupported => PageStatus::Unavailable {
                    failure: Failure::unsupported("outbound inventory is unsupported"),
                },
                OutboundInventoryState::Failed { failure } => PageStatus::Failed {
                    failure: failure.clone(),
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ErrorCode, Failure};
    use crate::identity::{CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId};
    use crate::outbound::details::OutboundSnapshot;
    use crate::outbound::identity::{OutboundId, SnapshotRevision};
    use crate::outbound::inventory::{OutboundInventorySnapshot, OutboundInventoryState};
    use serde_json::json;

    fn failure() -> Failure {
        Failure::new(ErrorCode::Network, "controller unreachable", true)
    }

    fn engine() -> EngineId {
        EngineId::new("mihomo").expect("valid engine ID")
    }

    fn inventory(state: OutboundInventoryState) -> OutboundInventorySnapshot {
        let scope = CoreScope {
            identity: CoreIdentity::new(
                engine(),
                CoreInstanceId::new("owner-a").expect("valid instance ID"),
                None,
            ),
            generation: CoreGeneration::new(1),
            session_token: None,
        };
        OutboundInventorySnapshot::new(scope, SnapshotRevision::new(1), state)
            .expect("inventory matches its scope")
    }

    fn node() -> OutboundSnapshot {
        OutboundSnapshot {
            id: OutboundId::new(engine(), "node-a").expect("valid outbound ID"),
            display_name: None,
        }
    }

    #[test]
    fn page_data_constructors_publish_exactly_one_status_and_payload_shape() {
        assert_eq!(PageData::<u8>::loading().status, PageStatus::Loading);
        assert_eq!(PageData::<u8>::loading().data, None);

        let ready = PageData::ready(7u8);
        assert_eq!(ready.status, PageStatus::Ready);
        assert_eq!(ready.data, Some(7));

        let empty = PageData::<Vec<u8>>::empty(Vec::new());
        assert!(matches!(empty.status, PageStatus::Empty));
        assert_eq!(empty.data, Some(Vec::new()));

        let unavailable = PageData::<u8>::unavailable(failure());
        assert_eq!(
            unavailable.status,
            PageStatus::Unavailable { failure: failure() }
        );
        assert_eq!(unavailable.data, None);

        let failed = PageData::<u8>::failed(failure());
        assert_eq!(failed.status, PageStatus::Failed { failure: failure() });
        assert_eq!(failed.data, None);
    }

    #[test]
    fn page_status_and_origin_keep_the_established_serde_shape() {
        let unavailable = PageStatus::Unavailable { failure: failure() };
        assert_eq!(
            serde_json::to_value(&unavailable).unwrap(),
            json!({"Unavailable": {"failure": serde_json::to_value(failure()).unwrap()}})
        );
        assert_eq!(
            serde_json::to_value(PageStatus::Loading).unwrap(),
            json!("Loading")
        );
        assert_eq!(
            serde_json::to_value(PageStatus::Failed { failure: failure() }).unwrap(),
            json!({"Failed": {"failure": serde_json::to_value(failure()).unwrap()}})
        );
        assert_eq!(
            serde_json::to_value(SurfaceOrigin::Demo).unwrap(),
            json!("demo")
        );
        assert_eq!(
            serde_json::to_value(SurfaceOrigin::Live).unwrap(),
            json!("live")
        );
    }

    #[test]
    fn outbound_inventory_fold_separates_unknown_ready_empty_and_unsupported() {
        assert_eq!(
            outbound_page_status(&PageData::<OutboundInventorySnapshot>::loading()),
            PageStatus::Loading
        );

        assert_eq!(
            outbound_page_status(&PageData::<OutboundInventorySnapshot>::ready(inventory(
                OutboundInventoryState::Unknown
            ))),
            PageStatus::Loading
        );

        assert_eq!(
            outbound_page_status(&PageData::ready(inventory(OutboundInventoryState::Ready {
                entries: vec![node()],
                collections: Vec::new(),
            }))),
            PageStatus::Ready
        );

        assert_eq!(
            outbound_page_status(&PageData::ready(inventory(OutboundInventoryState::Empty))),
            PageStatus::Empty
        );

        assert!(matches!(
            outbound_page_status(&PageData::ready(inventory(
                OutboundInventoryState::Unsupported
            ))),
            PageStatus::Unavailable { .. }
        ));

        assert_eq!(
            outbound_page_status(&PageData::ready(inventory(
                OutboundInventoryState::Failed { failure: failure() }
            ))),
            PageStatus::Failed { failure: failure() }
        );

        assert_eq!(
            outbound_page_status(&PageData::<OutboundInventorySnapshot>::unavailable(
                failure()
            )),
            PageStatus::Unavailable { failure: failure() }
        );
    }
}
