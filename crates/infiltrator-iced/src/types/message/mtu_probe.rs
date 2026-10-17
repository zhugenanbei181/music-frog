use infiltrator_contract::mtu::MtuNegotiationSnapshot;
use lurker_core_contract::session::SessionToken;

#[derive(Clone)]
pub struct MtuProbeCompletion {
    pub snapshot: MtuNegotiationSnapshot,
    pub generation: u64,
    pub session_token: Option<SessionToken>,
}
