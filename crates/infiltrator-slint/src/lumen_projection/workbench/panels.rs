//! The business panels of the Slint workbench pages.
//!
//! Every panel is one business of the shared parity catalogue. The panel shows
//! the facts the shared owner published and the business's own controls; the
//! phase of the panel is the surface's own presentation state, so the owner
//! stays the single source of every business fact.

mod connections;
mod editor;
mod filters;
mod logs;
mod overview;
mod proxies;
mod rules;
mod runtime;
mod settings;
mod shell;

use super::{Tone, button, header, text};
use crate::WorkbenchItem;
use crate::Workbenches;
use crate::surface::action::workbench::business::{
    cancel_action, confirm_action, open_action, refresh_action,
};
use crate::surface::business::{BusinessId, BusinessPanels, BusinessPhase};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_shared::locales::Lang;
use lurker_core_contract::page::PageStatus;
use lurker_ui_presentation::copy::TextCatalog;

/// The shared facts one business panel shows.
pub(super) struct Panel {
    /// The business the panel carries.
    pub business: BusinessId,
    /// The localized title of the business.
    pub title: String,
    /// The one-line summary of the facts the owner published.
    pub summary: String,
    /// The facts the business's own review shows.
    pub facts: Vec<String>,
    /// The typed failure the owner's read model reports.
    ///
    /// A host that cannot carry the business publishes the reason, and the
    /// panel shows it instead of a fact the owner never published.
    pub failure: Option<String>,
    /// Whether the owner's read model carries no observation at all.
    pub empty: bool,
    /// The guide the owner publishes for this business.
    pub guide: Option<String>,
}

impl Panel {
    /// Builds one business panel.
    pub fn new(
        business: BusinessId,
        title: impl Into<String>,
        summary: impl Into<String>,
        facts: Vec<String>,
    ) -> Self {
        Self {
            business,
            title: title.into(),
            summary: summary.into(),
            facts,
            failure: None,
            empty: false,
            guide: None,
        }
    }
}

/// Renders the business panels of one workbench page.
///
/// The compact row of a business is always on the page; the business's own
/// review opens on the business's own control, so a capture that presses the
/// control really changes what the surface renders.
pub(super) fn render(
    panels: &[Panel],
    phases: &BusinessPanels,
    catalog: &Lang<'_>,
) -> Vec<WorkbenchItem> {
    let mut items = Vec::new();
    for panel in panels {
        let phase = phases.phase(panel.business);
        let id = panel.business.name();
        items.push(header(
            id,
            &panel.title,
            &status(&phase, catalog),
            tone(&phase),
        ));
        items.push(text(&format!("{id}/summary"), &panel.summary));
        if let Some(failure) = &panel.failure {
            items.push(text(&format!("{id}/failure"), failure));
        }
        if let Some(guide) = &panel.guide {
            items.push(text(&format!("{id}/guide"), guide));
        }
        if panel.empty {
            items.push(text(&format!("{id}/empty"), &catalog.text("surface_empty")));
        }
        match &phase {
            BusinessPhase::Reviewing => {
                for (index, fact) in panel.facts.iter().enumerate() {
                    items.push(text(&format!("{id}/fact/{index}"), fact));
                }
                items.push(button(
                    &format!("{id}/confirm"),
                    &catalog.text("common_confirm"),
                    &confirm_action(panel.business),
                    !panel.empty,
                ));
                items.push(button(
                    &format!("{id}/cancel"),
                    &catalog.text("modal_cancel"),
                    &cancel_action(panel.business),
                    true,
                ));
            }
            BusinessPhase::Pending => {
                items.push(text(
                    &format!("{id}/pending"),
                    &catalog.text("doctor_operation_pending"),
                ));
            }
            BusinessPhase::Refused { message } => {
                items.push(text(&format!("{id}/failure"), message));
                items.push(button(
                    &format!("{id}/retry"),
                    &catalog.text("script_workbench_retry"),
                    &confirm_action(panel.business),
                    true,
                ));
            }
            BusinessPhase::Accepted | BusinessPhase::Applied => {
                items.push(text(
                    &format!("{id}/result"),
                    &catalog.text("dns_status_done"),
                ));
            }
            BusinessPhase::Idle => {}
        }
        if !matches!(phase, BusinessPhase::Reviewing) {
            items.push(button(
                &format!("{id}/open"),
                &catalog.text("common_details"),
                &open_action(panel.business),
                true,
            ));
            items.push(button(
                &format!("{id}/refresh"),
                &catalog.text("common_reload"),
                &refresh_action(panel.business),
                true,
            ));
        }
    }
    items
}

