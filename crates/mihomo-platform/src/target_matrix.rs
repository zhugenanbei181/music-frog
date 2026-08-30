/// Target Matrix & ABI Artifact Inventory for mihomo-platform.

/// Represents the CPU architecture of a target platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformArch {
    X86_64,
    Aarch64,
    Armv7,
    Universal2,
}

/// Represents the Operating System of a target platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformOs {
    Windows,
    Linux,
    MacOS,
    Android,
}

/// Information regarding a target platform and its upstream artifact details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetTripleInfo {
    /// The Rust target triple (e.g., `x86_64-pc-windows-msvc`).
    pub triple: &'static str,
    /// The operating system.
    pub os: PlatformOs,
    /// The CPU architecture.
    pub arch: PlatformArch,
    /// The expected extracted binary name.
    pub binary_name: &'static str,
    /// The upstream release asset naming pattern (e.g., containing `{version}`).
    pub upstream_asset_pattern: &'static str,
    /// Whether the target supports running in service mode.
    pub is_service_mode_supported: bool,
}

const TARGETS: &[TargetTripleInfo] = &[
    TargetTripleInfo {
        triple: "x86_64-pc-windows-msvc",
        os: PlatformOs::Windows,
        arch: PlatformArch::X86_64,
        binary_name: "mihomo.exe",
        upstream_asset_pattern: "mihomo-windows-amd64-{version}.zip",
        is_service_mode_supported: true,
    },
    TargetTripleInfo {
        triple: "aarch64-pc-windows-msvc",
        os: PlatformOs::Windows,
        arch: PlatformArch::Aarch64,
        binary_name: "mihomo.exe",
        upstream_asset_pattern: "mihomo-windows-arm64-{version}.zip",
        is_service_mode_supported: true,
    },
    TargetTripleInfo {
        triple: "x86_64-unknown-linux-gnu",
        os: PlatformOs::Linux,
        arch: PlatformArch::X86_64,
        binary_name: "mihomo",
        upstream_asset_pattern: "mihomo-linux-amd64-{version}.gz",
        is_service_mode_supported: true,
    },
    TargetTripleInfo {
        triple: "x86_64-unknown-linux-musl",
        os: PlatformOs::Linux,
        arch: PlatformArch::X86_64,
        binary_name: "mihomo",
        upstream_asset_pattern: "mihomo-linux-amd64-compatible-{version}.gz",
        is_service_mode_supported: true,
    },
    TargetTripleInfo {
        triple: "aarch64-unknown-linux-gnu",
        os: PlatformOs::Linux,
        arch: PlatformArch::Aarch64,
        binary_name: "mihomo",
        upstream_asset_pattern: "mihomo-linux-arm64-{version}.gz",
        is_service_mode_supported: true,
    },
    TargetTripleInfo {
        triple: "x86_64-apple-darwin",
        os: PlatformOs::MacOS,
        arch: PlatformArch::X86_64,
        binary_name: "mihomo",
        upstream_asset_pattern: "mihomo-darwin-amd64-{version}.gz",
        is_service_mode_supported: true,
    },
    TargetTripleInfo {
        triple: "aarch64-apple-darwin",
        os: PlatformOs::MacOS,
        arch: PlatformArch::Aarch64,
        binary_name: "mihomo",
        upstream_asset_pattern: "mihomo-darwin-arm64-{version}.gz",
        is_service_mode_supported: true,
    },
    TargetTripleInfo {
        triple: "universal2-apple-darwin",
        os: PlatformOs::MacOS,
        arch: PlatformArch::Universal2,
        binary_name: "mihomo",
        upstream_asset_pattern: "mihomo-darwin-universal-{version}.gz",
        is_service_mode_supported: true,
    },
    TargetTripleInfo {
        triple: "aarch64-linux-android",
        os: PlatformOs::Android,
        arch: PlatformArch::Aarch64,
        binary_name: "mihomo",
        upstream_asset_pattern: "mihomo-android-arm64-{version}.gz",
        is_service_mode_supported: false,
    },
    TargetTripleInfo {
        triple: "x86_64-linux-android",
        os: PlatformOs::Android,
        arch: PlatformArch::X86_64,
        binary_name: "mihomo",
        upstream_asset_pattern: "mihomo-android-amd64-{version}.gz",
        is_service_mode_supported: false,
    },
    TargetTripleInfo {
        triple: "armv7-linux-androideabi",
        os: PlatformOs::Android,
        arch: PlatformArch::Armv7,
        binary_name: "mihomo",
        upstream_asset_pattern: "mihomo-android-armv7-{version}.gz",
        is_service_mode_supported: false,
    },
];

