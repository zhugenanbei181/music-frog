//! The business panels of the shell workbench page.

use super::Panel;
use crate::surface::business::BusinessId;
use crate::surface::workbenches::Workbenches;
use infiltrator_contract::command_catalogue::CommandCatalogue;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_presentation::routing_intent_projection::{
    routing_intent_copy, routing_intent_copy_key,
};
use infiltrator_shared::locales::Lang;
use lurker_core_contract::routing_intent::RoutingIntent;
use lurker_ui_presentation::copy::TextCatalog;
use std::sync::PoisonError;

/// The business panels of the shell page.
pub(super) fn panels(
    snapshot: &SurfaceSnapshot,
    catalog: &Lang<'_>,
    workbenches: &Workbenches,
) -> Vec<Panel> {
    let intent = snapshot
        .core
        .routing_intent
        .current
        .clone()
        .unwrap_or(RoutingIntent::FollowRules);
    let intent_copy = routing_intent_copy(&intent, catalog);
    let intent_key = routing_intent_copy_key(&intent);
    let failure = snapshot
        .failure
        .as_ref()
        .map(|failure| failure.message.clone());

    vec![
        {
            // The palette is the shared command catalogue: the surface asks
            // the shared list which rows the query keeps and renders them, so
            // the palette never keeps a list of its own.
            let rows = palette_rows(catalog, workbenches);
            let mut panel = Panel::new(
                BusinessId::ShellCommandPalette,
                catalog.text("hotkey_command_palette"),
                format!("{}", rows.len()),
                rows,
            );
            panel.failure = failure.clone();
            panel
        },
        {
            let mut panel = Panel::new(
                BusinessId::ShellRoutingIntentControl,
                catalog.text("overview_routing_intent_title"),
                intent_copy,
                vec![
                    catalog.text("routing_intent_change_action").into_owned(),
                    catalog.text("routing_intent_settings").into_owned(),
                ],
            );
            panel.failure = failure.clone();
            panel.guide = Some(catalog.text("routing_intent_unavailable").into_owned());
            panel
        },
        {
            let mut panel = Panel::new(
                BusinessId::ShellRoutingIntentAuthentication,
                catalog.text("routing_intent_title"),
                intent_key.to_owned(),
                vec![
                    catalog.text("routing_intent_unavailable").into_owned(),
                    catalog.text("routing_intent_dismiss").into_owned(),
                ],
            );
            panel.failure = failure;
            panel.guide = Some(catalog.text("routing_intent_unavailable").into_owned());
            panel
        },
    ]
}

/// The command rows the shared catalogue keeps for the typed query.
///
/// The catalogue owns the list and the match rule, so the palette shows the
/// rows the shared list keeps and nothing else.
fn palette_rows(catalog: &Lang<'_>, workbenches: &Workbenches) -> Vec<String> {
    let query = {
        let drafts = workbenches
            .drafts
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        drafts.command_query().to_owned()
    };
    let catalogue = CommandCatalogue::new();
    catalogue
        .filtered_indices(&query)
        .into_iter()
        .filter_map(|index| catalogue.entry(index))
        .map(|entry| format!("{} · {}", entry.id, catalog.text(entry.title_key)))
        .collect()
}
