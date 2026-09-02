use super::*;
use anyhow::anyhow;
use std::collections::HashMap;

#[test]
fn test_unsupported_desktop_error_is_typed_and_explicit() {
    let err = anyhow!(UnsupportedDesktopError {
        backend: "gsettings"
    });
    let typed = err
        .downcast_ref::<UnsupportedDesktopError>()
        .expect("typed error survives the anyhow downcast");
    assert_eq!(typed.backend, "gsettings");

    let message = typed.to_string();
    assert!(message.contains("unsupported"), "{message}");
    assert!(message.contains("desktop environment"), "{message}");
    assert!(message.contains("gsettings"), "{message}");
    assert!(message.contains("KDE"), "{message}");
}

#[test]
fn test_desktop_environment_display_and_as_str() {
    assert_eq!(DesktopEnvironment::Gnome.as_str(), "GNOME");
    assert_eq!(DesktopEnvironment::Kde.as_str(), "KDE");
    assert_eq!(DesktopEnvironment::Xfce.as_str(), "XFCE");
    assert_eq!(DesktopEnvironment::Generic.as_str(), "Generic");
    assert_eq!(format!("{}", DesktopEnvironment::Gnome), "GNOME");
    assert_eq!(format!("{}", DesktopEnvironment::Kde), "KDE");
    assert_eq!(format!("{}", DesktopEnvironment::Xfce), "XFCE");
    assert_eq!(format!("{}", DesktopEnvironment::Generic), "Generic");
}

#[test]
fn test_detect_desktop_environment_gnome_variants() {
    let gnome_cases = [
        ("XDG_CURRENT_DESKTOP", "GNOME"),
        ("XDG_CURRENT_DESKTOP", "ubuntu:GNOME"),
        ("XDG_CURRENT_DESKTOP", "pop:GNOME"),
        ("XDG_CURRENT_DESKTOP", "Pantheon"),
        ("XDG_CURRENT_DESKTOP", "Unity"),
        ("XDG_CURRENT_DESKTOP", "Budgie:GNOME"),
        ("XDG_CURRENT_DESKTOP", "CINNAMON"),
        ("XDG_CURRENT_DESKTOP", "MATE"),
        ("XDG_SESSION_DESKTOP", "gnome"),
        ("XDG_SESSION_DESKTOP", "ubuntu"),
        ("XDG_SESSION_DESKTOP", "pop"),
        ("DESKTOP_SESSION", "gnome"),
        ("DESKTOP_SESSION", "ubuntu"),
        ("DESKTOP_SESSION", "pop"),
        ("DESKTOP_SESSION", "cinnamon"),
        ("DESKTOP_SESSION", "mate"),
        ("GNOME_DESKTOP_SESSION_ID", "session-1234"),
    ];

    for (var, val) in gnome_cases {
        let env_map = HashMap::from([(var, val.to_string())]);
        let detected = detect_desktop_environment_with(|k| env_map.get(k).cloned());
        assert_eq!(
            detected,
            DesktopEnvironment::Gnome,
            "Failed for {}={}",
            var,
            val
        );
    }
}

#[test]
fn test_detect_desktop_environment_kde_variants() {
    let kde_cases = [
        ("XDG_CURRENT_DESKTOP", "KDE"),
        ("XDG_CURRENT_DESKTOP", "KDE:Plasma"),
        ("XDG_CURRENT_DESKTOP", "plasma"),
        ("XDG_CURRENT_DESKTOP", "Plasma"),
        ("XDG_SESSION_DESKTOP", "kde"),
        ("XDG_SESSION_DESKTOP", "plasma"),
        ("XDG_SESSION_DESKTOP", "plasmashell"),
        ("DESKTOP_SESSION", "kde"),
        ("DESKTOP_SESSION", "plasma"),
        ("DESKTOP_SESSION", "plasma-wayland"),
        ("DESKTOP_SESSION", "plasma-x11"),
        ("KDE_FULL_SESSION", "true"),
        ("KDE_FULL_SESSION", "TRUE"),
        ("KDE_SESSION_VERSION", "5"),
        ("KDE_SESSION_VERSION", "6"),
    ];

    for (var, val) in kde_cases {
        let env_map = HashMap::from([(var, val.to_string())]);
        let detected = detect_desktop_environment_with(|k| env_map.get(k).cloned());
        assert_eq!(
            detected,
            DesktopEnvironment::Kde,
            "Failed for {}={}",
            var,
            val
        );
    }
}

