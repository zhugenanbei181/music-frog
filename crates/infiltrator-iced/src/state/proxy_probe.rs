//! Tracked inspector probes preserve failures and never reopen a dismissed panel.
use crate::state::AppState;
use crate::types::app;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_capabilities::{
    CapabilityAccess, CoreCapabilityStatus, CoreOperation, OutboundOperation,
};
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::outbound::identity::OutboundId;
use infiltrator_contract::outbound::{
    details::{
        OutboundDetailsSnapshot, OutboundDetailsState, OutboundObservation,
        OutboundProbeAvailability,
    },
    identity::SnapshotRevision,
    inventory::{OutboundInventorySnapshot, OutboundInventoryState, OutboundProbeSnapshot},
    probe::{
        OutboundProbeBatchScope, OutboundProbeBatchSnapshot, OutboundProbeConcurrency,
        ProbeOutboundBatchRequest, ProbeOutboundRequest,
    },
};
use infiltrator_contract::surface_snapshot::PageStatus;
use std::collections::HashSet;

/// Iced's readback state for a typed outbound probe batch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutboundProbeBatchState {
    Idle,
    Running {
        token: u64,
        scope: CoreScope,
        request: ProbeOutboundBatchRequest,
        expected_outbound_ids: Vec<OutboundId>,
    },
    Ready {
        scope: CoreScope,
        request: ProbeOutboundBatchRequest,
        snapshot: OutboundProbeBatchSnapshot,
    },
    Failed {
        scope: CoreScope,
        request: ProbeOutboundBatchRequest,
        failure: Failure,
    },
}

impl OutboundProbeBatchState {
    pub const fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }
}

/// The action requires evidence from both the selected outbound and the active
/// Core capability snapshot. Missing evidence remains visible and fail closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutboundProbeActionAvailability {
    Available,
    DetailsUnavailable,
    ItemUnsupported,
    ItemUnknown,
    CoreUnsupported,
    CoreUnknown,
    InventoryUnavailable,
    ProbeSettingsUnavailable,
}

impl OutboundProbeActionAvailability {
    pub(crate) fn status_text(self) -> Option<&'static str> {
        match self {
            Self::Available => None,
            Self::DetailsUnavailable => {
                Some("Probe availability is unknown because details are unavailable.")
            }
            Self::ItemUnsupported => Some("Probing is unsupported for this outbound."),
            Self::ItemUnknown => Some("Probe availability is unknown for this outbound."),
            Self::CoreUnsupported => Some("The active Core does not support outbound probing."),
            Self::CoreUnknown => Some("Outbound probe support is unknown for the active Core."),
            Self::InventoryUnavailable => {
                Some("Probe is unavailable without a current outbound inventory.")
            }
            Self::ProbeSettingsUnavailable => Some("Saved probe settings are unavailable."),
        }
    }
}

impl AppState {
    pub(crate) fn outbound_probe_action_availability(
        &self,
        outbound_id: &OutboundId,
    ) -> OutboundProbeActionAvailability {
        let Some(ingress) = self
            .command_ingress()
            .filter(|_| self.is_product_command_ingress())
        else {
            return OutboundProbeActionAvailability::CoreUnknown;
        };
        let Some(scope) = ingress.observed_scope() else {
            return OutboundProbeActionAvailability::CoreUnknown;
        };
        let Some(_inventory) = self
            .runtime
            .outbound_inventory
            .data
            .as_ref()
            .filter(|inventory| inventory.scope() == &scope)
        else {
            return OutboundProbeActionAvailability::InventoryUnavailable;
        };
        let Some(detail) = self.outbound_detail(outbound_id) else {
            return OutboundProbeActionAvailability::DetailsUnavailable;
        };
        match detail.probe_availability {
            OutboundProbeAvailability::Unsupported => {
                return OutboundProbeActionAvailability::ItemUnsupported;
            }
            OutboundProbeAvailability::Unknown => {
                return OutboundProbeActionAvailability::ItemUnknown;
            }
            OutboundProbeAvailability::Supported => {}
        }
        match self.core_outbound_probe_capability(&scope) {
            CoreProbeCapability::Supported => {}
            CoreProbeCapability::Unsupported => {
                return OutboundProbeActionAvailability::CoreUnsupported;
            }
            CoreProbeCapability::Unknown => {
                return OutboundProbeActionAvailability::CoreUnknown;
            }
        }
        if self.applied_runtime_probe_settings().is_err() {
            return OutboundProbeActionAvailability::ProbeSettingsUnavailable;
        }
        OutboundProbeActionAvailability::Available
    }

