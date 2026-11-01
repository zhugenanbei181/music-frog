//! The host-owned system tray seam (FLPARITY-006).
//!
//! The system tray is a **host** capability, not a surface feature: the
//! desktop host owns the OS tray, publishes the shared menu and forwards the
//! user's activations. The surface consumes that seam — it never talks to
//! the OS tray itself, and it never keeps a second copy of the tray state.
//!
//! The seam is deliberately `!Send`: the Windows/macOS backend owns a native
//! menu handle that must stay on the thread that created it, so the host
//! carries it as a Bevy non-send resource.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::surface::LatestSurfaceSnapshot;
use bevy::app::{App, AppExit, Plugin, Update};
use bevy::ecs::change_detection::NonSend;
use bevy::prelude::{MessageWriter, Query, Res, Window};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::system_proxy::SystemProxyStatus;
use infiltrator_contract::tray_menu::{TrayMenuCapabilities, TrayMenuContext, TraySpec};
use infiltrator_contract::tray_status::{
    TrayEvent, TrayEventContext, TrayIconColorMode, TrayIntent, TraySupport, resolve_tray_event_in,
};
use infiltrator_presentation::tray_projection::{build_tray_spec, tray_context_from_snapshot};
use lurker_core_contract::error::Failure;

/// The initial empty/stopped tray specification before the first snapshot arrives.
pub fn initial_tray_spec(color_mode: TrayIconColorMode) -> TraySpec {
    build_tray_spec(
        &TrayMenuContext {
            lang: "zh-CN".to_string(),
            capabilities: TrayMenuCapabilities::shared_desktop(),
            ..TrayMenuContext::default()
        },
        color_mode,
    )
}

/// The host-owned system tray seam the surface consumes.
pub trait TrayHost: 'static {
    /// Publish a full menu refresh to the OS tray.
    fn push(&self, spec: TraySpec);
    /// Drain the activations the user made since the last call.
    fn drain(&self) -> Vec<TrayEvent>;
    /// The capability this host honestly offers.
    fn support(&self) -> TraySupport;
}

/// A wrapper resource for the `!Send` tray host.
pub struct TrayHostResource(pub Box<dyn TrayHost>);

/// The typed capability the surface reports when the host owns no tray.
pub const fn unsupported_support(reason: &'static str) -> TraySupport {
    TraySupport::Unsupported { reason }
}

/// The shared tray context the surface publishes from the current snapshot.
pub fn tray_context(snapshot: &SurfaceSnapshot, lang: &str) -> TrayMenuContext {
    let mut ctx = tray_context_from_snapshot(snapshot, lang);
    ctx.capabilities = TrayMenuCapabilities::shared_desktop();
    ctx
}

/// The tray icon style the user chose, read from the shared settings
/// readback (never from a local mirror).
pub fn tray_icon_color_mode(snapshot: &SurfaceSnapshot) -> TrayIconColorMode {
    snapshot
        .pages
        .settings
        .data
        .as_ref()
        .map(|settings| settings.tray_icon_color_mode)
        .unwrap_or_default()
}

/// Installs the host tray seam into the Bevy world.
pub struct TrayHostPlugin;

impl Plugin for TrayHostPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, publish_tray_menu);
        app.add_systems(Update, apply_tray_activations);
    }
}

/// Publish the shared tray menu from the newest owner snapshot.
fn publish_tray_menu(
    tray: Option<NonSend<TrayHostResource>>,
    snapshot: Option<Res<LatestSurfaceSnapshot>>,
) {
    let (Some(tray), Some(snapshot)) = (tray, snapshot) else {
        return;
    };
    let spec = build_tray_spec(
        &tray_context(&snapshot.0, "zh-CN"),
        tray_icon_color_mode(&snapshot.0),
    );
    tray.0.push(spec);
}

/// Turn the user's tray clicks into the surface's typed commands.
fn apply_tray_activations(
    tray: Option<NonSend<TrayHostResource>>,
    snapshot: Option<Res<LatestSurfaceSnapshot>>,
    sink: Option<Res<CommandSinkHandle>>,
    mut exits: MessageWriter<AppExit>,
    mut windows: Query<&mut Window>,
) {
    let Some(tray) = tray else {
        return;
    };
    let events = tray.0.drain();
    if events.is_empty() {
        return;
    }
    let context = tray_event_context(snapshot.as_ref());
    let Some(sink) = sink else {
        return;
    };
    for event in events {
        let Some(intent) = resolve_tray_event_in(&event, &context) else {
            continue;
        };
        match intent {
            TrayIntent::ShowWindow => {
                if let Ok(mut window) = windows.single_mut() {
                    window.focused = true;
                    window.visible = true;
                }
            }
            TrayIntent::Exit => {
                exits.write(AppExit::Success);
            }
            TrayIntent::ToggleIconColorMode => {
                let next = tray_icon_color_mode(
                    &snapshot
                        .as_ref()
                        .map(|s| s.0.clone())
                        .unwrap_or_else(empty_snapshot),
                )
                .toggle();
                sink.submit(UiCommand::UpdateSetting {
                    key: "tray_icon_color_mode".into(),
                    value: next.as_str().to_string(),
                });
            }
            TrayIntent::SetRoutingIntent(intent) => {
                sink.submit(UiCommand::SetRoutingIntent(intent));
            }
            TrayIntent::SetSystemProxy(enabled) => {
                sink.submit(UiCommand::SetSystemProxy { enabled });
            }
            TrayIntent::SetTunEnabled(enabled) => {
                sink.submit(UiCommand::ToggleTun { enabled });
            }
            TrayIntent::ActivateProfile(name) => {
                sink.submit(UiCommand::ActivateProfile { id: name });
            }
            TrayIntent::UpdateAllProfilesNow => {
                sink.submit(UiCommand::UpdateAllSubscriptions);
            }
            TrayIntent::SetProfileAutoUpdate { name, enabled } => {
                sink.submit(UiCommand::SetSubscriptionAutoReload {
                    profile_id: name,
                    enabled,
                });
            }
            _ => {}
        }
    }
}

/// The app-side states the shared resolver needs for its toggles.
fn tray_event_context(snapshot: Option<&Res<LatestSurfaceSnapshot>>) -> TrayEventContext<'static> {
    let Some(snapshot) = snapshot else {
        return TrayEventContext {
            system_proxy: false,
            tun: false,
            autostart: false,
            profiles: &[],
        };
    };
    TrayEventContext {
        system_proxy: matches!(snapshot.0.system_proxy.status, SystemProxyStatus::Enabled),
        tun: snapshot.0.runtime_control.tun_enabled.unwrap_or(false),
        autostart: false,
        profiles: &[],
    }
}

/// An empty snapshot used when no owner snapshot has arrived yet.
fn empty_snapshot() -> SurfaceSnapshot {
    SurfaceSnapshot::unavailable(
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
        Failure::unsupported("no owner snapshot observed yet"),
    )
}
