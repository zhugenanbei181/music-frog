//! Behavior cases for windows.
//! test-intent: behavior

#[test]
fn test_windows_service_manager_args_and_sddl() {
    use crate::service::windows::{
        NamedPipeSecurity, WindowsServiceConfig, WindowsServiceManager, WindowsServiceStartType,
        WindowsServiceStatus,
    };

    let config = WindowsServiceConfig::new(r"C:\Program Files\MusicFrog\infiltrator.exe")
        .with_service_name("TestCustomService")
        .with_pipe_name(r"\\.\pipe\test-custom-pipe");

    let manager = WindowsServiceManager::new(config);
    assert_eq!(manager.config().service_name, "TestCustomService");

    // 1. Create args
    let create_args = manager.build_create_args();
    assert_eq!(create_args[0], "create");
    assert_eq!(create_args[1], "TestCustomService");
    assert!(create_args[2].contains(r"C:\Program Files\MusicFrog\infiltrator.exe"));
    assert_eq!(create_args[4], "start=auto");

    // 2. Delete / Start / Stop / Query args
    assert_eq!(
        manager.build_delete_args(),
        vec!["delete", "TestCustomService"]
    );
    assert_eq!(
        manager.build_start_args(),
        vec!["start", "TestCustomService"]
    );
    assert_eq!(manager.build_stop_args(), vec!["stop", "TestCustomService"]);
    assert_eq!(
        manager.build_query_args(),
        vec!["query", "TestCustomService"]
    );

    // 3. SDDL Generation
    let sddl_auth = NamedPipeSecurity::generate_sddl(true);
    assert!(sddl_auth.contains("AU"));
    assert!(sddl_auth.contains("SY"));
    assert!(sddl_auth.contains("BA"));

    let sddl_admin_only = NamedPipeSecurity::generate_sddl(false);
    assert!(!sddl_admin_only.contains("AU"));
    assert!(sddl_admin_only.contains("SY"));

    // 4. Pipe Name Validation
    assert!(NamedPipeSecurity::is_valid_pipe_name(
        r"\\.\pipe\musicfrog-infiltrator-service"
    ));
    assert!(NamedPipeSecurity::is_valid_pipe_name(
        r"\\.\pipe\test_pipe_123"
    ));
    assert!(!NamedPipeSecurity::is_valid_pipe_name(
        r"/var/run/test.sock"
    ));
    assert!(!NamedPipeSecurity::is_valid_pipe_name(
        r"\\.\pipe\nested\pipe"
    ));
    assert!(!NamedPipeSecurity::is_valid_pipe_name(r"\\.\pipe\"));

    // 5. sc.exe Query Parser
    assert_eq!(
        WindowsServiceManager::parse_sc_query_output(
            "[SC] EnumQueryServicesStatus:OpenService FAILED 1060: The specified service does not exist"
        ),
        WindowsServiceStatus::NotInstalled
    );
    assert_eq!(
        WindowsServiceManager::parse_sc_query_output("        STATE              : 4  RUNNING \n"),
        WindowsServiceStatus::Running
    );
    assert_eq!(
        WindowsServiceManager::parse_sc_query_output("        STATE              : 1  STOPPED \n"),
        WindowsServiceStatus::Stopped
    );
    assert_eq!(
        WindowsServiceManager::parse_sc_query_output(
            "        STATE              : 2  START_PENDING \n"
        ),
        WindowsServiceStatus::StartPending
    );
    assert_eq!(
        WindowsServiceManager::parse_sc_query_output(
            "        STATE              : 3  STOP_PENDING \n"
        ),
        WindowsServiceStatus::StopPending
    );
    assert_eq!(
        WindowsServiceManager::parse_sc_query_output("        STATE              : 7  PAUSED \n"),
        WindowsServiceStatus::Paused
    );
    assert_eq!(
        WindowsServiceManager::parse_sc_query_output(
            "        STATE              : 6  PAUSE_PENDING \n"
        ),
        WindowsServiceStatus::PausePending
    );
    assert_eq!(
        WindowsServiceManager::parse_sc_query_output(
            "        STATE              : 5  CONTINUE_PENDING \n"
        ),
        WindowsServiceStatus::ContinuePending
    );

    let unknown = WindowsServiceManager::parse_sc_query_output("SOMETHING UNEXPECTED");
    assert!(matches!(unknown, WindowsServiceStatus::Unknown(_)));

    assert!(WindowsServiceStatus::Running.is_running());
    assert!(!WindowsServiceStatus::Stopped.is_running());
    assert!(WindowsServiceStatus::Stopped.is_installed());
    assert!(!WindowsServiceStatus::NotInstalled.is_installed());

    assert_eq!(WindowsServiceStartType::Auto.as_sc_arg(), "auto");
    assert_eq!(WindowsServiceStartType::Demand.as_sc_arg(), "demand");
    assert_eq!(WindowsServiceStartType::Disabled.as_sc_arg(), "disabled");

    assert_eq!(WindowsServiceStatus::Running.to_string(), "Running");
    assert_eq!(WindowsServiceStatus::Stopped.to_string(), "Stopped");
    assert_eq!(
        WindowsServiceStatus::NotInstalled.to_string(),
        "Not Installed"
    );
    assert_eq!(
        WindowsServiceStatus::StartPending.to_string(),
        "Start Pending"
    );
    assert_eq!(
        WindowsServiceStatus::StopPending.to_string(),
        "Stop Pending"
    );
    assert_eq!(WindowsServiceStatus::Paused.to_string(), "Paused");
    assert_eq!(
        WindowsServiceStatus::PausePending.to_string(),
        "Pause Pending"
    );
    assert_eq!(
        WindowsServiceStatus::ContinuePending.to_string(),
        "Continue Pending"
    );
}
