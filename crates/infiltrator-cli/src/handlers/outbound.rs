use crate::commands::OutboundAction;
use crate::context::{OutboundContext, Runtime};
use crate::output::{print_info, print_success, print_table};
use infiltrator_application::core_capability_application::{
    CoreCapabilityGateFailure, CoreCapabilityRequirement, CoreCommandEffect,
};
use infiltrator_contract::core_capabilities::{CoreOperation, OutboundOperation};
use lurker_core_contract::outbound::{
    details::OutboundSnapshot,
    identity::OutboundId,
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundProbeState,
        OutboundSelectionSet, OutboundSelectionState,
    },
    probe::{OutboundSelectionRequest, ProbeOutboundRequest},
};

pub(crate) async fn handle(action: OutboundAction) -> anyhow::Result<()> {
    let runtime = Runtime::detect().await?;
    let context = runtime.outbound_context().await?;
    match action {
        OutboundAction::List => {
            authorize(
                &context,
                CoreOperation::Outbound(OutboundOperation::List),
                CoreCommandEffect::Read,
            )?;
            let snapshot = inventory(&context).await?;
            match snapshot.inventory() {
                OutboundInventoryState::Ready {
                    entries,
                    collections,
                } => {
                    let rows = inventory_rows(entries, collections);
                    print_table(&["Name", "ID", "Selected in"], &rows);
                }
                OutboundInventoryState::Empty => print_info("No outbounds found"),
                OutboundInventoryState::Unknown => {
                    anyhow::bail!("outbound inventory has not been observed")
                }
                OutboundInventoryState::Unsupported => {
                    anyhow::bail!("outbound inventory is unsupported by the active backend")
                }
                OutboundInventoryState::Failed { failure } => {
                    anyhow::bail!("{}", failure.message)
                }
            }
        }
        OutboundAction::Select {
            collection_id,
            outbound_id,
        } => {
            authorize(
                &context,
                CoreOperation::Outbound(OutboundOperation::Select),
                CoreCommandEffect::Write,
            )?;
            let observed = inventory(&context).await?;
            require_ready_inventory(&observed)?;
            let collection_id =
                OutboundId::new(context.scope.identity.engine.clone(), collection_id)
                    .map_err(anyhow::Error::msg)?;
            let outbound_id = OutboundId::new(context.scope.identity.engine.clone(), outbound_id)
                .map_err(anyhow::Error::msg)?;
            let request = OutboundSelectionRequest::new(
                collection_id.clone(),
                outbound_id.clone(),
                observed.revision(),
            )
            .map_err(anyhow::Error::msg)?;
            context
                .application
                .select_request(&context.scope, &request)
                .await
                .map_err(|failure| anyhow::anyhow!(failure.message))?;
            print_success(&format!(
                "Outbound '{}' selected in collection '{}'",
                outbound_id.value(),
                collection_id.value()
            ));
        }
        OutboundAction::Probe {
            outbound_id,
            url,
            timeout_ms,
        } => {
            authorize(
                &context,
                CoreOperation::Outbound(OutboundOperation::Probe),
                CoreCommandEffect::Write,
            )?;
            let observed = inventory(&context).await?;
            require_ready_inventory(&observed)?;
            let outbound_id = OutboundId::new(context.scope.identity.engine.clone(), outbound_id)
                .map_err(anyhow::Error::msg)?;
            let request = ProbeOutboundRequest::new(
                outbound_id.clone(),
                observed.revision(),
                url,
                timeout_ms,
            )
            .map_err(anyhow::Error::msg)?;
            let result = context
                .application
                .probe_request(&context.scope, &request)
                .await
                .map_err(|failure| anyhow::anyhow!(failure.message))?;
            match result.state() {
                OutboundProbeState::Measured { delay_ms } => {
                    print_success(&format!("{}: {delay_ms} ms", result.outbound_id().value()))
                }
                OutboundProbeState::TimedOut => {
                    print_info(&format!("{}: timed out", result.outbound_id().value()))
                }
                OutboundProbeState::Unsupported => {
                    anyhow::bail!("outbound probe is unsupported by the active backend")
                }
                OutboundProbeState::Failed { failure } => {
                    anyhow::bail!("{}", failure.message)
                }
                OutboundProbeState::Unknown | OutboundProbeState::Probing => {
                    anyhow::bail!("outbound probe did not reach a terminal state")
                }
            }
        }
    }
    Ok(())
}

async fn inventory(context: &OutboundContext) -> anyhow::Result<OutboundInventorySnapshot> {
    context
        .application
        .inventory(context.scope.clone())
        .await
        .map_err(|failure| anyhow::anyhow!(failure.message))
}

fn require_ready_inventory(snapshot: &OutboundInventorySnapshot) -> anyhow::Result<()> {
    match snapshot.inventory() {
        OutboundInventoryState::Ready { .. } => Ok(()),
        OutboundInventoryState::Empty => anyhow::bail!("outbound inventory is empty"),
        OutboundInventoryState::Unknown => {
            anyhow::bail!("outbound inventory has not been observed")
        }
        OutboundInventoryState::Unsupported => {
            anyhow::bail!("outbound inventory is unsupported by the active backend")
        }
        OutboundInventoryState::Failed { failure } => anyhow::bail!("{}", failure.message),
    }
}

fn authorize(
    context: &OutboundContext,
    operation: CoreOperation,
    effect: CoreCommandEffect,
) -> anyhow::Result<()> {
    context
        .capabilities
        .authorize(
            Some(&context.scope),
            CoreCapabilityRequirement::new(operation, effect),
        )
        .map(|_| ())
        .map_err(|failure: CoreCapabilityGateFailure| {
            anyhow::anyhow!(failure.into_failure().message)
        })
}

fn inventory_rows(
    entries: &[OutboundSnapshot],
    collections: &[OutboundSelectionSet],
) -> Vec<Vec<String>> {
    entries
        .iter()
        .map(|entry| {
            let selected_in = collections
                .iter()
                .filter_map(|collection| match &collection.selection {
                    OutboundSelectionState::Ready { selected } if selected == &entry.id => {
                        Some(collection.id.value())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(", ");
            vec![
                entry
                    .display_name
                    .as_deref()
                    .unwrap_or_else(|| entry.id.value())
                    .to_string(),
                entry.id.value().to_string(),
                selected_in,
            ]
        })
        .collect()
}