#[test]
fn test_detect_desktop_environment_xfce_variants() {
    let xfce_cases = [
        ("XDG_CURRENT_DESKTOP", "XFCE"),
        ("XDG_CURRENT_DESKTOP", "xfce"),
        ("XDG_CURRENT_DESKTOP", "X-Generic"),
        ("XDG_SESSION_DESKTOP", "xfce"),
        ("XDG_SESSION_DESKTOP", "xfce4"),
        ("XDG_SESSION_DESKTOP", "xubuntu"),
        ("DESKTOP_SESSION", "xfce"),
        ("DESKTOP_SESSION", "xfce4"),
        ("DESKTOP_SESSION", "xubuntu"),
    ];

    for (var, val) in xfce_cases {
        let env_map = HashMap::from([(var, val.to_string())]);
        let detected = detect_desktop_environment_with(|k| env_map.get(k).cloned());
        assert_eq!(
            detected,
            DesktopEnvironment::Xfce,
            "Failed for {}={}",
            var,
            val
        );
    }
}

#[test]
fn test_detect_desktop_environment_generic_fallback() {
    let generic_cases = [
        ("XDG_CURRENT_DESKTOP", "i3"),
        ("XDG_CURRENT_DESKTOP", "sway"),
        ("XDG_CURRENT_DESKTOP", "awesome"),
        ("XDG_CURRENT_DESKTOP", "unknown_wm"),
        ("DESKTOP_SESSION", "custom"),
        ("SOME_OTHER_VAR", "value"),
    ];

    for (var, val) in generic_cases {
        let env_map = HashMap::from([(var, val.to_string())]);
        let detected = detect_desktop_environment_with(|k| env_map.get(k).cloned());
        assert_eq!(
            detected,
            DesktopEnvironment::Generic,
            "Failed for {}={}",
            var,
            val
        );
    }

    let empty_map: HashMap<&str, String> = HashMap::new();
    assert_eq!(
        detect_desktop_environment_with(|k| empty_map.get(k).cloned()),
        DesktopEnvironment::Generic
    );
}

#[test]
fn test_parse_url_to_endpoint() {
    assert_eq!(
        parse_url_to_endpoint("http://127.0.0.1:7890"),
        Some("127.0.0.1:7890".to_string())
    );
    assert_eq!(
        parse_url_to_endpoint("https://127.0.0.1:7890/"),
        Some("127.0.0.1:7890".to_string())
    );
    assert_eq!(
        parse_url_to_endpoint("socks5://127.0.0.1:1080"),
        Some("127.0.0.1:1080".to_string())
    );
    assert_eq!(
        parse_url_to_endpoint("socks5h://localhost:9050"),
        Some("localhost:9050".to_string())
    );
    assert_eq!(
        parse_url_to_endpoint("socks://192.168.1.1:1080/path"),
        Some("192.168.1.1:1080".to_string())
    );
    assert_eq!(
        parse_url_to_endpoint("127.0.0.1:8888"),
        Some("127.0.0.1:8888".to_string())
    );
    assert_eq!(
        parse_url_to_endpoint("'http://127.0.0.1:7890'"),
        Some("127.0.0.1:7890".to_string())
    );
    assert_eq!(
        parse_url_to_endpoint("\"http://127.0.0.1:7890\""),
        Some("127.0.0.1:7890".to_string())
    );
    assert_eq!(parse_url_to_endpoint("invalid"), None);
    assert_eq!(parse_url_to_endpoint(""), None);
}

#[test]
fn test_kde_bypass_formatting() {
    assert_eq!(
        kde::format_bypass_for_kde("localhost;127.*;10.*;192.168.*"),
        "localhost,127.*,10.*,192.168.*"
    );
    assert_eq!(
        kde::format_bypass_for_kde(" localhost ; 127.0.0.1 ;; "),
        "localhost,127.0.0.1"
    );
    assert_eq!(
        kde::format_bypass_from_kde("localhost, 127.0.0.1, 10.0.0.0/8"),
        "localhost;127.0.0.1;10.0.0.0/8"
    );
}

