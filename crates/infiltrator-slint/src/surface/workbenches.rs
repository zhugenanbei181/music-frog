//! The shared presentation reducers the Slint workbench pages drive.
//!
//! Every workbench business owns its draft and its command/readback
//! correlation through the reducers `infiltrator-presentation` publishes. The
//! Slint surface never re-implements one of those state machines: it projects
//! the reducer and turns a gesture into the shared typed command.

use infiltrator_contract::command_output::CommandOutput;
use infiltrator_presentation::dns_cache_actions::DnsCacheActions;
use infiltrator_presentation::dns_hosts_editor::DnsHostsEditor;
use infiltrator_presentation::dns_query_actions::DnsQueryActions;
use infiltrator_presentation::doctor_actions::{DoctorActionPhase, DoctorActionState};
use infiltrator_presentation::script_workbench::ScriptWorkbench;
use infiltrator_presentation::snapshot_restore_workbench::SnapshotRestoreWorkbench;
use infiltrator_presentation::subscription_filter_editor::SubscriptionFilterEditor;
use lurker_core_contract::error::Failure;
use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The shared reducer that owns one in-flight owner command.
///
/// The surface names only which reducer waits for the owner's terminal; the
/// reducer keeps the operation identity and the phase, so a terminal reaches
/// the business that started the command and no other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingBusiness {
    /// The DNS query panel.
    DnsQuery,
    /// The Fake-IP cache flush confirmation.
    DnsCache,
    /// The Hosts editor.
    DnsHosts,
    /// The diagnostic panels.
    Doctor,
    /// The script sandbox and the reviewed export.
    Script,
    /// The snapshot restoration review.
    Restore,
    /// The subscription filter editor.
    Filter,
}

/// The drafts of the panels that no shared reducer owns.
///
/// Every business draft a shared reducer owns lives in that reducer. The
/// remaining drafts are inputs of a shared command without a reducer of their
/// own, so the surface keeps them in one typed place and the widget tree only
/// mirrors them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PanelDrafts {
    /// The Hosts row the user is typing.
    hosts_row: String,
    /// The name the aggregation wizard gives the profile it creates.
    aggregation_target: String,
    /// The routing rule the policy editor applies to one application.
    routing_rule: String,
}

impl PanelDrafts {
    /// The Hosts row the user is typing.
    pub fn hosts_row(&self) -> &str {
        &self.hosts_row
    }

    /// Replaces the Hosts row the user is typing.
    pub fn set_hosts_row(&mut self, value: String) {
        self.hosts_row = value;
    }

    /// Forgets the Hosts row the user is typing.
    pub fn clear_hosts_row(&mut self) {
        self.hosts_row.clear();
    }

    /// The aggregation target name the user typed.
    pub fn aggregation_target(&self) -> &str {
        &self.aggregation_target
    }

    /// Replaces the aggregation target name.
    pub fn set_aggregation_target(&mut self, value: String) {
        self.aggregation_target = value;
    }

    /// Forgets the aggregation target name.
    pub fn clear_aggregation_target(&mut self) {
        self.aggregation_target.clear();
    }

    /// The routing rule the user typed.
    pub fn routing_rule(&self) -> &str {
        &self.routing_rule
    }

    /// Replaces the routing rule.
    pub fn set_routing_rule(&mut self, value: String) {
        self.routing_rule = value;
    }

    /// Forgets the routing rule.
    pub fn clear_routing_rule(&mut self) {
        self.routing_rule.clear();
    }
}

/// Whether the configuration difference view shows the published difference.
///
/// The owner owns the difference; the surface owns whether the user still
/// looks at it, and a difference published after the dismissal opens the
/// view again.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DifferenceView {
    /// The view shows the published difference.
    #[default]
    Open,
    /// The user closed the difference the owner had published.
    Closed { source_id: String },
}

impl DifferenceView {
    /// Whether the published difference is shown.
    pub fn shows(&self, source_id: &str) -> bool {
        match self {
            Self::Open => true,
            Self::Closed { source_id: closed } => closed != source_id,
        }
    }
}

/// Whether the synchronisation conflict resolver is open.
///
/// The owner owns the conflict; the surface owns whether the user still looks
/// at the resolver, and a conflict published after the dismissal opens the
/// resolver again.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ConflictResolver {
    /// The resolver shows the published conflict.
    #[default]
    Open,
    /// The user deferred the resolution of the published conflict.
    Deferred { conflict_time: String },
}

impl ConflictResolver {
    /// Whether the published conflict is shown.
    pub fn shows(&self, conflict_time: &str) -> bool {
        match self {
            Self::Open => true,
            Self::Deferred {
                conflict_time: closed,
            } => closed != conflict_time,
        }
    }
}

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
    /// The snapshot restoration review and its correlated readback.
    pub restore: Mutex<SnapshotRestoreWorkbench>,
    /// The subscription filter draft and its correlated write.
    pub filter: Mutex<SubscriptionFilterEditor>,
    /// The drafts of the panels that no shared reducer owns.
    pub drafts: Mutex<PanelDrafts>,
    /// Whether the configuration difference view is open.
    pub difference: Mutex<DifferenceView>,
    /// Whether the synchronisation conflict resolver is open.
    pub conflict: Mutex<ConflictResolver>,
    /// The owner commands the surface dispatched, in dispatch order.
    ///
    /// A command without a shared reducer carries no reducer identity, but it
    /// still waits for the owner's terminal, so the surface keeps it in the
    /// same queue.
    in_flight: Mutex<VecDeque<Option<PendingBusiness>>>,
}

