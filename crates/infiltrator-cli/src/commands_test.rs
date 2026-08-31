use clap::{CommandFactory, Parser};

use crate::commands::{
    Cli, Commands, ConfigsDirAction, ConnectionAction, DoctorAction, KernelAction, ProfileAction,
    ProxyAction, ServiceAction, SyncAction,
};

fn parse(args: &[&str]) -> Commands {
    let mut full: Vec<&str> = vec!["infiltrator"];
    full.extend_from_slice(args);
    Cli::try_parse_from(full).expect("command should parse").command
}

#[test]
fn doctor_run_accepts_only_and_json_flags() {
    let command = parse(&["doctor", "run", "--only", "config,service.stale", "--json"]);
    match command {
        Commands::Doctor {
            action: DoctorAction::Run { only, json },
        } => {
            assert_eq!(only.as_deref(), Some("config,service.stale"));
            assert!(json);
        }
        _ => panic!("expected doctor run"),
    }
}

#[test]
fn doctor_run_defaults_to_no_filter_and_text_output() {
    let command = parse(&["doctor", "run"]);
    match command {
        Commands::Doctor {
            action: DoctorAction::Run { only, json },
        } => {
            assert_eq!(only, None);
            assert!(!json);
        }
        _ => panic!("expected doctor run"),
    }
}

#[test]
fn doctor_fix_list_and_explain_parse() {
    let fix = parse(&["doctor", "fix", "--only", "service.stale_pid", "--json"]);
    match fix {
        Commands::Doctor {
            action: DoctorAction::Fix { only, json },
        } => {
            assert_eq!(only.as_deref(), Some("service.stale_pid"));
            assert!(json);
        }
        _ => panic!("expected doctor fix"),
    }

    match parse(&["doctor", "list", "--json"]) {
        Commands::Doctor {
            action: DoctorAction::List { json },
        } => assert!(json),
        _ => panic!("expected doctor list"),
    }

    match parse(&["doctor", "explain", "config.current_yaml"]) {
        Commands::Doctor {
            action: DoctorAction::Explain { check_id },
        } => assert_eq!(check_id, "config.current_yaml"),
        _ => panic!("expected doctor explain"),
    }
}

#[test]
fn bootstrap_and_kernel_subcommands_parse() {
    assert!(matches!(parse(&["bootstrap"]), Commands::Bootstrap));

    match parse(&["kernel", "install", "stable"]) {
        Commands::Kernel {
            action: KernelAction::Install { target },
        } => assert_eq!(target, "stable"),
        _ => panic!("expected kernel install"),
    }

    match parse(&["kernel", "use", "v1.19.18"]) {
        Commands::Kernel {
            action: KernelAction::Use { version },
        } => assert_eq!(version, "v1.19.18"),
        _ => panic!("expected kernel use"),
    }

    match parse(&["kernel", "list", "--json"]) {
        Commands::Kernel {
            action: KernelAction::List { json },
        } => assert!(json),
        _ => panic!("expected kernel list"),
    }

    match parse(&["kernel", "list-remote"]) {
        Commands::Kernel {
            action: KernelAction::ListRemote { limit },
        } => assert_eq!(limit, 20, "list-remote defaults to 20 entries"),
        _ => panic!("expected kernel list-remote"),
    }

    match parse(&["kernel", "list-remote", "--limit", "5"]) {
        Commands::Kernel {
            action: KernelAction::ListRemote { limit },
        } => assert_eq!(limit, 5),
        _ => panic!("expected kernel list-remote with limit"),
    }

    assert!(matches!(
        parse(&["kernel", "uninstall", "v1.18.0"]),
        Commands::Kernel {
            action: KernelAction::Uninstall { .. }
        }
    ));
    assert!(matches!(
        parse(&["kernel", "update-stable"]),
        Commands::Kernel {
            action: KernelAction::UpdateStable
        }
    ));
}

#[test]
fn profile_subcommands_parse_with_flags() {
    match parse(&["profile", "list", "--json"]) {
        Commands::Profile {
            action: ProfileAction::List { json },
        } => assert!(json),
        _ => panic!("expected profile list"),
    }

    match parse(&["profile", "import", "--name", "work", "--url", "https://sub.example.com"]) {
        Commands::Profile {
            action: ProfileAction::Import { name, url },
        } => {
            assert_eq!(name, "work");
            assert_eq!(url, "https://sub.example.com");
        }
        _ => panic!("expected profile import"),
    }

    match parse(&["profile", "show"]) {
        Commands::Profile {
            action: ProfileAction::Show { name },
        } => assert_eq!(name, None, "profile show defaults to the current profile"),
        _ => panic!("expected profile show"),
    }

    match parse(&["profile", "configs-dir", "set", "/cloud/profiles"]) {
        Commands::Profile {
            action: ProfileAction::ConfigsDir {
                action: ConfigsDirAction::Set { path },
            },
        } => assert_eq!(path, "/cloud/profiles"),
        _ => panic!("expected profile configs-dir set"),
    }

    assert!(matches!(
        parse(&["profile", "configs-dir", "get"]),
        Commands::Profile {
            action: ProfileAction::ConfigsDir {
                action: ConfigsDirAction::Get
            }
        }
    ));
    assert!(matches!(
        parse(&["profile", "configs-dir", "unset"]),
        Commands::Profile {
            action: ProfileAction::ConfigsDir {
                action: ConfigsDirAction::Unset
            }
        }
    ));
}