#[test]
fn test_kde_generate_write_commands_enable() {
    let cmds = kde::generate_write_commands(
        "kwriteconfig6",
        Some("127.0.0.1:7890"),
        Some("localhost;127.*"),
    );
    assert_eq!(cmds.len(), 6);
    assert_eq!(
        cmds[0],
        vec![
            "kwriteconfig6",
            "--file",
            "kioslaverc",
            "--group",
            "Proxy Settings",
            "--key",
            "ProxyType",
            "1"
        ]
    );
    assert_eq!(
        cmds[1],
        vec![
            "kwriteconfig6",
            "--file",
            "kioslaverc",
            "--group",
            "Proxy Settings",
            "--key",
            "httpProxy",
            "http://127.0.0.1:7890"
        ]
    );
    assert_eq!(
        cmds[2],
        vec![
            "kwriteconfig6",
            "--file",
            "kioslaverc",
            "--group",
            "Proxy Settings",
            "--key",
            "httpsProxy",
            "http://127.0.0.1:7890"
        ]
    );
    assert_eq!(
        cmds[3],
        vec![
            "kwriteconfig6",
            "--file",
            "kioslaverc",
            "--group",
            "Proxy Settings",
            "--key",
            "ftpProxy",
            "http://127.0.0.1:7890"
        ]
    );
    assert_eq!(
        cmds[4],
        vec![
            "kwriteconfig6",
            "--file",
            "kioslaverc",
            "--group",
            "Proxy Settings",
            "--key",
            "socksProxy",
            "socks://127.0.0.1:7890"
        ]
    );
    assert_eq!(
        cmds[5],
        vec![
            "kwriteconfig6",
            "--file",
            "kioslaverc",
            "--group",
            "Proxy Settings",
            "--key",
            "NoProxyFor",
            "localhost,127.*"
        ]
    );
}

#[test]
fn test_kde_generate_write_commands_disable() {
    let cmds = kde::generate_write_commands("kwriteconfig5", None, None);
    assert_eq!(cmds.len(), 1);
    assert_eq!(
        cmds[0],
        vec![
            "kwriteconfig5",
            "--file",
            "kioslaverc",
            "--group",
            "Proxy Settings",
            "--key",
            "ProxyType",
            "0"
        ]
    );
}

#[test]
fn test_kde_generate_read_commands() {
    let (cmd_type, cmd_http, cmd_bypass) = kde::generate_read_commands("kreadconfig6");
    assert_eq!(
        cmd_type,
        vec![
            "kreadconfig6",
            "--file",
            "kioslaverc",
            "--group",
            "Proxy Settings",
            "--key",
            "ProxyType"
        ]
    );
    assert_eq!(
        cmd_http,
        vec![
            "kreadconfig6",
            "--file",
            "kioslaverc",
            "--group",
            "Proxy Settings",
            "--key",
            "httpProxy"
        ]
    );
    assert_eq!(
        cmd_bypass,
        vec![
            "kreadconfig6",
            "--file",
            "kioslaverc",
            "--group",
            "Proxy Settings",
            "--key",
            "NoProxyFor"
        ]
    );
}

#[test]
fn test_kde_kioslaverc_ini_update_and_parse() {
    let initial_ini = "[General]\nBrowser=firefox\n";
    let updated = kde::update_kioslaverc_content(
        initial_ini,
        Some("127.0.0.1:7890"),
        Some("localhost;127.0.0.1;10.0.0.0/8"),
    );
    assert!(updated.contains("[General]"));
    assert!(updated.contains("[Proxy Settings]"));
    assert!(updated.contains("ProxyType=1"));
    assert!(updated.contains("httpProxy=http://127.0.0.1:7890"));
    assert!(updated.contains("NoProxyFor=localhost,127.0.0.1,10.0.0.0/8"));

    let state = kde::parse_kioslaverc_content(&updated);
    assert!(state.enabled);
    assert_eq!(state.endpoint, Some("127.0.0.1:7890".to_string()));
    assert_eq!(
        state.bypass,
        Some("localhost;127.0.0.1;10.0.0.0/8".to_string())
    );

    let disabled = kde::update_kioslaverc_content(&updated, None, None);
    assert!(disabled.contains("ProxyType=0"));
    let state_disabled = kde::parse_kioslaverc_content(&disabled);
    assert!(!state_disabled.enabled);
    assert_eq!(state_disabled.endpoint, None);
}

