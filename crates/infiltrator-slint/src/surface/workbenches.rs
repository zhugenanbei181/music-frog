//! The shared presentation reducers the Slint workbench pages drive.
//!
//! Every workbench business owns its draft and its command/readback
//! correlation through the reducers `infiltrator-presentation` publishes. The
//! Slint surface never re-implements one of those state machines: it projects
//! the reducer and turns a gesture into the shared typed command.

use infiltrator_presentation::dns_cache_actions::DnsCacheActions;
use infiltrator_presentation::dns_hosts_editor::DnsHostsEditor;
use infiltrator_presentation::dns_query_actions::DnsQueryActions;
use infiltrator_presentation::doctor_actions::DoctorActionState;
use infiltrator_presentation::script_workbench::ScriptWorkbench;
use infiltrator_presentation::subscription_filter_editor::SubscriptionFilterEditor;
use std::sync::Mutex;

/// The surface-owned draft and correlation state of the workbench pages.
///
/// Every field is one shared reducer. The surface owns the draft, the shared
/// owner owns the product facts, and the reducer correlates the two.
#[derive(Debug, Default)]
pub struct Workbenches {
    /// The DNS query panel draft and its correlated readback.
    pub dns_query: Mutex<DnsQueryActions>,
    /// The Fake-IP cache flush confirmation and its correlated readback.
    pub dns_cache: Mutex<DnsCacheActions>,
    /// The Hosts editor draft, its validation and its guarded write.
    pub dns_hosts: Mutex<DnsHostsEditor>,
    /// The diagnostic action correlation shared by the doctor panels.
    pub doctor: Mutex<DoctorActionState>,
    /// The script sandbox and the reviewed export lifecycle.
    pub script: Mutex<ScriptWorkbench>,
    /// The subscription filter draft and its correlated write.
    pub filter: Mutex<SubscriptionFilterEditor>,
}
