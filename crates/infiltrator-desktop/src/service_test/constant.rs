//! Behavior cases for constant.
//! test-intent: behavior

use super::*;

#[test]
fn test_constant_time_eq() {
    assert!(constant_time_eq(b"secret_key_123", b"secret_key_123"));
    assert!(!constant_time_eq(b"secret_key_123", b"secret_key_124"));
    assert!(!constant_time_eq(b"secret", b"secret_longer"));
    assert!(constant_time_eq(b"", b""));
}
