//! Universal Widget Engine ABI contract and cross-project runner host interfaces.

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
}

/// Check if an incoming host ABI version is backward-compatible with this crate.
pub fn is_abi_compatible(host_version: (u32, u32, u32)) -> bool {
    let (major, minor, _) = WIDGET_ABI_VERSION;
    let (h_major, h_minor, _) = host_version;
    major == h_major && minor == h_minor
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
}

impl DesktopRunnerHost {
    pub fn capabilities(&self) -> HostCapabilities {
        HostCapabilities::all_desktop()
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
}
