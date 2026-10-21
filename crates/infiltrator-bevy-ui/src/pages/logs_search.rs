//! Native logs query and immutable highlight replay, with scoped ECS access.
//!
//! The page's two row filters live here: the regex query (the shared search
//! fold) and the level filter (the official headless tab strip). Both read
//! the shared page model and dispatch typed intents back; neither keeps a
//! second copy of the selection.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::logs::{LastLogsProjection, LogMessageText};
use crate::pages::logs_rows::{LogRowIdentity, LogRowMeta, level_badge_text, meta_runs};
use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::Insert;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, QueryFilter, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BorderRadius, Display, FlexDirection, FlexWrap, JustifyContent, Node, UiRect,
    percent,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{
    Activate, Button, ControlOrientation, SelectedTab, Tab, TabActivation, TabList, ValueChange,
};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedPlaceholder, LocalizedText};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, text_field_with_placeholder_scene};
use infiltrator_bevy_widgets::text_runs::{TextRuns, TextRunsInk, TextRunsSet};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::logs::{LogLevel, LogStreamState};
use infiltrator_contract::surface_snapshot::{LogSnapshot, LogsPageSnapshot};
use infiltrator_presentation::log_page_model::{LogLevelFilter, LogsPageModel};
use lurker_core_contract::page::PageData;

/// One-off layout value (not a ladder rung): the narrowest the logs regex
/// search field may shrink before the row wraps.
const LOGS_SEARCH_FIELD_MIN_WIDTH_PX: f32 = 180.0;

