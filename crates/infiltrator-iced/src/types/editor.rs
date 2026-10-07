//! Editor lazy-load flag and script sandbox state shared by the configuration editors.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorLazyState {
    #[default]
    Unloaded,
    Loaded,
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
