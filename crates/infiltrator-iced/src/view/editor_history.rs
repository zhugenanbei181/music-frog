//! DUAL-09-06/07/11: the editor page's snapshot-history panel and the shared
//! apply-transaction banner.
//!
//! Split out of `view/editor.rs` to keep both files inside the business line
//! budget. The panel renders the shared `SnapshotHistorySnapshot` (entries, the
//! prune view and the retention control) and never derives a retention rule of
//! its own; the banner renders the host core's typed apply outcome, so a
//! rollback is never inferred from an error string.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{
    BadgeKind, badge, card_surface, modern_scrollable, style_accent, style_ghost,
};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_contract::apply_transaction::ApplyTransactionStage;
use infiltrator_shared::locales::{Lang, Localizer};

pub(super) fn history_panel<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    use infiltrator_contract::snapshot_history::SNAPSHOT_DEFAULT_KEEP;
    use infiltrator_shared::i18n_interpolator::interpolate;

    let history = state.editor.snapshot_history.as_ref();
    let entry_count = history.map(|h| h.entries.len()).unwrap_or(0);
    let pending = history.map(|h| h.pending_prune).unwrap_or(0);
    let duplicates = history.map(|h| h.duplicate_entries).unwrap_or(0);
    let keep_limit = history
        .map(|h| h.keep_limit)
        .unwrap_or(SNAPSHOT_DEFAULT_KEEP);

    let mut history_header = row![
        icon_themed(Icon::RefreshCw, 14.0, |t: &Theme| tokens(t).text_secondary),
        text(lang.tr("editor_history").to_string())
            .font(FONT_SEMIBOLD)
            .size(13)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center);

    if entry_count > 0 {
        history_header = history_header
            .push(Space::new().width(Length::Fill))
            .push(badge(format!("{entry_count}"), BadgeKind::Neutral));
    }

    // DUAL-09-07: the shared prune view — the surface renders the decision the
    // application computed, never its own retention guess.
    let mut prune_facts = row![].spacing(theme::SP_SM).align_y(Alignment::Center);
    if pending > 0 {
        prune_facts = prune_facts.push(
            text(interpolate(
                &lang.tr("editor_prune_pending"),
                &[("count", &pending.to_string())],
            ))
            .size(10)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).warning),
            }),
        );
        if duplicates > 0 {
            prune_facts = prune_facts.push(
                text(interpolate(
                    &lang.tr("editor_prune_duplicates"),
                    &[("count", &duplicates.to_string())],
                ))
                .size(10)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
            );
        }
    } else {
        prune_facts = prune_facts.push(
            text(match history.and_then(|h| h.last_prune) {
                Some(report) => interpolate(
                    &lang.tr("editor_prune_last"),
                    &[
                        ("source", report.source.label_zh()),
                        ("count", &report.removed.to_string()),
                    ],
                ),
                None => lang.tr("editor_prune_never").to_string(),
            })
            .size(10)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
        );
    }

    let keep_button = |option: usize, selected: usize| {
        button(text(format!("{option}")).size(10).font(MONO))
            .padding([2, 7])
            .style(move |t: &Theme, status| {
                let tk = tokens(t);
                let active = status == button::Status::Hovered || option == selected;
                button::Style {
                    background: Some(if option == selected {
                        tk.accent.into()
                    } else if active {
                        tk.control_bg.into()
                    } else {
                        tk.chip_bg.into()
                    }),
                    border: Border {
                        radius: border::Radius::from(theme::R_CHIP),
                        width: theme::HAIRLINE,
                        color: tk.card_border,
                    },
                    ..Default::default()
                }
            })
            .on_press(Message::SetSnapshotPruneKeep(option))
    };
    let keep_row = row![
        text(lang.tr("editor_prune_keep").to_string())
            .size(10)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
        Space::new().width(theme::SP_XS),
    ]
    .align_y(Alignment::Center);
    let mut keep_row = keep_row;
    for option in [5usize, 10, 20, 50] {
        keep_row = keep_row
            .push(keep_button(option, keep_limit))
            .push(Space::new().width(theme::SP_XS));
    }

    let actions_row = row![
        // DUAL-09-14: explicit list refresh (the Bevy history card has the
        // same button); the panel no longer depends on an incidental reload.
        button(
            text(if state.editor.is_loading_snapshots {
                "...".to_string()
            } else {
                lang.tr("editor_history_refresh").to_string()
            })
            .size(10),
        )
        .padding([3, 8])
        .style(style_ghost)
        .on_press_maybe(
            (!state.editor.is_loading_snapshots).then_some(Message::LoadProfileSnapshots),
        ),
        Space::new().width(theme::SP_XS),
        button(
            text(if state.editor.is_backing_up_snapshot {
                "...".to_string()
            } else {
                lang.tr("editor_backup_now").to_string()
            })
            .size(10),
        )
        .padding([3, 8])
        .style(style_ghost)
        .on_press_maybe(
            (!state.editor.is_backing_up_snapshot).then_some(Message::BackupProfileSnapshot),
        ),
        Space::new().width(theme::SP_XS),
        button(
            text(if state.editor.is_pruning_snapshots {
                "...".to_string()
            } else {
                lang.tr("editor_prune_now").to_string()
            })
            .size(10),
        )
        .padding([3, 8])
        .style(style_ghost)
        .on_press_maybe(
            (!state.editor.is_pruning_snapshots && pending > 0)
                .then_some(Message::PruneProfileSnapshots),
        ),
    ]
    .align_y(Alignment::Center);

    let mut items_col = column![].spacing(theme::SP_SM);

    if state.editor.is_loading_snapshots {
        items_col = items_col.push(
            text(lang.tr("editor_history_loading").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        );
    } else if entry_count == 0 {
        items_col = items_col.push(
            text(lang.tr("editor_history_empty").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        );
    } else if let Some(history) = history {
        for entry in history.entries.iter().take(12) {
            let short_hash = entry.short_hash().to_string();
            let hash_pill =
                container(
                    text(short_hash)
                        .size(10)
                        .font(MONO)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary),
                        }),
                )
                .padding([2, 6])
                .style(|t: &Theme| {
                    let tk = tokens(t);
                    container::Style {
                        background: Some(tk.control_bg.into()),
                        border: Border {
                            radius: border::Radius::from(4.0),
                            width: theme::HAIRLINE,
                            color: tk.card_border,
                        },
                        ..Default::default()
                    }
                });

            let stamp = entry.stamp_label();
            let mut meta = column![
                text(stamp)
                    .size(11)
                    .font(FONT_MEDIUM)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary),
                    }),
                hash_pill,
            ]
            .spacing(3)
            .width(Length::Fill);
            if entry.is_duplicate {
                meta = meta.push(
                    text(lang.tr("editor_prune_duplicates").to_string())
                        .size(9)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).warning),
                        }),
                );
            }

            let snapshot_path = std::path::PathBuf::from(&entry.id);
            let confirm_pending = state.editor.pending_restore_snapshot.as_deref()
                == Some(std::path::Path::new(&entry.id));
            let restore_btn = button(
                text(if state.editor.is_restoring_snapshot {
                    "...".to_string()
                } else if confirm_pending {
                    lang.tr("editor_restore_confirm").to_string()
                } else {
                    lang.tr("editor_restore").to_string()
                })
                .size(11)
                .font(FONT_MEDIUM),
            )
            .padding([4, 10])
            .style(if confirm_pending {
                style_accent
            } else {
                style_ghost
            })
            .on_press_maybe(
                (!state.editor.is_restoring_snapshot).then_some(if confirm_pending {
                    Message::RestoreProfileSnapshot(snapshot_path.clone())
                } else {
                    Message::ArmRestoreProfileSnapshot(snapshot_path.clone())
                }),
            );

            let diff_btn = button(
                text(lang.tr("snapshot_diff_open").to_string())
                    .size(11)
                    .font(FONT_MEDIUM),
            )
            .padding([4, 10])
            .style(style_ghost)
            .on_press(Message::OpenSnapshotDiff(entry.id.clone()));

            let mut actions = row![diff_btn, Space::new().width(theme::SP_XS), restore_btn]
                .align_y(Alignment::Center);
            if confirm_pending {
                actions = actions.push(Space::new().width(theme::SP_XS)).push(
                    button(text(lang.tr("btn_cancel").to_string()).size(11))
                        .padding([4, 10])
                        .style(style_ghost)
                        .on_press(Message::CancelRestoreProfileSnapshot),
                );
            }

            let snapshot_card = container(
                row![meta, actions]
                    .spacing(theme::SP_SM)
                    .align_y(Alignment::Center),
            )
            .padding([8, 10])
            .width(Length::Fill)
            .style(|t: &Theme| {
                let tk = tokens(t);
                container::Style {
                    background: Some(tk.control_bg.into()),
                    border: Border {
                        radius: border::Radius::from(theme::R_CONTROL),
                        width: theme::HAIRLINE,
                        color: tk.card_border,
                    },
                    ..Default::default()
                }
            });

            items_col = items_col.push(snapshot_card);
        }
    }
    items_col = items_col.push(
        text(lang.tr("editor_apply_snapshot_hint").to_string())
            .size(10)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
    );

    let panel_body = column![
        history_header,
        Space::new().height(theme::SP_XS),
        prune_facts,
        keep_row,
        actions_row,
        modern_scrollable(items_col).height(Length::Fill),
    ]
    .spacing(theme::SP_SM);

    container(panel_body)
        .width(Length::Fixed(260.0))
        .height(Length::Fill)
        .padding(theme::SP_MD)
        .style(card_surface)
        .into()
}

