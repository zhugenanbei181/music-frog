//! Universal Widget Engine ABI contract and cross-project runner host interfaces.
//!
//! Charter (docs/bevy-ui/BEVY_UI_FRONTEND.md §8.1.15, §8.3): the widget layer is
//! the extraction candidate shared across projects, so the boundary is a stable,
//! versioned ABI. A host *advertises* its ABI version and capability set; a
//! widget *requires* a version and a set of capabilities; [`negotiate`] returns a
//! typed [`AbiNegotiation`] — compatible, version-mismatched or missing
//! capabilities — and never silently downgrades or panics.

/// Public semantic version of the shared Bevy widget ABI.
pub const WIDGET_ABI_VERSION: (u32, u32, u32) = (0, 30, 0);

/// Cross-project runner host trait for headless and windowed integration.
pub trait UniversalRunnerHost: Send + Sync {
    fn host_name(&self) -> &'static str;
    fn abi_version(&self) -> (u32, u32, u32) {
        WIDGET_ABI_VERSION
    }
    fn is_headless(&self) -> bool;
    fn notify_crash(&self, reason: &str);

    /// The capability set this host advertises to widgets.
    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities::none()
    }

    /// Negotiate a widget's ABI requirement against this host's advertisement.
    fn negotiate(&self, requirement: WidgetAbiRequirement) -> AbiNegotiation {
        negotiate(self.abi_version(), self.capabilities(), requirement)
    }
}

/// Default desktop runner host implementation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DesktopRunnerHost;

impl UniversalRunnerHost for DesktopRunnerHost {
    fn host_name(&self) -> &'static str {
        "MusicFrog Infiltrator Desktop Runner"
    }

    fn is_headless(&self) -> bool {
        false
    }

    fn notify_crash(&self, _reason: &str) {}

    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities::all_desktop()
    }
}

/// Check if an incoming host ABI version is backward-compatible with this crate.
pub fn is_abi_compatible(host_version: (u32, u32, u32)) -> bool {
    abi_versions_compatible(WIDGET_ABI_VERSION, host_version)
}

/// Whether `host` can run a widget built against `required`.
///
/// The major component must match exactly; the host's minor must be at least the
/// required minor (a newer minor is backward-compatible). The patch component is
/// free on both sides.
pub fn abi_versions_compatible(required: (u32, u32, u32), host: (u32, u32, u32)) -> bool {
    let (required_major, required_minor, _) = required;
    let (host_major, host_minor, _) = host;
    required_major == host_major && host_minor >= required_minor
}

/// Individual platform hardware or UI subsystem capabilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetCapability {
    GpuShaders = 1 << 0,
    TouchInput = 1 << 1,
    Gamepad = 1 << 2,
    ImeComposition = 1 << 3,
    MultiWindow = 1 << 4,
    HapticFeedback = 1 << 5,
}

impl WidgetCapability {
    /// Every capability the ABI knows, for enumeration and diagnostics.
    pub const ALL: [WidgetCapability; 6] = [
        WidgetCapability::GpuShaders,
        WidgetCapability::TouchInput,
        WidgetCapability::Gamepad,
        WidgetCapability::ImeComposition,
        WidgetCapability::MultiWindow,
        WidgetCapability::HapticFeedback,
    ];
}

/// Bitmask-backed capability set representing the host runtime environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct HostCapabilities(pub u32);

impl HostCapabilities {
    pub fn none() -> Self {
        Self(0)
    }

    pub fn all_desktop() -> Self {
        Self(
            WidgetCapability::GpuShaders as u32
                | WidgetCapability::TouchInput as u32
                | WidgetCapability::Gamepad as u32
                | WidgetCapability::ImeComposition as u32
                | WidgetCapability::MultiWindow as u32
                | WidgetCapability::HapticFeedback as u32,
        )
    }

    pub fn mobile_default() -> Self {
        Self(
            WidgetCapability::GpuShaders as u32
                | WidgetCapability::TouchInput as u32
                | WidgetCapability::ImeComposition as u32
                | WidgetCapability::HapticFeedback as u32,
        )
    }

    pub fn headless_minimal() -> Self {
        Self(WidgetCapability::ImeComposition as u32)
    }

    pub fn has(&self, cap: WidgetCapability) -> bool {
        (self.0 & (cap as u32)) != 0
    }

    pub fn enable(&mut self, cap: WidgetCapability) {
        self.0 |= cap as u32;
    }

    pub fn disable(&mut self, cap: WidgetCapability) {
        self.0 &= !(cap as u32);
    }

    /// Union of two capability sets.
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Capabilities present in `self` but absent from `available`.
    pub fn missing(self, available: Self) -> Self {
        Self(self.0 & !available.0)
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Enumerate the set capabilities in stable ABI order.
    pub fn iter(&self) -> impl Iterator<Item = WidgetCapability> + '_ {
        WidgetCapability::ALL
            .into_iter()
            .filter(|cap| self.has(*cap))
    }
}

/// A widget's declared ABI requirement: the ABI version it was built against and
/// the host capabilities it needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidgetAbiRequirement {
    pub abi_version: (u32, u32, u32),
    pub required_capabilities: HostCapabilities,
}

impl WidgetAbiRequirement {
    pub fn new(abi_version: (u32, u32, u32)) -> Self {
        Self {
            abi_version,
            required_capabilities: HostCapabilities::none(),
        }
    }

    /// A requirement pinned to this crate's current ABI version.
    pub fn for_current_abi() -> Self {
        Self::new(WIDGET_ABI_VERSION)
    }

    pub fn with_capability(mut self, cap: WidgetCapability) -> Self {
        self.required_capabilities.enable(cap);
        self
    }
}

