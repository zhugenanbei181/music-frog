//! Parity capture scenarios for the Slint surface.
//!
//! A capture scenario names one business of the shared parity catalogue. The
//! activation opens the same surface state a user gesture opens, and the
//! bounds are the real on-screen geometry of the surface that was opened. A
//! receipt therefore proves an interaction and never a route change.
//!
//! The set of businesses the surface carries is closed and typed: a
//! business the surface does not carry has no variant in
//! [`CarriedScenario`], so the capture path can never publish it as
//! activated.

use crate::lumen_projection::project_workbench;
use crate::surface::workbenches::Workbenches;
use crate::AppWindow;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use slint::ComponentHandle;

/// The geometry of the surface a capture activated, in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptureBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// The surface state one parity business is carried by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScenarioCarrier {
    /// The business is carried by a workbench page of the surface.
    WorkbenchPage(&'static str),
    /// The business is carried by the add-profile sheet.
    ProfileImportSheet,
    /// The business is carried by the delete-profile sheet.
    ProfileDeleteSheet,
}

/// The parity businesses the Slint surface carries.
///
/// Every variant is a business the surface really opens, so the capture
/// path can activate it and publish its real geometry. A business the
/// surface does not carry is simply absent, and the capture path refuses it
/// instead of publishing a receipt for a surface that never opened it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarriedScenario {
    /// The DNS leak probe panel.
    DnsLeakAlert,
    /// The DNS query panel with its record details.
    DnsQueryDetails,
    /// The Fake-IP cache flush confirmation.
    DnsFakeIpFlushConfirm,
    /// The hosts editor.
    DnsHostsEditor,
    /// The script sandbox.
    ProfilesScriptSandbox,
    /// The reviewed script export.
    ProfilesScriptExportReview,
    /// The configuration difference viewer.
    ProfilesDiffViewer,
    /// The snapshot restore confirmation.
    ProfilesSnapshotRestoreConfirm,
    /// The subscription filter editor.
    ProfilesFilterEditor,
    /// The profile aggregation wizard.
    ProfilesAggregatorWizard,
    /// The profile aggregation preview.
    ProfilesAggregatorPreview,
    /// The diagnostic failure recovery panel.
    DoctorFailureRecovery,
    /// The lifecycle recovery panel.
    LifecycleRecoveryPanel,
    /// The synchronisation conflict resolver.
    SyncConflictResolver,
    /// The synchronisation restore confirmation.
    SyncRestoreConfirm,
    /// The application routing policy editor.
    AppRoutingPolicyEditor,
    /// The add-profile import sheet.
    ProfilesImportModal,
    /// The delete-profile confirmation sheet.
    ProfilesDeleteConfirm,
}

impl CarriedScenario {
    /// Every parity business the Slint surface carries.
    pub const ALL: [Self; 18] = [
        Self::DnsLeakAlert,
        Self::DnsQueryDetails,
        Self::DnsFakeIpFlushConfirm,
        Self::DnsHostsEditor,
        Self::ProfilesScriptSandbox,
        Self::ProfilesScriptExportReview,
        Self::ProfilesDiffViewer,
        Self::ProfilesSnapshotRestoreConfirm,
        Self::ProfilesFilterEditor,
        Self::ProfilesAggregatorWizard,
        Self::ProfilesAggregatorPreview,
        Self::DoctorFailureRecovery,
        Self::LifecycleRecoveryPanel,
        Self::SyncConflictResolver,
        Self::SyncRestoreConfirm,
        Self::AppRoutingPolicyEditor,
        Self::ProfilesImportModal,
        Self::ProfilesDeleteConfirm,
    ];

