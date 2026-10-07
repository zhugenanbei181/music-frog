//! Scoped modal copy, visibility and SDK disabled state before UI preparation.
use crate::pages::logs_export::{
    LogsExportAction, LogsExportCard, LogsExportLine, LogsExportRoot, LogsExportState,
};
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{QueryData, QueryFilter, With, Without};
use bevy::ecs::system::{Query, Res, SystemParam};
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{BorderColor, Display, Node};
use infiltrator_application::log_export_projection::project_log_export;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::modal::ModalScrim;
use infiltrator_bevy_widgets::palette::UiPalette;
#[derive(QueryData)]
#[query_data(mutable)]
struct ExportControl {
    action: &'static LogsExportAction,
    node: &'static mut Node,
    disabled: &'static mut ButtonDisabled,
    children: Option<&'static Children>,
    label: Option<&'static mut LocalizedLabel>,
    scrim: Option<&'static ModalScrim>,
}
#[derive(QueryFilter)]
struct ExportControls {
    action: With<LogsExportAction>,
    root: Without<LogsExportRoot>,
}
#[derive(SystemParam)]
pub struct ExportSurface<'w, 's> {
    state: Res<'w, LogsExportState>,
    locale: Res<'w, UiLocale>,
    palette: Res<'w, UiPalette>,
    roots: Query<'w, 's, &'static mut Node, (With<LogsExportRoot>, Without<LogsExportAction>)>,
    actions: Query<'w, 's, ExportControl, ExportControls>,
    lines: Query<
        'w,
        's,
        (
            &'static LogsExportLine,
            &'static mut Text,
            &'static mut TextColor,
        ),
    >,
    copies: Query<'w, 's, &'static mut LocalizedText>,
    cards: Query<'w, 's, &'static mut BorderColor, With<LogsExportCard>>,
}
pub fn render(mut surface: ExportSurface) {
    let view = project_log_export(&surface.state.model, surface.locale.code());
    for mut root in &mut surface.roots {
        root.display = if surface.state.model.open {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut control in &mut surface.actions {
        let enabled = match control.action {
            LogsExportAction::Confirm => view.confirm,
            LogsExportAction::Cancel => view.close,
            LogsExportAction::Retry => view.retry,
        };
        let shown = match control.action {
            LogsExportAction::Confirm => view.confirm,
            LogsExportAction::Cancel => true,
            LogsExportAction::Retry => view.retry,
        };
        control.node.display = if shown { Display::Flex } else { Display::None };
        control.disabled.0 = !enabled;
        if matches!(control.action, LogsExportAction::Cancel) && control.scrim.is_none() {
            let key = if surface.state.model.receipt.is_some() {
                "logs_export_close"
            } else {
                "logs_export_cancel"
            };
            if let Some(mut label) = control.label {
                label.0 = LocalizedText::plain(key);
            }
            for entity in control
                .children
                .into_iter()
                .flat_map(|children| children.iter())
            {
                if let Ok(mut text) = surface.copies.get_mut(*entity) {
                    *text = LocalizedText::plain(key);
                }
            }
        }
    }
    for (line, mut text, mut ink) in &mut surface.lines {
        let value = match line {
            LogsExportLine::Status => &view.status,
            LogsExportLine::Details => &view.details,
            LogsExportLine::Path => &view.path,
        };
        if text.0 != *value {
            text.0.clone_from(value);
        }
        ink.0 = surface.palette.ink;
    }
    for mut border in &mut surface.cards {
        *border = BorderColor::all(if view.error {
            surface.palette.danger
        } else {
            surface.palette.border
        });
    }
}