    pub(crate) fn refresh_outbound_probe_block(&mut self) {
        if self.runtime.outbound_probe_blocked.is_none() {
            return;
        }
        let Some(outbound_id) = self.runtime.outbound_inspecting.clone() else {
            self.runtime.outbound_probe_blocked = None;
            return;
        };
        let availability = self.outbound_probe_action_availability(&outbound_id);
        self.runtime.outbound_probe_blocked =
            (availability != OutboundProbeActionAvailability::Available).then_some(availability);
    }

    fn core_outbound_probe_capability(&self, scope: &CoreScope) -> CoreProbeCapability {
        let Some(snapshot) = self.surface.latest() else {
            return CoreProbeCapability::Unknown;
        };
        if snapshot.core.scope().as_ref() != Some(scope)
            || !matches!(&snapshot.core_capability_snapshot.status, PageStatus::Ready)
        {
            return CoreProbeCapability::Unknown;
        }
        let Some(capabilities) = snapshot.core_capability_snapshot.data.as_ref() else {
            return CoreProbeCapability::Unknown;
        };
        if capabilities.scope.engine_id != scope.identity.engine
            || capabilities.scope.core_version != scope.identity.version
            || capabilities.scope.host != snapshot.capabilities.host
        {
            return CoreProbeCapability::Unknown;
        }
        let Some(resolution) =
            capabilities.resolution(CoreOperation::Outbound(OutboundOperation::Probe))
        else {
            return CoreProbeCapability::Unknown;
        };
        if resolution.key.scope != capabilities.scope {
            return CoreProbeCapability::Unknown;
        }
        match &resolution.status {
            CoreCapabilityStatus::Available {
                access: CapabilityAccess::Supported,
                ..
            } => CoreProbeCapability::Supported,
            CoreCapabilityStatus::Unsupported { .. }
            | CoreCapabilityStatus::Available {
                access: CapabilityAccess::ReadOnly,
                ..
            } => CoreProbeCapability::Unsupported,
            CoreCapabilityStatus::VersionUnavailable { .. }
            | CoreCapabilityStatus::HostUnavailable { .. }
            | CoreCapabilityStatus::AdapterMissing { .. }
            | CoreCapabilityStatus::Unregistered
            | CoreCapabilityStatus::ScopeMismatch => CoreProbeCapability::Unknown,
        }
    }

    pub(crate) fn outbound_probe_message(&self, outbound_id: &OutboundId) -> Option<Message> {
        let ingress = self
            .command_ingress()
            .filter(|_| self.is_product_command_ingress())?;
        let scope = ingress.observed_scope()?;
        let inventory = self
            .runtime
            .outbound_inventory
            .data
            .as_ref()
            .filter(|inventory| inventory.scope() == &scope)?;
        if self.outbound_probe_action_availability(outbound_id)
            != OutboundProbeActionAvailability::Available
        {
            return None;
        }
        let options = self.applied_runtime_probe_settings().ok()?;
        let request = ProbeOutboundRequest::new(
            outbound_id.clone(),
            inventory.revision(),
            options.url().to_owned(),
            u64::from(options.timeout_ms()),
        )
        .ok()?;
        Some(Message::StartOutboundProbe { scope, request })
    }

