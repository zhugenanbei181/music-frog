//! Parity capture scenarios for the Slint surface.
//!
//! A capture scenario names one business of the shared parity catalogue. The
//! activation performs the business's own gesture on the real surface, so the
//! pixels of the receipt show the business and never a route change.
//!
//! The set of businesses the surface carries is closed and typed: a
//! business the surface does not carry has no variant in
//! [`CarriedScenario`], so the capture path can never publish it as
//! activated.

use crate::AppWindow;
use crate::lumen_projection::project_workbench;
use crate::surface::action::workbench::{
    AGGREGATE_CREATE, AGGREGATE_PREVIEW, APP_ROUTING_RULE, DIFF_LOAD, DNS_CACHE_CONFIRM,
    DNS_HOSTS_ADD, DNS_HOSTS_SAVE, DNS_LEAK_PROBE, DNS_QUERY_RUN, DOCTOR_BOOTSTRAP,
    DOCTOR_DIAGNOSE, FILTER_SAVE, RESTORE_PREPARE, SCRIPT_EXPORT_PREPARE, SCRIPT_RUN, SYNC_RESTORE,
    SYNC_TAKE_REMOTE,
};
use crate::surface::workbenches::Workbenches;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use slint::ComponentHandle;
use slint::Model;
use std::cell::{Cell, RefCell};

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

/// The input one business edits.
///
/// Every field is the draft identity the shared reducer owns, so the capture
/// types into the same field the user types into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraftField {
    /// The DNS query name of the DNS page.
    DnsQueryName,
    /// The Hosts row of the DNS page.
    DnsHostsRow,
    /// The script source of the editor page.
    ScriptCode,
    /// The include pattern of the subscription filter editor.
    FilterInclude,
    /// The target name of the aggregation wizard.
    AggregatorTarget,
    /// The routing rule of the application routing editor.
    AppRoutingRule,
}

impl DraftField {
    /// The draft identity the shared reducer owns.
    pub fn id(self) -> &'static str {
        match self {
            Self::DnsQueryName => "dns-query-name",
            Self::DnsHostsRow => "dns-hosts-row",
            Self::ScriptCode => "script-code",
            Self::FilterInclude => "filter-include",
            Self::AggregatorTarget => "aggregator-target",
            Self::AppRoutingRule => "app-routing-rule",
        }
    }

    /// The value the capture types into the field.
    pub fn value(self) -> &'static str {
        match self {
            Self::DnsQueryName => "example.org",
            Self::DnsHostsRow => "10.0.0.9 blocked.example.com",
            Self::ScriptCode => "function main(config) { return config; }\n",
            Self::FilterInclude => "Hong Kong|Japan|Singapore",
            Self::AggregatorTarget => "Merged Global",
            Self::AppRoutingRule => "proxy",
        }
    }
}

/// One step of the gesture that opens a parity business.
///
/// The step is the one a user performs: the business's own input or the
/// business's own control. The capture never fabricates a surface state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusinessStep {
    /// Type the field's value into the business's own input.
    Type(DraftField),
    /// Press the business's own control.
    Press {
        /// The panel action identity.
        action: &'static str,
        /// The row the action addresses.
        row: &'static str,
    },
}

/// The real gesture that opens one parity business.
///
/// The gesture is the one a user performs: the business's own input and the
/// business's own control. The capture never fabricates a surface state.
pub type BusinessGesture = &'static [BusinessStep];

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
        Self::ALL
            .into_iter()
            .find(|scenario| scenario.name() == value)
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

    /// The gesture a user performs to open this business.
    ///
    /// Every business has a control of its own, so the capture presses the
    /// control the user presses instead of navigating to a route. The sheet
    /// businesses open their sheet instead of a gesture, so they carry none.
    pub fn gesture(self) -> Option<BusinessGesture> {
        let gesture: BusinessGesture = match self {
            Self::DnsLeakAlert => &[BusinessStep::Press {
                action: DNS_LEAK_PROBE,
                row: "",
            }],
            Self::DnsQueryDetails => &[
                BusinessStep::Type(DraftField::DnsQueryName),
                BusinessStep::Press {
                    action: DNS_QUERY_RUN,
                    row: "",
                },
            ],
            Self::DnsFakeIpFlushConfirm => &[BusinessStep::Press {
                action: DNS_CACHE_CONFIRM,
                row: "",
            }],
            Self::DnsHostsEditor => &[
                BusinessStep::Type(DraftField::DnsHostsRow),
                BusinessStep::Press {
                    action: DNS_HOSTS_ADD,
                    row: "",
                },
                BusinessStep::Press {
                    action: DNS_HOSTS_SAVE,
                    row: "",
                },
            ],
            Self::ProfilesScriptSandbox => &[
                BusinessStep::Type(DraftField::ScriptCode),
                BusinessStep::Press {
                    action: SCRIPT_RUN,
                    row: "",
                },
            ],
            Self::ProfilesScriptExportReview => &[BusinessStep::Press {
                action: SCRIPT_EXPORT_PREPARE,
                row: "",
            }],
            Self::ProfilesDiffViewer => &[BusinessStep::Press {
                action: DIFF_LOAD,
                row: "",
            }],
            Self::ProfilesSnapshotRestoreConfirm => &[BusinessStep::Press {
                action: RESTORE_PREPARE,
                row: "",
            }],
            Self::ProfilesFilterEditor => &[
                BusinessStep::Type(DraftField::FilterInclude),
                BusinessStep::Press {
                    action: FILTER_SAVE,
                    row: "",
                },
            ],
            Self::ProfilesAggregatorWizard => &[
                BusinessStep::Type(DraftField::AggregatorTarget),
                BusinessStep::Press {
                    action: AGGREGATE_CREATE,
                    row: "",
                },
            ],
            Self::ProfilesAggregatorPreview => &[
                BusinessStep::Type(DraftField::AggregatorTarget),
                BusinessStep::Press {
                    action: AGGREGATE_PREVIEW,
                    row: "",
                },
            ],
            Self::DoctorFailureRecovery => &[BusinessStep::Press {
                action: DOCTOR_DIAGNOSE,
                row: "",
            }],
            Self::LifecycleRecoveryPanel => &[BusinessStep::Press {
                action: DOCTOR_BOOTSTRAP,
                row: "",
            }],
            Self::SyncConflictResolver => &[BusinessStep::Press {
                action: SYNC_TAKE_REMOTE,
                row: "",
            }],
            Self::SyncRestoreConfirm => &[BusinessStep::Press {
                action: SYNC_RESTORE,
                row: "snap-20261020",
            }],
            Self::AppRoutingPolicyEditor => &[
                BusinessStep::Type(DraftField::AppRoutingRule),
                BusinessStep::Press {
                    action: APP_ROUTING_RULE,
                    row: "com.example.browser",
                },
            ],
            Self::ProfilesImportModal | Self::ProfilesDeleteConfirm => {
                // The sheet businesses open their sheet, not a gesture.
                return None;
            }
        };
        Some(gesture)
    }
}