/// The status line of one business panel.
fn status(phase: &BusinessPhase, catalog: &Lang<'_>) -> String {
    match phase {
        BusinessPhase::Idle => catalog.text("not_started").into_owned(),
        BusinessPhase::Reviewing => catalog.text("waiting").into_owned(),
        BusinessPhase::Pending => catalog.text("doctor_operation_pending").into_owned(),
        BusinessPhase::Refused { message } => message.clone(),
        BusinessPhase::Accepted | BusinessPhase::Applied => {
            catalog.text("dns_status_done").into_owned()
        }
    }
}

/// The tone of one business panel.
fn tone(phase: &BusinessPhase) -> Tone {
    match phase {
        BusinessPhase::Idle | BusinessPhase::Reviewing => Tone::Neutral,
        BusinessPhase::Pending => Tone::Warning,
        BusinessPhase::Refused { .. } => Tone::Danger,
        BusinessPhase::Accepted | BusinessPhase::Applied => Tone::Success,
    }
}

/// The typed failure the owner published for one read model.
///
/// A host that cannot carry the business publishes the reason, so the panel
/// shows the owner's own answer instead of a fact nobody published.
pub(super) fn page_failure(status: &PageStatus) -> Option<String> {
    match status {
        PageStatus::Unavailable { failure } | PageStatus::Failed { failure } => {
            Some(failure.message.clone())
        }
        PageStatus::Loading | PageStatus::Ready | PageStatus::Empty => None,
    }
}

/// Whether the owner's read model carries no observation.
pub(super) fn page_empty(status: &PageStatus) -> bool {
    matches!(status, PageStatus::Empty)
}

/// The business panels of the connections workbench page.
pub(super) fn connections(snapshot: &SurfaceSnapshot, catalog: &Lang<'_>) -> Vec<Panel> {
    connections::panels(snapshot, catalog)
}

/// The business panels of the logs workbench page.
pub(super) fn logs(
    snapshot: &SurfaceSnapshot,
    catalog: &Lang<'_>,
    workbenches: &Workbenches,
) -> Vec<Panel> {
    logs::panels(snapshot, catalog, workbenches)
}

/// The business panels of the runtime workbench page.
pub(super) fn runtime(snapshot: &SurfaceSnapshot, catalog: &Lang<'_>) -> Vec<Panel> {
    runtime::panels(snapshot, catalog)
}

/// The business panels of the proxies workbench page.
pub(super) fn proxies(snapshot: &SurfaceSnapshot, catalog: &Lang<'_>) -> Vec<Panel> {
    proxies::panels(snapshot, catalog)
}

/// The business panels of the rules workbench page.
pub(super) fn rules(snapshot: &SurfaceSnapshot, catalog: &Lang<'_>) -> Vec<Panel> {
    rules::panels(snapshot, catalog)
}

/// The business panels of the settings workbench page.
pub(super) fn settings(snapshot: &SurfaceSnapshot, catalog: &Lang<'_>) -> Vec<Panel> {
    settings::panels(snapshot, catalog)
}

/// The business panels of the shell workbench page.
pub(super) fn shell(
    snapshot: &SurfaceSnapshot,
    catalog: &Lang<'_>,
    workbenches: &Workbenches,
) -> Vec<Panel> {
    shell::panels(snapshot, catalog, workbenches)
}

/// The business panels of the overview workbench page.
pub(super) fn overview(snapshot: &SurfaceSnapshot, catalog: &Lang<'_>) -> Vec<Panel> {
    overview::panels(snapshot, catalog)
}

/// The business panels of the editor workbench page.
pub(super) fn editor(snapshot: &SurfaceSnapshot, catalog: &Lang<'_>) -> Vec<Panel> {
    editor::panels(snapshot, catalog)
}

/// The business panels of the filters workbench page.
pub(super) fn filters(snapshot: &SurfaceSnapshot, catalog: &Lang<'_>) -> Vec<Panel> {
    filters::panels(snapshot, catalog)
}
