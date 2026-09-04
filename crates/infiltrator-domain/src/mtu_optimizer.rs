use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum IpVersion {
    IPv4,
    IPv6,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MtuConfig {
    pub base_mtu: u16,
    pub auto_mss: bool,
    pub ip_version: IpVersion,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MssResult {
    pub calculated_mss: u16,
    pub header_overhead: u16,
    pub clamped: bool,
}

pub struct MtuOptimizer;

impl MtuOptimizer {
    /// Calculate TCP MSS based on MTU and IP version
    pub fn calculate_tcp_mss(mtu: u16, version: IpVersion) -> MssResult {
        let (header_overhead, min_mss) = match version {
            IpVersion::IPv4 => (40, 536),
            IpVersion::IPv6 => (60, 1220),
        };

        let raw_mss = mtu.saturating_sub(header_overhead);
        let clamped = raw_mss < min_mss;
        let calculated_mss = if clamped { min_mss } else { raw_mss };

        MssResult {
            calculated_mss,
            header_overhead,
            clamped,
        }
    }

    /// Recommend TUN MTU deducting encapsulation overhead
    pub fn recommend_tun_mtu(uplink_mtu: u16, encapsulation_overhead: u16) -> u16 {
        let recommended = uplink_mtu.saturating_sub(encapsulation_overhead);
        recommended.clamp(1280, 9000)
    }

    /// Validate if MTU is within typical range [1280, 9000]
    pub fn validate_mtu_range(mtu: u16) -> Result<(), anyhow::Error> {
        if (1280..=9000).contains(&mtu) {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "MTU {} is out of valid range [1280, 9000]",
                mtu
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_tcp_mss_ipv4() {
        let res = MtuOptimizer::calculate_tcp_mss(1500, IpVersion::IPv4);
        assert_eq!(res.calculated_mss, 1460);
        assert_eq!(res.header_overhead, 40);
        assert!(!res.clamped);

        let res = MtuOptimizer::calculate_tcp_mss(500, IpVersion::IPv4);
        assert_eq!(res.calculated_mss, 536);
        assert!(res.clamped);
    }

    #[test]
    fn test_calculate_tcp_mss_ipv6() {
        let res = MtuOptimizer::calculate_tcp_mss(1500, IpVersion::IPv6);
        assert_eq!(res.calculated_mss, 1440);
        assert_eq!(res.header_overhead, 60);
        assert!(!res.clamped);

        let res = MtuOptimizer::calculate_tcp_mss(1200, IpVersion::IPv6);
        assert_eq!(res.calculated_mss, 1220);
        assert!(res.clamped);
    }

    #[test]
    fn test_recommend_tun_mtu() {
        assert_eq!(MtuOptimizer::recommend_tun_mtu(1500, 50), 1450);
        assert_eq!(MtuOptimizer::recommend_tun_mtu(1300, 50), 1280);
        assert_eq!(MtuOptimizer::recommend_tun_mtu(9500, 100), 9000);
    }

    #[test]
    fn test_validate_mtu_range() {
        assert!(MtuOptimizer::validate_mtu_range(1500).is_ok());
        assert!(MtuOptimizer::validate_mtu_range(1280).is_ok());
        assert!(MtuOptimizer::validate_mtu_range(9000).is_ok());
        assert!(MtuOptimizer::validate_mtu_range(1279).is_err());
        assert!(MtuOptimizer::validate_mtu_range(9001).is_err());
    }
}
