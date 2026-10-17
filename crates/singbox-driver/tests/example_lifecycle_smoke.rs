//! CI-safe smoke test for `examples/lifecycle.rs`.
//!
//! It compiles the example's pure planning module directly, exercises the
//! "not configured" path and the configured-target validation, and pins the
//! documented run command and environment keys to the README. No engine
//! binary, filesystem state or network access is required.

#[path = "../examples/lifecycle/plan.rs"]
mod plan;

use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::Path;

use plan::{LifecyclePlan, PlanError, REQUIRED_ENV_KEYS, resolve_plan};

const README: &str = include_str!("../README.md");

fn environment(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

fn resolve(pairs: &[(&str, &str)]) -> Result<LifecyclePlan, PlanError> {
    let environment = environment(pairs);
    resolve_plan(|key| environment.get(key).cloned())
}

#[test]
fn documented_run_command_and_environment_keys_cannot_drift() {
    assert_eq!(plan::EXAMPLE_NAME, "lifecycle");
    assert_eq!(
        plan::RUN_COMMAND,
        "cargo run -p singbox-driver --example lifecycle"
    );
    assert_eq!(
        REQUIRED_ENV_KEYS,
        ["SINGBOX_BINARY", "SINGBOX_CONFIG", "SINGBOX_CONTROLLER"]
    );
    assert_eq!(plan::OPTIONAL_ENV_KEYS, ["SINGBOX_SECRET"]);

    assert!(
        README.contains(plan::RUN_COMMAND),
        "README must document `{}`",
        plan::RUN_COMMAND
    );
    for key in REQUIRED_ENV_KEYS
        .iter()
        .chain(plan::OPTIONAL_ENV_KEYS.iter())
    {
        assert!(README.contains(key), "README must document `{key}`");
    }
}

#[test]
fn absent_binary_or_config_returns_an_explicit_plan() {
    assert_eq!(
        resolve(&[]),
        Ok(LifecyclePlan::NotConfigured {
            missing: vec!["SINGBOX_BINARY", "SINGBOX_CONFIG"],
        })
    );

    assert_eq!(
        resolve(&[("SINGBOX_BINARY", "/opt/core/sing-box")]),
        Ok(LifecyclePlan::NotConfigured {
            missing: vec!["SINGBOX_CONFIG"],
        })
    );
    assert_eq!(
        resolve(&[("SINGBOX_CONFIG", "/tmp/sing-box.json")]),
        Ok(LifecyclePlan::NotConfigured {
            missing: vec!["SINGBOX_BINARY"],
        })
    );

    // Whitespace-only values are not configuration.
    assert_eq!(
        resolve(&[("SINGBOX_BINARY", "  "), ("SINGBOX_CONFIG", "\t")]),
        Ok(LifecyclePlan::NotConfigured {
            missing: vec!["SINGBOX_BINARY", "SINGBOX_CONFIG"],
        })
    );

    assert!(plan::PLAN_STEPS.len() >= 5);
    assert!(plan::PLAN_STEPS.iter().all(|step| !step.is_empty()));
}

#[test]
fn configured_plan_requires_a_controller_endpoint() {
    assert_eq!(
        resolve(&[
            ("SINGBOX_BINARY", "/opt/core/sing-box"),
            ("SINGBOX_CONFIG", "/tmp/sing-box.json"),
        ]),
        Err(PlanError::MissingEnv("SINGBOX_CONTROLLER"))
    );
}

#[test]
fn controller_must_be_a_fixed_loopback_socket_address() {
    let base = [
        ("SINGBOX_BINARY", "/opt/core/sing-box"),
        ("SINGBOX_CONFIG", "/tmp/sing-box.json"),
    ];

    let mut host_name = base.to_vec();
    host_name.push(("SINGBOX_CONTROLLER", "localhost:9090"));
    assert_eq!(
        resolve(&host_name),
        Err(PlanError::InvalidController("localhost:9090".to_owned()))
    );

    let mut any_interface = base.to_vec();
    any_interface.push(("SINGBOX_CONTROLLER", "0.0.0.0:9090"));
    assert_eq!(
        resolve(&any_interface),
        Err(PlanError::NonLoopbackController("0.0.0.0:9090".to_owned()))
    );

    let mut ephemeral = base.to_vec();
    ephemeral.push(("SINGBOX_CONTROLLER", "127.0.0.1:0"));
    assert_eq!(
        resolve(&ephemeral),
        Err(PlanError::EphemeralControllerPort("127.0.0.1:0".to_owned()))
    );
}

#[test]
fn configured_target_preserves_paths_and_the_optional_secret() {
    let resolved = resolve(&[
        ("SINGBOX_BINARY", " /opt/core/sing-box "),
        ("SINGBOX_CONFIG", " /tmp/sing-box.json "),
        ("SINGBOX_CONTROLLER", " 127.0.0.1:9090 "),
        ("SINGBOX_SECRET", " example-secret "),
    ])
    .expect("configured environment resolves");

    let LifecyclePlan::Run(target) = resolved else {
        panic!("unexpected not-configured plan");
    };
    assert_eq!(target.binary(), Path::new("/opt/core/sing-box"));
    assert_eq!(target.config(), Path::new("/tmp/sing-box.json"));
    assert_eq!(
        target.controller(),
        SocketAddr::from((Ipv4Addr::LOCALHOST, 9090))
    );
    assert_eq!(target.secret(), Some("example-secret"));

    let without_secret = resolve(&[
        ("SINGBOX_BINARY", "/opt/core/sing-box"),
        ("SINGBOX_CONFIG", "/tmp/sing-box.json"),
        ("SINGBOX_CONTROLLER", "[::1]:9090"),
        ("SINGBOX_SECRET", ""),
    ])
    .expect("configured environment resolves without a secret");
    let LifecyclePlan::Run(without_secret) = without_secret else {
        panic!("unexpected not-configured plan without a secret");
    };
    assert_eq!(
        without_secret.controller(),
        SocketAddr::from((Ipv6Addr::LOCALHOST, 9090))
    );
    assert_eq!(without_secret.secret(), None);
}
