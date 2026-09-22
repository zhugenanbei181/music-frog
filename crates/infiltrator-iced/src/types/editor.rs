//! Editor lazy-load flag and script sandbox state shared by the configuration editors.

use infiltrator_contract::script_sandbox::ScriptSandboxSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorLazyState {
    #[default]
    Unloaded,
    Loaded,
}

/// State for the directive-DSL script sandbox and lifecycle hook console.
#[derive(Debug, Clone, Default)]
pub struct ScriptSandboxState {
    pub script_code: String,
    pub input_yaml: String,
    /// The shared read model produced by `ScriptApplication`; the same
    /// projection the Bevy console renders. No local execution state.
    pub snapshot: Option<ScriptSandboxSnapshot>,
    pub is_running: bool,
    pub selected_preset: Option<String>,
}

impl ScriptSandboxState {
    /// The projected error detail, if the last run failed.
    pub fn error_detail(&self) -> Option<&str> {
        self.snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.error_detail.as_deref())
    }
}

/// Status and metadata for the GeoIP / GeoSite binary databases.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GeoDataStatus {
    pub geoip_version: String,
    pub geosite_version: String,
    pub geoip_size_bytes: u64,
    pub geosite_size_bytes: u64,
    pub is_updating: bool,
    pub update_message: Option<String>,
}