/// Typed result of host/widget capability negotiation.
///
/// Every rejection is one explicit variant; a caller can never observe a silent
/// fallback, and a missing capability is a value, not a panic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbiNegotiation {
    /// Host and widget agree; `granted` is the widget's required capability set.
    Compatible {
        host_version: (u32, u32, u32),
        granted: HostCapabilities,
    },
    /// The host ABI version cannot run the widget.
    VersionMismatch {
        required: (u32, u32, u32),
        host: (u32, u32, u32),
    },
    /// The host is the right version but lacks required capabilities.
    MissingCapabilities { missing: HostCapabilities },
}

impl AbiNegotiation {
    pub fn is_compatible(&self) -> bool {
        matches!(self, AbiNegotiation::Compatible { .. })
    }

    pub fn granted(&self) -> Option<HostCapabilities> {
        match self {
            AbiNegotiation::Compatible { granted, .. } => Some(*granted),
            _ => None,
        }
    }

    pub fn missing(&self) -> Option<HostCapabilities> {
        match self {
            AbiNegotiation::MissingCapabilities { missing } => Some(*missing),
            _ => None,
        }
    }
}

/// Negotiate a widget's ABI requirement against a host advertisement.
///
/// Ordering is deliberate: an incompatible ABI version is reported before any
/// capability check, so a version mismatch is never masked by a missing
/// capability (or vice versa).
pub fn negotiate(
    host_version: (u32, u32, u32),
    host_capabilities: HostCapabilities,
    requirement: WidgetAbiRequirement,
) -> AbiNegotiation {
    if !abi_versions_compatible(requirement.abi_version, host_version) {
        return AbiNegotiation::VersionMismatch {
            required: requirement.abi_version,
            host: host_version,
        };
    }
    let missing = requirement.required_capabilities.missing(host_capabilities);
    if !missing.is_empty() {
        return AbiNegotiation::MissingCapabilities { missing };
    }
    AbiNegotiation::Compatible {
        host_version,
        granted: requirement.required_capabilities,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_capabilities_flags() {
        let mut caps = HostCapabilities::none();
        assert!(!caps.has(WidgetCapability::GpuShaders));

        caps.enable(WidgetCapability::GpuShaders);
        assert!(caps.has(WidgetCapability::GpuShaders));
        assert!(!caps.has(WidgetCapability::TouchInput));

        caps.enable(WidgetCapability::TouchInput);
        assert!(caps.has(WidgetCapability::TouchInput));

        caps.disable(WidgetCapability::GpuShaders);
        assert!(!caps.has(WidgetCapability::GpuShaders));
        assert!(caps.has(WidgetCapability::TouchInput));

        let desktop = HostCapabilities::all_desktop();
        assert!(desktop.has(WidgetCapability::MultiWindow));
        assert!(desktop.has(WidgetCapability::Gamepad));

        let mobile = HostCapabilities::mobile_default();
        assert!(!mobile.has(WidgetCapability::MultiWindow));
        assert!(mobile.has(WidgetCapability::TouchInput));
    }

    #[test]
    fn test_compatible_negotiation() {
        let requirement = WidgetAbiRequirement::for_current_abi()
            .with_capability(WidgetCapability::GpuShaders)
            .with_capability(WidgetCapability::MultiWindow);
        let outcome = negotiate(
            WIDGET_ABI_VERSION,
            HostCapabilities::all_desktop(),
            requirement,
        );
        assert!(outcome.is_compatible());
        let granted = outcome.granted().expect("compatible grant");
        assert!(granted.has(WidgetCapability::GpuShaders));
        assert!(granted.has(WidgetCapability::MultiWindow));
        assert_eq!(outcome.missing(), None);
    }

    #[test]
    fn test_missing_capability_is_typed_not_panic() {
        let requirement =
            WidgetAbiRequirement::for_current_abi().with_capability(WidgetCapability::MultiWindow);
        let outcome = negotiate(
            WIDGET_ABI_VERSION,
            HostCapabilities::mobile_default(),
            requirement,
        );
        assert!(!outcome.is_compatible());
        assert_eq!(outcome.granted(), None);
        let missing = outcome.missing().expect("typed missing set");
        assert!(missing.has(WidgetCapability::MultiWindow));
        assert!(!missing.has(WidgetCapability::TouchInput));
    }

    #[test]
    fn test_version_mismatch_rejected() {
        let requirement = WidgetAbiRequirement::new((1, 0, 0));
        let outcome = negotiate(
            WIDGET_ABI_VERSION,
            HostCapabilities::all_desktop(),
            requirement,
        );
        assert_eq!(
            outcome,
            AbiNegotiation::VersionMismatch {
                required: (1, 0, 0),
                host: WIDGET_ABI_VERSION,
            }
        );
        assert!(!outcome.is_compatible());
    }

    #[test]
    fn test_runner_host_advertises_and_negotiates() {
        let host = DesktopRunnerHost;
        assert!(host.capabilities().has(WidgetCapability::MultiWindow));
        let outcome = host.negotiate(
            WidgetAbiRequirement::for_current_abi()
                .with_capability(WidgetCapability::HapticFeedback),
        );
        assert!(outcome.is_compatible());
    }

    #[test]
    fn test_abi_version_compatibility_rule() {
        assert!(abi_versions_compatible((0, 30, 0), (0, 30, 0)));
        assert!(abi_versions_compatible((0, 30, 0), (0, 31, 2)));
        assert!(!abi_versions_compatible((0, 30, 0), (0, 29, 9)));
        assert!(!abi_versions_compatible((0, 30, 0), (1, 30, 0)));
    }
}