impl Workbenches {
    /// Records the owner command the surface dispatched.
    ///
    /// The owner executes the commands in dispatch order, so its terminals
    /// arrive in the same order and every terminal reaches the reducer that
    /// started its command. A command no shared reducer owns carries no
    /// reducer identity, but the surface still waits for its terminal.
    pub fn record(&self, business: Option<PendingBusiness>) {
        let mut queue = guard(&self.in_flight);
        queue.push_back(business);
    }

    /// Applies the owner's terminal to the reducer that owns the command.
    ///
    /// The reducer keeps the correlation: it accepts a terminal only when the
    /// identity matches its own pending operation, so a terminal that belongs
    /// to another business never completes this one.
    pub fn settle(&self, result: Result<CommandOutput, Failure>) -> bool {
        let Some(business) = self.next_in_flight() else {
            return false;
        };
        let Some(business) = business else {
            // The command no shared reducer owns: the terminal arrived, so
            // the surface stops waiting for it.
            return true;
        };
        match business {
            PendingBusiness::DnsQuery => {
                let mut query = guard(&self.dns_query);
                let Some(operation) = query.requested_operation() else {
                    return false;
                };
                query.finish(operation, result.map(|_| ()))
            }
            PendingBusiness::DnsCache => {
                let mut cache = guard(&self.dns_cache);
                let Some(operation) = cache.requested_operation() else {
                    return false;
                };
                cache.finish(operation, result.map(|_| ()))
            }
            PendingBusiness::DnsHosts => {
                let mut hosts = guard(&self.dns_hosts);
                let Some(token) = hosts.pending().map(|pending| pending.token) else {
                    return false;
                };
                hosts.finish(token, result.map(|_| ()))
            }
            PendingBusiness::Doctor => {
                let mut doctor = guard(&self.doctor);
                let token = match &doctor.phase {
                    DoctorActionPhase::Pending(pending) => pending.token,
                    DoctorActionPhase::Idle | DoctorActionPhase::Failed { .. } => return false,
                };
                doctor.finish(token, result.map(|_| ()));
                true
            }
            PendingBusiness::Script => {
                let mut script = guard(&self.script);
                let Some(operation) = script.pending().map(|pending| pending.operation) else {
                    return false;
                };
                script.finish(operation, result);
                true
            }
            PendingBusiness::Restore => {
                let mut restore = guard(&self.restore);
                let Some(operation) = restore.pending().map(|pending| pending.operation) else {
                    return false;
                };
                restore.finish(operation, result);
                true
            }
            PendingBusiness::Filter => {
                let mut filter = guard(&self.filter);
                let Some(token) = filter.pending().map(|pending| pending.token) else {
                    return false;
                };
                match result {
                    Ok(CommandOutput::SubscriptionFilterApplied(applied)) => {
                        filter.finish(token, Ok(applied))
                    }
                    Ok(_) => false,
                    Err(failure) => filter.finish(token, Err(failure)),
                }
            }
        }
    }

    /// Shows the configuration difference the owner published.
    pub fn open_difference_view(&self) {
        let mut view = guard(&self.difference);
        *view = DifferenceView::Open;
    }

    /// Closes the configuration difference the owner published.
    pub fn close_difference_view(&self) {
        let mut view = guard(&self.difference);
        *view = DifferenceView::Closed {
            source_id: String::new(),
        };
    }

    /// Closes the configuration difference of one published source.
    pub fn close_difference_view_of(&self, source_id: &str) {
        let mut view = guard(&self.difference);
        *view = DifferenceView::Closed {
            source_id: source_id.to_owned(),
        };
    }

    /// Defers the resolution of the published synchronisation conflict.
    pub fn close_conflict_resolver(&self, conflict_time: &str) {
        let mut resolver = guard(&self.conflict);
        *resolver = ConflictResolver::Deferred {
            conflict_time: conflict_time.to_owned(),
        };
    }

    /// Whether the surface waits for an owner terminal.
    ///
    /// The surface owns the correlation, so it knows whether a command it
    /// dispatched is still outstanding. A capture waits for the owner's
    /// answer before it publishes the surface as activated.
    pub fn has_pending_command(&self) -> bool {
        !guard(&self.in_flight).is_empty()
    }

    /// The reducer that owns the next owner terminal.
    fn next_in_flight(&self) -> Option<Option<PendingBusiness>> {
        let mut queue = guard(&self.in_flight);
        queue.pop_front()
    }
}

/// Locks one shared reducer of the surface.
fn guard<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
