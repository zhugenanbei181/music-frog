use super::*;
use std::path::PathBuf;

#[test]
fn test_normalize_display_name() {
    assert_eq!(
        ProcessEnumerator::normalize_display_name("chrome.exe"),
        "Google Chrome"
    );
    assert_eq!(
        ProcessEnumerator::normalize_display_name("discord"),
        "Discord"
    );
    assert_eq!(
        ProcessEnumerator::normalize_display_name("firefox.exe"),
        "Mozilla Firefox"
    );
    assert_eq!(
        ProcessEnumerator::normalize_display_name("code.exe"),
        "Visual Studio Code"
    );
    assert_eq!(
        ProcessEnumerator::normalize_display_name("spotify"),
        "Spotify"
    );
    assert_eq!(
        ProcessEnumerator::normalize_display_name("steam.exe"),
        "Steam"
    );
    assert_eq!(
        ProcessEnumerator::normalize_display_name("mihomo.exe"),
        "Mihomo Core"
    );
    assert_eq!(
        ProcessEnumerator::normalize_display_name("custom_app.exe"),
        "custom_app"
    );
}

#[test]
fn test_is_system_process() {
    assert!(is_system_process("any", None, 0));
    assert!(is_system_process("svchost.exe", None, 1234));
    assert!(is_system_process("csrss.exe", None, 456));
    assert!(is_system_process("services.exe", None, 789));
    assert!(is_system_process("lsass.exe", None, 800));

    assert!(is_system_process("systemd", None, 1));
    assert!(is_system_process("kthreadd", None, 2));
    assert!(is_system_process("[kworker/0:1H]", None, 100));
    assert!(is_system_process("dbus-daemon", None, 500));
    assert!(is_system_process("systemd-resolved", None, 600));

    assert!(is_system_process("kernel_task", None, 0));
    assert!(is_system_process("launchd", None, 1));
    assert!(is_system_process("windowserver", None, 300));

    assert!(is_system_process(
        "custom_daemon",
        Some("/usr/lib/systemd/custom_daemon"),
        1000
    ));
    assert!(is_system_process(
        "daemon",
        Some("/usr/libexec/daemon"),
        1001
    ));

    assert!(!is_system_process("chrome.exe", None, 5000));
    assert!(!is_system_process("discord", None, 5001));
    assert!(!is_system_process("spotify.exe", None, 5002));
    assert!(!is_system_process("code", None, 5003));
    assert!(!is_system_process("steam.exe", None, 5004));
}

#[test]
fn test_classify_process_category() {
    assert_eq!(
        classify_process_category("chrome.exe", None, false),
        ProcessCategory::Browser
    );
    assert_eq!(
        classify_process_category("firefox", None, false),
        ProcessCategory::Browser
    );
    assert_eq!(
        classify_process_category("discord.exe", None, false),
        ProcessCategory::Communication
    );
    assert_eq!(
        classify_process_category("telegram", None, false),
        ProcessCategory::Communication
    );
    assert_eq!(
        classify_process_category("code.exe", None, false),
        ProcessCategory::Developer
    );
    assert_eq!(
        classify_process_category("idea64.exe", None, false),
        ProcessCategory::Developer
    );
    assert_eq!(
        classify_process_category("steam.exe", None, false),
        ProcessCategory::Gaming
    );
    assert_eq!(
        classify_process_category("spotify.exe", None, false),
        ProcessCategory::Media
    );
    assert_eq!(
        classify_process_category("obsidian", None, false),
        ProcessCategory::Office
    );
    assert_eq!(
        classify_process_category("mihomo.exe", None, false),
        ProcessCategory::NetworkVpn
    );
    assert_eq!(
        classify_process_category("svchost.exe", None, true),
        ProcessCategory::SystemDaemon
    );
    assert_eq!(
        classify_process_category("random_binary", None, false),
        ProcessCategory::Other
    );
}

#[test]
fn test_resolve_icon_hint() {
    assert_eq!(
        resolve_icon_hint("chrome.exe", None),
        Some("google-chrome".to_string())
    );
    assert_eq!(
        resolve_icon_hint("firefox", None),
        Some("firefox".to_string())
    );
    assert_eq!(
        resolve_icon_hint("discord", None),
        Some("discord".to_string())
    );
    assert_eq!(
        resolve_icon_hint("code", None),
        Some("visual-studio-code".to_string())
    );
    assert_eq!(
        resolve_icon_hint("spotify.exe", None),
        Some("spotify".to_string())
    );
    assert_eq!(resolve_icon_hint("vlc.exe", None), Some("vlc".to_string()));
    assert_eq!(
        resolve_icon_hint("my_custom_tool.exe", Some("/opt/bin/my_custom_tool.exe")),
        Some("my_custom_tool".to_string())
    );
}

