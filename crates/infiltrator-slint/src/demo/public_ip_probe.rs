//! The demo host's typed public-egress probe owner.
//!
//! The capture host emits the same `RefreshPublicIpProbe` intent the product
//! emits. This module owns one demo probe lifecycle: a command publishes
//! `Probing`, the next owner read keeps that stage, and the following read
//! admits the stamped terminal answer. The stable surface snapshot is only a
//! projection; no widget owns a loading flag or a second copy of the status.

use infiltrator_contract::public_ip::PublicIpProbeSnapshot;
use infiltrator_contract::public_ip_probe::{
    PublicIpObservation, PublicIpProbeAnswer, PublicIpProbeError, PublicIpProbeRouteStamp,
    PublicIpProbeStamp, PublicIpProbeState,
};
use infiltrator_shared::locales::{Lang, Localizer};

/// The key whose shared catalog value the typed failure reason publishes.
const PROBE_FAILURE_KEY: &str = "public_ip_failed";

/// The shared catalog value the typed failure reason publishes.
pub fn failure_reason(language: &str) -> String {
    Lang(language).tr(PROBE_FAILURE_KEY).into_owned()
}

/// The fixture's initial answer, projected byte-identically to the shared
/// demo fixture. The shared typed observation cannot express `isp`/`asn`, so
/// the demo answer owns those two facts in a typed extras value instead of
/// changing the contract or overloading a geographic field.
const DEMO_IP: &str = "203.0.113.7";
const DEMO_COUNTRY: &str = "HK";
const DEMO_CITY: &str = "Hong Kong";
const DEMO_ISP: &str = "Cloudflare / APNIC";
const DEMO_ASN: u32 = 13335;
const DEMO_PROVIDER: &str = "ipapi.is";
const DEMO_CHECKED_AT_EPOCH_MS: u64 = 1_741_300_000_000;

const INITIAL_GENERATION: u64 = 1;
const INITIAL_REVISION: u64 = 1;
const INITIAL_REQUEST_ID: u64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DemoSucceededExtras {
    isp: Option<&'static str>,
    asn: Option<u32>,
}

const DEMO_SUCCEEDED_EXTRAS: DemoSucceededExtras = DemoSucceededExtras {
    isp: Some(DEMO_ISP),
    asn: Some(DEMO_ASN),
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum RefreshStage {
    Idle,
    ProbingPublished(PublicIpProbeAnswer),
    AwaitingTerminal(PublicIpProbeAnswer),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicIpProbeOwner {
    state: PublicIpProbeState,
    stage: RefreshStage,
    next_request_id: u64,
}

impl Default for PublicIpProbeOwner {
    fn default() -> Self {
        let stamp = PublicIpProbeStamp {
            request_id: INITIAL_REQUEST_ID,
            core_generation: INITIAL_GENERATION,
            revision: INITIAL_REVISION,
            route: PublicIpProbeRouteStamp::Direct,
        };
        Self {
            state: PublicIpProbeState::Succeeded {
                stamp,
                observation: demo_observation(),
            },
            stage: RefreshStage::Idle,
            next_request_id: INITIAL_REQUEST_ID + 1,
        }
    }
}

impl PublicIpProbeOwner {
    /// Starts one refresh and returns its typed pending answer. The command
    /// itself never invents a terminal readback.
    pub fn begin_refresh(
        &mut self,
        language: &str,
    ) -> Result<PublicIpProbeAnswer, PublicIpProbeError> {
        if self.stage != RefreshStage::Idle {
            return Err(PublicIpProbeError::AlreadyProbing);
        }
        let stamp = PublicIpProbeStamp {
            request_id: self.next_request_id,
            core_generation: INITIAL_GENERATION,
            revision: INITIAL_REVISION + self.next_request_id,
            route: PublicIpProbeRouteStamp::Direct,
        };
        self.state = self.state.begin(stamp)?;
        self.next_request_id += 1;
        let answer = if self.next_request_id.is_multiple_of(2) {
            PublicIpProbeAnswer::Succeeded {
                stamp,
                observation: demo_observation(),
            }
        } else {
            PublicIpProbeAnswer::Failed {
                stamp,
                reason: failure_reason(language),
            }
        };
        self.stage = RefreshStage::ProbingPublished(answer.clone());
        Ok(answer)
    }

    /// Publishes the owner's readback. The command has already made `Probing`
    /// the current state; the first read keeps that stage, and the following
    /// read admits the stamped terminal answer.
    pub fn snapshot(&mut self) -> PublicIpProbeSnapshot {
        if self.stage == RefreshStage::Idle {
            return self.snapshot_from_state();
        }
        if let RefreshStage::AwaitingTerminal(answer) = self.stage.clone() {
            let current = self.state.clone();
            self.state = current
                .accept(answer)
                .expect("the staged answer carries the active stamp");
            self.stage = RefreshStage::Idle;
        } else if let RefreshStage::ProbingPublished(answer) = self.stage.clone() {
            self.stage = RefreshStage::AwaitingTerminal(answer);
        }
        self.snapshot_from_state()
    }

    /// Returns the current readback without advancing the refresh lifecycle.
    pub fn peek_snapshot(&self) -> PublicIpProbeSnapshot {
        self.snapshot_from_state()
    }

    /// Admits a stamped terminal answer, returning the typed fence error when
    /// it does not belong to the active request.
    #[cfg(test)]
    pub fn accept_answer(
        &mut self,
        answer: PublicIpProbeAnswer,
    ) -> Result<PublicIpProbeState, PublicIpProbeError> {
        let current = self.state.clone();
        let state = current.accept(answer)?;
        self.state = state.clone();
        self.stage = RefreshStage::Idle;
        Ok(state)
    }

    /// The current typed state, read only by the demo host's phase assertions.
    pub fn state(&self) -> &PublicIpProbeState {
        &self.state
    }

    fn snapshot_from_state(&self) -> PublicIpProbeSnapshot {
        let mut snapshot = PublicIpProbeSnapshot::from_state(&self.state);
        if matches!(&self.state, PublicIpProbeState::Succeeded { .. }) {
            snapshot.isp = DEMO_SUCCEEDED_EXTRAS.isp.map(str::to_owned);
            snapshot.asn = DEMO_SUCCEEDED_EXTRAS.asn;
        }
        snapshot
    }
}

fn demo_observation() -> PublicIpObservation {
    PublicIpObservation {
        ip: DEMO_IP.to_owned(),
        country: Some(DEMO_COUNTRY.to_owned()),
        region: None,
        city: Some(DEMO_CITY.to_owned()),
        provider: DEMO_PROVIDER.to_owned(),
        checked_at_epoch_ms: DEMO_CHECKED_AT_EPOCH_MS,
    }
}

#[cfg(test)]
#[path = "public_ip_probe_tests.rs"]
mod tests;
