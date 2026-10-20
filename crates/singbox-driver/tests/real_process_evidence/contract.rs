//! The typed real-binary environment contract of the sing-box driver.
//!
//! This module is the single source of truth for the environment variables of
//! the real-binary evidence run. The local entry point
//! (`scripts/core-backend-smoke-singbox.sh`) reads the same names from this
//! file, so the script and the test can never disagree about the contract.
//!
//! The contract is one typed value with three mutually exclusive outcomes:
//!
//! * [`RealProcessEnvironment::Absent`] — the environment provides no engine
//!   binary, so the run proves nothing and says exactly that;
//! * [`RealProcessEnvironment::Present`] — the environment provides a runnable
//!   engine target;
//! * [`RealProcessEnvironment::Invalid`] — the environment names an engine
//!   that cannot run, which is a defect of the invocation and never a skip.
//!
//! No combination of flags carries that decision.

use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

/// The environment variable that names the real sing-box executable.
pub const BINARY_ENV: &str = "SINGBOX_REAL_BINARY";
/// The environment variable that names the sing-box configuration document.
pub const CONFIG_ENV: &str = "SINGBOX_REAL_CONFIG";
/// The environment variable that names the stable Core instance identity.
pub const INSTANCE_ENV: &str = "SINGBOX_REAL_INSTANCE";
/// The environment variable that names the loopback Clash API address.
pub const CONTROLLER_ENV: &str = "SINGBOX_REAL_CONTROLLER";
/// The environment variable that names the optional controller bearer secret.
pub const SECRET_ENV: &str = "SINGBOX_REAL_SECRET";
/// The environment variable that names the engine's own SOCKS5 listener.
pub const PROXY_ENV: &str = "SINGBOX_REAL_PROXY";
/// The environment variable that names the loopback traffic fixture.
pub const FIXTURE_ENV: &str = "SINGBOX_REAL_FIXTURE";
/// The environment variable that names the outbound the speed test measures.
pub const PROBE_OUTBOUND_ENV: &str = "SINGBOX_REAL_PROBE_OUTBOUND";
/// The environment variable that names the group the configuration write moves.
pub const SELECTOR_ENV: &str = "SINGBOX_REAL_SELECTOR";
/// The environment variable that names the file the receipt is written to.
pub const RECEIPT_ENV: &str = "SINGBOX_REAL_RECEIPT";
/// The environment variable that names the absolute URL the typed node speed
/// test measures.
pub const PROBE_TARGET_ENV: &str = "SINGBOX_REAL_PROBE_TARGET";

/// The URL the typed node speed test measures when the environment names no
/// target of its own.
///
/// sing-box measures HTTPS targets only: its delay endpoint replaces a
/// plain-HTTP target with the engine's own URL test target. The evidence
/// therefore asks for that target explicitly instead of pretending it
/// measured a loopback fixture, and an environment that cannot reach it
/// reports the failure honestly instead of a measurement.
pub const DEFAULT_PROBE_TARGET: &str = "https://www.gstatic.com/generate_204";

/// The environment keys every real-binary run must provide.
pub const REQUIRED_ENV_KEYS: [&str; 8] = [
    BINARY_ENV,
    CONFIG_ENV,
    INSTANCE_ENV,
    CONTROLLER_ENV,
    PROXY_ENV,
    FIXTURE_ENV,
    PROBE_OUTBOUND_ENV,
    SELECTOR_ENV,
];

/// The environment keys a real-binary run may provide in addition.
pub const OPTIONAL_ENV_KEYS: [&str; 3] = [SECRET_ENV, RECEIPT_ENV, PROBE_TARGET_ENV];

/// One resolved real-binary target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealProcessTarget {
    binary: PathBuf,
    config: PathBuf,
    instance: String,
    controller: String,
    secret: Option<String>,
    proxy: String,
    fixture: String,
    probe_outbound: String,
    probe_target: String,
    selector: String,
    receipt: Option<PathBuf>,
}

impl RealProcessTarget {
    /// The real sing-box executable the driver may launch.
    pub fn binary(&self) -> &PathBuf {
        &self.binary
    }

    /// The configuration document the core reads.
    pub fn config(&self) -> &PathBuf {
        &self.config
    }

    /// The stable Core instance identity the core runs under.
    pub fn instance(&self) -> &str {
        &self.instance
    }

    /// The loopback Clash API address of the core.
    pub fn controller(&self) -> &str {
        &self.controller
    }

    /// The controller bearer secret, when the environment named one.
    pub fn secret(&self) -> Option<&str> {
        self.secret.as_deref()
    }

    /// The engine's own SOCKS5 listener.
    pub fn proxy(&self) -> &str {
        &self.proxy
    }

    /// The loopback fixture the dispatch proof uses.
    pub fn fixture(&self) -> &str {
        &self.fixture
    }

    /// The absolute HTTP URL the typed node speed test measures.
    ///
    /// The engine measures HTTPS targets only, so the target is an HTTPS URL
    /// the engine can reach.
    pub fn probe_target(&self) -> &str {
        &self.probe_target
    }

