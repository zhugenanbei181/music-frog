//! Behavior cases for bandwidth.
//! test-intent: behavior

use super::*;

#[test]
fn test_bandwidth_conversion() {
    assert_eq!(
        Bandwidth::Text("100 Mbps".to_string()).to_bps(),
        Some(100_000_000)
    );
    assert_eq!(
        Bandwidth::Text("1 Gbps".to_string()).to_bps(),
        Some(1_000_000_000)
    );
    assert_eq!(
        Bandwidth::Text("500 kbps".to_string()).to_bps(),
        Some(500_000)
    );
    assert_eq!(Bandwidth::U64(1024).to_bps(), Some(1024));
}
