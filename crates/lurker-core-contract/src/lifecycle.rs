//! Engine-neutral lifecycle vocabulary shared by every surface.

use serde::{Deserialize, Serialize};

/// Exact inactive lifecycle fact carried by operations that persist selected
/// profile content without starting or applying Core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreInactiveLifecycle {
    Stopped,
    Failed,
}
