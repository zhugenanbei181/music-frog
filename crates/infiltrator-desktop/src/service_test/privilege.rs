//! Behavior cases for privilege.
//! test-intent: behavior

use super::*;

#[test]
fn test_privilege_level_display_and_elevation() {
    assert_eq!(PrivilegeLevel::Admin.to_string(), "Administrator");
    assert_eq!(PrivilegeLevel::Root.to_string(), "Root");
    assert_eq!(PrivilegeLevel::CapNetAdmin.to_string(), "cap_net_admin");
    assert_eq!(PrivilegeLevel::Unprivileged.to_string(), "Unprivileged");

    assert!(PrivilegeLevel::Admin.is_elevated());
    assert!(PrivilegeLevel::Root.is_elevated());
    assert!(PrivilegeLevel::CapNetAdmin.is_elevated());
    assert!(!PrivilegeLevel::Unprivileged.is_elevated());

    let detected = PrivilegeLevel::detect();
    let _ = detected.to_string();
}