    pub(crate) fn start_outbound_probe(
        &mut self,
        scope: CoreScope,
        request: ProbeOutboundRequest,
    ) -> Task<Message> {
        if self.runtime.outbound_probe_pending.is_some()
            || !self.is_product_command_ingress()
            || self
                .command_ingress()
                .and_then(|ingress| ingress.observed_scope())
                .as_ref()
                != Some(&scope)
            || request.validate_scope(&scope).is_err()
        {
            return Task::none();
        }
        let current_inventory = self
            .runtime
            .outbound_inventory
            .data
            .as_ref()
            .filter(|inventory| inventory.scope() == &scope)
            .filter(|inventory| inventory.revision() == request.expected_inventory_revision());
        let Some(current_inventory) = current_inventory else {
            return Task::none();
        };
        let _ = current_inventory;
        let action_availability = self.outbound_probe_action_availability(request.outbound_id());
        if action_availability != OutboundProbeActionAvailability::Available {
            self.runtime.outbound_probe_blocked = Some(action_availability);
            return Task::none();
        }
        self.runtime.outbound_probe_blocked = None;
        self.runtime.outbound_inspecting = Some(request.outbound_id().clone());
        self.runtime.outbound_probe_token = self.runtime.outbound_probe_token.wrapping_add(1);
        let token = self.runtime.outbound_probe_token;
        self.runtime.outbound_probe_pending = Some(token);
        self.runtime.outbound_probe_failure = None;
        let Some(ingress) = self.command_ingress() else {
            self.runtime.outbound_probe_pending = None;
            self.runtime.outbound_probe_failure = Some(Failure::new(
                ErrorCode::NotReady,
                "outbound probe command is not composed",
                true,
            ));
            return Task::none();
        };
        let command_request = request.clone();
        Task::perform(
            async move {
                let outcome = ingress
                    .execute_outcome(CommandIntent::ProbeOutbound {
                        request: command_request,
                    })
                    .await;
                let terminal_scope = outcome.scope;
                let result = outcome
                    .result
                    .into_output()
                    .and_then(|output| match output {
                        CommandOutput::OutboundProbe(snapshot) => Ok(snapshot),
                        _ => Err(Failure::new(
                            ErrorCode::InvalidState,
                            "outbound probe command returned another output type",
                            false,
                        )),
                    });
                (terminal_scope, result)
            },
            move |(terminal_scope, result)| Message::OutboundProbeFinished {
                token,
                scope,
                request,
                terminal_scope,
                result,
            },
        )
    }

    pub(crate) fn probe_inspected_outbound(&mut self) -> Task<Message> {
        if self.runtime.outbound_probe_pending.is_some() {
            return Task::none();
        }
        let Some(outbound_id) = self.runtime.outbound_inspecting.clone() else {
            return Task::none();
        };
        let Some(Message::StartOutboundProbe { scope, request }) =
            self.outbound_probe_message(&outbound_id)
        else {
            let availability = self.outbound_probe_action_availability(&outbound_id);
            self.runtime.outbound_probe_blocked = (availability
                != OutboundProbeActionAvailability::Available)
                .then_some(availability);
            return Task::none();
        };
        self.start_outbound_probe(scope, request)
    }

    pub(crate) fn finish_outbound_probe(
        &mut self,
        token: u64,
        scope: CoreScope,
        request: ProbeOutboundRequest,
        terminal_scope: Option<CoreScope>,
        result: Result<OutboundProbeSnapshot, Failure>,
    ) -> Task<Message> {
        if self.runtime.outbound_probe_pending != Some(token) {
            return Task::none();
        }
        self.runtime.outbound_probe_pending = None;
        self.runtime.outbound_probe_blocked = None;
        let current_scope = self
            .command_ingress()
            .and_then(|ingress| ingress.observed_scope());
        let current_revision = self
            .runtime
            .outbound_inventory
            .data
            .as_ref()
            .filter(|inventory| inventory.scope() == &scope)
            .map(|inventory| inventory.revision());
        if current_scope.as_ref() != Some(&scope)
            || terminal_scope.as_ref() != Some(&scope)
            || self.runtime.outbound_inspecting.as_ref() != Some(request.outbound_id())
            || current_revision != Some(request.expected_inventory_revision())
        {
            return Task::none();
        }
        match result {
            Ok(snapshot) if snapshot.scope() == &scope && request.is_confirmed_by(&snapshot) => {
                self.runtime.outbound_probe_result = Some(snapshot);
                self.runtime.outbound_probe_failure = None;
            }
            Ok(_) => {
                self.runtime.outbound_probe_failure = Some(Failure::new(
                    ErrorCode::InvalidState,
                    "outbound probe readback did not match its request",
                    false,
                ));
            }
            Err(failure) => self.runtime.outbound_probe_failure = Some(failure),
        }
        Task::none()
    }
}

