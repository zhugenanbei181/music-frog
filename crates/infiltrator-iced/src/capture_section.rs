//! Page-level capture anchor: which part of a shared route a page-variant
//! capture names. The shell state carries it in every build, so the enum
//! lives outside the `capture-tools`-gated demo module.

/// Page-level capture anchor: the scenario page a capture names, typed.
///
/// `connections`, `logs` and `about` share their route with `runtime` /
/// `settings` but name one section of that route's page. A capture with a
/// section anchor scrolls that section into view before the marker is
/// written; the route default stays at the page top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaptureSection {
    #[default]
    RouteTop,
    RuntimeConnections,
    RuntimeLogs,
    SettingsAbout,
}

impl CaptureSection {
    /// A section capture is only honest once the layout probe measured the
    /// named section on screen; the route top needs no region receipt.
    pub fn is_ready(self, region_measured: bool) -> bool {
        match self {
            Self::RouteTop => true,
            Self::RuntimeConnections | Self::RuntimeLogs | Self::SettingsAbout => region_measured,
        }
    }
}
