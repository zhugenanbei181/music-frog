use serde::{Deserialize, Serialize};

/// Host capabilities are orthogonal to the UI toolkit running on the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostKind {
    Desktop,
    Android,
    Ios,
}

/// The inbound surface presenting the shared application contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SurfaceKind {
    IcedDesktop,
    BevyDesktop,
    BevyAndroid,
    AndroidCompose,
    IosCompose,
    AdminRest,
    Cli,
}
