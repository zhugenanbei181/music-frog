//! Command intent bound to the engine instance observed by the requesting surface.
//!
//! The neutral identity/fencing types live in `lurker-core-contract::identity`;
//! only the product command envelope stays here.

use crate::command::{CommandIntent, CommandResult};
use lurker_core_contract::identity::CoreScope;
use serde::{Deserialize, Serialize};

/// User intent bound to the engine instance observed by the requesting surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreCommandRequest {
    pub scope: CoreScope,
    pub intent: CommandIntent,
}

/// Terminal result plus the scope observed after execution or rejection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreCommandOutcome {
    pub scope: Option<CoreScope>,
    pub result: CommandResult,
}