/// DUAL-09-11: the shared apply-transaction banner. It renders the typed
/// outcome the host core recorded (`committed` / `rolled back` /
/// `rollback failed`), never a locally inferred state.
fn apply_stage_color(t: &Theme, stage: ApplyTransactionStage) -> Color {
    let tk = tokens(t);
    match stage {
        ApplyTransactionStage::Idle => tk.text_secondary,
        ApplyTransactionStage::Committed => tk.success,
        ApplyTransactionStage::RolledBack => tk.warning,
        ApplyTransactionStage::RollbackFailed => tk.danger,
    }
}

pub(super) fn apply_banner<'a>(
    state: &'a AppState,
    lang: &Lang<'_>,
) -> Option<Element<'a, Message>> {
    let record = state.editor.apply_transaction.as_ref()?;
    let stage = record.stage;
    let key = match stage {
        ApplyTransactionStage::Idle => "editor_apply_stage_idle",
        ApplyTransactionStage::Committed => "editor_apply_stage_committed",
        ApplyTransactionStage::RolledBack => "editor_apply_stage_rolled_back",
        ApplyTransactionStage::RollbackFailed => "editor_apply_stage_rollback_failed",
    };
    let mut body = column![
        text(lang.tr(key).to_string())
            .size(12)
            .font(FONT_SEMIBOLD)
            .style(move |t: &Theme| text::Style {
                color: Some(apply_stage_color(t, stage)),
            }),
    ]
    .spacing(theme::SP_XS);
    if !record.detail.is_empty() {
        body = body.push(
            text(record.detail.clone())
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
        );
    }
    if let Some(rollback) = &record.rollback_error {
        body = body.push(
            text(rollback.clone())
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).danger),
                }),
        );
    }
    Some(
        container(
            row![
                icon_themed(Icon::Activity, 16.0, move |t: &Theme| apply_stage_color(
                    t, stage
                )),
                body,
            ]
            .spacing(theme::SP_SM)
            .align_y(Alignment::Center),
        )
        .padding([10, 14])
        .width(Length::Fill)
        .style(card_surface)
        .into(),
    )
}