#[test]
fn service_subcommands_parse() {
    match parse(&["service", "logs", "--level", "debug"]) {
        Commands::Service {
            action: ServiceAction::Logs { level },
        } => assert_eq!(level.as_deref(), Some("debug")),
        _ => panic!("expected service logs"),
    }

    match parse(&["service", "logs"]) {
        Commands::Service {
            action: ServiceAction::Logs { level },
        } => assert_eq!(level, None, "logs without --level streams everything"),
        _ => panic!("expected service logs"),
    }

    for (args, expected) in [
        (vec!["service", "start"], ServiceAction::Start),
        (vec!["service", "stop"], ServiceAction::Stop),
        (vec!["service", "restart"], ServiceAction::Restart),
        (vec!["service", "status"], ServiceAction::Status),
        (vec!["service", "traffic"], ServiceAction::Traffic),
        (vec!["service", "memory"], ServiceAction::Memory),
    ] {
        let slice = args.clone();
        match parse(&slice) {
            Commands::Service { action } => assert_eq!(
                std::mem::discriminant(&action),
                std::mem::discriminant(&expected)
            ),
            _ => panic!("expected service command {slice:?}"),
        }
    }
}

#[test]
fn proxy_test_has_sensible_defaults() {
    match parse(&["proxy", "test", "HK-01"]) {
        Commands::Proxy {
            action:
                ProxyAction::Test {
                    name,
                    url,
                    timeout_ms,
                },
        } => {
            assert_eq!(name, "HK-01");
            assert_eq!(url, "http://www.gstatic.com/generate_204");
            assert_eq!(timeout_ms, 5000);
        }
        _ => panic!("expected proxy test"),
    }

    match parse(&["proxy", "test", "HK-01", "--url", "https://x.example", "--timeout-ms", "2500"]) {
        Commands::Proxy {
            action:
                ProxyAction::Test {
                    name,
                    url,
                    timeout_ms,
                },
        } => {
            assert_eq!(name, "HK-01");
            assert_eq!(url, "https://x.example");
            assert_eq!(timeout_ms, 2500);
        }
        _ => panic!("expected proxy test with overrides"),
    }

    match parse(&["proxy", "switch", "GLOBAL", "HK-01"]) {
        Commands::Proxy {
            action: ProxyAction::Switch { group, proxy },
        } => {
            assert_eq!(group, "GLOBAL");
            assert_eq!(proxy, "HK-01");
        }
        _ => panic!("expected proxy switch"),
    }
}

#[test]
fn connection_list_accepts_all_filters() {
    match parse(&[
        "connection",
        "list",
        "--host",
        "example",
        "--process",
        "curl",
        "--rule",
        "PROXY",
        "--json",
    ]) {
        Commands::Connection {
            action:
                ConnectionAction::List {
                    host,
                    process,
                    rule,
                    json,
                },
        } => {
            assert_eq!(host.as_deref(), Some("example"));
            assert_eq!(process.as_deref(), Some("curl"));
            assert_eq!(rule.as_deref(), Some("PROXY"));
            assert!(json);
        }
        _ => panic!("expected connection list"),
    }
}

#[test]
fn connection_close_requires_exactly_one_selector() {
    let parsed = Cli::try_parse_from(["infiltrator", "connection", "close"]);
    assert!(parsed.is_err(), "a selector is required");

    let parsed = Cli::try_parse_from([
        "infiltrator",
        "connection",
        "close",
        "--id",
        "abc",
        "--all",
    ]);
    assert!(parsed.is_err(), "selectors are mutually exclusive");

    match parse(&["connection", "close", "--host", "example"]) {
        Commands::Connection {
            action: ConnectionAction::Close { id, all, host, process },
        } => {
            assert_eq!(id, None);
            assert!(!all);
            assert_eq!(host.as_deref(), Some("example"));
            assert_eq!(process, None);
        }
        _ => panic!("expected connection close"),
    }
}

#[test]
fn sync_test_and_now_parse() {
    assert!(matches!(
        parse(&["sync", "test"]),
        Commands::Sync {
            action: SyncAction::Test
        }
    ));
    assert!(matches!(
        parse(&["sync", "now"]),
        Commands::Sync {
            action: SyncAction::Now
        }
    ));
}

#[test]
fn root_help_lists_all_namespaced_groups() {
    let mut command = Cli::command();
    let mut buffer = Vec::new();
    command.write_long_help(&mut buffer).unwrap();
    let help = String::from_utf8(buffer).unwrap();

    for group in [
        "doctor", "bootstrap", "kernel", "profile", "service", "proxy", "connection", "sync",
    ] {
        assert!(help.contains(group), "help is missing '{group}':\n{help}");
    }
}
