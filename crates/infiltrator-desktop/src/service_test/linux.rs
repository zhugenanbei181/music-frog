//! Behavior cases for linux.
//! test-intent: behavior

#[test]
fn test_linux_privilege_wizard_and_generators() {
    use crate::service::linux::LinuxPrivilegeWizard;
    use std::path::Path;

    // 1. Capability parser
    assert!(LinuxPrivilegeWizard::parse_getcap_output(
        "/usr/bin/mihomo = cap_net_admin,cap_net_bind_service+ep"
    ));
    assert!(LinuxPrivilegeWizard::parse_getcap_output(
        "/usr/bin/mihomo cap_net_bind_service,cap_net_admin=ep"
    ));
    assert!(!LinuxPrivilegeWizard::parse_getcap_output(
        "/usr/bin/mihomo = cap_sys_admin+ep"
    ));
    assert!(!LinuxPrivilegeWizard::parse_getcap_output(""));

    // 2. TUN device check
    let (tun_ok, tun_info) = LinuxPrivilegeWizard::check_tun_device();
    let _ = tun_ok;
    assert!(!tun_info.is_empty());

    // 3. Full diagnostic report
    let dummy_path = Path::new("/opt/musicfrog/mihomo");
    let report = LinuxPrivilegeWizard::diagnose(dummy_path);
    assert!(!report.checks.is_empty());
    let summary = report.summary();
    assert!(summary.contains("Linux Privilege Diagnostic"));

    // 4. Polkit action XML generation
    let polkit_xml = LinuxPrivilegeWizard::generate_polkit_action_xml(Some("/usr/sbin/setcap"));
    assert!(polkit_xml.contains("com.musicfrog.infiltrator.setcap"));
    assert!(polkit_xml.contains("/usr/sbin/setcap"));
    assert!(polkit_xml.contains("auth_admin_keep"));

    // 5. Polkit rule generation
    let polkit_rule = LinuxPrivilegeWizard::generate_polkit_rule();
    assert!(polkit_rule.contains("com.musicfrog.infiltrator.setcap"));
    assert!(polkit_rule.contains("wheel"));
    assert!(polkit_rule.contains("sudo"));

    // 6. Install / Uninstall script generation
    let install_sh = LinuxPrivilegeWizard::generate_install_script(dummy_path);
    assert!(install_sh.contains("#!/bin/bash"));
    assert!(install_sh.contains("cap_net_admin,cap_net_bind_service+ep"));
    assert!(install_sh.contains("/opt/musicfrog/mihomo"));

    let uninstall_sh = LinuxPrivilegeWizard::generate_uninstall_script(dummy_path);
    assert!(uninstall_sh.contains("setcap -r"));
    assert!(uninstall_sh.contains("/opt/musicfrog/mihomo"));

    // 7. Systemd service generation
    let systemd_unit = LinuxPrivilegeWizard::generate_systemd_service(
        "musicfrog-infiltrator.service",
        dummy_path,
        Some(Path::new("/etc/mihomo/config.yaml")),
        Some("musicfrog"),
    );
    assert!(systemd_unit.contains("Description=musicfrog-infiltrator.service"));
    assert!(systemd_unit.contains("AmbientCapabilities=CAP_NET_ADMIN CAP_NET_BIND_SERVICE"));
    assert!(systemd_unit.contains("CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_BIND_SERVICE"));
    assert!(systemd_unit.contains("User=musicfrog"));
    assert!(systemd_unit.contains("-f \"/etc/mihomo/config.yaml\""));
}
