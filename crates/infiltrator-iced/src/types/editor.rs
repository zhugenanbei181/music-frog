//! Editor lazy-load flag and script sandbox state shared by the configuration editors.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorLazyState {
    #[default]
    Unloaded,
    Loaded,
}

/// Update operation phase for GeoData databases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GeoDataUpdatePhase {
    #[default]
    Idle,
    Updating,
}

impl GeoDataUpdatePhase {
    pub const fn is_updating(self) -> bool {
        matches!(self, Self::Updating)
    }
}

/// Status and metadata for the GeoIP / GeoSite binary databases.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GeoDataStatus {
    pub geoip_version: String,
    pub geosite_version: String,
    pub geoip_size_bytes: u64,
    pub geosite_size_bytes: u64,
    pub phase: GeoDataUpdatePhase,
    pub update_message: Option<String>,
}
