//! Small shared projections and failure helpers for the surface reader.
//!
//! These are pure functions over already-read facts, split out of the
//! reader module so the page assemblers and the port adapters can share
//! them without the parent file owning every helper.

use super::*;

pub(super) fn missing(what: &str) -> Failure {
    Failure::new(
        ErrorCode::Unsupported,
        format!("{what} is not composed for this host"),
        false,
    )
}

pub(super) fn merge_applied_mtu(
    mut snapshot: infiltrator_contract::mtu::MtuNegotiationSnapshot,
    runtime_config: Option<&Result<infiltrator_domain::runtime::ConfigSnapshot, PortError>>,
) -> infiltrator_contract::mtu::MtuNegotiationSnapshot {
    if let Some(Ok(config)) = runtime_config
        && let Some(mtu) = config.tun.as_ref().and_then(|tun| tun.mtu)
    {
        snapshot.applied_tun_mtu = Some(mtu);
    }
    snapshot
}

pub(super) fn page_from_result<T, U, E>(
    result: Option<Result<T, E>>,
    map: impl FnOnce(T) -> U,
    what: &str,
) -> surface_snapshot::PageData<U>
where
    E: std::fmt::Display,
{
    match result {
        Some(Ok(value)) => surface_snapshot::PageData::ready(map(value)),
        Some(Err(error)) => surface_snapshot::PageData::failed(Failure::new(
            ErrorCode::Network,
            format!("{what}: {error}"),
            true,
        )),
        None => surface_snapshot::PageData::unavailable(missing(what)),
    }
}

pub(super) fn proxy_groups(
    proxies: &HashMap<String, Proxy>,
) -> Vec<surface_snapshot::ProxyGroupSnapshot> {
    let mut groups = proxies
        .iter()
        .filter_map(|(name, proxy)| {
            let members = proxy.all()?;
            let current = proxy.now().unwrap_or_default().to_owned();
            Some(surface_snapshot::ProxyGroupSnapshot {
                name: name.clone(),
                group_type: proxy.proxy_type().to_owned(),
                classification: None,
                current: current.clone(),
                expanded: true,
                proxies: members
                    .iter()
                    .filter_map(|member| {
                        let proxy = proxies.get(member)?;
                        Some(surface_snapshot::ProxyNodeSnapshot {
                            name: member.clone(),
                            node_type: proxy.proxy_type().to_owned(),
                            delay_ms: proxy.delay(),
                            selected: member == &current,
                            favorite: false,
                            features: proxy.udp().then(|| "UDP".to_owned()).into_iter().collect(),
                        })
                    })
                    .collect(),
            })
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| left.name.cmp(&right.name));
    groups
}

pub(super) fn active_exit(proxies: &HashMap<String, Proxy>) -> String {
    proxies
        .iter()
        .find(|(name, proxy)| name.eq_ignore_ascii_case("proxies") && proxy.is_group())
        .and_then(|(_, proxy)| proxy.now())
        .or_else(|| {
            proxies
                .values()
                .find(|proxy| proxy.is_group())
                .and_then(Proxy::now)
        })
        .unwrap_or("—")
        .to_owned()
}
