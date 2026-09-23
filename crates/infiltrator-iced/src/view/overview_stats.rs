//! The six-tile runtime metrics grid with mono numerals, wrapped to the
//! shell tier's column count.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::card_surface;
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, MONO, R_CONTROL, tokens};
use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

/// 连接数 / 内存 / 上传 / 下载 tiles with mono numerals.
///
/// The six tiles wrap into rows of `metrics_grid_columns` for the shell's
/// current tier (2 / 3 / 6 / 6) so a compact window never squeezes six columns
/// into one unreadable strip.
pub fn stats_grid<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let connections = state
        .diag
        .connections
        .as_ref()
        .map(|snapshot| snapshot.connections.len().to_string())
        .unwrap_or_else(|| "—".to_string());
    let memory = state
        .diag
        .memory
        .as_ref()
        .map(|memory| crate::utils::format_bytes(memory.in_use))
        .unwrap_or_else(|| "—".to_string());
    let upload = state
        .diag
        .traffic
        .as_ref()
        .map(|traffic| format!("{}/s", crate::utils::format_bytes(traffic.up)))
        .unwrap_or_else(|| "—".to_string());
    let cpu = state
        .runtime
        .core_resources
        .cpu_percent
        .map(|p| format!("{:.1}%", p))
        .unwrap_or_else(|| "—".to_string());
    let download = state
        .diag
        .traffic
        .as_ref()
        .map(|traffic| format!("{}/s", crate::utils::format_bytes(traffic.down)))
        .unwrap_or_else(|| "—".to_string());
    let total = state
        .diag
        .connections
        .as_ref()
        .map(|c| crate::utils::format_bytes(c.download_total + c.upload_total))
        .unwrap_or_else(|| "—".to_string());

    let tiles: Vec<Element<'a, Message>> = vec![
        metric_tile(
            Icon::Activity,
            lang.tr("overview_connections").to_string(),
            connections,
            |t| tokens(t).accent,
        ),
        metric_tile(
            Icon::Server,
            lang.tr("overview_memory").to_string(),
            memory,
            |t| tokens(t).warning,
        ),
        metric_tile(Icon::Zap, "CPU".to_string(), cpu, |t| tokens(t).accent),
        metric_tile(
            Icon::ArrowUp,
            lang.tr("overview_upload").to_string(),
            upload,
            |t| tokens(t).success,
        ),
        metric_tile(
            Icon::ArrowDown,
            lang.tr("overview_download").to_string(),
            download,
            |t| tokens(t).accent,
        ),
        metric_tile(
            Icon::Globe,
            lang.tr("overview_total_traffic").to_string(),
            total,
            |t| tokens(t).success,
        ),
    ];

    // Column count comes from the shared tier operator, so Iced and Bevy agree.
    let columns = state
        .shell
        .viewport
        .tier
        .metrics_grid_columns()
        .clamp(1, tiles.len().max(1));

    let mut grid = column![].spacing(theme::SP_MD).width(Length::Fill);
    let mut row_tiles: Vec<Element<'a, Message>> = Vec::with_capacity(columns);
    for tile in tiles {
        row_tiles.push(tile);
        if row_tiles.len() == columns {
            grid = grid.push(
                row::Row::with_children(std::mem::take(&mut row_tiles))
                    .spacing(theme::SP_MD)
                    .width(Length::Fill),
            );
        }
    }
    if !row_tiles.is_empty() {
        // Pad the trailing row so the last tiles keep the same width as a full row.
        for _ in row_tiles.len()..columns {
            row_tiles.push(Space::new().width(Length::FillPortion(1)).into());
        }
        grid = grid.push(
            row::Row::with_children(row_tiles)
                .spacing(theme::SP_MD)
                .width(Length::Fill),
        );
    }

    grid.into()
}

fn metric_tile<'a>(
    glyph: Icon,
    label: String,
    value: String,
    color: impl Fn(&Theme) -> Color + Copy + 'a,
) -> Element<'a, Message> {
    let icon_chip = container(icon_themed(glyph, 16.0, color))
        .width(36)
        .height(36)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |t: &Theme| {
            let c = color(t);
            container::Style {
                background: Some(Color { a: 0.14, ..c }.into()),
                border: Border {
                    radius: border::Radius::from(R_CONTROL),
                    ..Default::default()
                },
                ..Default::default()
            }
        });

    container(
        row![
            icon_chip,
            column![
                text(label)
                    .size(11)
                    .font(FONT_MEDIUM)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }),
                text(value)
                    .size(15)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary),
                    }),
            ]
            .spacing(2),
        ]
        .spacing(theme::SP_SM)
        .align_y(Alignment::Center),
    )
    .width(Length::FillPortion(1))
    .padding([theme::SP_MD, theme::SP_MD])
    .style(card_surface)
    .into()
}