/// The one page model of the logs page.
///
/// The query, the level filter, the typed page status and the typed search
/// outcome are owned by the shared model, so this surface only renders the
/// model and reports the owner's readback back to it.
#[derive(Resource, Default)]
pub struct LogsViewState(pub LogsPageModel);
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
        Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: rem(space::S4) }
        Children [
            Node { width: percent(100), align_items: AlignItems::Center, flex_wrap: FlexWrap::Wrap, column_gap: rem(space::S8), row_gap: rem(space::S4) }
            Children [
                Node { width: percent(80), min_width: rem(LOGS_SEARCH_FIELD_MIN_WIDTH_PX) } LogsSearchField
                Children [
                    @{ text_field_with_placeholder_scene(String::new(), String::new(), palette) }
                    ThemeBackgroundColor({ Token::Surface })
                    NativeTextField(11) LocalizedPlaceholder::plain("logs_regex_placeholder")
                ]
                --
                Node { padding: UiRect::all(rem(space::S8)) } Button ClearLogsSearch
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
            .init_resource::<LogLevelConfirmation>()
            .add_observer(initialize_query)
            .add_observer(clear_query)
            .add_observer(on_log_level_changed)
            .add_systems(
                Update,
                (
                    replay.before(TextRunsSet),
                    seed_log_level_strip,
                    sync_log_level_strip,
                ),
            );
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
    meta: Option<&'static mut LogRowMeta>,
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
    // The level filter is a separate owner readback: the page observation
    // never carries it, so the surface hands the owner's active level to the
    // model instead of deriving the filter here.
    view.0.observe_level(LogLevelFilter::from_active_level(
        projection.active_level.map(|level| level.label()),
    ));
    for (identity, mut node) in &mut rows {
        node.display = if view.0.row(identity.0).is_some_and(|row| row.visible) {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut copy in &mut copies {
        if let Some(marker) = copy.message {
            let Some(entry) = projection.entries.get(marker.0) else {
                continue;
            };
            let Some(row) = view.0.row(entry.id) else {
                continue;
            };
            if copy.runs.0 != row.message {
                copy.runs.0 = row.message.clone();
            }
        } else if let Some(meta) = copy.meta.as_deref_mut() {
            let Some(entry) = projection.entries.get(meta.index) else {
                continue;
            };
            let Some(row) = view.0.row(entry.id) else {
                continue;
            };
            // The rail is ONE paragraph: the line number, the timestamp, the
            // level badge and the module tag, each its own run, so the search
            // highlight rides the field it matched.
            let rail = meta_runs(
                meta.index,
                &row.timestamp,
                &level_badge_text(row.level),
                &row.tag,
            );
            if copy.runs.0 != rail {
                copy.runs.0 = rail;
            }
            // The rail is the dim half of the line: the ink is re-derived
            // from the live palette, so a skin switch never leaves it stale.
            if let Some(ink) = copy.ink.as_deref_mut()
                && ink.0 != palette.ink_dim
            {
                ink.0 = palette.ink_dim;
            }
            meta.level = row.level;
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

// ---- The level filter over the OFFICIAL headless tab list -----------------

/// The level filter's strip root: the official `TabList`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogLevelStrip;

/// One level tab; the payload is the level it filters by.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogLevelTab {
    /// The level this tab selects; `None` is "every level".
    pub level: Option<LogLevel>,
}

/// Marker on a level tab's label: the label ink follows the official
/// selection.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogLevelTabLabel;

/// What the owner has confirmed about the level filter.
///
/// The owner is the authority; the strip only mirrors it. The confirmation
/// is a typed fact so a user's own selection is never overwritten before the
/// owner has answered.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LogLevelConfirmation {
    /// The owner has not confirmed a level yet.
    #[default]
    Unconfirmed,
    /// The owner confirmed this level (`None` is "every level").
    Confirmed(Option<LogLevel>),
}

/// Every level the filter offers, "all" first.
fn filter_levels() -> Vec<Option<LogLevel>> {
    let mut levels = vec![None];
    levels.extend(LogLevel::ALL.map(Some));
    levels
}

/// The level filter strip: the official tab list over every level.
pub fn level_filter_strip_scene(
    active_level: Option<LogLevel>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let tabs: Vec<Box<dyn Scene>> = filter_levels()
        .into_iter()
        .map(|level| level_tab_scene(level, level == active_level, palette))
        .collect();

    bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: rem(space::S4),
                row_gap: rem(space::S4),
                flex_wrap: FlexWrap::Wrap,
            }
            LogLevelStrip
            TabList {
                orientation: ControlOrientation::Horizontal,
                activation: TabActivation::Automatic,
            }
            Children [
                { tabs }
            ]
    }
}

/// One level tab: an official `Tab` carrying its typed level.
fn level_tab_scene(level: Option<LogLevel>, selected: bool, palette: &UiPalette) -> Box<dyn Scene> {
    let fill = if selected {
        Token::AccentContainer
    } else {
        Token::SurfaceElevated
    };
    let ink = if selected {
        Token::Accent
    } else {
        Token::InkDim
    };
    let label: Box<dyn Scene> = match level {
        None => Box::new(bsn! {
            LocalizedText::plain("logs_level_all") TextRole(Role::Caption) ThemeTextColor({ ink }) LogLevelTabLabel
        }),
        Some(level) => Box::new(bsn! {
            Text({ level.label().to_owned() }) TextRole(Role::Caption) ThemeTextColor({ ink }) LogLevelTabLabel
        }),
    };

    Box::new(bsn! {
            Node {
                min_height: rem(palette.control_height_px * 0.8),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(rem(space::S12)),
                border_radius: BorderRadius::MAX,
            }
            ThemeBackgroundColor({ fill })
            Tab
            LogLevelTab { level: { level } }
            Children [
                @{ label }
            ]
    })
}

/// The owner's readback seeds the strip's OFFICIAL selection.
///
/// The selection moves only when the owner confirms a level: a user's own
/// click stays on screen until the owner answers, so the strip never flickers
/// back to the previous level mid-flight.
fn seed_log_level_strip(
    mut commands: Commands,
    mut confirmation: ResMut<LogLevelConfirmation>,
    projection: Res<LastLogsProjection>,
    strips: Query<(Entity, &Children, &SelectedTab), With<LogLevelStrip>>,
    tabs: Query<&LogLevelTab, With<Tab>>,
) {
    let Some(projection) = projection.0.as_ref() else {
        return;
    };
    let active = projection.active_level;
    let confirmed = LogLevelConfirmation::Confirmed(active);
    let changed = *confirmation != confirmed;
    if changed {
        *confirmation = confirmed;
    }
    for (entity, children, selected) in &strips {
        if !changed && selected.0.is_some() {
            continue;
        }
        let Some(tab) = children
            .iter()
            .copied()
            .find(|child| tabs.get(*child).is_ok_and(|tab| tab.level == active))
        else {
            continue;
        };
        if selected.0 != Some(tab) {
            commands.entity(entity).insert(SelectedTab(Some(tab)));
        }
    }
}

/// Paint the chips from the OFFICIAL selection state.
///
/// The chip fill and the label ink follow the strip's `SelectedTab`, so the
/// official selection is the only source of the segmented control's look.
fn sync_log_level_strip(
    mut commands: Commands,
    strips: Query<(&Children, &SelectedTab), With<LogLevelStrip>>,
    tabs: Query<(&LogLevelTab, &Children), With<Tab>>,
    labels: Query<Entity, With<LogLevelTabLabel>>,
) {
    for (children, selected) in &strips {
        let Some(selected_entity) = selected.0 else {
            continue;
        };
        for child in children.iter() {
            let Ok((_, tab_children)) = tabs.get(*child) else {
                continue;
            };
            let is_selected = *child == selected_entity;
            commands
                .entity(*child)
                .insert(ThemeBackgroundColor(if is_selected {
                    Token::AccentContainer.theme_token()
                } else {
                    Token::SurfaceElevated.theme_token()
                }));
            let ink = if is_selected {
                Token::Accent
            } else {
                Token::InkDim
            };
            for label in tab_children.iter() {
                if labels.contains(*label) {
                    commands
                        .entity(*label)
                        .insert(ThemeTextColor(ink.theme_token()));
                }
            }
        }
    }
}

/// The OFFICIAL strip selection changed: ask the owner for that level.
///
/// The owner stays the authority: the strip mirrors the user's choice into
/// the official controlled selection and dispatches the typed command; the
/// owner's readback confirms it.
fn on_log_level_changed(
    change: On<ValueChange<Option<Entity>>>,
    strips: Query<(), With<LogLevelStrip>>,
    tabs: Query<&LogLevelTab>,
    mut commands: Commands,
    handle: Option<Res<CommandSinkHandle>>,
) {
    if !strips.contains(change.source) {
        return;
    }
    let Some(selected) = change.value else {
        return;
    };
    let Ok(tab) = tabs.get(selected) else {
        return;
    };
    commands
        .entity(change.source)
        .insert(SelectedTab(Some(selected)));
    let Some(handle) = handle else {
        return;
    };
    handle.submit(UiCommand::SetLogLevelFilter {
        level: tab.level.map(|level| level.label().to_string()),
    });
}
