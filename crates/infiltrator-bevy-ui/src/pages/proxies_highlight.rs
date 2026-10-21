//! Replay shared proxy runs; the widget layer owns native span lifecycle and palette updates.
use crate::pages::proxies::{LastProxiesProjection, NodeNameText, ProxyNodeButton};
use crate::pages::proxies_identity::node_for_entity;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::ui::prelude::{Node, Val};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::smart_truncate::truncate_tail;
use infiltrator_bevy_widgets::text_runs::TextRuns;
use zhuge_ui_core::text::SearchTextRun;

/// The card's own chrome around the node name: the card's padding, the
/// selection rail, the flag and the trailing latency/action cluster.
const NODE_NAME_CHROME_PX: f32 = 170.0;

/// The mark that says the name was cut.
const NAME_ELLIPSIS: &str = "…";

/// The narrowest name the card ever shows, in half-width units: a name that
/// is cut below this would say nothing at all.
const MIN_NAME_UNITS: usize = 8;

/// The estimated width of one text unit, in half-widths: a narrow character
/// counts one, a wide one (CJK, an emoji) counts two.
fn width_units(text: &str) -> usize {
    text.chars()
        .map(|character| if character.is_ascii() { 1 } else { 2 })
        .sum()
}

/// Cut the name to the width the card can show.
///
/// The cut lands on the shared grapheme boundary, so a flag emoji or a
/// combining mark is never split in half; the width estimate keeps a CJK
/// name and a latin name to the same number of pixels.
fn truncate_name(text: &str, budget_units: usize) -> String {
    let mut best = truncate_tail(text, 1);
    for count in 2..=text.chars().count() {
        let cut = truncate_tail(text, count);
        if width_units(&cut) > budget_units {
            break;
        }
        best = cut;
    }
    best
}

/// The width the node name may take, in half-width units.
///
/// The name shares the card with the flag, the latency tile and the card's
/// actions, so the budget follows the card's own measured width. A card whose
/// width is not measured yet has no budget: the name is left whole.
fn name_budget(
    entity: Entity,
    parents: &Query<&ChildOf>,
    cards: &Query<&Node, With<ProxyNodeButton>>,
    palette: &UiPalette,
) -> Option<usize> {
    let mut current = entity;
    let card = loop {
        if let Ok(node) = cards.get(current) {
            break Some(node);
        }
        let Ok(parent) = parents.get(current) else {
            break None;
        };
        current = parent.parent();
    };
    let card = card?;
    // The card's own measured width: a physical quantity, not a design size.
    let Val::Px(item_width_px) = card.width else {
        return None;
    };
    let unit_px = palette.body_font_px / 2.0;
    let units = (item_width_px - NODE_NAME_CHROME_PX) / unit_px;
    if units <= 0.0 {
        return None;
    }
    Some((units as usize).max(MIN_NAME_UNITS))
}

/// Cut the highlighted runs to the budget, keeping every highlight span.
fn truncate_runs(runs: &[SearchTextRun], budget_units: Option<usize>) -> Vec<SearchTextRun> {
    let Some(budget_units) = budget_units else {
        return runs.to_vec();
    };
    let full: String = runs.iter().map(|run| run.text.as_str()).collect();
    let truncated = truncate_name(&full, budget_units);
    if truncated == full {
        return runs.to_vec();
    }
    let keep_bytes = truncated.len().saturating_sub(NAME_ELLIPSIS.len());
    let mut cut: Vec<SearchTextRun> = Vec::with_capacity(runs.len() + 1);
    let mut consumed = 0usize;
    for run in runs {
        if consumed >= keep_bytes {
            break;
        }
        let mut end = (keep_bytes - consumed).min(run.text.len());
        while end > 0 && !run.text.is_char_boundary(end) {
            end -= 1;
        }
        if end == 0 {
            break;
        }
        cut.push(SearchTextRun {
            text: run.text[..end].to_owned(),
            highlighted: run.highlighted,
        });
        consumed += end;
    }
    cut.push(SearchTextRun {
        text: NAME_ELLIPSIS.to_owned(),
        highlighted: false,
    });
    cut
}

pub fn sync_name_highlights(
    mut commands: Commands,
    last: Option<Res<LastProxiesProjection>>,
    palette: Res<UiPalette>,
    names: Query<(Entity, Option<&TextRuns>), With<NodeNameText>>,
    parents: Query<&ChildOf>,
    identities: Query<&ProxyNodeButton>,
    cards: Query<&Node, With<ProxyNodeButton>>,
) {
    let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) else {
        return;
    };
    for (entity, current) in &names {
        let Some(node) = node_for_entity(entity, &parents, &identities, projection) else {
            continue;
        };
        let fallback = [SearchTextRun {
            text: node.name.clone(),
            highlighted: false,
        }];
        let runs = projection
            .name_runs
            .get(&node.name)
            .map(Vec::as_slice)
            .unwrap_or(&fallback);
        let cut = truncate_runs(runs, name_budget(entity, &parents, &cards, &palette));
        if current.is_none_or(|value| value.0 != cut) {
            commands.entity(entity).insert(TextRuns(cut));
        }
    }
}
