//! Behavior cases for macos.
//! test-intent: behavior

use super::*;
use crate::tun_service::ServiceModeStatus;

#[test]
fn test_macos_privileged_helper_contract_and_xpc() {
    use crate::service::macos::{
        AUTH_RIGHT_PROXY_MANAGE, AUTH_RIGHT_TUN_MANAGE, MacHelperDoctor,
        MacPrivilegedHelperContract, MacPrivilegedHelperSpec, XpcMessage, XpcResponse,
    };

    let spec = MacPrivilegedHelperSpec::default();
    assert_eq!(spec.helper_bundle_id, "com.musicfrog.infiltrator.helper");
    assert_eq!(spec.app_bundle_id, "com.musicfrog.infiltrator");
    assert_eq!(
        spec.mach_service_name,
        "com.musicfrog.infiltrator.helper.xpc"
    );

    // 1. Designated Requirement Generator
    let dr = MacPrivilegedHelperContract::generate_designated_requirement(
        &spec.team_id,
        &spec.app_bundle_id,
    );
    assert!(dr.contains("com.musicfrog.infiltrator"));
    assert!(dr.contains(&spec.team_id));
    assert!(dr.contains("anchor apple generic"));

    // 2. Launchd Plist Generator
    let plist = MacPrivilegedHelperContract::generate_launchd_plist(&spec);
    assert!(plist.contains("<key>Label</key>"));
    assert!(plist.contains("com.musicfrog.infiltrator.helper"));
    assert!(plist.contains("com.musicfrog.infiltrator.helper.xpc"));
    assert!(plist.contains("/Library/PrivilegedHelperTools/com.musicfrog.infiltrator.helper"));

    // 3. Helper Info Plist & App Info Plist
    let helper_info = MacPrivilegedHelperContract::generate_helper_info_plist(&spec, &dr);
    assert!(helper_info.contains("SMAuthorizedClients"));
    assert!(helper_info.contains("com.musicfrog.infiltrator.helper"));

    let app_info = MacPrivilegedHelperContract::generate_app_info_plist(&spec, &dr);
    assert!(app_info.contains("SMPrivilegedExecutables"));
    assert!(app_info.contains("com.musicfrog.infiltrator"));

    // 4. Authorization Rights
    let rights = MacPrivilegedHelperContract::authorization_rights_spec();
    assert!(rights.contains_key(AUTH_RIGHT_TUN_MANAGE));
    assert!(rights.contains_key(AUTH_RIGHT_PROXY_MANAGE));

    // 5. XPC Message & Response
    let xpc_msg = XpcMessage::new(
        "msg-1",
        "com.musicfrog.infiltrator",
        ServiceCommand::StartTun {
            tun_interface: Some("utun3".to_string()),
            config_path: None,
        },
    );
    assert_eq!(xpc_msg.protocol_version, 1);
    assert_eq!(
        xpc_msg.required_right,
        Some(AUTH_RIGHT_TUN_MANAGE.to_string())
    );

    let json_xpc = serde_json::to_string(&xpc_msg).unwrap();
    let de_xpc: XpcMessage = serde_json::from_str(&json_xpc).unwrap();
    assert_eq!(xpc_msg, de_xpc);

    let xpc_resp = XpcResponse::ok(
        "msg-1",
        ServiceResponsePayload::TunStarted {
            interface_name: Some("utun3".to_string()),
        },
    );
    assert!(xpc_resp.success);
    assert_eq!(xpc_resp.in_reply_to, "msg-1");

    let json_resp = serde_json::to_string(&xpc_resp).unwrap();
    let de_resp: XpcResponse = serde_json::from_str(&json_resp).unwrap();
    assert_eq!(xpc_resp, de_resp);

    // 6. Helper Doctor
    let doctor_status = MacHelperDoctor::check_helper_status(&spec);
    assert_eq!(doctor_status, ServiceModeStatus::NotInstalled);

    let verify_res = MacHelperDoctor::verify_support("install_service");
    #[cfg(target_os = "macos")]
    assert!(verify_res.is_err());
    #[cfg(not(target_os = "macos"))]
    assert!(verify_res.is_ok());
}
