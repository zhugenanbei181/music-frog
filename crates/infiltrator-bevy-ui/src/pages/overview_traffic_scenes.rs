//! The Overview traffic scene cluster: the mini rate cards, the detail
//! card, the historical trend chart, the scrubber controls and the stat
//! chip rows. It is the Overview page's own traffic composition, split
//! out of `overview.rs` so one file carries one business seam.

use crate::history::ScrubberAction;
use crate::localized_widgets::localized_button_scene;
use crate::pages::overview::overview_reactive::{OverviewMetricKind, OverviewMetricSource};
use crate::pages::overview::{
    CHART_HEIGHT_PX, CHART_WIDTH_PX, OverviewChip, OverviewChipKind, OverviewLine,
    OverviewLineKind, OverviewMetricsBand, OverviewScrubberButton, OverviewScrubberStatus,
    OverviewTrafficChart, TrafficMiniReading, chip_label, format_cpu, format_memory,
    format_total_traffic, stat_group_semantic_node,
};
use crate::projection::OverviewProjection;
use bevy::ecs::hierarchy::Children;
use bevy::feathers::theme::ThemeTextColor;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{AlignItems, FlexDirection, FlexWrap, JustifyContent, Node, percent, px};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::button::{ButtonSize, ButtonVariant, button_sized_scene};
use infiltrator_bevy_widgets::chart::chart_scene_with_scale;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::stat_chip::stat_chip_scene;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::TextRole;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_bevy_widgets::waveform::{WaveformSeries, waveform_scene};
use infiltrator_contract::design_resources;
use infiltrator_presentation::shell_readout::{rate_copy, rate_status};
use infiltrator_shared::locales::Lang;

pub(crate) fn traffic_mini_card_scene(
    reading: TrafficMiniReading,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let TrafficMiniReading {
        series,
        icon,
        label_key,
        rate,
        peak,
        total,
        samples,
    } = reading;
    let is_up = series == WaveformSeries::Upload;
    let rate_kind = if is_up {
        OverviewLineKind::Upload
    } else {
        OverviewLineKind::Download
    };
    let peak_kind = if is_up {
        OverviewLineKind::UploadPeak
    } else {
        OverviewLineKind::DownloadPeak
    };
    let total_kind = if is_up {
        OverviewLineKind::UploadTotal
    } else {
        OverviewLineKind::DownloadTotal
    };
    let rate_ink = if is_up {
        palette.success
    } else {
        palette.accent
    };

    surface_scene(
        vec![
            Box::new(bsn! {
                    Node {
                        width: percent(100),
                        min_width: px(0.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(space::S6),
                    }
                    Children [
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                        }
                        Children [
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: rem(space::S6),
                            }
                            Children [
                                @{ icon_scene(icon, 14.0, palette.accent) }
                                --
                                LocalizedText::plain(label_key) TextRole(design_resources::TextRole::Caption)
                            ]
                            --
                            Text({ peak }) OverviewLine(peak_kind) TextRole(design_resources::TextRole::Caption) ThemeTextColor({ Token::InkDim })
                        ]
                        --
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            min_width: px(0.0),
                        }
                        Children [
                            Text({ format!("{} {}", if is_up { "↑" } else { "↓" }, rate) })
                            OverviewLine(rate_kind) TextRole(design_resources::TextRole::Heading) TextColor({ rate_ink })
                        ]
                        --
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                        }
                        Children [
                            Text({ total }) OverviewLine(total_kind) TextRole(design_resources::TextRole::Caption) ThemeTextColor({ Token::InkDim })
                        ]
                    ]
            }),
            Box::new(waveform_scene(series, samples)),
        ],
        palette,
    )
}

