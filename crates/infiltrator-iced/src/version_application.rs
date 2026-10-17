//! Iced host access to the shared core-version application.

use crate::host::storage::version_application;
use infiltrator_application::version_application::VersionApplication;
use infiltrator_contract::version::{EngineReleaseChannel, ReleaseChannelId};
use lurker_core_contract::error::InfiltratorError;
use lurker_core_contract::identity::EngineId;

pub fn application() -> Result<VersionApplication, InfiltratorError> {
    version_application().map_err(|error| InfiltratorError::Download(error.to_string()))
}

pub fn release_channel(engine_id: EngineId, raw: &str) -> EngineReleaseChannel {
    EngineReleaseChannel::new(engine_id, release_channel_id(raw))
}

pub fn normalized_release_channel(raw: &str) -> String {
    release_channel_id(raw).as_str().to_owned()
}

fn release_channel_id(raw: &str) -> ReleaseChannelId {
    match raw.trim().to_ascii_lowercase().as_str() {
        "alpha" | "pre-release" | "prerelease" => ReleaseChannelId::alpha(),
        "meta" | "meta-core" | "metacore" | "nightly" => {
            ReleaseChannelId::new("meta-core").expect("Mihomo release channel ID is valid")
        }
        _ => ReleaseChannelId::stable(),
    }
}
