//! Pure lifecycle-example planning shared by `examples/lifecycle.rs` and its
//! smoke test.
//!
//! Nothing here touches the filesystem, the process table or the network: the
//! example resolves the engine target from the environment, then hands it to
//! the driver's neutral ports.

use std::path::{Path, PathBuf};

/// Example target name; must match `examples/lifecycle.rs`.
pub const EXAMPLE_NAME: &str = "lifecycle";

/// The documented run command, asserted against the README by the smoke test.
pub const RUN_COMMAND: &str = "cargo run -p mihomo-driver --example lifecycle";

/// Environment keys that must all be present to run a real engine.
pub const REQUIRED_ENV_KEYS: [&str; 3] = ["MIHOMO_BINARY", "MIHOMO_CONFIG", "MIHOMO_CONTROLLER"];

/// Environment keys that are optional.
pub const OPTIONAL_ENV_KEYS: [&str; 1] = ["MIHOMO_SECRET"];

/// What the example would do with a configured engine. Printed verbatim when
/// the environment does not name a binary and config document.
pub const PLAN_STEPS: [&str; 5] = [
    "resolve the mihomo binary and config document from MIHOMO_BINARY / MIHOMO_CONFIG",
    "start one standalone process through the CoreProcess port (ProcessCoreController)",
    "wait for controller readiness through the CoreReadiness port at MIHOMO_CONTROLLER",
    "read the live version and outbound inventory through CoreReadiness / OutboundPort",
    "stop through CoreProcess and require CoreLifecycle::Stopped with no PID readback",
];

/// One resolved engine target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineTarget {
    binary: PathBuf,
    config: PathBuf,
    controller: String,
    secret: Option<String>,
}

impl EngineTarget {
    pub fn new(
        binary: impl Into<PathBuf>,
        config: impl Into<PathBuf>,
        controller: String,
        secret: Option<String>,
    ) -> Self {
        Self {
            binary: binary.into(),
            config: config.into(),
            controller,
            secret,
        }
    }

    pub fn binary(&self) -> &Path {
        &self.binary
    }

    pub fn config(&self) -> &Path {
        &self.config
    }

    pub fn controller(&self) -> &str {
        &self.controller
    }

    pub fn secret(&self) -> Option<&str> {
        self.secret.as_deref()
    }
}

/// Mutually exclusive example outcomes: either the engine is configured and a
/// lifecycle run is possible, or the example prints what it would have done.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LifecyclePlan {
    NotConfigured { missing: Vec<&'static str> },
    Run(EngineTarget),
}

/// Environment values that are present but cannot describe a runnable target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    MissingEnv(&'static str),
    InvalidController(String),
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingEnv(key) => {
                write!(formatter, "the engine is configured but {key} is not set")
            }
            Self::InvalidController(endpoint) => write!(
                formatter,
                "MIHOMO_CONTROLLER must be an absolute http(s) endpoint, got `{endpoint}`"
            ),
        }
    }
}

impl std::error::Error for PlanError {}

/// Resolves the example target from an environment lookup.
///
/// A binary and config document are what make the example "configured"; when
/// either is absent the example prints the plan instead of starting a process.
/// The controller endpoint is required only for a configured run.
pub fn resolve_plan<F>(mut lookup: F) -> Result<LifecyclePlan, PlanError>
where
    F: FnMut(&str) -> Option<String>,
{
    let binary = normalize(lookup(REQUIRED_ENV_KEYS[0]));
    let config = normalize(lookup(REQUIRED_ENV_KEYS[1]));

    let mut missing = Vec::new();
    if binary.is_none() {
        missing.push(REQUIRED_ENV_KEYS[0]);
    }
    if config.is_none() {
        missing.push(REQUIRED_ENV_KEYS[1]);
    }
    let (Some(binary), Some(config)) = (binary, config) else {
        return Ok(LifecyclePlan::NotConfigured { missing });
    };

    let controller = normalize(lookup(REQUIRED_ENV_KEYS[2]))
        .ok_or(PlanError::MissingEnv(REQUIRED_ENV_KEYS[2]))?;
    validate_controller(&controller)?;
    let secret = normalize(lookup(OPTIONAL_ENV_KEYS[0]));

    Ok(LifecyclePlan::Run(EngineTarget::new(
        binary, config, controller, secret,
    )))
}

fn normalize(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn validate_controller(controller: &str) -> Result<(), PlanError> {
    match url::Url::parse(controller) {
        Ok(url)
            if matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none() =>
        {
            Ok(())
        }
        _ => Err(PlanError::InvalidController(controller.to_owned())),
    }
}
