//! Behavior cases for ipc.
//! test-intent: behavior

use super::*;

#[test]
fn test_ipc_endpoint_properties() {
    let pipe = IpcEndpoint::from_named_pipe(r"\\.\pipe\custom-pipe");
    assert_eq!(pipe.display_target(), r"\\.\pipe\custom-pipe");
    assert!(pipe.is_available());

    let temp_dir = TempDir::new().unwrap();
    let non_existent_sock = temp_dir.path().join("missing.sock");
    let unix_ep = IpcEndpoint::from_unix_path(&non_existent_sock);
    assert_eq!(
        unix_ep.display_target(),
        non_existent_sock.to_string_lossy()
    );
    assert!(!unix_ep.is_available());

    let default_ep = IpcEndpoint::default_for_platform();
    let _ = default_ep.display_target();
}
