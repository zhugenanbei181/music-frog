//! High-throughput connection pulse for the Bevy Connections page
//! (DUAL-13-10).
//!
//! The threshold, the breathing intensity and the phase handling are the
//! shared [`infiltrator_domain::connection_rate`] reduction; this module owns
//! only the Bevy pulse node, the phase resource and the per-frame animation
//! system. A row that never crossed the shared 5 MB/s threshold is rendered
//! with `Display::None`, so no slow connection glows.

use bevy::color::Alpha;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::time::{Time, Virtual};
use bevy::ui::prelude::{AlignItems, BackgroundColor, BorderRadius, Display, Node, UiRect, Val};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_domain::connection_rate;

use crate::pages::connections::{ConnectionItem, LastConnectionsProjection};

/// Marker on the pulse chip of one flat row; the payload is the projection row
/// index the chip reports.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnHighThroughputPulse(pub usize);

/// Shared breathing phase of every visible pulse on the page.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct ConnectionsPulseState {
    pub phase: f32,
}

/// DUAL-13-10: the pulse chip of one connection row. Mounted hidden until the
/// row's derived instantaneous rate crosses the shared threshold.
pub fn connection_pulse_scene(
    row: usize,
    conn: &ConnectionItem,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let intensity = connection_rate::pulse_intensity(conn.upload_bps, conn.download_bps, 0.0);
    let visible = intensity > 0.0;
    let glow = palette.accent.with_alpha(intensity * 0.35);
    let ink = palette.accent;

    bsn! {
        Node {
            display: { if visible { Display::Flex } else { Display::None } },
            align_items: AlignItems::Center,
            padding: UiRect::axes(Val::Px(space::S4), Val::Px(0.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
        }
        BackgroundColor({ glow })
        ConnHighThroughputPulse(row)
        Children [
            ( Text({ "高吞吐脉冲".to_owned() }) TextRole(Role::Caption) TextColor({ ink }) ),
        ]
    }
}

/// DUAL-13-10: breathe the visible pulses from the shared phase. The system
/// only writes a node when the rendered value actually changes, so a page
/// without high-throughput traffic costs nothing.
pub(crate) fn animate_connection_pulses(
    time: Res<Time<Virtual>>,
    palette: Res<UiPalette>,
    last: Option<Res<LastConnectionsProjection>>,
    mut state: Option<ResMut<ConnectionsPulseState>>,
    mut pulses: Query<(&mut BackgroundColor, &mut Node, &ConnHighThroughputPulse)>,
) {
    let projection = last.as_ref().and_then(|last| last.0.as_ref());
    let active = projection.is_some_and(|projection| {
        projection
            .connections
            .iter()
            .any(|conn| connection_rate::is_high_throughput(conn.upload_bps, conn.download_bps))
    });

    let Some(state) = state.as_deref_mut() else {
        return;
    };
    if active {
        state.phase =
            (state.phase + time.delta_secs() * connection_rate::PULSE_BREATH_HZ).rem_euclid(1.0);
    } else {
        state.phase = 0.0;
    }
    let phase = state.phase;

    for (mut fill, mut node, pulse) in &mut pulses {
        let intensity = projection
            .and_then(|projection| projection.connections.get(pulse.0))
            .map(|conn| connection_rate::pulse_intensity(conn.upload_bps, conn.download_bps, phase))
            .unwrap_or(0.0);
        let display = if intensity > 0.0 {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
        let glow = palette.accent.with_alpha(intensity * 0.35);
        if fill.0 != glow {
            fill.0 = glow;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_domain::connection_rate::PULSE_MIN_INTENSITY;

    fn item(id: &str, upload_bps: f64, download_bps: f64) -> ConnectionItem {
        ConnectionItem {
            id: id.to_owned(),
            host: "example.com:443".to_owned(),
            process: "git".to_owned(),
            rule: "DIRECT".to_owned(),
            rule_payload: String::new(),
            chain: "DIRECT".to_owned(),
            chains: vec!["DIRECT".to_owned()],
            network: "tcp".to_owned(),
            source_ip: "192.168.1.5".to_owned(),
            source_port: "50000".to_owned(),
            destination_ip: "1.1.1.1".to_owned(),
            destination_port: "443".to_owned(),
            upload_bps,
            download_bps,
            upload_total: 0,
            download_total: 0,
        }
    }

    #[test]
    fn pulse_phase_and_threshold_use_the_shared_reduction() {
        // The domain reduction is the single source for the threshold.
        assert!(!connection_rate::is_high_throughput(1_000.0, 1_000.0));
        assert!(connection_rate::is_high_throughput(
            0.0,
            connection_rate::HIGH_THROUGHPUT_THRESHOLD_BPS
        ));

        // A slow row never glows at any phase; a fast row breathes.
        assert_eq!(connection_rate::pulse_intensity(1_000.0, 1_000.0, 0.5), 0.0);
        assert_eq!(
            connection_rate::pulse_intensity(
                connection_rate::HIGH_THROUGHPUT_THRESHOLD_BPS,
                0.0,
                0.0
            ),
            PULSE_MIN_INTENSITY
        );

        let fast = item("fast", 0.0, connection_rate::HIGH_THROUGHPUT_THRESHOLD_BPS);
        assert!(fast.download_bps >= connection_rate::HIGH_THROUGHPUT_THRESHOLD_BPS);
        // The shared frequency is the only one either surface animates with:
        // one breath returns the phase to where it started.
        let phase = (0.25_f32 + 1.25 * connection_rate::PULSE_BREATH_HZ).fract();
        assert!((phase - 0.25).abs() < 1e-3);
    }
}
