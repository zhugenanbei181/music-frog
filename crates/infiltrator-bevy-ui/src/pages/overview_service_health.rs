//! The Overview service-health strip: the four fastest owner-published probe
//! facts, never invented service names.
//!
//! The card is a passive projection of the shared speedtest snapshot. A tile
//! without an observed result stays "not observed"; a timed-out result keeps
//! its danger status instead of being rewritten as a latency.

use crate::host_capabilities::{HostPreferences, MotionPreference};
use crate::pages::overview::LastOverviewProjection;
use crate::pages::overview::overview_pulse::OverviewPulseState;
use bevy::color::{Alpha, Color};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::Without;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::text::TextLayout;
use bevy::ui::Overflow;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect,
    percent, px,
};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::smart_truncate::truncate_tail;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::TextRole;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::design_resources;
use infiltrator_contract::speedtest::{SpeedtestResultState, SpeedtestSnapshot};
use infiltrator_presentation::latency_projection::{LatencyPresentation, project_measured_latency};
use infiltrator_shared::locales::Lang;
use std::f32::consts::TAU;

/// The dashboard keeps a compact 2x2 health matrix.
pub const SERVICE_HEALTH_TILE_COUNT: usize = 4;

/// The maximum visible service-name length in a compact two-column tile.
const SERVICE_HEALTH_NAME_MAX_CHARS: usize = 9;
/// One health tile's status dot square, in baseline pixels.
const SERVICE_HEALTH_DOT_PX: f32 = 8.0;

/// One reorder-stable tile in the service-health card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServiceHealthTile(pub usize);

/// Marker on a tile's display-name text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServiceHealthName(pub usize);

/// Marker on a tile's latency text and ink.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServiceHealthLatency(pub usize);

/// The owner-published facts one pulse dot projects.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ServiceHealthPulse {
    pub index: usize,
    pub latency_ms: Option<u32>,
    pub observed: bool,
}

/// The owner-published latency of the tile at `index`.
pub(crate) fn service_health_fact(
    speedtest: &SpeedtestSnapshot,
    index: usize,
) -> Option<(Option<String>, Option<u32>, bool)> {
    speedtest.results.get(index).map(|result| {
        (
            result.display_name.clone(),
            result.delay_ms(),
            !matches!(
                result.state,
                SpeedtestResultState::Unsupported | SpeedtestResultState::Cancelled
            ),
        )
    })
}

pub(crate) fn service_health_card_scene(
    speedtest: &SpeedtestSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                Children [
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: rem(space::S8),
                    }
                    Children [
                        @{ icon_scene(IconId::Activity, 16.0, palette.accent) }
                        --
                    LocalizedText::plain("overview_service_health") TextRole(design_resources::TextRole::Heading)
                    ]
                ]
            }),
            Box::new(bsn! {
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(space::S8),
                    }
                    Children [
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Row,
                            column_gap: rem(space::S8),
                        }
                        Children [
                            @{ service_health_tile_scene(speedtest, 0, palette) }
                            --
                            @{ service_health_tile_scene(speedtest, 1, palette) }
                        ]
                        --
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Row,
                            column_gap: rem(space::S8),
                        }
                        Children [
                            @{ service_health_tile_scene(speedtest, 2, palette) }
                            --
                            @{ service_health_tile_scene(speedtest, 3, palette) }
                        ]
                    ]
            }),
        ],
        palette,
    )
}

