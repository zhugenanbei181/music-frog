//! Behavior cases for port.
//! test-intent: behavior

use super::*;

#[test]
fn test_port_hopping_parser() {
    let hopping = PortHopping::parse("20000-30000, 8443, 443, 50000-50005").unwrap();
    assert_eq!(hopping.specs.len(), 4);
    assert!(hopping.contains(8443));
    assert!(hopping.contains(25000));
    assert!(hopping.contains(50003));
    assert!(!hopping.contains(80));
    assert_eq!(hopping.total_ports(), 10001 + 1 + 1 + 6);
    assert_eq!(
        hopping.to_canonical_string(),
        "20000-30000,8443,443,50000-50005"
    );

    assert!(PortHopping::parse("30000-20000").is_err());
    assert!(PortHopping::parse("0").is_err());
    assert!(PortHopping::parse("").is_err());
}