impl AppState {
    pub(crate) fn outbound_probe_batch_message(
        &self,
        target_scope: OutboundProbeBatchScope,
    ) -> Option<Message> {
        let ingress = self
            .command_ingress()
            .filter(|_| self.is_product_command_ingress())?;
        let scope = ingress.observed_scope()?;
        target_scope.validate_scope(&scope).ok()?;
        let inventory_page = &self.runtime.outbound_inventory;
        if !matches!(&inventory_page.status, PageStatus::Ready) {
            return None;
        }
        let inventory = inventory_page
            .data
            .as_ref()
            .filter(|inventory| inventory.scope() == &scope)?;
        match (inventory.inventory(), &target_scope) {
            (OutboundInventoryState::Ready { .. }, OutboundProbeBatchScope::All) => {}
            (
                OutboundInventoryState::Ready { collections, .. },
                OutboundProbeBatchScope::Collection(id),
            ) => {
                collections.iter().find(|collection| &collection.id == id)?;
            }
            (
                OutboundInventoryState::Ready {
                    entries,
                    collections,
                },
                OutboundProbeBatchScope::Selected(ids),
            ) => {
                let unique = ids.iter().collect::<HashSet<_>>();
                if ids.is_empty()
                    || unique.len() != ids.len()
                    || ids.iter().any(|id| {
                        !entries.iter().any(|entry| &entry.id == id)
                            || collections.iter().any(|collection| &collection.id == id)
                    })
                {
                    return None;
                }
            }
            (OutboundInventoryState::Empty, OutboundProbeBatchScope::All) => {}
            _ => return None,
        }
        Some(Message::StartOutboundProbeBatch {
            scope,
            target_scope,
            expected_inventory_revision: inventory.revision(),
        })
    }

    pub(crate) fn start_outbound_probe_batch(
        &mut self,
        scope: CoreScope,
        target_scope: OutboundProbeBatchScope,
        expected_inventory_revision: SnapshotRevision,
    ) -> Task<Message> {
        if self.runtime.outbound_probe_batch.is_running() {
            return Task::none();
        }
        let Some(ingress) = self
            .command_ingress()
            .filter(|_| self.is_product_command_ingress())
        else {
            return Task::done(Message::ShowToast(
                "Outbound probing requires an accepted Core owner".into(),
                app::ToastStatus::Error,
            ));
        };
        if ingress.observed_scope().as_ref() != Some(&scope)
            || target_scope.validate_scope(&scope).is_err()
        {
            return Task::none();
        }
        let inventory = self
            .runtime
            .outbound_inventory
            .data
            .as_ref()
            .filter(|inventory| inventory.scope() == &scope)
            .cloned();
        let Some(inventory) = inventory else {
            return Task::done(Message::ShowToast(
                "Outbound inventory is unavailable for this Core scope".into(),
                app::ToastStatus::Info,
            ));
        };
        if !matches!(&self.runtime.outbound_inventory.status, PageStatus::Ready)
            || inventory.revision() != expected_inventory_revision
        {
            return Task::done(Message::ShowToast(
                "Outbound inventory changed; refresh the page before probing".into(),
                app::ToastStatus::Info,
            ));
        }
        let details = self
            .runtime
            .outbound_details
            .data
            .as_ref()
            .filter(|details| details.scope() == &scope)
            .filter(|_| matches!(&self.runtime.outbound_details.status, PageStatus::Ready))
            .cloned();
        let Some(details) = details else {
            return Task::done(Message::ShowToast(
                "Outbound details are unavailable for batch probing".into(),
                app::ToastStatus::Info,
            ));
        };
        match self.core_outbound_probe_capability(&scope) {
            CoreProbeCapability::Supported => {}
            CoreProbeCapability::Unsupported => {
                return Task::done(Message::ShowToast(
                    "The active Core does not support outbound probing".into(),
                    app::ToastStatus::Warning,
                ));
            }
            CoreProbeCapability::Unknown => {
                return Task::done(Message::ShowToast(
                    "Outbound probe support is unknown for the active Core".into(),
                    app::ToastStatus::Info,
                ));
            }
        }
        let options = match self.applied_runtime_probe_settings() {
            Ok(options) => options,
            Err(failure) => {
                return Task::done(Message::ShowToast(failure.message, app::ToastStatus::Error));
            }
        };
        let expected_outbound_ids =
            match outbound_probe_batch_member_ids(&inventory, &details, &target_scope) {
                Ok(ids) => ids,
                Err(failure) => {
                    return Task::done(Message::ShowToast(
                        failure.message,
                        app::ToastStatus::Error,
                    ));
                }
            };
        let concurrency = OutboundProbeConcurrency::new(self.diag.speedtest.concurrency.get())
            .expect("typed speedtest concurrency matches the outbound batch bound");
        let request = match ProbeOutboundBatchRequest::new(
            target_scope,
            expected_inventory_revision,
            options.url().to_owned(),
            u64::from(options.timeout_ms()),
            concurrency,
        ) {
            Ok(request) => request,
            Err(error) => {
                return Task::done(Message::ShowToast(
                    error.to_string(),
                    app::ToastStatus::Error,
                ));
            }
        };
        self.runtime.outbound_probe_batch_token =
            self.runtime.outbound_probe_batch_token.wrapping_add(1);
        let token = self.runtime.outbound_probe_batch_token;
        self.runtime.outbound_probe_batch = OutboundProbeBatchState::Running {
            token,
            scope: scope.clone(),
            request: request.clone(),
            expected_outbound_ids,
        };
        let command_request = request.clone();
        Task::perform(
            async move {
                let outcome = ingress
                    .execute_outcome(CommandIntent::ProbeOutboundBatch {
                        request: command_request,
                    })
                    .await;
                let terminal_scope = outcome.scope;
                let result = outcome
                    .result
                    .into_output()
                    .and_then(|output| match output {
                        CommandOutput::OutboundProbeBatch(snapshot) => Ok(snapshot),
                        _ => Err(Failure::new(
                            ErrorCode::InvalidState,
                            "outbound batch probe returned another output type",
                            false,
                        )),
                    });
                (terminal_scope, result)
            },
            move |(terminal_scope, result)| Message::OutboundProbeBatchFinished {
                token,
                scope,
                request,
                terminal_scope,
                result,
            },
        )
    }

