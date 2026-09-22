//! Mini HUD runtime wiring for the Bevy shell (DUAL-15-03/04).
//!
//! Builds the shared read model from the live projections, mounts/unmounts the
//! overlay scene, persists the pin state through the shared settings command
//! (`mini_hud.pinned`, validated by `CommandApplication::update_setting`) and
//! routes the `Ctrl+Alt+M` shortcut / palette row into the toggle event.

use bevy::app::{App, Plugin, Update};
use bevy::asset::Assets;
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::image::Image;
use bevy::scene::CommandsSceneExt;
use bevy::ui::widget::ImageNode;
use bevy::ui_widgets::Activate;
use infiltrator_application::system_toggle_application::SystemToggleApplication;
use infiltrator_bevy_widgets::chart::sparkline_image;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_contract::mini_hud::MiniHudWaveformStrip;
use infiltrator_contract::system_toggle::SystemToggle;

use crate::app::SidebarToggleProjection;
use crate::command::{CommandSinkHandle, UiCommand};
use crate::mini_hud::{
    MiniHudDownWaveform, MiniHudExpandButton, MiniHudMode, MiniHudModel, MiniHudPinButton,
    MiniHudRoot, MiniHudSystemProxyToggle, MiniHudTunToggle, MiniHudUpWaveform, SetMiniHudPinned,
    ToggleMiniHud, mini_hud_scene,
};
use crate::pages::overview::{LastOverviewProjection, mode_label};
use crate::surface::LatestSurfaceSnapshot;

/// Mount signature: the overlay is rebuilt only when visibility or a rendered
/// value actually changed.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct MiniHudMountSignature {
    pub visible: bool,
    pub model: MiniHudModel,
}

/// Rebuild the shared read model from the live projections each frame.
pub fn sync_mini_hud_model(
    overview: Option<Res<LastOverviewProjection>>,
    toggles: Option<Res<SidebarToggleProjection>>,
    surface: Option<Res<LatestSurfaceSnapshot>>,
    mut model: ResMut<MiniHudModel>,
) {
    let mut next = MiniHudModel(infiltrator_contract::mini_hud::MiniHudReadModel::default());
    if let Some(toggles) = toggles.as_deref() {
        next.0 = next.0.with_toggles(&toggles.0);
    }
    if let Some(surface) = surface.as_deref()
        && let Some(settings) = surface.0.pages.settings.data.as_ref()
    {
        next.0 = next.0.with_placement(settings.mini_hud);
    }
    if let Some(overview) = overview
        .as_deref()
        .and_then(|projection| projection.0.as_ref())
    {
        next.0 = next
            .0
            .with_traffic(
                sanitize_rate(overview.upload_bps),
                sanitize_rate(overview.download_bps),
            )
            .with_mode(mode_label(overview.mode))
            .with_exit_node(overview.active_exit.name.clone().unwrap_or_default())
            .with_waveform(&overview.traffic_waveform);
    }
    if *model != next {
        *model = next;
    }
}

fn sanitize_rate(rate: f64) -> u64 {
    if rate.is_finite() && rate > 0.0 {
        rate.round() as u64
    } else {
        0
    }
}

/// Mount / unmount the HUD overlay when visibility or the rendered model
/// changes.
pub fn sync_mini_hud_overlay(
    mut commands: Commands,
    palette: Res<UiPalette>,
    mode: Res<MiniHudMode>,
    model: Res<MiniHudModel>,
    mut signature: ResMut<MiniHudMountSignature>,
    mounted: Query<Entity, With<MiniHudRoot>>,
) {
    let next = MiniHudMountSignature {
        visible: mode.0,
        model: model.clone(),
    };
    if next == *signature {
        return;
    }
    *signature = next;
    for entity in &mounted {
        commands.entity(entity).despawn();
    }
    if !mode.0 {
        return;
    }
    commands.spawn_scene(mini_hud_scene(&model.0, &palette));
}

/// Observer toggling the HUD visibility.
pub fn on_toggle_mini_hud(_trigger: On<ToggleMiniHud>, mut mode: ResMut<MiniHudMode>) {
    mode.0 = !mode.0;
}

/// Rasterize every mounted HUD waveform slot from the shared contract bars.
///
/// Follows the widget layer's chart pattern: the first run stamps an
/// `ImageNode`, later runs rewrite the same texture handle, so a live sample
/// never allocates a new asset. The bars are already normalized by the shared
/// [`MiniHudWaveformStrip`], so the Bevy strip and the Iced canvas draw the
/// same shape from the same snapshot.
pub fn sync_mini_hud_waveforms(
    model: Res<MiniHudModel>,
    palette: Res<UiPalette>,
    images: Option<ResMut<Assets<Image>>>,
    downstream: Query<(Entity, Option<&ImageNode>), With<MiniHudDownWaveform>>,
    upstream: Query<(Entity, Option<&ImageNode>), With<MiniHudUpWaveform>>,
    mut commands: Commands,
) {
    let Some(mut images) = images else {
        return;
    };
    for (entity, node) in &downstream {
        rasterize_waveform_slot(
            &model.0.waveform.down,
            palette.accent,
            palette.chart_fill_down(),
            entity,
            node,
            &mut images,
            &mut commands,
        );
    }
    for (entity, node) in &upstream {
        rasterize_waveform_slot(
            &model.0.waveform.up,
            palette.success,
            palette.chart_fill_up(),
            entity,
            node,
            &mut images,
            &mut commands,
        );
    }
}

