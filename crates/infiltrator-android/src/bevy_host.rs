//! BANDROID-003/004: the Gradle Bevy host launch entry.
//!
//! Compiled only with the `bevy-host` feature. The native driver
//! (`android-activity`'s `android_main`, built with the same feature) calls
//! [`launch_bevy_android_host`] after installing `bevy_android::ANDROID_APP`.
//! This entry composes the Android product session, attaches the shared
//! application to the Bevy UI and launches the shell with the Android surface
//! pump, so the `:vpn` process is the owner of the bridge and no UI process is
//! required to have run first.

use crate::product::AndroidProductComposition;
use crate::product::AndroidProductConfig;
use infiltrator_bevy_ui::surface::ApplicationSurfaceSource;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};

/// Launch the Bevy product on Android with the shared application and surface
/// pump. Blocks for the lifetime of the Bevy app.
///
/// A missing or unregistered Android bridge is surfaced as a typed
/// `NotReady` unavailable shell instead of a working-looking demo.
pub fn launch_bevy_android_host(config: AndroidProductConfig) {
    match AndroidProductComposition::open(config) {
        Ok(composition) => {
            let application = composition.application();
            let source = ApplicationSurfaceSource::from_application(
                composition.surface_pump(),
                application.clone(),
            );
            infiltrator_bevy_ui::attach_application(application.clone());
            infiltrator_bevy_ui::run_with_application_surface_pump(application, source);
        }
        Err(error) => {
            infiltrator_bevy_ui::run_unavailable(
                SurfaceKind::BevyAndroid,
                HostKind::Android,
                Failure::new(ErrorCode::NotReady, error.to_string(), true),
            );
        }
    }
}
