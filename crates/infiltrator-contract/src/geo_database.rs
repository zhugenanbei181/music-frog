//! Honest result of asking a core to refresh its GeoIP/GeoSite databases.

use crate::core_identity::CoreScope;
use serde::{Deserialize, Serialize};

/// Mihomo acknowledges this asynchronously; Accepted never means that new
/// database bytes have already been downloaded or installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeoDatabaseTriggerState {
    Accepted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeoDatabaseTriggerReceipt {
    pub scope: CoreScope,
    pub state: GeoDatabaseTriggerState,
}

impl GeoDatabaseTriggerReceipt {
    pub fn accepted(scope: CoreScope) -> Self {
        Self {
            scope,
            state: GeoDatabaseTriggerState::Accepted,
        }
    }
}
