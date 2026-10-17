//! CI-safe smoke test for `examples/lifecycle.rs`.
//!
//! It compiles the example's pure planning module directly, exercises the
//! "not configured" path and the configured-target validation, and pins the
//! documented run command and environment keys to the README. No engine
//! binary, filesystem state or network access is required.

#[path = "../examples/lifecycle/plan.rs"]
mod plan;

use std::collections::HashMap;
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
        "cargo run -p mihomo-driver --example lifecycle"
    );
    assert_eq!(
        REQUIRED_ENV_KEYS,
        ["MIHOMO_BINARY", "MIHOMO_CONFIG", "MIHOMO_CONTROLLER"]
    );
    assert_eq!(plan::OPTIONAL_ENV_KEYS, ["MIHOMO_SECRET"]);

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
            missing: vec!["MIHOMO_BINARY", "MIHOMO_CONFIG"],
        })
    );

    assert_eq!(
        resolve(&[("MIHOMO_BINARY", "/opt/core/mihomo")]),
        Ok(LifecyclePlan::NotConfigured {
            missing: vec!["MIHOMO_CONFIG"],
        })
    );
    assert_eq!(
        resolve(&[("MIHOMO_CONFIG", "/tmp/mihomo.yaml")]),
        Ok(LifecyclePlan::NotConfigured {
            missing: vec!["MIHOMO_BINARY"],
        })
    );

    // Whitespace-only values are not configuration.
    assert_eq!(
        resolve(&[("MIHOMO_BINARY", "  "), ("MIHOMO_CONFIG", "\t")]),
        Ok(LifecyclePlan::NotConfigured {
            missing: vec!["MIHOMO_BINARY", "MIHOMO_CONFIG"],
        })
    );

    assert!(plan::PLAN_STEPS.len() >= 5);
    assert!(plan::PLAN_STEPS.iter().all(|step| !step.is_empty()));
}

#[test]
fn configured_plan_requires_a_controller_endpoint() {
    assert_eq!(
        resolve(&[
            ("MIHOMO_BINARY", "/opt/core/mihomo"),
            ("MIHOMO_CONFIG", "/tmp/mihomo.yaml"),
        ]),
        Err(PlanError::MissingEnv("MIHOMO_CONTROLLER"))
    );
}

#[test]
fn controller_must_be_an_absolute_http_endpoint() {
    let base = [
        ("MIHOMO_BINARY", "/opt/core/mihomo"),
        ("MIHOMO_CONFIG", "/tmp/mihomo.yaml"),
    ];

    let mut loopback = base.to_vec();
    loopback.push(("MIHOMO_CONTROLLER", "127.0.0.1:9090"));
    assert_eq!(
        resolve(&loopback),
        Err(PlanError::InvalidController("127.0.0.1:9090".to_owned()))
    );

    let mut ftp = base.to_vec();
    ftp.push(("MIHOMO_CONTROLLER", "ftp://127.0.0.1:9090"));
    assert_eq!(
        resolve(&ftp),
        Err(PlanError::InvalidController(
            "ftp://127.0.0.1:9090".to_owned()
        ))
    );
}

#[test]
fn configured_target_preserves_paths_and_the_optional_secret() {
    let resolved = resolve(&[
        ("MIHOMO_BINARY", " /opt/core/mihomo "),
        ("MIHOMO_CONFIG", " /tmp/mihomo.yaml "),
        ("MIHOMO_CONTROLLER", " http://127.0.0.1:9090 "),
        ("MIHOMO_SECRET", " example-secret "),
    ])
    .expect("configured environment resolves");

    let LifecyclePlan::Run(target) = resolved else {
        panic!("unexpected not-configured plan");
    };
    assert_eq!(target.binary(), Path::new("/opt/core/mihomo"));
    assert_eq!(target.config(), Path::new("/tmp/mihomo.yaml"));
    assert_eq!(target.controller(), "http://127.0.0.1:9090");
    assert_eq!(target.secret(), Some("example-secret"));

    let without_secret = resolve(&[
        ("MIHOMO_BINARY", "/opt/core/mihomo"),
        ("MIHOMO_CONFIG", "/tmp/mihomo.yaml"),
        ("MIHOMO_CONTROLLER", "http://127.0.0.1:9090"),
        ("MIHOMO_SECRET", ""),
    ])
    .expect("configured environment resolves without a secret");
    let LifecyclePlan::Run(without_secret) = without_secret else {
        panic!("unexpected not-configured plan without a secret");
    };
    assert_eq!(without_secret.secret(), None);
}
