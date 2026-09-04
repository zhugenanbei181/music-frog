//! Editor lazy-load flag and script sandbox state shared by the configuration editors.

use infiltrator_domain::script_engine::ScriptExecutionResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorLazyState {
    #[default]
    Unloaded,
    Loaded,
}

/// State for the QuickJS script sandbox and lifecycle hook console.
#[derive(Debug, Clone, Default)]
pub struct ScriptSandboxState {
    pub script_code: String,
    pub input_yaml: String,
    pub execution_result: Option<ScriptExecutionResult>,
    pub execution_error: Option<String>,
    pub is_running: bool,
    pub selected_preset: Option<String>,
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
