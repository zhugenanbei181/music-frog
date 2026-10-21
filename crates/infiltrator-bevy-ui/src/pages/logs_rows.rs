//! Log row topology: the console stream's flat rows.
//!
//! A log row is a flat band, not a card: the stream reads as one tiled
//! console, so the row carries no radius and no fill of its own — only a
//! hairline that separates two consecutive lines.
//!
//! The row's content is TWO text blocks, each ONE paragraph whose runs are
//! `TextSpan`s: the meta rail (the line number, the timestamp, the level
//! badge and the module tag) and the message. A paragraph wraps inside its
//! own column, so a long record never bursts the stream card, and the badge
//! rides its line's baseline instead of drifting on a flex row.

use crate::pages::connections_clipboard::{ClipboardHost, ClipboardWrite, write_clipboard};
use crate::pages::logs::{
    LastLogsProjection, LogEntry, LogMessageText, LogsProjectionUpdated, log_level_color,
};
use crate::pages::logs_ring::LogsRing;
use crate::pages::logs_virtual::LogsVirtualNodes;
use crate::toast::ShellToast;
use bevy::app::{App, Plugin, Update};
use bevy::clipboard::Clipboard;
use bevy::color::{Alpha, Color};
use bevy::ecs::change_detection::Mut;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::message::MessageWriter;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::text::{LineBreak, LineHeight, TextBackgroundColor, TextColor, TextLayout, TextSpan};
use bevy::ui::prelude::{AlignItems, BorderColor, Node, Overflow, UiRect, Val, percent, px};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::button::{
    ButtonLabel, ButtonSize, ButtonVariant, button_with_label_scene,
};
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_runs::{TextRuns, TextRunsInk, TextRunsSet};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::logs::LogLevel;
use infiltrator_shared::locales::{Lang, Localizer};
use zhuge_ui_core::text::SearchTextRun;

/// The container that owns the mounted log rows.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogRowsContainer;

/// The stable identity of one mounted row.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogRowIdentity(pub u64);

/// The row's meta rail: ONE text block whose runs are the line number, the
/// timestamp, the level badge and the module tag.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(TextRuns)]
pub struct LogRowMeta {
    /// The row's index in the mounted window (its line number).
    pub index: usize,
    /// The level the badge chip paints.
    pub level: LogLevel,
}

/// The per-row copy control: it copies that one line to the clipboard.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CopyLogLineButton(pub u64);

/// The level chip's background: the level ink at a low alpha, so the chip
/// reads as a tint of its own color instead of a solid block.
const LOG_LEVEL_CHIP_ALPHA: f32 = 0.18;

/// A console line needs air between the lines: the mono face is small, and
/// the default line height runs the ascenders of two lines together.
const LOG_ROW_LINE_HEIGHT: f32 = 1.45;

/// The level badge's label, in the same bracketed form the contract's search
/// projection uses.
pub fn level_badge_text(level: LogLevel) -> String {
    format!("[{}]", level.label())
}

/// The module tag's label, in the same bracketed form the contract's search
/// projection uses.
pub fn module_tag_text(tag: &str) -> String {
    format!("[{tag}]")
}

/// The line number of a row, right aligned so the timestamp column stays
/// straight.
pub fn line_number_text(index: usize) -> String {
    format!("{:>4}", index + 1)
}

/// A plain (never highlighted) run of one field.
pub fn plain_run(text: &str) -> SearchTextRun {
    SearchTextRun {
        text: text.to_owned(),
        highlighted: false,
    }
}

/// The single space that separates two rail fields.
fn separator() -> SearchTextRun {
    plain_run(" ")
}

/// The rail's runs: the line number, the timestamp, the level badge and the
/// module tag, separated by a single space.
///
/// The separator rides its own run, so the badge's chip still hugs its own
/// label instead of a padded block.
pub fn meta_runs(
    index: usize,
    timestamp: &[SearchTextRun],
    badge: &str,
    tag: &[SearchTextRun],
) -> Vec<SearchTextRun> {
    let mut runs = Vec::with_capacity(timestamp.len() + tag.len() + 4);
    runs.push(plain_run(&line_number_text(index)));
    runs.push(separator());
    runs.extend(timestamp.iter().cloned());
    runs.push(separator());
    runs.push(plain_run(badge));
    runs.push(separator());
    runs.extend(tag.iter().cloned());
    runs
}

