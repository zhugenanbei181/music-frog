//! The typed real-binary environment contract of the Mihomo driver.
//!
//! This module is the single source of truth for the environment variables of
//! the real-process evidence run. The local entry point
//! (`scripts/core-backend-smoke-mihomo.sh`) reads the same names from this
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

/// The environment variable that names the real Mihomo executable.
pub const BINARY_ENV: &str = "MIHOMO_REAL_BINARY";
/// The environment variable that names the Mihomo configuration document.
pub const CONFIG_ENV: &str = "MIHOMO_REAL_CONFIG";
/// The environment variable that names the Mihomo data directory.
pub const HOME_ENV: &str = "MIHOMO_REAL_HOME";
/// The environment variable that names the loopback controller endpoint.
pub const CONTROLLER_ENV: &str = "MIHOMO_REAL_CONTROLLER";
/// The environment variable that names the optional controller bearer secret.
pub const SECRET_ENV: &str = "MIHOMO_REAL_SECRET";
/// The environment variable that names the engine's own SOCKS5 listener.
pub const PROXY_ENV: &str = "MIHOMO_REAL_PROXY";
/// The environment variable that names the loopback traffic fixture.
pub const FIXTURE_ENV: &str = "MIHOMO_REAL_FIXTURE";
/// The environment variable that names the outbound the speed test measures.
pub const PROBE_OUTBOUND_ENV: &str = "MIHOMO_REAL_PROBE_OUTBOUND";
/// The environment variable that names the group the configuration write moves.
pub const SELECTOR_ENV: &str = "MIHOMO_REAL_SELECTOR";
/// The environment variable that names the file the receipt is written to.
pub const RECEIPT_ENV: &str = "MIHOMO_REAL_RECEIPT";

/// The environment keys every real-binary run must provide.
pub const REQUIRED_ENV_KEYS: [&str; 8] = [
    BINARY_ENV,
    CONFIG_ENV,
    HOME_ENV,
    CONTROLLER_ENV,
    PROXY_ENV,
    FIXTURE_ENV,
    PROBE_OUTBOUND_ENV,
    SELECTOR_ENV,
];

/// The environment keys a real-binary run may provide in addition.
pub const OPTIONAL_ENV_KEYS: [&str; 2] = [SECRET_ENV, RECEIPT_ENV];

/// One resolved real-binary target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealProcessTarget {
    binary: PathBuf,
    config: PathBuf,
    home: PathBuf,
    controller: String,
    secret: Option<String>,
    proxy: String,
    fixture: String,
    probe_outbound: String,
    selector: String,
    receipt: Option<PathBuf>,
}

impl RealProcessTarget {
    /// The real Mihomo executable the driver may launch.
    pub fn binary(&self) -> &PathBuf {
        &self.binary
    }

    /// The configuration document the core reads.
    pub fn config(&self) -> &PathBuf {
        &self.config
    }

    /// The data directory that owns the PID record and the engine logs.
    pub fn home(&self) -> &PathBuf {
        &self.home
    }

    /// The loopback controller endpoint of the core.
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

    /// The loopback fixture the engine's outbound and the dispatch proof use.
    pub fn fixture(&self) -> &str {
        &self.fixture
    }

    /// The absolute HTTP URL the typed node speed test measures.
    pub fn fixture_url(&self) -> String {
        format!("http://{}/health", self.fixture)
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
        home,
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
    if !is_loopback_controller(&controller) {
        return RealProcessEnvironment::Invalid {
            reason: format!(
                "{CONTROLLER_ENV} must name an explicit loopback HTTP endpoint, got `{controller}`"
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
        home: PathBuf::from(&home),
        controller,
        secret: optional_entry(&optional, SECRET_ENV),
        proxy,
        fixture,
        probe_outbound,
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

/// Whether the endpoint names an explicit loopback HTTP controller.
///
/// The driver's own typed validation stays the authority; this check only
/// rejects an environment that cannot name a loopback controller at all.
fn is_loopback_controller(endpoint: &str) -> bool {
    let Some(rest) = endpoint.strip_prefix("http://") else {
        return false;
    };
    let Some((host, port)) = rest.split_once(':') else {
        return false;
    };
    if !matches!(host, "127.0.0.1" | "[::1]") {
        return false;
    }
    match port.parse::<u16>() {
        Ok(port) => port != 0,
        Err(_) => false,
    }
}

/// Whether the address names a fixed loopback socket.
fn is_loopback_address(address: &str) -> bool {
    match address.parse::<SocketAddr>() {
        Ok(socket) => socket.ip().is_loopback() && socket.port() != 0,
        Err(_) => false,
    }
}