/// Returns a slice of all supported target triples and their metadata.
pub fn all_targets() -> &'static [TargetTripleInfo] {
    TARGETS
}

/// Finds target information by exact triple matching.
pub fn find_target_by_triple(triple: &str) -> Option<TargetTripleInfo> {
    TARGETS.iter().find(|t| t.triple == triple).cloned()
}

/// Generates the upstream release download filename for a given target triple and version string.
pub fn get_upstream_download_filename(triple: &str, version: &str) -> Option<String> {
    let target = find_target_by_triple(triple)?;
    Some(target.upstream_asset_pattern.replace("{version}", version))
}

/// Attempts to detect the current host target triple based on compile-time configuration.
pub fn detect_current_host_target() -> Option<TargetTripleInfo> {
    if cfg!(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc")) {
        find_target_by_triple("x86_64-pc-windows-msvc")
    } else if cfg!(all(target_os = "windows", target_arch = "aarch64", target_env = "msvc")) {
        find_target_by_triple("aarch64-pc-windows-msvc")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")) {
        find_target_by_triple("x86_64-unknown-linux-gnu")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64", target_env = "musl")) {
        find_target_by_triple("x86_64-unknown-linux-musl")
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64", target_env = "gnu")) {
        find_target_by_triple("aarch64-unknown-linux-gnu")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        find_target_by_triple("x86_64-apple-darwin")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        find_target_by_triple("aarch64-apple-darwin")
    } else if cfg!(all(target_os = "android", target_arch = "aarch64")) {
        find_target_by_triple("aarch64-linux-android")
    } else if cfg!(all(target_os = "android", target_arch = "x86_64")) {
        find_target_by_triple("x86_64-linux-android")
    } else if cfg!(all(target_os = "android", target_arch = "arm")) {
        find_target_by_triple("armv7-linux-androideabi")
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        find_target_by_triple("x86_64-pc-windows-msvc") // fallback
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        find_target_by_triple("x86_64-unknown-linux-gnu") // fallback
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_completeness() {
        let targets = all_targets();
        assert_eq!(targets.len(), 11);
        
        let win_msvc = find_target_by_triple("x86_64-pc-windows-msvc").unwrap();
        assert_eq!(win_msvc.os, PlatformOs::Windows);
        assert_eq!(win_msvc.arch, PlatformArch::X86_64);
        assert_eq!(win_msvc.binary_name, "mihomo.exe");
        assert!(win_msvc.is_service_mode_supported);
    }

    #[test]
    fn test_asset_filename_formatting() {
        let win_filename = get_upstream_download_filename("x86_64-pc-windows-msvc", "v1.18.3").unwrap();
        assert_eq!(win_filename, "mihomo-windows-amd64-v1.18.3.zip");
        
        let mac_filename = get_upstream_download_filename("aarch64-apple-darwin", "v1.18.3").unwrap();
        assert_eq!(mac_filename, "mihomo-darwin-arm64-v1.18.3.gz");

        let linux_filename = get_upstream_download_filename("x86_64-unknown-linux-gnu", "v1.18.3").unwrap();
        assert_eq!(linux_filename, "mihomo-linux-amd64-v1.18.3.gz");
    }

    #[test]
    fn test_host_target_detection() {
        // Will match the compile-time host environment this test runs on
        let host = detect_current_host_target();
        assert!(host.is_some(), "Current host target should be detectable");
        
        let host_info = host.unwrap();
        
        if cfg!(windows) {
            assert_eq!(host_info.os, PlatformOs::Windows);
            assert_eq!(host_info.binary_name, "mihomo.exe");
        } else if cfg!(target_os = "linux") {
            assert_eq!(host_info.os, PlatformOs::Linux);
            assert_eq!(host_info.binary_name, "mihomo");
        } else if cfg!(target_os = "macos") {
            assert_eq!(host_info.os, PlatformOs::MacOS);
            assert_eq!(host_info.binary_name, "mihomo");
        } else if cfg!(target_os = "android") {
            assert_eq!(host_info.os, PlatformOs::Android);
            assert_eq!(host_info.binary_name, "mihomo");
        }
    }
}