/// The retained trend detail card: caption title, both mono rate lines and
/// the shared dual-series chart with its time-travel scrubber.
pub(crate) fn traffic_detail_card_scene(
    projection: &OverviewProjection,
    up: Vec<f32>,
    down: Vec<f32>,
    smooth: bool,
    scale_max: f32,
    palette: &UiPalette,
) -> impl Scene + use<> {
    surface_scene(
        vec![
            Box::new(
                bsn! { LocalizedText::plain("overview_traffic") TextRole(design_resources::TextRole::Caption) },
            ),
            Box::new(rates_row_scene(
                rate_copy(&projection.readout.upload_bps, &Lang("en-US")),
                rate_copy(&projection.readout.download_bps, &Lang("en-US")),
            )),
            Box::new(
                bsn! { Text({rate_status(&projection.readout, &Lang("en-US"))})
                OverviewLine(OverviewLineKind::TelemetryFailure) TextRole(design_resources::TextRole::Caption) },
            ),
            Box::new(overview_chart_scene(up, down, smooth, Some(scale_max))),
            Box::new(scrubber_scene(palette)),
        ],
        palette,
    )
}

/// The trend chart plate, marked so the projection restamp and the replay
/// scrubber address only the Overview chart.
pub(crate) fn overview_chart_scene(
    up: Vec<f32>,
    down: Vec<f32>,
    smooth: bool,
    scale_max: Option<f32>,
) -> impl Scene + use<> {
    bsn! {
        @{ chart_scene_with_scale(
            up,
            down,
            CHART_WIDTH_PX,
            CHART_HEIGHT_PX,
            smooth,
            scale_max,
        ) }
        OverviewTrafficChart
    }
}

/// The time-travel scrubber row under the trend chart: coarse seeks, single
/// steps, a return-to-live button and the typed status readout. Purely
/// structural — the buttons' `Activate` observer and
/// [`sync_overview_replay`](crate::pages::overview_restamp::sync_overview_replay)
/// own the behavior.
pub(crate) fn scrubber_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                align_items: AlignItems::Center,
                column_gap: rem(space::S8),
                row_gap: rem(space::S4),
                flex_wrap: FlexWrap::Wrap,
            }
            Children [
                @{ scrubber_button_scene("⏮".to_owned(), ScrubberAction::SeekBackward, palette) }
                --
                @{ scrubber_button_scene("◀".to_owned(), ScrubberAction::StepBackward, palette) }
                --
                @{ scrubber_button_scene("▶".to_owned(), ScrubberAction::StepForward, palette) }
                --
                @{ scrubber_button_scene("⏭".to_owned(), ScrubberAction::SeekForward, palette) }
                --
                @{ scrubber_live_button_scene(palette) }
                --
                Text({ String::new() }) OverviewScrubberStatus TextRole(design_resources::TextRole::Caption)
            ]
    }
}

/// One transport button: the shared button skin with a typed scrub action.
pub(crate) fn scrubber_button_scene(
    label: String,
    action: ScrubberAction,
    palette: &UiPalette,
) -> impl Scene + use<> {
    bsn! {
        @{ button_sized_scene(label, ButtonVariant::Ghost, ButtonSize::Sm, palette) }
        OverviewScrubberButton(action)
    }
}

/// The return-to-live button: a primary-labeled control carrying the live
/// copy key, so it follows the active locale.
pub(crate) fn scrubber_live_button_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        @{ localized_button_scene(
            LocalizedText::plain("conn_state_live"),
            ButtonVariant::Primary,
            palette,
        ) }
        OverviewScrubberButton(ScrubberAction::ReturnToLive)
    }
}

/// The up/down rates side by side on one row (the reference layout).
pub(crate) fn rates_row_scene(upload: String, download: String) -> impl Scene + use<> {
    bsn! {
            Node {
                align_items: AlignItems::Center,
                column_gap: rem(space::S16),
            }
            Children [
                @{ rate_line("↑ ", OverviewLineKind::Upload, upload) }
                --
                @{ rate_line("↓ ", OverviewLineKind::Download, download) }
            ]
    }
}

