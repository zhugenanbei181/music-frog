//! Projection of the shared workbench read models into the Slint items.
//!
//! Every value shown here comes from `SurfaceSnapshot`, from the shared
//! presentation projections, or from the shared presentation reducer the
//! surface owns. The surface never re-implements a business rule.

mod editor;
mod pages;
mod routing;
mod sync;

use super::super::slint_model;
use super::super::{AppWindow, WorkbenchItem};
use crate::surface::workbenches::Workbenches;
use infiltrator_contract::subscription_filter_form::FilterObservation;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_shared::locales::Lang;
use lurker_core_contract::error::Failure;
use lurker_ui_presentation::copy::TextCatalog;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The tone an item status carries into the surface.
#[derive(Clone, Copy)]
pub(super) enum Tone {
    Neutral,
    Success,
    Warning,
    Danger,
}

impl Tone {
    fn as_str(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Danger => "danger",
        }
    }
}

/// Projects the workbench page the window currently shows.
pub(super) fn project(
    window: &AppWindow,
    snapshot: &SurfaceSnapshot,
    language: &str,
    workbenches: &Workbenches,
) {
    let catalog = Lang(language);
    observe(snapshot, workbenches);
    let page = window.get_page().to_string();
    let (items, draft, draft2, draft_id, draft2_id) = match page.as_str() {
        "Dns" => pages::dns_page(snapshot, &catalog, workbenches),
        "Editor" => editor::page(snapshot, &catalog, workbenches),
        "Filters" => pages::filters_page(snapshot, &catalog, workbenches),
        "Doctor" => pages::doctor_page(snapshot, &catalog, workbenches),
        "Sync" => sync::page(snapshot, &catalog, workbenches),
        "AppRouting" => pages::app_routing_page(snapshot, &catalog, workbenches),
        _ => (Vec::new(), None, None, String::new(), String::new()),
    };
    window.set_workbench_items(slint_model(items));
    // Only a reducer-backed draft is written back. A draft the user types
    // without a reducer stays the value the user entered.
    if let Some(draft) = draft {
        window.set_workbench_draft(draft.into());
    }
    if let Some(draft2) = draft2 {
        window.set_workbench_draft_2(draft2.into());
    }
    window.set_workbench_draft_id(draft_id.into());
    window.set_workbench_draft_2_id(draft2_id.into());
    project_draft_placeholders(window, &page, &catalog);
}

/// Publishes the localized placeholder of the draft fields the page shows.
fn project_draft_placeholders(window: &AppWindow, page: &str, catalog: &Lang<'_>) {
    let (first, second) = match page {
        "Dns" => ("dns_query_name", "dns_hosts_editor_hint"),
        "Editor" => ("script_sandbox_code_input", ""),
        "Filters" => ("field_filter_include", "aggregator_name_placeholder"),
        "AppRouting" => ("", "app_routing_rules_hint"),
        _ => ("", ""),
    };
    window.set_workbench_draft_placeholder(catalog.text(first).into_owned().into());
    window.set_workbench_draft_2_placeholder(catalog.text(second).into_owned().into());
}

/// Feeds the shared owner snapshot into the shared reducers the surface owns.
///
/// The reducer keeps the correlated readback of its own operation; the owner
/// snapshot only refreshes the facts the reducer projects.
fn observe(snapshot: &SurfaceSnapshot, workbenches: &Workbenches) {
    {
        let mut query = lock(&workbenches.dns_query);
        query.observe(&snapshot.dns_query);
    }
    {
        let mut cache = lock(&workbenches.dns_cache);
        cache.observe(&snapshot.dns_cache);
    }
    {
        let mut hosts = lock(&workbenches.dns_hosts);
        hosts.observe(&snapshot.dns_hosts);
    }
    {
        let mut script = lock(&workbenches.script);
        script.observe_snapshot(snapshot.script_sandbox.clone());
        script.observe_export(snapshot.script_export.clone());
    }
    {
        let mut filter = lock(&workbenches.filter);
        filter.observe(filter_observation(snapshot));
    }
}

/// The observed filter draft of the active profile.
fn filter_observation(snapshot: &SurfaceSnapshot) -> Result<Option<FilterObservation>, Failure> {
    let Some(page) = snapshot.pages.profiles.data.as_ref() else {
        return Ok(None);
    };
    let Some(profile) = page.profiles.iter().find(|profile| profile.is_active) else {
        return Ok(None);
    };
    let workspace_identity = profile.filter_source.clone()?;
    Ok(Some(FilterObservation {
        workspace_identity,
        filter: profile.filter.clone(),
    }))
}

/// Builds the header item of one business panel.
pub(super) fn header(id: &str, title: &str, status: &str, tone: Tone) -> WorkbenchItem {
    WorkbenchItem {
        id: id.into(),
        kind: "header".into(),
        title: title.into(),
        status: status.into(),
        detail: String::new().into(),
        trailing: String::new().into(),
        tone: tone.as_str().into(),
        label: String::new().into(),
        enabled: false,
        action_id: String::new().into(),
        row_id: String::new().into(),
    }
}

/// Builds the detail line of one business panel.
pub(super) fn text(id: &str, detail: &str) -> WorkbenchItem {
    WorkbenchItem {
        id: id.into(),
        kind: "text".into(),
        title: String::new().into(),
        status: String::new().into(),
        detail: detail.into(),
        trailing: String::new().into(),
        tone: "neutral".into(),
        label: String::new().into(),
        enabled: false,
        action_id: String::new().into(),
        row_id: String::new().into(),
    }
}

/// Builds one observation row of a business panel.
pub(super) fn row(id: &str, title: &str, subtitle: &str, trailing: &str) -> WorkbenchItem {
    WorkbenchItem {
        id: id.into(),
        kind: "row".into(),
        title: title.into(),
        status: String::new().into(),
        detail: subtitle.into(),
        trailing: trailing.into(),
        tone: "neutral".into(),
        label: String::new().into(),
        enabled: false,
        action_id: String::new().into(),
        row_id: String::new().into(),
    }
}

/// Builds one observation row that owns a row action.
pub(super) fn row_action(
    id: &str,
    row_id: &str,
    title: &str,
    subtitle: &str,
    trailing: &str,
    action: &str,
    label: &str,
) -> WorkbenchItem {
    WorkbenchItem {
        id: id.into(),
        kind: "row".into(),
        title: title.into(),
        status: String::new().into(),
        detail: subtitle.into(),
        trailing: trailing.into(),
        tone: "neutral".into(),
        label: label.into(),
        enabled: !action.is_empty(),
        action_id: action.into(),
        row_id: row_id.into(),
    }
}

/// Builds one action button of a business panel.
pub(super) fn button(id: &str, label: &str, action: &str, enabled: bool) -> WorkbenchItem {
    WorkbenchItem {
        id: id.into(),
        kind: "button".into(),
        title: String::new().into(),
        status: String::new().into(),
        detail: String::new().into(),
        trailing: String::new().into(),
        tone: "neutral".into(),
        label: label.into(),
        enabled,
        action_id: action.into(),
        row_id: String::new().into(),
    }
}

/// Locks one shared reducer for a read.
pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