#[test]
fn test_deduplicate_processes() {
    let procs = vec![
        ProcessInfo {
            pid: 1020,
            name: "chrome.exe".to_string(),
            binary_path: Some("/opt/google/chrome/chrome".to_string()),
            is_system: false,
            icon_hint: Some("google-chrome".to_string()),
        },
        ProcessInfo {
            pid: 1050,
            name: "chrome.exe".to_string(),
            binary_path: None,
            is_system: false,
            icon_hint: None,
        },
        ProcessInfo {
            pid: 1010,
            name: "chrome.exe".to_string(),
            binary_path: None,
            is_system: false,
            icon_hint: None,
        },
        ProcessInfo {
            pid: 500,
            name: "svchost.exe".to_string(),
            binary_path: Some("C:\\Windows\\System32\\svchost.exe".to_string()),
            is_system: true,
            icon_hint: None,
        },
        ProcessInfo {
            pid: 501,
            name: "svchost.exe".to_string(),
            binary_path: Some("C:\\Windows\\System32\\svchost.exe".to_string()),
            is_system: true,
            icon_hint: None,
        },
    ];

    let deduped = deduplicate_processes(procs);
    assert_eq!(deduped.len(), 2);

    let chrome = deduped.iter().find(|p| p.name == "chrome.exe").unwrap();
    assert_eq!(chrome.pid, 1010);
    assert_eq!(
        chrome.binary_path.as_deref(),
        Some("/opt/google/chrome/chrome")
    );
    assert_eq!(chrome.icon_hint.as_deref(), Some("google-chrome"));
    assert!(!chrome.is_system);

    let svchost = deduped.iter().find(|p| p.name == "svchost.exe").unwrap();
    assert_eq!(svchost.pid, 500);
    assert!(svchost.is_system);
}

#[test]
fn test_process_conversions() {
    let info = ProcessInfo {
        pid: 4321,
        name: "firefox.exe".to_string(),
        binary_path: Some("/usr/bin/firefox".to_string()),
        is_system: false,
        icon_hint: Some("firefox".to_string()),
    };

    let item: ProcessItem = info.clone().into();
    assert_eq!(item.pid, 4321);
    assert_eq!(item.name, "firefox.exe");
    assert_eq!(item.display_name, "Mozilla Firefox");
    assert_eq!(item.exe_path, Some(PathBuf::from("/usr/bin/firefox")));

    let roundtrip: ProcessInfo = item.into();
    assert_eq!(roundtrip.pid, info.pid);
    assert_eq!(roundtrip.name, info.name);
    assert_eq!(roundtrip.binary_path, info.binary_path);
    assert!(!roundtrip.is_system);
}

#[test]
fn test_process_hierarchy_tree() {
    let procs = vec![
        ExtendedProcessInfo {
            pid: 1000,
            ppid: Some(1),
            name: "chrome".to_string(),
            display_name: "Google Chrome".to_string(),
            canonical_name: "chrome".to_string(),
            binary_path: Some("/usr/bin/chrome".to_string()),
            is_system: false,
            category: ProcessCategory::Browser,
            icon_hint: Some("google-chrome".to_string()),
            memory_bytes: 100_000_000,
            total_memory_bytes: 100_000_000,
            child_pids: Vec::new(),
        },
        ExtendedProcessInfo {
            pid: 1001,
            ppid: Some(1000),
            name: "chrome".to_string(),
            display_name: "Google Chrome".to_string(),
            canonical_name: "chrome".to_string(),
            binary_path: Some("/usr/bin/chrome".to_string()),
            is_system: false,
            category: ProcessCategory::Browser,
            icon_hint: Some("google-chrome".to_string()),
            memory_bytes: 50_000_000,
            total_memory_bytes: 50_000_000,
            child_pids: Vec::new(),
        },
        ExtendedProcessInfo {
            pid: 1002,
            ppid: Some(1000),
            name: "chrome".to_string(),
            display_name: "Google Chrome".to_string(),
            canonical_name: "chrome".to_string(),
            binary_path: Some("/usr/bin/chrome".to_string()),
            is_system: false,
            category: ProcessCategory::Browser,
            icon_hint: Some("google-chrome".to_string()),
            memory_bytes: 30_000_000,
            total_memory_bytes: 30_000_000,
            child_pids: Vec::new(),
        },
        ExtendedProcessInfo {
            pid: 50,
            ppid: Some(0),
            name: "systemd-journald".to_string(),
            display_name: "systemd-journald".to_string(),
            canonical_name: "systemd-journald".to_string(),
            binary_path: Some("/usr/lib/systemd/systemd-journald".to_string()),
            is_system: true,
            category: ProcessCategory::SystemDaemon,
            icon_hint: None,
            memory_bytes: 10_000_000,
            total_memory_bytes: 10_000_000,
            child_pids: Vec::new(),
        },
    ];

    let tree = ProcessHierarchyTree::from_processes(procs);
    assert_eq!(tree.total_process_count(), 4);
    assert_eq!(tree.roots().len(), 2);

    let chrome_root = tree.get_by_pid(1000).unwrap();
    assert_eq!(chrome_root.child_pids.len(), 2);
    assert_eq!(chrome_root.total_memory_bytes, 180_000_000);

    let user_apps = tree.user_applications();
    assert_eq!(user_apps.len(), 1);
    assert_eq!(user_apps[0].pid, 1000);
}