    pub(crate) fn finish_outbound_probe_batch(
        &mut self,
        token: u64,
        scope: CoreScope,
        request: ProbeOutboundBatchRequest,
        terminal_scope: Option<CoreScope>,
        result: Result<OutboundProbeBatchSnapshot, Failure>,
    ) -> Task<Message> {
        let expected_outbound_ids = match &self.runtime.outbound_probe_batch {
            OutboundProbeBatchState::Running {
                token: pending_token,
                scope: pending_scope,
                request: pending_request,
                expected_outbound_ids,
            } if *pending_token == token
                && pending_scope == &scope
                && pending_request == &request =>
            {
                expected_outbound_ids.clone()
            }
            _ => return Task::none(),
        };
        let active_scope = self
            .command_ingress()
            .and_then(|ingress| ingress.observed_scope());
        if active_scope.as_ref() != Some(&scope) {
            self.dismiss_outbound_probe_batch();
            return Task::none();
        }
        let inventory = self
            .runtime
            .outbound_inventory
            .data
            .as_ref()
            .filter(|inventory| inventory.scope() == &scope);
        if terminal_scope.as_ref() != Some(&scope)
            || !inventory.is_some_and(|inventory| {
                inventory.revision() == request.expected_inventory_revision()
            })
        {
            self.runtime.outbound_probe_batch = OutboundProbeBatchState::Failed {
                scope,
                request,
                failure: Failure::new(
                    ErrorCode::NotReady,
                    "outbound batch probe result is stale for the accepted scope or inventory revision",
                    true,
                ),
            };
            return Task::none();
        }
        let details = self
            .runtime
            .outbound_details
            .data
            .as_ref()
            .filter(|details| details.scope() == &scope);
        let expected_now = inventory
            .zip(details)
            .filter(|_| {
                matches!(&self.runtime.outbound_inventory.status, PageStatus::Ready)
                    && matches!(&self.runtime.outbound_details.status, PageStatus::Ready)
            })
            .and_then(|(inventory, details)| {
                outbound_probe_batch_member_ids(inventory, details, request.target_scope()).ok()
            });
        let detail_revision = details.map(OutboundDetailsSnapshot::detail_revision);
        if expected_now.as_ref() != Some(&expected_outbound_ids) {
            self.runtime.outbound_probe_batch = OutboundProbeBatchState::Failed {
                scope,
                request,
                failure: Failure::new(
                    ErrorCode::NotReady,
                    "outbound batch membership changed before result readback",
                    true,
                ),
            };
            return Task::none();
        }
        match result {
            Ok(snapshot)
                if snapshot.scope() == &scope
                    && snapshot.request() == &request
                    && snapshot.is_confirmed_by(&request)
                    && detail_revision
                        .is_some_and(|revision| snapshot.detail_revision() >= revision)
                    && snapshot
                        .items()
                        .iter()
                        .map(|item| item.outbound_id.clone())
                        .collect::<Vec<_>>()
                        == expected_outbound_ids =>
            {
                self.runtime.outbound_probe_batch = OutboundProbeBatchState::Ready {
                    scope,
                    request,
                    snapshot,
                };
            }
            Ok(_) => {
                self.runtime.outbound_probe_batch = OutboundProbeBatchState::Failed {
                    scope,
                    request,
                    failure: Failure::new(
                        ErrorCode::InvalidState,
                        "outbound batch probe readback did not match its request and members",
                        false,
                    ),
                };
            }
            Err(failure) => {
                self.runtime.outbound_probe_batch = OutboundProbeBatchState::Failed {
                    scope,
                    request,
                    failure,
                };
            }
        }
        Task::none()
    }