    /// The parity catalogue name of this business.
    pub fn name(self) -> &'static str {
        match self {
            Self::DnsLeakAlert => "dns-leak-alert",
            Self::DnsQueryDetails => "dns-query-details",
            Self::DnsFakeIpFlushConfirm => "dns-fake-ip-flush-confirm",
            Self::DnsHostsEditor => "dns-hosts-editor",
            Self::ProfilesScriptSandbox => "profiles-script-sandbox",
            Self::ProfilesScriptExportReview => "profiles-script-export-review",
            Self::ProfilesDiffViewer => "profiles-diff-viewer",
            Self::ProfilesSnapshotRestoreConfirm => "profiles-snapshot-restore-confirm",
            Self::ProfilesFilterEditor => "profiles-filter-editor",
            Self::ProfilesAggregatorWizard => "profiles-aggregator-wizard",
            Self::ProfilesAggregatorPreview => "profiles-aggregator-preview",
            Self::DoctorFailureRecovery => "doctor-failure-recovery",
            Self::LifecycleRecoveryPanel => "lifecycle-recovery-panel",
            Self::SyncConflictResolver => "sync-conflict-resolver",
            Self::SyncRestoreConfirm => "sync-restore-confirm",
            Self::AppRoutingPolicyEditor => "app-routing-policy-editor",
            Self::ProfilesImportModal => "profiles-import-modal",
            Self::ProfilesDeleteConfirm => "profiles-delete-confirm",
        }
    }

    /// The business the parity catalogue names.
    ///
    /// A name the surface does not carry has no business, so the capture
    /// path can refuse it.
    pub fn from_name(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|scenario| scenario.name() == value)
    }

    /// The surface state this business is carried by.
    pub fn carrier(self) -> ScenarioCarrier {
        match self {
            Self::DnsLeakAlert
            | Self::DnsQueryDetails
            | Self::DnsFakeIpFlushConfirm
            | Self::DnsHostsEditor => ScenarioCarrier::WorkbenchPage("Dns"),
            Self::ProfilesScriptSandbox
            | Self::ProfilesScriptExportReview
            | Self::ProfilesDiffViewer
            | Self::ProfilesSnapshotRestoreConfirm => ScenarioCarrier::WorkbenchPage("Editor"),
            Self::ProfilesFilterEditor
            | Self::ProfilesAggregatorWizard
            | Self::ProfilesAggregatorPreview => ScenarioCarrier::WorkbenchPage("Filters"),
            Self::DoctorFailureRecovery | Self::LifecycleRecoveryPanel => {
                ScenarioCarrier::WorkbenchPage("Doctor")
            }
            Self::SyncConflictResolver | Self::SyncRestoreConfirm => {
                ScenarioCarrier::WorkbenchPage("Sync")
            }
            Self::AppRoutingPolicyEditor => ScenarioCarrier::WorkbenchPage("AppRouting"),
            Self::ProfilesImportModal => ScenarioCarrier::ProfileImportSheet,
            Self::ProfilesDeleteConfirm => ScenarioCarrier::ProfileDeleteSheet,
        }
    }
}

/// Opens the surface state a parity scenario names.
///
/// The surface owns the state, so the activation only asks the surface to
/// open what a user gesture opens. A scenario the surface does not carry
/// returns `false` and is never published as activated.
pub fn activate(
    window: &AppWindow,
    scenario: &str,
    snapshot: &SurfaceSnapshot,
    workbenches: &Workbenches,
) -> bool {
    let Some(carried) = CarriedScenario::from_name(scenario) else {
        return false;
    };
    match carried.carrier() {
        ScenarioCarrier::WorkbenchPage(page) => {
            window.set_page(page.into());
            window.set_page_origin("Settings".into());
            // The page must really carry the business, so the workbench
            // projection runs again for the page the capture opened.
            project_workbench(window, snapshot, workbenches);
            true
        }
        ScenarioCarrier::ProfileImportSheet => {
            window.set_profile_add_kind_index(0);
            window.set_profile_add_open(true);
            true
        }
        ScenarioCarrier::ProfileDeleteSheet => {
            let Some(profiles) = snapshot
                .pages
                .profiles
                .data
                .as_ref()
                .map(|page| page.profiles.as_slice())
            else {
                return false;
            };
            let Some(profile) = profiles
                .iter()
                .find(|profile| !profile.is_active)
                .or_else(|| profiles.first())
            else {
                return false;
            };
            window.set_pending_delete_id(profile.id.clone().into());
            window.set_pending_delete_name(profile.name.clone().into());
            true
        }
    }
}

/// Reads the real geometry of the sheet the surface opened.
///
/// The bounds are empty until the surface really opened a sheet, so a capture
/// never publishes the geometry of a surface that stayed closed.
pub fn bounds(window: &AppWindow) -> Option<CaptureBounds> {
    if !window.get_capture_sheet_open() {
        return None;
    }
    let scale = window.window().scale_factor();
    let bounds = CaptureBounds {
        x: window.get_capture_bounds_x() * scale,
        y: window.get_capture_bounds_y() * scale,
        width: window.get_capture_bounds_width() * scale,
        height: window.get_capture_bounds_height() * scale,
    };
    if bounds.width <= 0.0 || bounds.height <= 0.0 || bounds.x < 0.0 || bounds.y < 0.0 {
        return None;
    }
    Some(bounds)
}
