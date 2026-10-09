//! BANDROID-003/004 — Bevy Android native driver (template, isolated).
//!
//! The `android-activity` `native-activity` backend requires the packaged
//! cdylib to export `android_main`, and `bevy_winit` requires
//! `bevy_android::ANDROID_APP` to be set before `DefaultPlugins`. This driver
//! does exactly that and then hands control to the Android product entry,
//! which composes the shared application, spawns the Android surface pump and
//! attaches the application to the Bevy UI.
//!
//! Not verified: see ../../README.md for the Gradle/NDK/device checklist.

use infiltrator_android::bevy_host::launch_bevy_android_host;
use infiltrator_android::product::AndroidProductConfig;

/// Default loopback controller endpoint exposed by the packaged Mihomo core.
const DEFAULT_CONTROLLER_URL: &str = "http://127.0.0.1:9090";

#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    bevy_android::ANDROID_APP
        .set(app)
        .expect("ANDROID_APP must only be set once, by android_main");
    let controller_url = std::env::var("INFILTRATOR_CONTROLLER_URL")
        .unwrap_or_else(|_| DEFAULT_CONTROLLER_URL.to_string());
    launch_bevy_android_host(AndroidProductConfig::new(controller_url));
}