/// What the capture already did for one scenario.
///
/// The capture drives the surface until the business shows its own outcome,
/// so the state it compares against is the route-only render of the same
/// page.
#[derive(Debug, Default)]
pub struct Activation {
    /// Whether the business's own gesture already ran.
    performed: Cell<bool>,
    /// The projected page state before the gesture ran.
    baseline: RefCell<Option<String>>,
}

impl Activation {
    /// Whether the business's own gesture already ran.
    pub fn performed(&self) -> bool {
        self.performed.get()
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
    activation: &Activation,
) -> bool {
    let Some(carried) = CarriedScenario::from_name(scenario) else {
        return false;
    };
    match carried.carrier() {
        ScenarioCarrier::WorkbenchPage(page) => {
            if !activation.performed.get() {
                // The page must really carry the business, so the workbench
                // projection runs again for the page the capture opened.
                window.set_page(page.into());
                window.set_page_origin("Settings".into());
                project_workbench(window, snapshot, workbenches);
                // The route-only render is the state the business's own
                // gesture is measured against.
                activation.baseline.replace(Some(page_signature(window)));
                if let Some(gesture) = carried.gesture() {
                    perform(window, gesture);
                }
                // The gesture moved the surface, so the page shows it again.
                project_workbench(window, snapshot, workbenches);
                activation.performed.set(true);
            }
            let baseline = activation.baseline.borrow().clone().unwrap_or_default();
            // The business shows its own outcome only after the owner
            // answered, so the capture waits for the terminal.
            page_signature(window) != baseline && !workbenches.has_pending_command()
        }
        ScenarioCarrier::ProfileImportSheet => {
            // The sheet is opened the way the "add profile" tile opens it, so
            // the profile count the sheet remembers stays the published one.
            window.invoke_open_profile_add();
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

/// Performs the business's own gesture on the real surface.
///
/// The gesture runs the callbacks the widget tree binds, so the capture
/// drives the same code the running product drives.
fn perform(window: &AppWindow, gesture: BusinessGesture) {
    for step in gesture {
        match *step {
            BusinessStep::Type(field) => {
                window.invoke_workbench_draft_edited(field.id().into(), field.value().into());
            }
            BusinessStep::Press { action, row } => {
                window.invoke_workbench_action(action.into(), row.into());
            }
        }
    }
}

/// The rendered state of the open workbench page.
///
/// The signature names every fact the page shows, so a capture can tell that
/// the business really changed the surface from a capture that only changed
/// the route.
fn page_signature(window: &AppWindow) -> String {
    let mut signature = String::new();
    let items = window.get_workbench_items();
    for row in 0..items.row_count() {
        let Some(item) = items.row_data(row) else {
            continue;
        };
        signature.push_str(item.id.as_str());
        signature.push('|');
        signature.push_str(item.kind.as_str());
        signature.push('|');
        signature.push_str(item.status.as_str());
        signature.push('|');
        signature.push_str(item.detail.as_str());
        signature.push('|');
        signature.push_str(item.trailing.as_str());
        signature.push('|');
        signature.push_str(item.label.as_str());
        signature.push('|');
        signature.push_str(if item.enabled { "1" } else { "0" });
        signature.push(';');
    }
    signature.push_str(window.get_workbench_draft().as_str());
    signature.push('|');
    signature.push_str(window.get_workbench_draft_2().as_str());
    signature.push('|');
    signature.push_str(window.get_notice().as_str());
    signature
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