#[test]
fn test_env_fallback_generate_vars() {
    let vars =
        env_fallback::generate_env_vars(Some("127.0.0.1:7890"), Some("localhost;127.*;192.168.*"));
    assert_eq!(vars.get("http_proxy").unwrap(), "http://127.0.0.1:7890");
    assert_eq!(vars.get("HTTP_PROXY").unwrap(), "http://127.0.0.1:7890");
    assert_eq!(vars.get("https_proxy").unwrap(), "http://127.0.0.1:7890");
    assert_eq!(vars.get("HTTPS_PROXY").unwrap(), "http://127.0.0.1:7890");
    assert_eq!(vars.get("all_proxy").unwrap(), "socks5://127.0.0.1:7890");
    assert_eq!(vars.get("ALL_PROXY").unwrap(), "socks5://127.0.0.1:7890");
    assert_eq!(vars.get("no_proxy").unwrap(), "localhost,127.*,192.168.*");
    assert_eq!(vars.get("NO_PROXY").unwrap(), "localhost,127.*,192.168.*");

    let empty = env_fallback::generate_env_vars(None, None);
    assert!(empty.is_empty());
}

#[test]
fn test_env_fallback_generate_shell_export() {
    let export_script =
        env_fallback::generate_shell_export(Some("127.0.0.1:7890"), Some("localhost;127.*"));
    assert!(export_script.contains("export http_proxy=\"http://127.0.0.1:7890\""));
    assert!(export_script.contains("export HTTP_PROXY=\"http://127.0.0.1:7890\""));
    assert!(export_script.contains("export all_proxy=\"socks5://127.0.0.1:7890\""));
    assert!(export_script.contains("export no_proxy=\"localhost,127.*\""));

    let unset_script = env_fallback::generate_shell_export(None, None);
    assert!(unset_script.starts_with("unset "));
    assert!(unset_script.contains("http_proxy"));
    assert!(unset_script.contains("all_proxy"));
}

#[test]
fn test_env_fallback_read_state() {
    let mut map = HashMap::new();
    map.insert(
        "http_proxy".to_string(),
        "http://127.0.0.1:7890".to_string(),
    );
    map.insert(
        "no_proxy".to_string(),
        "localhost,127.0.0.1,10.0.0.0/8".to_string(),
    );

    let state = env_fallback::read_state_with(|k| map.get(k).cloned());
    assert!(state.enabled);
    assert_eq!(state.endpoint, Some("127.0.0.1:7890".to_string()));
    assert_eq!(
        state.bypass,
        Some("localhost;127.0.0.1;10.0.0.0/8".to_string())
    );

    let mut socks_map = HashMap::new();
    socks_map.insert(
        "ALL_PROXY".to_string(),
        "socks5://127.0.0.1:1080".to_string(),
    );
    let state_socks = env_fallback::read_state_with(|k| socks_map.get(k).cloned());
    assert!(state_socks.enabled);
    assert_eq!(state_socks.endpoint, Some("127.0.0.1:1080".to_string()));

    let empty_map: HashMap<String, String> = HashMap::new();
    let state_empty = env_fallback::read_state_with(|k| empty_map.get(k).cloned());
    assert!(!state_empty.enabled);
    assert_eq!(state_empty.endpoint, None);
    assert_eq!(state_empty.bypass, None);
}

#[test]
fn test_gnome_command_generation() {
    let cmds = gnome::generate_gsettings_commands(Some("127.0.0.1:7890"), Some("localhost;127.*"));
    assert_eq!(cmds.len(), 8);
    assert_eq!(
        cmds[0],
        vec!["set", "org.gnome.system.proxy", "mode", "'manual'"]
    );
    assert_eq!(
        cmds[7],
        vec![
            "set",
            "org.gnome.system.proxy",
            "ignore-hosts",
            "['localhost', '127.*']"
        ]
    );

    let disable_cmds = gnome::generate_gsettings_commands(None, None);
    assert_eq!(disable_cmds.len(), 1);
    assert_eq!(
        disable_cmds[0],
        vec!["set", "org.gnome.system.proxy", "mode", "'none'"]
    );
}
