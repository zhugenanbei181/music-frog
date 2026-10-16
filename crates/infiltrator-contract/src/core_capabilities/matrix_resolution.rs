use super::*;

pub(super) fn apply_engine_decision(
    decision: &EngineCapabilityDecision,
    access: &mut CapabilityAccess,
    maturity: &mut CapabilityMaturity,
    blockers: &mut Vec<CapabilityBlocker>,
) {
    match decision {
        EngineCapabilityDecision::Supported => {}
        EngineCapabilityDecision::OwnerManaged { .. } => {}
        EngineCapabilityDecision::ReadOnly => *access = CapabilityAccess::ReadOnly,
        EngineCapabilityDecision::Experimental => *maturity = CapabilityMaturity::Experimental,
        EngineCapabilityDecision::Unsupported { reason } => {
            blockers.push(CapabilityBlocker::Unsupported {
                layer: CapabilityLayer::Engine,
                reason: reason.clone(),
            });
        }
        EngineCapabilityDecision::VersionUnavailable { reason } => {
            blockers.push(CapabilityBlocker::VersionUnavailable {
                reason: reason.clone(),
            });
        }
    }
}

pub(super) fn apply_host_decision(
    decision: &HostCapabilityDecision,
    access: &mut CapabilityAccess,
    maturity: &mut CapabilityMaturity,
    blockers: &mut Vec<CapabilityBlocker>,
) {
    match decision {
        HostCapabilityDecision::Supported => {}
        HostCapabilityDecision::ReadOnly => *access = CapabilityAccess::ReadOnly,
        HostCapabilityDecision::Experimental => *maturity = CapabilityMaturity::Experimental,
        HostCapabilityDecision::Unsupported { reason } => {
            blockers.push(CapabilityBlocker::Unsupported {
                layer: CapabilityLayer::Host,
                reason: reason.clone(),
            });
        }
        HostCapabilityDecision::HostUnavailable { reason } => {
            blockers.push(CapabilityBlocker::HostUnavailable {
                reason: reason.clone(),
            });
        }
    }
}

pub(super) fn apply_adapter_decision(
    decision: &AdapterCapabilityDecision,
    access: &mut CapabilityAccess,
    maturity: &mut CapabilityMaturity,
    blockers: &mut Vec<CapabilityBlocker>,
) {
    match decision {
        AdapterCapabilityDecision::Supported => {}
        AdapterCapabilityDecision::ReadOnly => *access = CapabilityAccess::ReadOnly,
        AdapterCapabilityDecision::Experimental => *maturity = CapabilityMaturity::Experimental,
        AdapterCapabilityDecision::Unsupported { reason } => {
            blockers.push(CapabilityBlocker::Unsupported {
                layer: CapabilityLayer::Adapter,
                reason: reason.clone(),
            });
        }
        AdapterCapabilityDecision::AdapterMissing { reason } => {
            blockers.push(CapabilityBlocker::AdapterMissing {
                reason: reason.clone(),
            });
        }
    }
}

#[derive(Clone, Copy)]
enum CapabilityBlockerClass {
    None,
    AdapterMissing,
    VersionUnavailable,
    HostUnavailable,
    Unsupported,
}

impl CapabilityBlockerClass {
    pub(super) fn include(self, blocker: &CapabilityBlocker) -> Self {
        use CapabilityBlocker::{AdapterMissing, HostUnavailable, Unsupported, VersionUnavailable};

        match self {
            Self::Unsupported => Self::Unsupported,
            Self::HostUnavailable => match blocker {
                Unsupported { .. } => Self::Unsupported,
                HostUnavailable { .. } | VersionUnavailable { .. } | AdapterMissing { .. } => {
                    Self::HostUnavailable
                }
            },
            Self::VersionUnavailable => match blocker {
                Unsupported { .. } => Self::Unsupported,
                HostUnavailable { .. } => Self::HostUnavailable,
                VersionUnavailable { .. } | AdapterMissing { .. } => Self::VersionUnavailable,
            },
            Self::AdapterMissing => match blocker {
                Unsupported { .. } => Self::Unsupported,
                HostUnavailable { .. } => Self::HostUnavailable,
                VersionUnavailable { .. } => Self::VersionUnavailable,
                AdapterMissing { .. } => Self::AdapterMissing,
            },
            Self::None => match blocker {
                Unsupported { .. } => Self::Unsupported,
                HostUnavailable { .. } => Self::HostUnavailable,
                VersionUnavailable { .. } => Self::VersionUnavailable,
                AdapterMissing { .. } => Self::AdapterMissing,
            },
        }
    }
}

pub(super) fn status_from_blockers(blockers: Vec<CapabilityBlocker>) -> CoreCapabilityStatus {
    let classification = blockers.iter().fold(
        CapabilityBlockerClass::None,
        CapabilityBlockerClass::include,
    );
    match classification {
        CapabilityBlockerClass::Unsupported => CoreCapabilityStatus::Unsupported { blockers },
        CapabilityBlockerClass::HostUnavailable => {
            CoreCapabilityStatus::HostUnavailable { blockers }
        }
        CapabilityBlockerClass::VersionUnavailable => {
            CoreCapabilityStatus::VersionUnavailable { blockers }
        }
        CapabilityBlockerClass::AdapterMissing => CoreCapabilityStatus::AdapterMissing { blockers },
        CapabilityBlockerClass::None => CoreCapabilityStatus::Unregistered,
    }
}
