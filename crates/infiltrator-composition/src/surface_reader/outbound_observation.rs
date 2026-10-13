use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SurfaceReadScope {
    identity: Option<CoreIdentity>,
    generation: CoreGeneration,
    session_token: Option<SessionToken>,
}

impl From<&CoreSnapshot> for SurfaceReadScope {
    fn from(snapshot: &CoreSnapshot) -> Self {
        Self {
            identity: snapshot.identity.clone(),
            generation: CoreGeneration::from(snapshot.generation),
            session_token: snapshot.session_token,
        }
    }
}

pub(super) fn surface_read_scope_changed() -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::NotReady,
        "core scope changed during surface read; retry the read",
        true,
    ))
}

pub(super) fn authorize_outbound_inventory(
    capabilities: &CoreCapabilityApplication,
    scope: &CoreScope,
) -> Result<(), Failure> {
    capabilities
        .authorize(
            Some(scope),
            CoreCapabilityRequirement::new(
                CoreOperation::Outbound(OutboundOperation::List),
                CoreCommandEffect::Read,
            ),
        )
        .map(|_| ())
        .map_err(CoreCapabilityGateFailure::into_failure)
}

pub(super) fn gate_outbound_observation(
    capabilities: &CoreCapabilityApplication,
    observation: OutboundObservationSnapshot,
) -> Result<OutboundObservationSnapshot, Failure> {
    let inventory = gate_outbound_selection(capabilities, observation.inventory().clone())?;
    OutboundObservationSnapshot::new(inventory, observation.details().clone())
        .map_err(|error| Failure::new(ErrorCode::InvalidState, error.to_string(), false))
}

pub(super) fn gate_outbound_selection(
    capabilities: &CoreCapabilityApplication,
    snapshot: OutboundInventorySnapshot,
) -> Result<OutboundInventorySnapshot, Failure> {
    let availability = match capabilities.resolve(
        Some(snapshot.scope()),
        CoreOperation::Outbound(OutboundOperation::Select),
    ) {
        CoreCapabilityLookup::Resolved(resolution) => match resolution.status {
            CoreCapabilityStatus::Available {
                access: CapabilityAccess::Supported,
                ..
            } => SelectionAvailability::Selectable,
            CoreCapabilityStatus::Available {
                access: CapabilityAccess::ReadOnly,
                ..
            } => SelectionAvailability::ReadOnly,
            CoreCapabilityStatus::Unsupported { .. } => SelectionAvailability::Unsupported,
            CoreCapabilityStatus::VersionUnavailable { .. }
            | CoreCapabilityStatus::HostUnavailable { .. }
            | CoreCapabilityStatus::AdapterMissing { .. }
            | CoreCapabilityStatus::Unregistered
            | CoreCapabilityStatus::ScopeMismatch => SelectionAvailability::Unknown,
        },
        CoreCapabilityLookup::Unavailable { .. }
        | CoreCapabilityLookup::InvalidProviderMatrix { .. } => SelectionAvailability::Unknown,
    };
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = snapshot.inventory()
    else {
        return Ok(snapshot);
    };
    let collections = collections
        .iter()
        .cloned()
        .map(|mut collection| {
            if collection.availability == SelectionAvailability::Selectable
                && availability != SelectionAvailability::Selectable
            {
                collection.availability = availability;
            }
            collection
        })
        .collect();
    OutboundInventorySnapshot::new(
        snapshot.scope().clone(),
        snapshot.revision(),
        OutboundInventoryState::Ready {
            entries: entries.clone(),
            collections,
        },
    )
    .map_err(|error| Failure::new(ErrorCode::InvalidState, error.to_string(), false))
}