    pub(crate) fn dismiss_outbound_probe_batch(&mut self) {
        self.runtime.outbound_probe_batch_token =
            self.runtime.outbound_probe_batch_token.wrapping_add(1);
        self.runtime.outbound_probe_batch = OutboundProbeBatchState::Idle;
    }
}

fn outbound_probe_batch_member_ids(
    inventory: &OutboundInventorySnapshot,
    details: &OutboundDetailsSnapshot,
    target_scope: &OutboundProbeBatchScope,
) -> Result<Vec<OutboundId>, Failure> {
    let (entries, collections) = match inventory.inventory() {
        OutboundInventoryState::Ready {
            entries,
            collections,
        } => (entries, collections),
        OutboundInventoryState::Empty if matches!(target_scope, OutboundProbeBatchScope::All) => {
            return Ok(Vec::new());
        }
        _ => {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "outbound inventory is not ready for batch probing",
                true,
            ));
        }
    };
    let detail_list = match details.state() {
        OutboundDetailsState::Ready { details } => details,
        _ => {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "outbound details are not ready for batch probing",
                true,
            ));
        }
    };
    let mut outbound_ids = match target_scope {
        OutboundProbeBatchScope::One(id) => vec![id.clone()],
        OutboundProbeBatchScope::Collection(id) => collections
            .iter()
            .find(|collection| &collection.id == id)
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::InvalidInput,
                    "outbound probe collection is absent from the accepted inventory",
                    false,
                )
            })?
            .member_ids
            .clone(),
        OutboundProbeBatchScope::Selected(ids) => {
            if ids
                .iter()
                .any(|id| collections.iter().any(|collection| &collection.id == id))
            {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    "a selected outbound probe target is a collection",
                    false,
                ));
            }
            ids.clone()
        }
        OutboundProbeBatchScope::All => entries.iter().map(|entry| entry.id.clone()).collect(),
    };
    for outbound_id in &outbound_ids {
        if !entries.iter().any(|entry| &entry.id == outbound_id)
            || !detail_list.iter().any(|detail| &detail.id == outbound_id)
        {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "outbound batch contains a member without accepted details",
                true,
            ));
        }
    }
    if matches!(
        target_scope,
        OutboundProbeBatchScope::Collection(_) | OutboundProbeBatchScope::All
    ) {
        outbound_ids.retain(|outbound_id| {
            !collections
                .iter()
                .any(|collection| &collection.id == outbound_id)
                && detail_list
                    .iter()
                    .find(|detail| &detail.id == outbound_id)
                    .is_some_and(|detail| {
                        !matches!(detail.collection_kind, OutboundObservation::Observed(_))
                    })
        });
    }
    Ok(outbound_ids)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoreProbeCapability {
    Supported,
    Unsupported,
    Unknown,
}
