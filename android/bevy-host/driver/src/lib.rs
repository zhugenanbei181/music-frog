//! BANDROID-003/004 — Bevy Android NativeActivity driver.
//!
//! The driver installs the Android window surface and starts Bevy. It does
//! not link `infiltrator-android` or create an application session; product
//! state and commands are obtained through the typed `:vpn` IPC client.

#[cfg(any(target_os = "android", test))]
mod insets;
#[cfg(any(target_os = "android", test))]
mod owner_client;

#[cfg(target_os = "android")]
mod android_jni;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    bevy_android::ANDROID_APP
        .set(app.clone())
        .expect("ANDROID_APP must only be set once, by android_main");
    use infiltrator_bevy_ui::command::UiCommandSink;
    use infiltrator_bevy_ui::surface::SurfaceSource;
    use infiltrator_bevy_ui::surface::host_insets::window_insets_input_channel;
    use infiltrator_contract::surface::{HostKind, SurfaceKind};
    use std::sync::Arc;

    if let Err(failure) = android_jni::register_window_insets_callback(&app) {
        infiltrator_bevy_ui::run_unavailable(SurfaceKind::BevyAndroid, HostKind::Android, failure);
        return;
    }
    let transport = Arc::new(android_jni::AndroidActivityOwnerTransport::new(app));
    let client = match owner_client::AndroidOwnerClient::start(transport.clone()) {
        Ok(client) => Arc::new(client),
        Err(failure) => {
            infiltrator_bevy_ui::run_unavailable(
                SurfaceKind::BevyAndroid,
                HostKind::Android,
                failure,
            );
            return;
        }
    };
    let (insets_sender, insets_receiver) = window_insets_input_channel();
    insets::register_window_insets_sender(insets_sender);
    if let Err(failure) = transport.publish_current_window_insets() {
        infiltrator_bevy_ui::run_unavailable(SurfaceKind::BevyAndroid, HostKind::Android, failure);
        return;
    }
    let source: Arc<dyn SurfaceSource> = client.clone();
    let sink: Arc<dyn UiCommandSink> = client;
    infiltrator_bevy_ui::run_with_host_surface_and_command_sink(source, sink, insets_receiver);
}