    /// The outbound the typed node speed test measures.
    pub fn probe_outbound(&self) -> &str {
        &self.probe_outbound
    }

    /// The group the typed configuration write moves.
    pub fn selector(&self) -> &str {
        &self.selector
    }

    /// The file the machine-readable receipt is written to, when the
    /// environment named one.
    pub fn receipt(&self) -> Option<&Path> {
        self.receipt.as_deref()
    }
}

/// The resolved real-binary environment of one run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RealProcessEnvironment {
    /// The environment provides no engine binary, so nothing can be proven.
    Absent {
        /// The environment keys that are missing.
        missing: Vec<&'static str>,
    },
    /// The environment provides a runnable engine target.
    Present(RealProcessTarget),
    /// The environment names an engine that cannot run.
    Invalid {
        /// Why the environment cannot run an engine.
        reason: String,
    },
}

/// Resolves the real-binary environment from an environment lookup.
///
/// The lookup receives one environment key at a time and returns its raw
/// value, so the contract stays a pure function of the environment.
pub fn resolve<F>(mut lookup: F) -> RealProcessEnvironment
where
    F: FnMut(&str) -> Option<String>,
{
    let mut missing: Vec<&'static str> = Vec::new();
    let mut values: Vec<String> = Vec::with_capacity(REQUIRED_ENV_KEYS.len());
    for key in REQUIRED_ENV_KEYS {
        match optional_value(&mut lookup, key) {
            Some(value) => values.push(value),
            None => missing.push(key),
        }
    }
    if !missing.is_empty() {
        return RealProcessEnvironment::Absent { missing };
    }

    let Ok(values) = <[String; 8]>::try_from(values) else {
        return RealProcessEnvironment::Absent { missing };
    };
    let [
        binary,
        config,
        instance,
        controller,
        proxy,
        fixture,
        probe_outbound,
        selector,
    ] = values;

    let binary_path = PathBuf::from(&binary);
    if !binary_path.is_file() {
        return RealProcessEnvironment::Invalid {
            reason: format!("{BINARY_ENV} does not name an existing file: {binary}"),
        };
    }
    let config_path = PathBuf::from(&config);
    if !config_path.is_file() {
        return RealProcessEnvironment::Invalid {
            reason: format!("{CONFIG_ENV} does not name an existing file: {config}"),
        };
    }
    if instance.trim().is_empty() {
        return RealProcessEnvironment::Invalid {
            reason: format!("{INSTANCE_ENV} must name a stable Core instance identity"),
        };
    }
    if !is_loopback_address(&controller) {
        return RealProcessEnvironment::Invalid {
            reason: format!(
                "{CONTROLLER_ENV} must name a fixed loopback socket address, got `{controller}`"
            ),
        };
    }
    if !is_loopback_address(&proxy) {
        return RealProcessEnvironment::Invalid {
            reason: format!("{PROXY_ENV} must name a fixed loopback socket address, got `{proxy}`"),
        };
    }
    if !is_loopback_address(&fixture) {
        return RealProcessEnvironment::Invalid {
            reason: format!(
                "{FIXTURE_ENV} must name a fixed loopback socket address, got `{fixture}`"
            ),
        };
    }

    let optional: Vec<(&str, Option<String>)> = OPTIONAL_ENV_KEYS
        .iter()
        .map(|key| (*key, optional_value(&mut lookup, key)))
        .collect();

    RealProcessEnvironment::Present(RealProcessTarget {
        binary: binary_path,
        config: config_path,
        instance,
        controller,
        secret: optional_entry(&optional, SECRET_ENV),
        proxy,
        fixture,
        probe_outbound,
        probe_target: optional_entry(&optional, PROBE_TARGET_ENV)
            .unwrap_or_else(|| DEFAULT_PROBE_TARGET.to_owned()),
        selector,
        receipt: optional_entry(&optional, RECEIPT_ENV).map(PathBuf::from),
    })
}

/// Returns the receipt file the environment names, when it names one.
pub fn receipt_path() -> Option<PathBuf> {
    optional_value(&mut lookup_from_process, RECEIPT_ENV).map(PathBuf::from)
}

fn lookup_from_process(key: &str) -> Option<String> {
    env::var(key).ok()
}

fn optional_value<F>(lookup: &mut F, key: &str) -> Option<String>
where
    F: FnMut(&str) -> Option<String>,
{
    lookup(key)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Returns the value the environment named for one optional key.
fn optional_entry(optional: &[(&str, Option<String>)], key: &str) -> Option<String> {
    optional
        .iter()
        .find(|(name, _)| *name == key)
        .and_then(|(_, value)| value.clone())
}

/// Whether the address names a fixed loopback socket.
fn is_loopback_address(address: &str) -> bool {
    match address.parse::<SocketAddr>() {
        Ok(socket) => socket.ip().is_loopback() && socket.port() != 0,
        Err(_) => false,
    }
}