/// One rate line: the arrow and the mono rate share one marked text so
/// the refresh observer restamps them together (the arrow keeps the
/// line's ink — success for uplink, ordinary for downlink).
pub(crate) fn rate_line(arrow: &str, kind: OverviewLineKind, value: String) -> impl Scene + use<> {
    bsn! {
            Node {
                align_items: AlignItems::Center,
                column_gap: rem(space::S8),
            }
            Children [
                Text({ format!("{arrow}{value}") }) OverviewLine(kind) TextRole(design_resources::TextRole::Mono)
            ]
    }
}

/// A plain caption line inside a card.
/// The metrics band: four stat chips sharing the width evenly. Each chip
/// root carries a labeled `Group` semantic node ("name value") that the
/// refresh observer restamps alongside the visible value.
pub(crate) fn chips_row_scene(
    projection: &OverviewProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let connections = projection.active_connections.to_string();
    let memory = format_memory(&projection.memory_bytes);
    let cpu = format_cpu(&projection.cpu_percent);
    let upload = rate_copy(&projection.readout.upload_bps, &Lang("en-US"));
    let download = rate_copy(&projection.readout.download_bps, &Lang("en-US"));
    let total = format_total_traffic(projection.total_traffic_bytes);

    let connections_node = stat_group_semantic_node(&format!(
        "{} {connections}",
        chip_label(OverviewChipKind::Connections, "en-US")
    ));
    let memory_node = stat_group_semantic_node(&format!(
        "{} {memory}",
        chip_label(OverviewChipKind::Memory, "en-US")
    ));
    let cpu_node = stat_group_semantic_node(&format!(
        "{} {cpu}",
        chip_label(OverviewChipKind::Cpu, "en-US")
    ));
    let upload_node = stat_group_semantic_node(&format!(
        "{} {upload}",
        chip_label(OverviewChipKind::Upload, "en-US")
    ));
    let download_node = stat_group_semantic_node(&format!(
        "{} {download}",
        chip_label(OverviewChipKind::Download, "en-US")
    ));
    let total_node = stat_group_semantic_node(&format!(
        "{} {total}",
        chip_label(OverviewChipKind::TotalTraffic, "en-US")
    ));

    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: rem(space::S12),
                row_gap: rem(space::S12),
            }
            OverviewMetricsBand
            Children [
                @{ stat_chip_scene(IconId::Activity, chip_label(OverviewChipKind::Connections, "en-US").to_owned(), connections, palette) }
                OverviewChip(OverviewChipKind::Connections)
                OverviewMetricSource(OverviewMetricKind::Connections)
                connections_node
                --
                @{ stat_chip_scene(IconId::Zap, chip_label(OverviewChipKind::Memory, "en-US").to_owned(), memory, palette) }
                OverviewChip(OverviewChipKind::Memory)
                OverviewMetricSource(OverviewMetricKind::Memory)
                memory_node
                --
                @{ stat_chip_scene(IconId::Settings, chip_label(OverviewChipKind::Cpu, "en-US").to_owned(), cpu, palette) }
                OverviewChip(OverviewChipKind::Cpu)
                OverviewMetricSource(OverviewMetricKind::Cpu)
                cpu_node
                --
                @{ stat_chip_scene(IconId::ArrowUp, chip_label(OverviewChipKind::Upload, "en-US").to_owned(), upload, palette) }
                OverviewChip(OverviewChipKind::Upload)
                upload_node
                --
                @{ stat_chip_scene(IconId::ArrowDown, chip_label(OverviewChipKind::Download, "en-US").to_owned(), download, palette) }
                OverviewChip(OverviewChipKind::Download)
                download_node
                --
                @{ stat_chip_scene(IconId::Globe, chip_label(OverviewChipKind::TotalTraffic, "en-US").to_owned(), total, palette) }
                OverviewChip(OverviewChipKind::TotalTraffic)
                total_node
            ]
    }
}
