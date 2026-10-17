//! Read-only real-controller smoke; callers own and clean up the isolated kernel process.
use anyhow::{Context, Result, bail, ensure};
use infiltrator_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use lurker_core_contract::outbound::inventory::OutboundInventoryState;
use lurker_core_ports::outbound::OutboundPort;
use mihomo_api::client::MihomoClient;
use mihomo_api::outbound::MihomoOutboundAdapter;
use serde_json::json;
use std::env;
use std::sync::Arc;

struct FixedScope(CoreScope);

impl CoreScopeSource for FixedScope {
    fn scope_observation(&self) -> CoreScopeObservation {
        CoreScopeObservation::Observed(self.0.clone())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let endpoint = env::args()
        .nth(1)
        .context("an explicit isolated controller URL is required")?;
    let client = MihomoClient::new(&endpoint, None)?;
    let version = client.get_version().await?;
    let scope = CoreScope {
        identity: CoreIdentity::new(
            EngineId::mihomo(),
            CoreInstanceId::new("controller-proxy-smoke")?,
            None,
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    };
    let adapter = MihomoOutboundAdapter::new(Arc::new(client), Arc::new(FixedScope(scope.clone())));
    let observation = OutboundPort::observation(&adapter, scope.clone()).await?;
    ensure!(
        observation.inventory().scope() == &scope,
        "controller returned an outbound observation for a different owner scope"
    );
    let (outbound_count, selection_set_count) = match observation.inventory().inventory() {
        OutboundInventoryState::Ready {
            entries,
            collections,
        } => (entries.len(), collections.len()),
        OutboundInventoryState::Empty => (0, 0),
        OutboundInventoryState::Failed { failure } => bail!(
            "controller outbound observation failed: {}",
            failure.message
        ),
        OutboundInventoryState::Unknown => bail!("controller outbound observation is unknown"),
        OutboundInventoryState::Unsupported => {
            bail!("controller outbound inventory is unsupported")
        }
    };
    ensure!(
        outbound_count > 0,
        "controller supplied no proxy observations"
    );
    println!(
        "{}",
        json!({
            "version":version.version,
            "outbound_count":outbound_count,
            "selection_set_count":selection_set_count,
            "scope_generation":scope.generation.value(),
        })
    );
    Ok(())
}
