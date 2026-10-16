use super::*;

impl SurfaceSnapshot {
    /// Ensure the target-scoped capability evidence belongs to this exact
    /// Core engine/version and host surface. Capability evidence is static
    /// across Core process generations, so it is intentionally not bound to
    /// the runtime generation or session token.
    pub fn validate_core_capability_scope_binding(&self) -> Result<(), Failure> {
        let capability = match (
            &self.core_capability_snapshot.status,
            &self.core_capability_snapshot.data,
        ) {
            (PageStatus::Ready | PageStatus::Empty, Some(capability)) => capability,
            (PageStatus::Ready | PageStatus::Empty, None) => {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "ready Core capability page has no typed snapshot",
                    true,
                ));
            }
            (_, None) => return Ok(()),
            (_, Some(_)) => {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "unavailable Core capability page carries untrusted snapshot data",
                    true,
                ));
            }
        };
        capability.validate().map_err(|_| {
            Failure::new(
                ErrorCode::InvalidState,
                "Core capability snapshot contains stale, duplicate, or missing operation evidence",
                true,
            )
        })?;
        let identity = self.core.identity.as_ref().ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidState,
                "Core capability evidence is present without a Core identity",
                true,
            )
        })?;
        let scope = &capability.scope;
        let same_engine_version_host = scope.engine_id == identity.engine
            && scope.core_version == identity.version
            && scope.host == self.capabilities.host
            && scope.binary_target.host_kind() == Some(self.capabilities.host);
        if same_engine_version_host {
            Ok(())
        } else {
            Err(Failure::new(
                ErrorCode::InvalidState,
                "Core capability scope does not match the surface Core identity, version, or target host",
                true,
            ))
        }
    }

    /// Ensure the Core scope embedded in the Android VPN owner snapshot agrees
    /// with the top-level Core facts from this surface read.
    pub fn validate_vpn_owner_core_binding(&self) -> Result<(), Failure> {
        let owner = match &self.vpn {
            VpnSurfaceObservation::AndroidOwner(owner) => owner,
            VpnSurfaceObservation::Unsupported { .. }
            | VpnSurfaceObservation::NotApplicable
            | VpnSurfaceObservation::Unavailable { .. } => return Ok(()),
        };
        match &owner.core {
            VpnOwnerCoreReadback::Available(owner_core) => {
                if owner_core == &self.core.lifecycle_snapshot() {
                    Ok(())
                } else {
                    Err(Failure::new(
                        ErrorCode::InvalidState,
                        "surface Core facts and :vpn owner Core scope/generation do not match",
                        true,
                    ))
                }
            }
            VpnOwnerCoreReadback::Unavailable { .. }
                if self.core.identity.is_none() =>
            {
                Ok(())
            }
            VpnOwnerCoreReadback::Unavailable { failure_message } => Err(
                Failure::new(ErrorCode::NotReady, failure_message.clone(), true),
            ),
        }
    }

}
