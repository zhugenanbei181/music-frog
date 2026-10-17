//! Typed process ownership context shared by the Core application and host adapters.

use crate::core_identity::{CoreGeneration, CoreIdentity};
use lurker_core_contract::session::SessionToken;
use serde::{Deserialize, Serialize};

/// Explicit owner context supplied for every process launch.
///
/// Android accepts only `Managed`; desktop command-line process control uses
/// `Standalone` until that surface is routed through a CoreApplication.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreProcessStartContext {
    Managed {
        identity: CoreIdentity,
        generation: CoreGeneration,
        session_token: SessionToken,
    },
    Standalone {
        identity: CoreIdentity,
    },
}

impl CoreProcessStartContext {
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

    pub fn standalone(identity: CoreIdentity) -> Self {
        Self::Standalone { identity }
    }

    pub fn identity(&self) -> &CoreIdentity {
        match self {
            Self::Managed { identity, .. } | Self::Standalone { identity } => identity,
        }
    }
}
