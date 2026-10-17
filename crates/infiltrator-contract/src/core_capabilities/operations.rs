use super::*;

impl CoreOperation {
    pub const fn all() -> &'static [Self] {
        CORE_OPERATIONS
    }

    pub const fn version_requirement(self) -> CoreVersionRequirement {
        match self {
            Self::Version(VersionOperation::Inspect)
            | Self::Version(VersionOperation::Install)
            | Self::Version(VersionOperation::Activate)
            | Self::Version(VersionOperation::Uninstall)
            | Self::Version(VersionOperation::Rollback)
            | Self::Lifecycle(CoreLifecycleOperation::Start)
            | Self::Lifecycle(CoreLifecycleOperation::Stop)
            | Self::Lifecycle(CoreLifecycleOperation::Restart) => {
                CoreVersionRequirement::IndependentOfActiveVersion
            }
            Self::Profile(_)
            | Self::Dns(_)
            | Self::Rules(_)
            | Self::Subscription(_)
            | Self::Provider(_)
            | Self::Outbound(_)
            | Self::Connection(_)
            | Self::Logs(_)
            | Self::Geo(_)
            | Self::FakeIpCache(_)
            | Self::Tun(_)
            | Self::ProxyControl(ProxyControlOperation::GetRoutingIntent)
            | Self::ProxyControl(ProxyControlOperation::SetRoutingIntent)
            | Self::CoreGarbageCollection(_) => CoreVersionRequirement::RequiresActiveVersion,
        }
    }

    /// The only operations that may be authorized by a shared lifecycle or
    /// version-management owner instead of engine API evidence.
    pub const fn owner_managed_owner(self) -> Option<AdapterOperationOwner> {
        match self {
            Self::Lifecycle(_) => Some(AdapterOperationOwner::CoreLifecycle),
            Self::Version(_) => Some(AdapterOperationOwner::VersionManagement),
            _ => None,
        }
    }
}

impl EngineBinaryTarget {
    pub fn from_target_triple(value: &str) -> Option<Self> {
        match value {
            "x86_64-unknown-linux-gnu" => Some(Self::X86_64UnknownLinuxGnu),
            "aarch64-unknown-linux-gnu" => Some(Self::Aarch64UnknownLinuxGnu),
            "x86_64-unknown-linux-musl" => Some(Self::X86_64UnknownLinuxMusl),
            "aarch64-unknown-linux-musl" => Some(Self::Aarch64UnknownLinuxMusl),
            "x86_64-pc-windows-msvc" => Some(Self::X86_64PcWindowsMsvc),
            "aarch64-pc-windows-msvc" => Some(Self::Aarch64PcWindowsMsvc),
            "x86_64-pc-windows-gnu" => Some(Self::X86_64PcWindowsGnu),
            "x86_64-pc-windows-gnullvm" => Some(Self::X86_64PcWindowsGnullvm),
            "aarch64-pc-windows-gnullvm" => Some(Self::Aarch64PcWindowsGnullvm),
            "x86_64-apple-darwin" => Some(Self::X86_64AppleDarwin),
            "aarch64-apple-darwin" => Some(Self::Aarch64AppleDarwin),
            "x86_64-linux-android" => Some(Self::X86_64LinuxAndroid),
            "aarch64-linux-android" => Some(Self::Aarch64LinuxAndroid),
            "x86_64-apple-ios" => Some(Self::X86_64AppleIos),
            "aarch64-apple-ios" => Some(Self::Aarch64AppleIos),
            "aarch64-apple-ios-sim" => Some(Self::Aarch64AppleIosSim),
            "aarch64-apple-ios-macabi" => Some(Self::Aarch64AppleIosMacabi),
            _ => None,
        }
    }

    pub fn current() -> Self {
        Self::from_target_triple(env!("INFILTRATOR_BUILD_TARGET")).unwrap_or(Self::Unregistered)
    }

    pub const fn target_triple(self) -> Option<&'static str> {
        match self {
            Self::X86_64UnknownLinuxGnu => Some("x86_64-unknown-linux-gnu"),
            Self::Aarch64UnknownLinuxGnu => Some("aarch64-unknown-linux-gnu"),
            Self::X86_64UnknownLinuxMusl => Some("x86_64-unknown-linux-musl"),
            Self::Aarch64UnknownLinuxMusl => Some("aarch64-unknown-linux-musl"),
            Self::X86_64PcWindowsMsvc => Some("x86_64-pc-windows-msvc"),
            Self::Aarch64PcWindowsMsvc => Some("aarch64-pc-windows-msvc"),
            Self::X86_64PcWindowsGnu => Some("x86_64-pc-windows-gnu"),
            Self::X86_64PcWindowsGnullvm => Some("x86_64-pc-windows-gnullvm"),
            Self::Aarch64PcWindowsGnullvm => Some("aarch64-pc-windows-gnullvm"),
            Self::X86_64AppleDarwin => Some("x86_64-apple-darwin"),
            Self::Aarch64AppleDarwin => Some("aarch64-apple-darwin"),
            Self::X86_64LinuxAndroid => Some("x86_64-linux-android"),
            Self::Aarch64LinuxAndroid => Some("aarch64-linux-android"),
            Self::X86_64AppleIos => Some("x86_64-apple-ios"),
            Self::Aarch64AppleIos => Some("aarch64-apple-ios"),
            Self::Aarch64AppleIosSim => Some("aarch64-apple-ios-sim"),
            Self::Aarch64AppleIosMacabi => Some("aarch64-apple-ios-macabi"),
            Self::Unregistered => None,
        }
    }

    pub const fn host_kind(self) -> Option<HostKind> {
        match self {
            Self::X86_64UnknownLinuxGnu
            | Self::Aarch64UnknownLinuxGnu
            | Self::X86_64UnknownLinuxMusl
            | Self::Aarch64UnknownLinuxMusl
            | Self::X86_64PcWindowsMsvc
            | Self::Aarch64PcWindowsMsvc
            | Self::X86_64PcWindowsGnu
            | Self::X86_64PcWindowsGnullvm
            | Self::Aarch64PcWindowsGnullvm
            | Self::X86_64AppleDarwin
            | Self::Aarch64AppleDarwin => Some(HostKind::Desktop),
            Self::X86_64LinuxAndroid | Self::Aarch64LinuxAndroid => Some(HostKind::Android),
            Self::X86_64AppleIos
            | Self::Aarch64AppleIos
            | Self::Aarch64AppleIosSim
            | Self::Aarch64AppleIosMacabi => Some(HostKind::Ios),
            Self::Unregistered => None,
        }
    }
}
