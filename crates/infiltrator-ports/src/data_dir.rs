use std::path::PathBuf;

/// Host-owned locations used by application services.
pub trait DataDirProvider: Send + Sync {
    fn data_dir(&self) -> Option<PathBuf>;
    fn cache_dir(&self) -> Option<PathBuf> {
        None
    }
}
