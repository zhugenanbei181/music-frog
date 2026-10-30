//! Outbound port definition for user-owned palette preference storage (FLPARITY-036).

use async_trait::async_trait;
use infiltrator_contract::palette_preference::PalettePreference;
use lurker_core_ports::error::PortError;

/// Port abstraction for reading and writing the user-owned palette preference.
#[async_trait]
pub trait PalettePreferenceStorePort: Send + Sync {
    /// Load the saved palette preference from persistent storage.
    async fn load_preference(&self) -> Result<PalettePreference, PortError>;

    /// Save the palette preference to persistent storage.
    async fn save_preference(&self, preference: &PalettePreference) -> Result<(), PortError>;
}