fn rasterize_waveform_slot(
    bars: &[u16],
    line: Color,
    fill: Color,
    entity: Entity,
    node: Option<&ImageNode>,
    images: &mut Assets<Image>,
    commands: &mut Commands,
) {
    let samples: Vec<f32> = bars
        .iter()
        .map(|bar| MiniHudWaveformStrip::bar_fraction(*bar))
        .collect();
    let image = sparkline_image(
        &samples,
        MiniHudWaveformStrip::WIDTH_PX,
        MiniHudWaveformStrip::HEIGHT_PX,
        line,
        Some(fill),
    );
    match node {
        Some(node) => {
            if let Some(mut existing) = images.get_mut(&node.image) {
                *existing = image;
            }
        }
        None => {
            let handle = images.add(image);
            commands.entity(entity).insert(ImageNode {
                image: handle,
                ..ImageNode::default()
            });
        }
    }
}

/// Observer: the expand button leaves HUD mode.
pub fn on_expand_mini_hud(
    activate: On<Activate>,
    buttons: Query<(), With<MiniHudExpandButton>>,
    mut mode: ResMut<MiniHudMode>,
) {
    if buttons.get(activate.entity).is_ok() {
        mode.0 = false;
    }
}

/// Observer: the pin button flips always-on-top and persists through the
/// shared settings command path.
pub fn on_pin_mini_hud(
    activate: On<Activate>,
    buttons: Query<(), With<MiniHudPinButton>>,
    surface: Option<Res<LatestSurfaceSnapshot>>,
    mut commands: Commands,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let current = surface
        .as_deref()
        .and_then(|surface| surface.0.pages.settings.data.as_ref())
        .map(|settings| settings.mini_hud)
        .unwrap_or_default();
    commands.trigger(SetMiniHudPinned(!current.pinned));
}

/// Observer for the persistence request (kept separate so tests can drive it
/// without a mounted scene).
pub fn on_set_mini_hud_pinned(trigger: On<SetMiniHudPinned>, sink: Option<Res<CommandSinkHandle>>) {
    let pinned = trigger.event().0;
    let Some(sink) = sink else {
        return;
    };
    // The write is validated by the shared application (`mini_hud.pinned`);
    // the authoritative placement comes back through the settings snapshot.
    sink.submit(UiCommand::UpdateSetting {
        key: "mini_hud.pinned".to_owned(),
        value: pinned.to_string(),
    });
}

/// The HUD quick switch and the sidebar switch share one action rule
/// (`SystemToggleApplication::intent`) and one command vocabulary; this maps
/// the desired state onto the UI command and the local pending projection.
fn submit_toggle(
    toggle: SystemToggle,
    desired: bool,
    projection: &SidebarToggleProjection,
    sink: &CommandSinkHandle,
    commands: &mut Commands,
) {
    if SystemToggleApplication::intent(&projection.0, toggle, desired).is_err() {
        return;
    }
    let command = match toggle {
        SystemToggle::SystemProxy => UiCommand::SetSystemProxy { enabled: desired },
        SystemToggle::Tun => UiCommand::ToggleTun { enabled: desired },
    };
    sink.submit(command);
    commands.insert_resource(SidebarToggleProjection(
        projection.0.clone().with_pending(toggle, desired),
    ));
}

/// Observer: the HUD system-proxy quick switch.
pub fn on_mini_hud_system_proxy_activated(
    activate: On<Activate>,
    buttons: Query<(), With<MiniHudSystemProxyToggle>>,
    projection: Res<SidebarToggleProjection>,
    sink: Option<Res<CommandSinkHandle>>,
    mut commands: Commands,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let Some(sink) = sink else {
        return;
    };
    let desired = !projection.0.state(SystemToggle::SystemProxy).is_enabled();
    submit_toggle(
        SystemToggle::SystemProxy,
        desired,
        &projection,
        &sink,
        &mut commands,
    );
}

/// Observer: the HUD TUN quick switch.
pub fn on_mini_hud_tun_activated(
    activate: On<Activate>,
    buttons: Query<(), With<MiniHudTunToggle>>,
    projection: Res<SidebarToggleProjection>,
    sink: Option<Res<CommandSinkHandle>>,
    mut commands: Commands,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let Some(sink) = sink else {
        return;
    };
    let desired = !projection.0.state(SystemToggle::Tun).is_enabled();
    submit_toggle(
        SystemToggle::Tun,
        desired,
        &projection,
        &sink,
        &mut commands,
    );
}

/// The Mini HUD plugin: model, mount latch, observers.
pub struct MiniHudPlugin;

impl Plugin for MiniHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MiniHudMode>();
        app.init_resource::<MiniHudModel>();
        app.init_resource::<MiniHudMountSignature>();
        app.add_observer(on_toggle_mini_hud);
        app.add_observer(on_expand_mini_hud);
        app.add_observer(on_pin_mini_hud);
        app.add_observer(on_set_mini_hud_pinned);
        app.add_observer(on_mini_hud_system_proxy_activated);
        app.add_observer(on_mini_hud_tun_activated);
        app.add_systems(
            Update,
            (
                sync_mini_hud_model,
                sync_mini_hud_overlay,
                sync_mini_hud_waveforms,
            )
                .chain(),
        );
    }
}