/// The one line of a record, as the clipboard receives it.
pub fn log_line_text(entry: &LogEntry) -> String {
    format!(
        "{} {} {} {}",
        entry.timestamp,
        level_badge_text(entry.level),
        module_tag_text(&entry.tag),
        entry.message
    )
}

/// The scene of one log row: a flat band with a meta rail and a message.
pub fn log_row_scene(idx: usize, entry: &LogEntry, palette: &UiPalette) -> impl Scene + use<> {
    let level = entry.level;
    let badge = level_badge_text(level);
    let rail = meta_runs(
        idx,
        &[plain_run(&entry.timestamp)],
        &badge,
        &[plain_run(&module_tag_text(&entry.tag))],
    );
    let message = vec![plain_run(&entry.message)];

    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                flex_shrink: 0.0,
                align_items: AlignItems::FlexStart,
                column_gap: rem(space::S12),
                padding: UiRect::vertical(rem(space::S2)),
                border: UiRect::bottom(Val::Px(palette.hairline_px)),
            }
            BorderColor {
                top: { Color::NONE },
                right: { Color::NONE },
                bottom: { palette.border },
                left: { Color::NONE },
            }
            LogRowIdentity({ entry.id })
            Children [
                Node {
                    flex_shrink: 0.0,
                    min_width: px(0.0),
                }
                Children [
                    TextRuns({ rail })
                    LogRowMeta { index: { idx }, level: { level } }
                    TextRole(Role::Mono)
                    TextRunsInk({ palette.ink_dim })
                ]
                --
                Node {
                    flex_grow: 1.0,
                    min_width: px(0.0),
                    overflow: Overflow::clip(),
                }
                Children [
                    TextRuns({ message })
                    LogMessageText({ idx })
                    TextRole(Role::Mono)
                    TextLayout::linebreak(LineBreak::WordOrCharacter)
                ]
                --
                Node {
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                }
                Children [
                    @{ copy_button_scene(entry.id, palette) }
                ]
            ]
    }
}

/// The row's trailing copy control.
fn copy_button_scene(id: u64, palette: &UiPalette) -> Box<dyn Scene> {
    Box::new(bsn! {
            @{ button_with_label_scene(
                bsn! { LocalizedText::plain("common_copy") TextRole(Role::Caption) ButtonLabel },
                ButtonVariant::Ghost,
                ButtonSize::Sm,
                palette,
            ) }
            CopyLogLineButton({ id })
    })
}

pub struct LogsRowsPlugin;
impl Plugin for LogsRowsPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(reconcile_rows)
            .add_observer(copy_log_line)
            .add_systems(Update, stamp_row_chrome.after(TextRunsSet));
    }
}
fn reconcile_rows(
    update: On<LogsProjectionUpdated>,
    mut ring: ResMut<LogsRing>,
    containers: Query<(Entity, Option<&Children>), With<LogRowsContainer>>,
    virtual_containers: Query<(), With<LogsVirtualNodes>>,
    identities: Query<&LogRowIdentity>,
    palette: Res<UiPalette>,
    mut commands: Commands,
) {
    if !virtual_containers.is_empty() {
        // BANDROID-010: the recycler owns the row topology whenever its
        // container is mounted, so the full-mount reconcile must stand down.
        // The per-frame search replay restamps the mounted rows in place.
        return;
    }
    let Ok((root, children)) = containers.single() else {
        return;
    };
    // BEVY-020: reconcile against the ring's bounded window. Observer order is
    // not guaranteed, so ingestion is idempotent and performed here too; the
    // mounted row count tracks the window, never the raw stream.
    ring.ingest(&update.0);
    let projection = ring.snapshot();
    let existing: Vec<_> = children
        .into_iter()
        .flat_map(|children| children.iter())
        .filter_map(|child| identities.get(*child).ok().map(|id| id.0))
        .collect();
    let requested: Vec<_> = projection.entries.iter().map(|entry| entry.id).collect();
    if existing == requested {
        return;
    }
    commands.entity(root).despawn_children();
    for (index, entry) in projection.entries.iter().enumerate() {
        commands
            .spawn_scene(log_row_scene(index, entry, &palette))
            .insert(ChildOf(root));
    }
}

