use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ProfileSnapshot {
    pub current_profile: Option<String>,
    pub profiles_count: usize,
    pub has_subscription: bool,
    pub last_updated_epoch: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RuntimeSnapshot {
    pub is_running: bool,
    pub mode: String,
    pub mixed_port: u16,
    pub controller_port: u16,
    pub memory_bytes: u64,
    pub uptime_secs: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TunSnapshot {
    pub is_enabled: bool,
    pub mtu: u32,
    pub stack: String,
    pub allocated_ip: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DnsSnapshot {
    pub enhanced_mode: String,
    pub listen_port: u16,
    pub nameservers: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TrafficSnapshot {
    pub upload_rate_bps: u64,
    pub download_rate_bps: u64,
    pub total_upload_bytes: u64,
    pub total_download_bytes: u64,
    pub active_connections: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VersionSnapshot {
    pub version: String,
    pub channel: String,
    pub is_prerelease: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AppDomainSnapshot {
    pub profile: ProfileSnapshot,
    pub runtime: RuntimeSnapshot,
    pub tun: TunSnapshot,
    pub dns: DnsSnapshot,
    pub traffic: TrafficSnapshot,
    pub version: VersionSnapshot,
    pub timestamp_epoch_ms: i64,
}

pub struct DomainSnapshotBuilder {
    profile: Option<ProfileSnapshot>,
    runtime: Option<RuntimeSnapshot>,
    tun: Option<TunSnapshot>,
    dns: Option<DnsSnapshot>,
    traffic: Option<TrafficSnapshot>,
    version: Option<VersionSnapshot>,
    timestamp_epoch_ms: Option<i64>,
}

impl Default for DomainSnapshotBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DomainSnapshotBuilder {
    pub fn new() -> Self {
        Self {
            profile: None,
            runtime: None,
            tun: None,
            dns: None,
            traffic: None,
            version: None,
            timestamp_epoch_ms: None,
        }
    }

    pub fn profile(mut self, profile: ProfileSnapshot) -> Self {
        self.profile = Some(profile);
        self
    }

    pub fn runtime(mut self, runtime: RuntimeSnapshot) -> Self {
        self.runtime = Some(runtime);
        self
    }

    pub fn tun(mut self, tun: TunSnapshot) -> Self {
        self.tun = Some(tun);
        self
    }

    pub fn dns(mut self, dns: DnsSnapshot) -> Self {
        self.dns = Some(dns);
        self
    }

    pub fn traffic(mut self, traffic: TrafficSnapshot) -> Self {
        self.traffic = Some(traffic);
        self
    }

    pub fn version(mut self, version: VersionSnapshot) -> Self {
        self.version = Some(version);
        self
    }

    pub fn timestamp_epoch_ms(mut self, timestamp: i64) -> Self {
        self.timestamp_epoch_ms = Some(timestamp);
        self
    }

    pub fn build(self) -> AppDomainSnapshot {
        AppDomainSnapshot {
            profile: self.profile.expect("profile must be set"),
            runtime: self.runtime.expect("runtime must be set"),
            tun: self.tun.expect("tun must be set"),
            dns: self.dns.expect("dns must be set"),
            traffic: self.traffic.expect("traffic must be set"),
            version: self.version.expect("version must be set"),
            timestamp_epoch_ms: self.timestamp_epoch_ms.expect("timestamp must be set"),
        }
    }
}

pub fn sample_snapshot() -> AppDomainSnapshot {
    DomainSnapshotBuilder::new()
        .profile(ProfileSnapshot {
            current_profile: Some("main".to_string()),
            profiles_count: 3,
            has_subscription: true,
            last_updated_epoch: Some(1693440000),
        })
        .runtime(RuntimeSnapshot {
            is_running: true,
            mode: "rule".to_string(),
            mixed_port: 7890,
            controller_port: 9090,
            memory_bytes: 104857600,
            uptime_secs: 3600,
        })
        .tun(TunSnapshot {
            is_enabled: true,
            mtu: 1500,
            stack: "gvisor".to_string(),
            allocated_ip: Some("198.18.0.1".to_string()),
        })
        .dns(DnsSnapshot {
            enhanced_mode: "fake-ip".to_string(),
            listen_port: 1053,
            nameservers: vec!["8.8.8.8".to_string(), "1.1.1.1".to_string()],
        })
        .traffic(TrafficSnapshot {
            upload_rate_bps: 1024,
            download_rate_bps: 2048,
            total_upload_bytes: 1024000,
            total_download_bytes: 2048000,
            active_connections: 42,
        })
        .version(VersionSnapshot {
            version: "v1.2.3".to_string(),
            channel: "stable".to_string(),
            is_prerelease: false,
        })
        .timestamp_epoch_ms(1693443600000)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_snapshot_creation() {
        let snapshot = sample_snapshot();
        assert_eq!(snapshot.profile.profiles_count, 3);
        assert_eq!(snapshot.runtime.mode, "rule");
        assert!(snapshot.tun.is_enabled);
        assert_eq!(snapshot.dns.enhanced_mode, "fake-ip");
        assert_eq!(snapshot.traffic.active_connections, 42);
        assert_eq!(snapshot.version.version, "v1.2.3");
        assert_eq!(snapshot.timestamp_epoch_ms, 1693443600000);
    }

    #[test]
    fn test_json_roundtrip() {
        let original = sample_snapshot();
        let json = serde_json::to_string(&original).expect("Failed to serialize");
        let deserialized: AppDomainSnapshot = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(original, deserialized);
    }

    #[test]
    #[should_panic(expected = "profile must be set")]
    fn test_builder_missing_field() {
        let _ = DomainSnapshotBuilder::new()
            .timestamp_epoch_ms(12345)
            .build();
    }
}
