pub mod admin_api;
pub mod config;
pub mod editor;
pub mod proxy;
pub mod profiles;
pub mod runtime;
pub mod servers;
pub mod settings;
pub mod version;

pub use profiles::{ProfileDetail, ProfileInfo};
pub use proxy::SystemProxyState;
pub use runtime::{MihomoRuntime, MihomoSummary};
pub use settings::AppSettings;