fn service_health_tile_scene(
    speedtest: &SpeedtestSnapshot,
    index: usize,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let fact = service_health_fact(speedtest, index);
    let name = display_service_health_name(
        fact.as_ref()
            .and_then(|(name, _, _)| name.clone())
            .unwrap_or_else(|| UiLocale::default().text("overview_service_tile_unobserved")),
    );
    let latency = fact.as_ref().and_then(|(_, latency, _)| *latency);
    let presentation = project_measured_latency(latency);
    let latency_text = if fact.is_some() {
        latency.map_or_else(
            || "—".to_owned(),
            |_| presentation.render(&Lang(UiLocale::default().code())),
        )
    } else {
        "—".to_owned()
    };
    let ink = fact.as_ref().map_or_else(
        || palette.ink_dim,
        |(_, _, observed)| {
            if *observed {
                latency_color(&presentation, palette)
            } else {
                palette.ink_dim
            }
        },
    );
    bsn! {
            Node {
                flex_grow: 1.0,
                flex_basis: px(0.0),
                min_width: px(0.0),
                padding: UiRect::all(rem(space::S8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: rem(space::S6),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            BackgroundColor({ palette.surface_elevated })
            ServiceHealthTile({ index })
            Children [
                Node {
                    align_items: AlignItems::Center,
                    column_gap: rem(space::S6),
                    flex_shrink: 1.0,
                    min_width: px(0.0),
                    overflow: Overflow::clip_x(),
                }
                Children [
                    Node {
                        width: rem(SERVICE_HEALTH_DOT_PX),
                        height: rem(SERVICE_HEALTH_DOT_PX),
                        border_radius: BorderRadius::MAX,
                        flex_shrink: 0.0,
                    }
                    BackgroundColor({ ink })
                    ServiceHealthPulse { index: { index }, latency_ms: { latency }, observed: { fact.is_some() } }
                    --
                    Text({ name })
                    ServiceHealthName({ index })
                    TextLayout::no_wrap()
                    TextRole(design_resources::TextRole::Caption)
                ]
                --
                Text({ latency_text })
                ServiceHealthLatency({ index })
                TextLayout::no_wrap()
                TextRole(design_resources::TextRole::Mono)
                TextColor({ ink })
            ]
    }
}

fn display_service_health_name(name: String) -> String {
    truncate_tail(&name, SERVICE_HEALTH_NAME_MAX_CHARS)
}

fn latency_color(presentation: &LatencyPresentation, palette: &UiPalette) -> Color {
    use infiltrator_contract::latency_display::LatencyBand;
    match presentation.band {
        LatencyBand::Fast => palette.success,
        LatencyBand::Medium => palette.warning,
        LatencyBand::Slow | LatencyBand::UnconfirmedZero => palette.danger,
        LatencyBand::NotObserved => palette.ink_dim,
    }
}

/// Keep the passive tile projection aligned with the shared snapshot.
pub(crate) fn sync_service_health_tiles(
    last: Option<Res<LastOverviewProjection>>,
    locale: Res<UiLocale>,
    palette: Res<UiPalette>,
    mut names: Query<(&ServiceHealthName, &mut Text), Without<ServiceHealthLatency>>,
    mut latencies: Query<(&ServiceHealthLatency, &mut Text, &mut TextColor)>,
    mut pulses: Query<&mut ServiceHealthPulse>,
) {
    let Some(projection) = last.as_deref().and_then(|last| last.0.as_ref()) else {
        return;
    };
    for (marker, mut text) in names.iter_mut() {
        let fact = service_health_fact(&projection.speedtest, marker.0);
        let name = display_service_health_name(
            fact.as_ref()
                .and_then(|(name, _, _)| name.clone())
                .unwrap_or_else(|| locale.text("overview_service_tile_unobserved")),
        );
        if text.0 != name {
            text.0 = name;
        }
    }
    for (marker, mut text, mut color) in latencies.iter_mut() {
        let fact = service_health_fact(&projection.speedtest, marker.0);
        let latency = fact.as_ref().and_then(|(_, latency, _)| *latency);
        let presentation = project_measured_latency(latency);
        let latency_text = if fact.is_some() {
            latency.map_or_else(
                || "—".to_owned(),
                |_| presentation.render(&Lang(locale.code())),
            )
        } else {
            "—".to_owned()
        };
        let ink = fact.as_ref().map_or_else(
            || palette.ink_dim,
            |(_, _, observed)| {
                if *observed {
                    latency_color(&presentation, &palette)
                } else {
                    palette.ink_dim
                }
            },
        );
        if text.0 != latency_text {
            text.0 = latency_text;
        }
        if color.0 != ink {
            color.0 = ink;
        }
    }
    for mut pulse in &mut pulses {
        let fact = service_health_fact(&projection.speedtest, pulse.index);
        let latency = fact.as_ref().and_then(|(_, latency, _)| *latency);
        let wanted = ServiceHealthPulse {
            index: pulse.index,
            latency_ms: latency,
            observed: fact.is_some(),
        };
        if *pulse != wanted {
            *pulse = wanted;
        }
    }
}

/// Breathe each health dot with a tile-specific phase offset. Reduced motion
/// keeps one honest, quiet state instead of animating.
pub(crate) fn animate_service_health_pulses(
    palette: Res<UiPalette>,
    state: Option<Res<OverviewPulseState>>,
    preferences: Option<Res<HostPreferences>>,
    mut pulses: Query<(&ServiceHealthPulse, &mut BackgroundColor)>,
) {
    let Some(state) = state.as_deref() else {
        return;
    };
    let motion = preferences
        .as_deref()
        .map_or(MotionPreference::Standard, |prefs| prefs.effective_motion());
    let phase = if motion == MotionPreference::Standard {
        state.phase
    } else {
        0.0
    };
    for (pulse, mut fill) in &mut pulses {
        let tile_phase = (phase + pulse.index as f32 * 0.25).rem_euclid(1.0);
        let breath = 0.5 - 0.5 * (tile_phase * TAU).cos();
        let presentation = project_measured_latency(pulse.latency_ms);
        let base = if pulse.observed {
            latency_color(&presentation, &palette)
        } else {
            palette.ink_dim
        };
        let alpha = if motion == MotionPreference::Standard {
            0.45 + 0.35 * breath
        } else {
            0.7
        };
        let color = base.with_alpha(alpha);
        if fill.0 != color {
            fill.0 = color;
        }
    }
}
