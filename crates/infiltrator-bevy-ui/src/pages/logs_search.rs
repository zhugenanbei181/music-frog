//! Native logs query and immutable highlight replay, with scoped ECS access.
use crate::pages::logs::{
    LastLogsProjection, LogLevelText, LogMessageText, LogTagText, LogTimestampText, log_level_color,
};
use crate::pages::logs_rows::LogRowIdentity;
use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::Insert;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, QueryFilter, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res, ResMut, SystemParam};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{AlignItems, Display, FlexDirection, FlexWrap, Node, UiRect, Val, percent};
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::log_search::LogSearchState;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedPlaceholder, LocalizedText};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, text_field_with_placeholder_scene};
use infiltrator_bevy_widgets::text_runs::{TextRuns, TextRunsInk, TextRunsSet};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::logs::LogStreamState;
use infiltrator_contract::surface_snapshot::{LogSnapshot, LogsPageSnapshot, PageData};

#[derive(Resource, Default)]
pub struct LogsViewState(pub LogSearchState);
#[derive(Component, Clone, Copy, Default)]
pub struct LogsSearchField;
#[derive(Component, Clone, Copy, Default)]
#[require(ButtonDisabled)]
pub struct ClearLogsSearch;
#[derive(Component, Clone, Copy, Default)]
pub struct LogsSearchSummary;
#[derive(Component, Clone, Copy, Default)]
pub struct LogsSearchError;

pub fn logs_search_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: Val::Px(space::S4) }
        Children [
            Node { width: percent(100), align_items: AlignItems::Center, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(space::S8), row_gap: Val::Px(space::S4) }
            Children [
                Node { width: percent(80), min_width: Val::Px(180.0) } LogsSearchField
                Children [
                    @{ text_field_with_placeholder_scene(String::new(), String::new(), palette) } NativeTextField(11) LocalizedPlaceholder::plain("logs_regex_placeholder")
                ]
                --
                Node { padding: UiRect::all(Val::Px(space::S8)) } Button ClearLogsSearch
                Children [ LocalizedText::plain("common_clear") TextRole(Role::Caption) ]
            ]
            --
            LocalizedText::plain("logs_search_unavailable") LogsSearchSummary TextRole(Role::Caption)
            --
            LocalizedText::plain("logs_search_invalid_detail") LogsSearchError TextRole(Role::Body)
        ]
    }
}
pub struct LogsSearchPlugin;
impl Plugin for LogsSearchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LogsViewState>()
            .add_observer(initialize_query)
            .add_observer(clear_query)
            .add_systems(Update, replay.before(TextRunsSet));
    }
}
fn initialize_query(
    inserted: On<Insert<NativeTextField>>,
    view: Res<LogsViewState>,
    mut fields: Query<(&NativeTextField, &mut TextField)>,
) {
    if let Ok((marker, mut field)) = fields.get_mut(inserted.entity)
        && marker.0 == 11
    {
        field
            .0
            .apply(TextFieldInput::SetText(view.0.query().into()));
    }
}
fn clear_query(
    activate: On<Activate>,
    buttons: Query<(), With<ClearLogsSearch>>,
    mut fields: Query<(&NativeTextField, &mut TextField)>,
) {
    if !buttons.contains(activate.entity) {
        return;
    }
    for (marker, mut field) in &mut fields {
        if marker.0 == 11 {
            field.0.apply(TextFieldInput::Clear);
        }
    }
}
#[derive(QueryData)]
#[query_data(mutable)]
struct LogText {
    runs: &'static mut TextRuns,
    ink: Option<&'static mut TextRunsInk>,
    message: Option<&'static LogMessageText>,
    timestamp: Option<&'static LogTimestampText>,
    level: Option<&'static LogLevelText>,
    tag: Option<&'static LogTagText>,
}
#[derive(QueryFilter)]
struct LogErrorFilter {
    error: With<LogsSearchError>,
    row: Without<LogRowIdentity>,
    summary: Without<LogsSearchSummary>,
}
#[derive(SystemParam)]
struct LogSurface<'w, 's> {
    view: ResMut<'w, LogsViewState>,
    last: Res<'w, LastLogsProjection>,
    palette: Res<'w, UiPalette>,
    fields: Query<'w, 's, (&'static NativeTextField, &'static TextField)>,
    rows: Query<'w, 's, (&'static LogRowIdentity, &'static mut Node), Without<LogsSearchError>>,
    copies: Query<'w, 's, LogText>,
    summaries: Query<
        'w,
        's,
        &'static mut LocalizedText,
        (With<LogsSearchSummary>, Without<LogsSearchError>),
    >,
    errors: Query<'w, 's, (&'static mut LocalizedText, &'static mut Node), LogErrorFilter>,
    clears: Query<'w, 's, &'static mut ButtonDisabled, With<ClearLogsSearch>>,
}
fn replay(surface: LogSurface) {
    let LogSurface {
        mut view,
        last,
        palette,
        fields,
        mut rows,
        mut copies,
        mut summaries,
        mut errors,
        mut clears,
    } = surface;
    if let Some((_, field)) = fields.iter().find(|(marker, _)| marker.0 == 11) {
        view.0.edit_query(field.0.text());
    }
    let Some(projection) = last.0.as_ref() else {
        return;
    };
    let data = LogsPageSnapshot {
        total_entries: projection.total_entries,
        active_level: projection.active_level.map(|level| level.label().into()),
        stream: LogStreamState::Live,
        entries: projection
            .entries
            .iter()
            .map(|entry| LogSnapshot {
                id: entry.id,
                raw: None,
                level: entry.level.label().into(),
                timestamp: entry.timestamp.clone(),
                tag: entry.tag.clone(),
                message: entry.message.clone(),
            })
            .collect(),
    };
    view.0.observe(
        projection.generation,
        projection.session_token,
        &PageData {
            status: projection.status.clone(),
            data: Some(data),
        },
    );
    for (identity, mut node) in &mut rows {
        node.display = if view.0.row(identity.0).is_some_and(|row| row.visible) {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut copy in &mut copies {
        let (index, kind) = if let Some(marker) = copy.message {
            (marker.0, 0)
        } else if let Some(marker) = copy.timestamp {
            (marker.0, 1)
        } else if let Some(marker) = copy.level {
            (marker.0, 2)
        } else if let Some(marker) = copy.tag {
            (marker.0, 3)
        } else {
            continue;
        };
        let Some(row) = projection
            .entries
            .get(index)
            .and_then(|entry| view.0.row(entry.id))
        else {
            continue;
        };
        let runs = match kind {
            0 => &row.message,
            1 => &row.timestamp,
            2 => &row.level_label,
            _ => &row.tag,
        };
        if kind == 2
            && let Some(ref mut ink) = copy.ink
        {
            ink.0 = log_level_color(row.level, &palette);
        }
        if copy.runs.0 != *runs {
            copy.runs.0 = runs.clone();
        }
    }
    for mut summary in &mut summaries {
        *summary = LocalizedText::new(
            view.0.status_key(),
            vec![
                ("matched", view.0.matched_count().to_string()),
                ("count", view.0.source_count().to_string()),
            ],
        );
    }
    for (mut copy, mut node) in &mut errors {
        node.display = if view.0.invalid_pattern().is_some() {
            Display::Flex
        } else {
            Display::None
        };
        *copy = LocalizedText::new(
            "logs_search_invalid_detail",
            vec![(
                "reason",
                view.0.invalid_pattern().unwrap_or_default().into(),
            )],
        );
    }
    for mut disabled in &mut clears {
        disabled.0 = view.0.query().is_empty();
    }
}
