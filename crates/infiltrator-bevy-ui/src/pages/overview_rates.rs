//! Native overview copy replays shared facts and locale without replacing entities.
use crate::pages::overview::{
    LastOverviewProjection, OverviewChip, OverviewChipKind, OverviewLine, OverviewLineKind,
    banner_note, chip_label, format_cpu, format_memory, format_scale, format_total_traffic,
    state_ink,
};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{Has, Or, QueryData, QueryFilter, With, Without};
use bevy::ecs::system::{Query, Res, SystemParam};
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use infiltrator_application::core_status_projection::{failure_copy, lifecycle_copy};
use infiltrator_application::proxy_mode_projection::mode_status_copy;
use infiltrator_application::shell_readout_projection::{rate_copy, rate_status};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::stat_chip::{StatChipLabel, StatChipValue};

#[derive(QueryData)]
#[query_data(mutable)]
pub struct OverviewCopyLine {
    kind: &'static OverviewLine,
    text: &'static mut Text,
    color: &'static mut TextColor,
    semantic: Option<&'static mut AccessibilityNode>,
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct OverviewCopyChip {
    entity: Entity,
    kind: &'static OverviewChip,
    semantic: Option<&'static mut AccessibilityNode>,
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct ChipCaption {
    text: &'static mut Text,
    is_label: Has<StatChipLabel>,
}
#[derive(QueryFilter)]
pub struct ChipCaptionFilter {
    copy: Or<(With<StatChipValue>, With<StatChipLabel>)>,
    line: Without<OverviewLine>,
}
#[derive(SystemParam)]
pub struct OverviewCopy<'w, 's> {
    last: Res<'w, LastOverviewProjection>,
    locale: Res<'w, UiLocale>,
    palette: Res<'w, UiPalette>,
    lines: Query<'w, 's, OverviewCopyLine>,
    chips: Query<'w, 's, OverviewCopyChip, Without<OverviewLine>>,
    children: Query<'w, 's, &'static Children>,
    captions: Query<'w, 's, ChipCaption, ChipCaptionFilter>,
}
pub fn refresh(mut copy: OverviewCopy) {
    if !copy.last.is_changed() && !copy.locale.is_changed() {
        return;
    }
    let Some(projection) = &copy.last.0 else {
        return;
    };
    let language = copy.locale.code();
    let upload = rate_copy(&projection.readout.upload_bps, language);
    let download = rate_copy(&projection.readout.download_bps, language);
    for mut line in &mut copy.lines {
        let value = match line.kind.0 {
            OverviewLineKind::State => {
                line.color.0 = state_ink(&projection.lifecycle, &copy.palette);
                lifecycle_copy(&projection.lifecycle, language)
            }
            OverviewLineKind::Upload => format!("↑ {upload}"),
            OverviewLineKind::Download => format!("↓ {download}"),
            OverviewLineKind::TelemetryFailure => rate_status(&projection.readout, language),
            OverviewLineKind::ModeChip => mode_status_copy(&projection.proxy_mode, language),
            OverviewLineKind::BannerNote => banner_note(projection, language),
            OverviewLineKind::Failure => failure_copy(
                &projection.lifecycle,
                projection.failure.as_deref(),
                language,
            ),
            OverviewLineKind::Scale => format_scale(&projection.traffic_scale),
        };
        if let Some(mut semantic) = line.semantic {
            semantic.0.set_label(value.as_str());
        }
        if line.text.0 != value {
            line.text.0 = value;
        }
    }
    for chip in &mut copy.chips {
        let label = chip_label(chip.kind.0, language);
        let value = match chip.kind.0 {
            OverviewChipKind::Connections => projection.active_connections.to_string(),
            OverviewChipKind::Memory => format_memory(projection.memory_bytes),
            OverviewChipKind::Cpu => format_cpu(projection.cpu_percent),
            OverviewChipKind::Upload => upload.clone(),
            OverviewChipKind::Download => download.clone(),
            OverviewChipKind::TotalTraffic => format_total_traffic(projection.total_traffic_bytes),
        };
        if let Some(mut semantic) = chip.semantic {
            semantic.0.set_label(format!("{label} {value}"));
        }
        for descendant in copy.children.iter_descendants(chip.entity) {
            if let Ok(mut caption) = copy.captions.get_mut(descendant) {
                let desired = if caption.is_label { &label } else { &value };
                if caption.text.0 != *desired {
                    caption.text.0.clone_from(desired);
                }
            }
        }
    }
}
