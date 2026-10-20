//! Typed process ownership context shared by the Core application and host adapters.

use crate::identity::{CoreGeneration, CoreIdentity};
use crate::session::SessionToken;
use serde::{Deserialize, Serialize};

/// Explicit owner context supplied for every process launch.
///
/// Android accepts only `Managed`; desktop command-line process control uses
/// `Standalone` until that surface is routed through a CoreApplication.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreProcessStartContext {
    /// The process is owned by a Core application session.
    ///
    /// The identity, generation and session token form the fence every later
    /// readback of that process must match; a process started without them
    /// cannot be proven to belong to the current owner.
    Managed {
        /// Stable identity of the core instance that will own the process.
        identity: CoreIdentity,
        /// Activation counter of the owner that will own the process.
        generation: CoreGeneration,
        /// Session token of the owner that will own the process.
        session_token: SessionToken,
    },
    /// The process is started outside any Core application session.
    ///
    /// Only the stable identity is known, so a later readback can prove the
    /// engine and instance but never the owning session.
    Standalone {
        /// Stable identity of the core instance the process serves.
        identity: CoreIdentity,
    },
}

impl CoreProcessStartContext {
    /// Builds the managed ownership context for a Core application session.
    pub fn managed(
        identity: CoreIdentity,
        generation: CoreGeneration,
        session_token: SessionToken,
    ) -> Self {
        Self::Managed {
            identity,
            generation,
            session_token,
        }
    }

    /// Builds the standalone ownership context used by direct CLI launches.
    pub fn standalone(identity: CoreIdentity) -> Self {
        Self::Standalone { identity }
    }

    /// Returns the identity of the core instance the process belongs to.
    pub fn identity(&self) -> &CoreIdentity {
        match self {
            Self::Managed { identity, .. } | Self::Standalone { identity } => identity,
        }
    }
}
