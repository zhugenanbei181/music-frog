use serde::{Deserialize, Serialize};

/// Host capabilities are orthogonal to the UI toolkit running on the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostKind {
    Desktop,
    Android,
    Ios,
}

/// The inbound surface presenting the shared application contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SurfaceKind {
    IcedDesktop,
    BevyDesktop,
    SlintDesktop,
    BevyAndroid,
    SlintAndroid,
    AndroidCompose,
    IosCompose,
    AdminRest,
    Cli,
}

impl SurfaceKind {
    /// Authoritative product name for this surface.
    pub const fn product_name(self) -> &'static str {
        match self {
            Self::IcedDesktop => "Ice",
            Self::BevyDesktop | Self::BevyAndroid => "Lurker",
            Self::AndroidCompose | Self::IosCompose => "Source",
            Self::SlintDesktop | Self::SlintAndroid => "Lumen",
            Self::AdminRest => "Admin",
            Self::Cli => "Cli",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_kind_product_names_are_formalized() {
        assert_eq!(SurfaceKind::IcedDesktop.product_name(), "Ice");
        assert_eq!(SurfaceKind::BevyDesktop.product_name(), "Lurker");
        assert_eq!(SurfaceKind::AndroidCompose.product_name(), "Source");
        assert_eq!(SurfaceKind::SlintDesktop.product_name(), "Lumen");
    }
}
