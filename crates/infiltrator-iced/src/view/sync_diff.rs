//! Sync-conflict key-level merge panel on the Sync page: shows the computed
//! local-vs-remote top-level diff and lets the user adopt remote / keep local
//! per key, then commit the merged document through the apply transaction.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::options::{SyncDiffKeyKind, SyncDiffState};
use crate::view::components::{BadgeKind, card, badge, segmented_control};
use crate::view::theme::{self, MONO, R_CONTROL, tokens};
use iced::widget::{button, column, row, scrollable, text, Space};
use iced::{Alignment, Border, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

fn style_accent(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
    button::Style {
        background: Some(Into::into(match status {
            button::Status::Disabled => tk.accent_soft,
            _ => tk.accent,
        })),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            ..Default::default()
        },
        text_color: match status {
            button::Status::Disabled => tk.accent,
            _ => tk.on_accent,
        },
        ..Default::default()
    }
}

fn style_ghost(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
    button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(tk.control_bg.into()),
            _ => None,
        },
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: 1.0,
            color: tk.card_border,
        },
        text_color: match status {
            button::Status::Disabled => tk.text_tertiary,
            button::Status::Hovered | button::Status::Pressed => tk.text_primary,
            _ => tk.text_secondary,
        },
        ..Default::default()
    }
}

fn kind_badge(lang: &Lang<'_>, kind: SyncDiffKeyKind) -> Element<'static, Message> {
    let badge_kind = match kind {
        SyncDiffKeyKind::Added => BadgeKind::Success,
        SyncDiffKeyKind::Removed => BadgeKind::Danger,
        SyncDiffKeyKind::Modified => BadgeKind::Warning,
    };
    badge(lang.tr(kind.label_key()).into_owned(), badge_kind)
}

fn mono_value<'a>(label: String, value: &'a str) -> Element<'a, Message> {
    column![
        text(label)
            .size(10)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
        text(value.to_string())
            .size(10)
            .font(MONO)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
    ]
    .spacing(1)
    .into()
}

fn key_row<'a>(
    lang: &Lang<'a>,
    diff: &'a SyncDiffState,
    key: String,
    kind: SyncDiffKeyKind,
) -> Element<'a, Message> {
    let take_remote = diff.picks.get(&key).copied().unwrap_or(false);
    let labels = vec![
        lang.tr("sync_diff_keep_local").to_string(),
        lang.tr("sync_diff_take_remote").to_string(),
    ];
    let key_for_closure = key.clone();
    let picker = segmented_control(&labels, if take_remote { 1 } else { 0 }, move |index| {
        Message::PickSyncDiffKey(key_for_closure.clone(), index == 1)
    });

    let mut body = column![row![
        text(key.clone()).size(12).font(MONO),
        Space::new().width(theme::SP_SM),
        kind_badge(lang, kind),
    ]
    .align_y(Alignment::Center)]
    .spacing(theme::SP_XS);

    if kind == SyncDiffKeyKind::Modified
        && let Some((_, local, remote)) =
            diff.bundle.modified.iter().find(|(name, _, _)| name == &key)
    {
        body = body.push(row![
            mono_value(lang.tr("sync_diff_local").to_string(), local),
            Space::new().width(theme::SP_MD),
            mono_value(lang.tr("sync_diff_remote").to_string(), remote),
        ]);
    }
    column![body, picker].spacing(theme::SP_SM).into()
}

/// The active merge session panel, rendered in place of the simple
/// adopt/keep conflict rows while `profile.sync_diff` is set.
pub fn diff_panel(state: &AppState) -> Option<Element<'_, Message>> {
    let diff = state.profile.sync_diff.as_ref()?;
    let lang = Lang(&state.shell.lang);
    let total = diff.bundle.all_keys().len();
    let adopted = diff.picks.values().filter(|pick| **pick).count();

    let mut rows = column![].spacing(theme::SP_MD);
    for (key, kind) in diff.bundle.key_entries() {
        rows = rows.push(key_row(&lang, diff, key, kind));
    }

    let mut actions = row![
        button(
            text(format!(
                "{}（{adopted}/{total} {}）",
                lang.tr("sync_diff_save"),
                lang.tr("sync_diff_keys_remote")
            ))
            .size(12)
            .font(theme::FONT_MEDIUM),
        )
        .padding([7, 14])
        .style(style_accent)
        .on_press_maybe(
            (!state.profile.is_applying_sync_diff).then_some(Message::ApplySyncDiffMerge),
        ),
        Space::new().width(theme::SP_SM),
        button(text(lang.tr("sync_diff_all_local").to_string()).size(12))
            .padding([7, 10])
            .style(style_ghost)
            .on_press(Message::SetSyncDiffPicks(false)),
        Space::new().width(theme::SP_SM),
        button(text(lang.tr("sync_diff_all_remote").to_string()).size(12))
            .padding([7, 10])
            .style(style_ghost)
            .on_press(Message::SetSyncDiffPicks(true)),
        Space::new().width(Length::Fill),
        button(text(lang.tr("sync_diff_close").to_string()).size(12))
            .padding([7, 10])
            .style(style_ghost)
            .on_press(Message::CloseSyncDiff),
    ]
    .align_y(Alignment::Center);

    if state.profile.is_loading_sync_diff {
        actions = row![text(lang.tr("sync_diff_computing").to_string()).size(12)];
    }

    Some(card(
        Some(format!(
            "{}：{}",
            lang.tr("sync_diff_title"),
            diff.bundle.profile
        )),
        column![
            text(lang.tr("sync_diff_desc").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
            scrollable(rows).height(Length::Shrink).width(Length::Fill),
            actions,
        ]
        .spacing(theme::SP_MD),
    ))
}
