use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "infiltrator",
    version,
    about = "MusicFrog Infiltrator — mihomo kernel manager CLI",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Environment and runtime diagnostics (run, fix, list, explain)
    Doctor {
        #[command(subcommand)]
        action: DoctorAction,
    },
    /// Create the default configs directory, profile, and controller settings
    Bootstrap,
    /// Install, switch, and inspect mihomo kernel versions
    Kernel {
        #[command(subcommand)]
        action: KernelAction,
    },
    /// Manage mihomo configuration profiles
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Core process lifecycle and live telemetry
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
    /// Inspect and switch proxies through the controller API
    Proxy {
        #[command(subcommand)]
        action: ProxyAction,
    },
    /// Inspect and close active connections
    Connection {
        #[command(subcommand)]
        action: ConnectionAction,
    },
    /// WebDAV configuration sync
    Sync {
        #[command(subcommand)]
        action: SyncAction,
    },
}

#[derive(Subcommand)]
pub enum DoctorAction {
    /// Run doctor checks and report the results
    Run {
        /// Comma-separated check ids or categories (e.g. config,service.stale)
        #[arg(long)]
        only: Option<String>,
        /// Render the report as JSON
        #[arg(long)]
        json: bool,
    },
    /// Apply safe doctor fixes
    Fix {
        /// Comma-separated check ids or categories (e.g. config,service.stale_pid)
        #[arg(long)]
        only: Option<String>,
        /// Render the fix report as JSON
        #[arg(long)]
        json: bool,
    },
    /// List available doctor checks
    List {
        /// Render the check catalog as JSON
        #[arg(long)]
        json: bool,
    },
    /// Explain one doctor check in detail
    Explain {
        /// Check id (see `doctor list`)
        check_id: String,
    },
}

#[derive(Subcommand)]
pub enum KernelAction {
    /// Install a kernel version or the latest of a channel
    Install {
        /// Version tag (v1.19.18) or channel (stable/beta/nightly)
        target: String,
    },
    /// Set the default kernel version
    Use {
        /// Installed version tag to make default
        version: String,
    },
    /// List installed kernel versions
    List {
        /// Render the installed versions as JSON
        #[arg(long)]
        json: bool,
    },
    /// List upstream releases
    ListRemote {
        /// Number of releases to show
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Uninstall an installed kernel version
    Uninstall {
        /// Version tag to remove (cannot be the default)
        version: String,
    },
    /// Install the latest stable kernel and set it as default
    UpdateStable,
}

#[derive(Subcommand)]
pub enum ProfileAction {
    /// List configuration profiles
    List {
        /// Render the profiles as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show the current profile name and its config path
    Current,
    /// Show the resolved configs directory path
    Path,
    /// Switch the active profile
    Use {
        /// Profile name to activate
        name: String,
    },
    /// Show a profile's YAML content (default: the current profile)
    Show {
        /// Profile name; defaults to the current profile
        name: Option<String>,
    },
    /// Delete a profile
    Delete {
        /// Profile name to delete
        name: String,
    },
    /// Import a profile from a subscription URL
    Import {
        /// Name for the new profile
        #[arg(long)]
        name: String,
        /// Subscription URL to fetch
        #[arg(long)]
        url: String,
    },
    /// Show or change the configs storage directory override
    ConfigsDir {
        #[command(subcommand)]
        action: ConfigsDirAction,
    },
}

#[derive(Subcommand)]
pub enum ConfigsDirAction {
    /// Show the configured and resolved configs directory
    Get,
    /// Store a configs directory override in the app settings
    Set {
        /// Directory path ('~' expands to the user home; relative joins home)
        path: String,
    },
    /// Remove the configs directory override
    Unset,
}

#[derive(Subcommand)]
pub enum ServiceAction {
    /// Start the core service (bootstraps the default profile first)
    Start,
    /// Stop the core service
    Stop,
    /// Restart the core service
    Restart,
    /// Show whether the core service is running
    Status,
    /// Stream controller logs until Ctrl-C
    Logs {
        /// Log level filter (debug/info/warning/error/silent)
        #[arg(long)]
        level: Option<String>,
    },
    /// Stream live traffic rates until Ctrl-C
    Traffic,
    /// Show current core memory usage
    Memory,
}

#[derive(Subcommand)]
pub enum ProxyAction {
    /// List proxy nodes
    List,
    /// List selectable proxy groups
    Groups,
    /// Select a proxy for a group
    Switch {
        /// Group name
        group: String,
        /// Proxy name to select
        proxy: String,
    },
    /// Test one proxy's delay
    Test {
        /// Proxy name
        name: String,
        /// Delay test URL
        #[arg(long, default_value = "http://www.gstatic.com/generate_204")]
        url: String,
        /// Delay test timeout in milliseconds
        #[arg(long, default_value_t = 5000)]
        timeout_ms: u32,
    },
    /// Show the current proxy selection of a group
    Current {
        /// Group name
        group: String,
    },
}

#[derive(Subcommand)]
pub enum ConnectionAction {
    /// List active connections
    List {
        /// Filter by host name or IP (substring match)
        #[arg(long)]
        host: Option<String>,
        /// Filter by process path (substring match)
        #[arg(long)]
        process: Option<String>,
        /// Filter by rule (substring match)
        #[arg(long)]
        rule: Option<String>,
        /// Render the filtered connections as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show connection statistics
    Stats,
    /// Stream connection snapshots until Ctrl-C
    Stream,
    /// Close connections (exactly one selector required)
    #[group(required = true, multiple = false)]
    Close {
        /// Connection id to close
        #[arg(long)]
        id: Option<String>,
        /// Close every active connection
        #[arg(long, default_value_t = false)]
        all: bool,
        /// Close connections whose host contains this string
        #[arg(long)]
        host: Option<String>,
        /// Close connections whose process path contains this string
        #[arg(long)]
        process: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum SyncAction {
    /// Verify the WebDAV settings and server connectivity
    Test,
    /// Run one full WebDAV sync round over the configs directory
    Now,
}

#[cfg(test)]
#[path = "commands_test.rs"]
mod commands_test;
