//! Read-only real-controller smoke; callers own and clean up the isolated kernel process.
use anyhow::{Context, Result, ensure};
use infiltrator_domain::proxy::Proxy;
use mihomo_api::client::MihomoClient;
use serde_json::json;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    let endpoint = env::args()
        .nth(1)
        .context("an explicit isolated controller URL is required")?;
    let client = MihomoClient::new(&endpoint, None)?;
    let version = client.get_version().await?;
    let proxies = client.get_proxies().await?;
    ensure!(
        !proxies.is_empty(),
        "controller supplied no proxy observations"
    );
    for (identity, proxy) in &proxies {
        ensure!(
            identity == proxy.name(),
            "controller returned a mismatched identity"
        );
        let single = client.get_proxy(identity).await?;
        ensure!(
            &single == proxy,
            "list and single-node observations disagree for {identity}"
        );
    }
    println!(
        "{}",
        json!({
            "version":version.version,
            "proxy_count":proxies.len(),
            "group_count":proxies.values().filter(|proxy|proxy.is_group()).count(),
            "partial_observation_count":proxies.values().filter(|proxy|matches!(proxy,Proxy::Observed(_))).count(),
            "list_and_node_endpoints":"verified"
        })
    );
    Ok(())
}