#[test]
fn test_desktop_entry_parsing() {
    let sample = r#"
[Desktop Entry]
Name=Visual Studio Code
GenericName=Text Editor
Comment=Code Editing. Redefined.
Exec=/usr/share/code/code --unity-launch %F
Icon=vscode
Type=Application
Categories=Development;IDE;
"#;
    let entry = DesktopEntryScanner::parse_desktop_file(sample).unwrap();
    assert_eq!(entry.name, "Visual Studio Code");
    assert_eq!(entry.generic_name.as_deref(), Some("Text Editor"));
    assert_eq!(entry.exec, "/usr/share/code/code");
    assert_eq!(entry.icon.as_deref(), Some("vscode"));
    assert!(entry.categories.contains(&"Development".to_string()));
    assert!(entry.categories.contains(&"IDE".to_string()));
}

#[test]
fn test_process_filter() {
    let procs = vec![
        ExtendedProcessInfo {
            pid: 1,
            ppid: None,
            name: "chrome.exe".to_string(),
            display_name: "Google Chrome".to_string(),
            canonical_name: "chrome".to_string(),
            binary_path: Some("C:\\Chrome\\chrome.exe".to_string()),
            is_system: false,
            category: ProcessCategory::Browser,
            icon_hint: Some("google-chrome".to_string()),
            memory_bytes: 100,
            total_memory_bytes: 100,
            child_pids: Vec::new(),
        },
        ExtendedProcessInfo {
            pid: 2,
            ppid: None,
            name: "code.exe".to_string(),
            display_name: "Visual Studio Code".to_string(),
            canonical_name: "code".to_string(),
            binary_path: Some("C:\\VSCode\\code.exe".to_string()),
            is_system: false,
            category: ProcessCategory::Developer,
            icon_hint: Some("visual-studio-code".to_string()),
            memory_bytes: 200,
            total_memory_bytes: 200,
            child_pids: Vec::new(),
        },
        ExtendedProcessInfo {
            pid: 3,
            ppid: None,
            name: "svchost.exe".to_string(),
            display_name: "svchost.exe".to_string(),
            canonical_name: "svchost".to_string(),
            binary_path: Some("C:\\Windows\\System32\\svchost.exe".to_string()),
            is_system: true,
            category: ProcessCategory::SystemDaemon,
            icon_hint: None,
            memory_bytes: 50,
            total_memory_bytes: 50,
            child_pids: Vec::new(),
        },
    ];

    // Filter by category
    let filter_dev = ProcessFilter {
        category: Some(ProcessCategory::Developer),
        ..Default::default()
    };
    let devs = filter_dev.filter(&procs);
    assert_eq!(devs.len(), 1);
    assert_eq!(devs[0].name, "code.exe");

    // Filter by query string
    let filter_query = ProcessFilter {
        query: Some("studio".to_string()),
        ..Default::default()
    };
    let queried = filter_query.filter(&procs);
    assert_eq!(queried.len(), 1);
    assert_eq!(queried[0].pid, 2);

    // Filter excluding system
    let filter_no_sys = ProcessFilter {
        exclude_system: true,
        ..Default::default()
    };
    let no_sys = filter_no_sys.filter(&procs);
    assert_eq!(no_sys.len(), 2);
}

#[test]
fn test_enumerate_active_processes_smoke() {
    let processes = enumerate_active_processes().unwrap();
    assert!(!processes.is_empty());

    let user_apps = enumerate_user_applications().unwrap();
    assert!(user_apps.iter().all(|p| !p.is_system));

    let legacy_items = ProcessEnumerator::enumerate_running_processes();
    assert!(!legacy_items.is_empty());

    let extended = enumerate_extended_processes().unwrap();
    assert!(!extended.is_empty());
}