/// Copy one line to the system clipboard.
///
/// The copy goes through the shell's clipboard seam, so a platform that
/// denies the write reports a typed failure instead of a silent success.
fn copy_log_line(
    activate: On<Activate>,
    buttons: Query<&CopyLogLineButton>,
    projection: Res<LastLogsProjection>,
    locale: Option<Res<UiLocale>>,
    host: Option<Res<ClipboardHost>>,
    mut clipboard: Option<ResMut<Clipboard>>,
    mut toasts: MessageWriter<ShellToast>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let Some(projection) = projection.0.as_ref() else {
        return;
    };
    let Some(entry) = projection.entries.iter().find(|entry| entry.id == button.0) else {
        return;
    };
    let language = locale
        .as_deref()
        .map(|locale| locale.code().to_string())
        .unwrap_or_default();
    let lang = Lang(&language);
    let text = log_line_text(entry);
    match write_clipboard(host.as_deref(), clipboard.as_deref_mut(), &text) {
        ClipboardWrite::Written => {
            toasts.write(ShellToast::info(lang.tr("clipboard_copied").into_owned()));
        }
        ClipboardWrite::Denied => {
            toasts.write(ShellToast::danger(format!(
                "{}: denied",
                lang.tr("clipboard_failed")
            )));
        }
        ClipboardWrite::Failed(error) => {
            toasts.write(ShellToast::danger(format!(
                "{}: {error}",
                lang.tr("clipboard_failed")
            )));
        }
        ClipboardWrite::Unsupported(reason) => {
            toasts.write(ShellToast::warning(format!(
                "{}: {reason}",
                lang.tr("clipboard_unsupported")
            )));
        }
    }
}

/// The level badge's chip and the console line height.
///
/// The shared run replay paints every run of a paragraph with the paragraph's
/// own ink, so the badge — the one run that carries a semantic color — is
/// stamped after it. The badge is the run whose text is the level label, so
/// the stamp never guesses an index.
///
/// The whole stamp reads ONE mutable query: the rail's paragraph, its runs
/// and the message's paragraph share the same text nodes, so a second
/// mutable query over the same components would be a conflicting access.
fn stamp_row_chrome(
    palette: Res<UiPalette>,
    rails: Query<(Entity, &LogRowMeta, &Children, &TextRuns)>,
    messages: Query<(Entity, &LogMessageText, &Children)>,
    mut text_nodes: Query<(
        Option<&TextSpan>,
        &mut TextColor,
        Option<&mut TextBackgroundColor>,
        &mut LineHeight,
    )>,
    mut commands: Commands,
) {
    for (root, meta, children, runs) in &rails {
        // The rail's own paragraph: the dim half of the line.
        if let Ok((_, mut ink, _, mut line_height)) = text_nodes.get_mut(root) {
            if ink.0 != palette.ink_dim {
                ink.0 = palette.ink_dim;
            }
            set_console_line_height(&mut line_height);
        }
        let badge = level_badge_text(meta.level);
        let level_color = log_level_color(meta.level, &palette);
        let badge_highlighted = runs
            .0
            .iter()
            .any(|run| run.highlighted && run.text == badge);
        let chip = if badge_highlighted {
            palette.accent.with_alpha(LOG_LEVEL_CHIP_ALPHA)
        } else {
            level_color.with_alpha(LOG_LEVEL_CHIP_ALPHA)
        };
        for child in children.iter() {
            let Ok((span, mut ink, mut background, mut line_height)) = text_nodes.get_mut(*child)
            else {
                continue;
            };
            set_console_line_height(&mut line_height);
            let Some(span) = span else {
                continue;
            };
            if span.0 != badge {
                continue;
            }
            if ink.0 != level_color {
                ink.0 = level_color;
            }
            match background.as_deref_mut() {
                Some(background) => {
                    if background.0 != chip {
                        background.0 = chip;
                    }
                }
                None => {
                    commands.entity(*child).insert(TextBackgroundColor(chip));
                }
            }
        }
    }
    for (root, _, children) in &messages {
        // The message's own paragraph: the shared replay owns its ink, only
        // the console's line height belongs here.
        if let Ok((_, _, _, mut line_height)) = text_nodes.get_mut(root) {
            set_console_line_height(&mut line_height);
        }
        for child in children.iter() {
            let Ok((_, _, _, mut line_height)) = text_nodes.get_mut(*child) else {
                continue;
            };
            set_console_line_height(&mut line_height);
        }
    }
}

/// The console's mono line height, re-projected onto one text node.
fn set_console_line_height(line_height: &mut Mut<'_, LineHeight>) {
    let target = LineHeight::RelativeToFont(LOG_ROW_LINE_HEIGHT);
    if **line_height != target {
        **line_height = target;
    }
}
