//! Page-level capture anchor: which part of a shared route a page-variant
//! capture names. The shell state carries it in every build, so the enum
//! lives outside the `capture-tools`-gated demo module.

/// Page-level capture anchor: the scenario page a capture names, typed.
///
/// `about` shares its route with `settings` but names one section of that
/// route's page. The runtime traffic audit also names one section of its
/// route. A capture with a section anchor scrolls that section into view before
/// the marker is written; the route default stays at the page top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaptureSection {
    #[default]
    RouteTop,
    /// The runtime page's traffic audit card (its range/dimension controls and
    /// the window's ranked entities), which sits below the live rates chart.
    RuntimeTrafficAudit,
    SettingsAbout,
}

impl CaptureSection {
    /// A section capture is only honest once the layout probe measured the
    /// named section on screen; the route top needs no region receipt.
    pub fn is_ready(self, region_measured: bool) -> bool {
        match self {
            Self::RouteTop => true,
            Self::RuntimeTrafficAudit | Self::SettingsAbout => region_measured,
        }
    }
}
