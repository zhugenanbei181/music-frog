//! The one typed page model of the app-routing page, shared by every surface.
//!
//! The app-routing page owns two read models: the per-application routing
//! rules and the Windows AppContainer loopback exemptions. Iced kept a
//! private `UwpLoopbackState` that shadowed the contract's
//! `UwpLoopbackSnapshot` — with its own package list, its own revision and a
//! `is_scanning` flag — so the same fact had two shapes and two truths.
//!
//! This module owns the page state. The contract snapshot stays the truth,
//! the scan is one typed phase with its own token, and the filters are typed
//! folds of the observed snapshot.

use crate::operation_phase::OperationPhase;
use crate::routing_projection::mode_option_key;
use infiltrator_contract::app_routing::{AppRoutingMode, AppRoutingRule};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::surface_snapshot::{AppRoutingPageSnapshot, AppSnapshot};
use infiltrator_contract::uwp::{UwpLoopbackAvailability, UwpLoopbackSnapshot, UwpPackageSnapshot};
use infiltrator_domain::app_routing;

/// The one page model of the app-routing page.
///
/// A surface renders the app-routing page from this model alone: the routing
/// mode, the application list, the loopback packages and the scan phase come
/// from here, and the surface only dispatches typed intents back.
#[derive(Clone, Debug, Default)]
pub struct AppRoutingPageModel {
    page: Option<AppRoutingPageSnapshot>,
    uwp: UwpLoopbackSnapshot,
    app_filter: String,
    uwp_filter: String,
    scan: OperationPhase,
    exemption_write: OperationPhase,
}

impl AppRoutingPageModel {
    /// Fold the owner's app-routing page snapshot into the model.
    pub fn observe_page(&mut self, page: AppRoutingPageSnapshot) {
        self.uwp = page.uwp_loopback.clone();
        self.page = Some(page);
    }

    /// The routing mode the owner reported.
    ///
    /// A page that has not been observed reports no mode: the surface shows
    /// "unknown" instead of guessing the default.
    pub fn mode(&self) -> Option<app_routing::AppRoutingMode> {
        self.page
            .as_ref()
            .map(|page| app_routing::AppRoutingMode::from(page.mode))
    }

    /// The copy key of the routing mode, including the unobserved case.
    ///
    /// An unobserved page renders the unobserved copy; it never falls back
    /// to a mode the owner never reported.
    pub fn mode_copy_key(&self) -> &'static str {
        mode_option_key(self.mode())
    }

    /// Whether the owner published a routing mode.
    pub fn mode_observed(&self) -> bool {
        self.page.is_some()
    }

    /// Whether the owner includes the system applications.
    pub fn include_system(&self) -> Option<bool> {
        self.page.as_ref().map(|page| page.include_system)
    }

    /// The applications the owner reported.
    pub fn apps(&self) -> &[AppSnapshot] {
        match &self.page {
            Some(page) => &page.apps,
            None => &[],
        }
    }

    /// The filter the page applies to the application list.
    pub fn app_filter(&self) -> &str {
        &self.app_filter
    }

    /// Edit the filter the page applies to the application list.
    pub fn edit_app_filter(&mut self, filter: &str) {
        self.app_filter = filter.trim().to_owned();
    }

    /// The applications the page shows.
    ///
    /// The filter matches the application name, its process name and its
    /// identity.
    pub fn visible_apps(&self) -> Vec<&AppSnapshot> {
        let query = self.app_filter.to_ascii_lowercase();
        self.apps()
            .iter()
            .filter(|app| {
                query.is_empty()
                    || app.name.to_ascii_lowercase().contains(&query)
                    || app.process_name.to_ascii_lowercase().contains(&query)
                    || app.id.to_ascii_lowercase().contains(&query)
            })
            .collect()
    }

    /// The routing rule of one application, when the owner reported it.
    pub fn app_rule(&self, app_id: &str) -> Option<AppRoutingRule> {
        self.apps()
            .iter()
            .find(|app| app.id == app_id)
            .map(|app| app.rule)
    }

    /// The owner intent that changes the routing rule of one application.
    pub fn set_app_rule_intent(&self, app_id: &str, rule: AppRoutingRule) -> CommandIntent {
        CommandIntent::SetAppRule {
            app_id: app_id.to_owned(),
            rule,
        }
    }

    /// The owner intent that changes the routing mode.
    pub fn set_mode_intent(&self, mode: AppRoutingMode) -> CommandIntent {
        CommandIntent::SetAppRoutingMode { mode }
    }

    /// The loopback packages the owner reported.
    pub fn uwp_packages(&self) -> &[UwpPackageSnapshot] {
        &self.uwp.packages
    }

    /// The loopback availability the owner reported.
    pub fn uwp_availability(&self) -> &UwpLoopbackAvailability {
        &self.uwp.availability
    }

    /// The revision of the loopback readback the owner reported.
    pub fn uwp_revision(&self) -> u64 {
        self.uwp.revision
    }

    /// The filter the page applies to the loopback package list.
    pub fn uwp_filter(&self) -> &str {
        &self.uwp_filter
    }

    /// Edit the filter the page applies to the loopback package list.
    pub fn edit_uwp_filter(&mut self, filter: &str) {
        self.uwp_filter = filter.trim().to_owned();
    }

    /// The loopback packages the page shows.
    ///
    /// The filter matches the package display name and its family name.
    pub fn visible_uwp_packages(&self) -> Vec<&UwpPackageSnapshot> {
        let query = self.uwp_filter.to_ascii_lowercase();
        self.uwp_packages()
            .iter()
            .filter(|package| {
                query.is_empty()
                    || package.display_name.to_ascii_lowercase().contains(&query)
                    || package
                        .package_family_name
                        .to_ascii_lowercase()
                        .contains(&query)
                    || package.sid.to_ascii_lowercase().contains(&query)
            })
            .collect()
    }

    /// The owner intent that changes the loopback exemption of one package.
    pub fn set_exemption_intent(&self, sid: &str, exempt: bool) -> CommandIntent {
        CommandIntent::SetUwpAppExemption {
            sid: sid.to_owned(),
            exempt,
        }
    }

    /// The owner intent that changes every loopback exemption at once.
    pub fn set_all_exemptions_intent(&self, exempt: bool) -> CommandIntent {
        CommandIntent::SetAllUwpExemptions { exempt }
    }

    /// The owner intent that asks the host to rescan the installed packages.
    pub fn scan_intent(&self) -> CommandIntent {
        CommandIntent::ScanUwpApps
    }

    /// The phase of the submitted package scan.
    pub fn scan_phase(&self) -> &OperationPhase {
        &self.scan
    }

    /// The phase of the submitted package scan, for the fold that changes it.
    pub fn scan_phase_mut(&mut self) -> &mut OperationPhase {
        &mut self.scan
    }

    /// The phase of the submitted exemption write.
    pub fn exemption_write_phase(&self) -> &OperationPhase {
        &self.exemption_write
    }

    /// The phase of the submitted exemption write, for the fold that changes it.
    pub fn exemption_write_phase_mut(&mut self) -> &mut OperationPhase {
        &mut self.exemption_write
    }
}

#[cfg(test)]
#[path = "app_routing_page_model_tests.rs"]
mod tests;
