use super::*;
use crate::api::proxy::{ProxyHistory, ProxyObservation};

pub(super) fn map_inventory(
    proxies: &HashMap<String, ProxyObservation>,
) -> Result<OutboundInventoryState, Failure> {
    if proxies.is_empty() {
        return Ok(OutboundInventoryState::Empty);
    }
    let mut ordered = proxies.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.0.cmp(right.0));
    let mut names = BTreeSet::new();
    for (key, proxy) in &ordered {
        let name = proxy.name();
        if name.trim().is_empty() || *key != name {
            return Err(invalid_wire("Mihomo returned an invalid outbound identity"));
        }
        names.insert(name.to_owned());
    }
    for (_, proxy) in &ordered {
        if group_observation(proxy)
            .and_then(|group| group.members)
            .is_some_and(|members| members.iter().any(|member| !names.contains(member)))
        {
            return Err(invalid_wire(format!(
                "group {} references a member absent from /proxies",
                proxy.name()
            )));
        }
    }

    let engine = EngineId::new("mihomo").map_err(|error| invalid_wire(error.to_string()))?;
    let entries = names
        .iter()
        .map(|name| {
            Ok(OutboundSnapshot {
                id: OutboundId::new(engine.clone(), name.clone())
                    .map_err(|error| invalid_wire(error.to_string()))?,
                display_name: None,
            })
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    let collections = ordered
        .iter()
        .filter_map(|(_, proxy)| group_observation(proxy).map(|group| (proxy, group)))
        .map(|(proxy, group)| {
            let GroupObservation {
                kind,
                members,
                selected,
            } = group;
            if members.is_some_and(|members| {
                members.iter().collect::<BTreeSet<_>>().len() != members.len()
            }) {
                return Err(invalid_wire(
                    "Mihomo group contains duplicate outbound members",
                ));
            }
            let member_ids = members
                .unwrap_or_default()
                .iter()
                .map(|member| {
                    OutboundId::new(engine.clone(), member.clone())
                        .map_err(|error| invalid_wire(error.to_string()))
                })
                .collect::<Result<Vec<_>, Failure>>()?;
            let availability = match (kind, member_ids.is_empty()) {
                (SelectionAvailability::Selectable, true) => SelectionAvailability::Unknown,
                _ => kind,
            };
            let selection = selection_observation(
                selected,
                &member_ids,
                members.is_some(),
                availability,
                &engine,
            );
            Ok(OutboundSelectionSet {
                id: OutboundId::new(engine.clone(), proxy.name().to_owned())
                    .map_err(|error| invalid_wire(error.to_string()))?,
                member_ids,
                availability,
                selection,
            })
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    Ok(OutboundInventoryState::Ready {
        entries,
        collections,
    })
}

pub(super) fn map_details(
    proxies: &HashMap<String, ProxyObservation>,
) -> Result<OutboundDetailsState, Failure> {
    if proxies.is_empty() {
        return Ok(OutboundDetailsState::Empty);
    }
    let engine = EngineId::new("mihomo").map_err(|error| invalid_wire(error.to_string()))?;
    let mut ordered = proxies.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.0.cmp(right.0));
    let details = ordered
        .into_iter()
        .map(|(key, proxy)| {
            if key != proxy.name() || proxy.name().trim().is_empty() {
                return Err(invalid_wire(
                    "Mihomo returned an invalid outbound identity while reading details",
                ));
            }
            let id = OutboundId::new(engine.clone(), key.clone())
                .map_err(|error| invalid_wire(error.to_string()))?;
            let protocol_name = proxy.proxy_type();
            let (protocol, protocol_display_name) =
                if protocol_name.trim().is_empty() || protocol_name == "Unknown" {
                    (OutboundProtocolObservation::Unknown, None)
                } else {
                    let protocol_id = OutboundProtocolId::new(engine.clone(), protocol_name)
                        .map_err(|error| invalid_wire(error.to_string()))?;
                    (
                        OutboundProtocolObservation::Reported(protocol_id),
                        Some(protocol_name.to_owned()),
                    )
                };
            let collection_kind = collection_kind(proxy);
            let (cipher, cipher_display_name) = cipher_observation(proxy, &engine)?;
            let endpoint = endpoint(proxy);
            let udp = match proxy.udp_observation() {
                Some(true) => OutboundUdpSupport::Supported,
                Some(false) => OutboundUdpSupport::Unsupported,
                None => OutboundUdpSupport::Unknown,
            };
            let health = match proxy.health_observation() {
                Some(true) => OutboundHealthObservation::Healthy,
                Some(false) => OutboundHealthObservation::Unhealthy,
                None => OutboundHealthObservation::Unknown,
            };
            let features = match udp {
                OutboundUdpSupport::Supported => {
                    OutboundObservation::Observed(vec![OutboundFeature::Udp])
                }
                OutboundUdpSupport::Unsupported => OutboundObservation::Observed(Vec::new()),
                OutboundUdpSupport::Unknown => OutboundObservation::Unknown,
            };
            let history = latency_history(proxy);
            let (latency_history, rtt) = match history {
                Some(samples) => {
                    let reported = samples
                        .iter()
                        .map(|sample| OutboundLatencySample {
                            observed_at: sample.time.clone(),
                            delay_ms: sample.delay,
                        })
                        .collect::<Vec<_>>();
                    let statistics = rtt_statistics(samples);
                    (
                        OutboundObservation::Observed(reported),
                        OutboundObservation::Observed(statistics),
                    )
                }
                None => (OutboundObservation::Unknown, OutboundObservation::Unknown),
            };
            let current_latency_ms = proxy
                .delay()
                .map(OutboundObservation::Observed)
                .unwrap_or(OutboundObservation::Unknown);
            let probe_availability = probe_availability(proxy);
            Ok(OutboundDetail {
                id,
                protocol,
                protocol_display_name,
                cipher,
                cipher_display_name,
                collection_kind,
                features,
                endpoint,
                udp,
                health,
                current_latency_ms,
                latency_history,
                rtt,
                historical_egress: OutboundObservation::Unsupported,
                probe_availability,
            })
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    Ok(OutboundDetailsState::Ready { details })
}

fn cipher_observation(
    proxy: &ProxyObservation,
    engine: &EngineId,
) -> Result<(OutboundObservation<OutboundCipherId>, Option<String>), Failure> {
    let cipher = proxy.cipher.as_deref();
    if let Some(cipher) = cipher {
        if cipher.trim().is_empty() {
            return Err(invalid_wire("Mihomo reported an empty cipher value"));
        }
        let id = OutboundCipherId::new(engine.clone(), cipher)
            .map_err(|error| invalid_wire(error.to_string()))?;
        return Ok((OutboundObservation::Observed(id), Some(cipher.to_owned())));
    }
    let state = match proxy.proxy_type() {
        "Shadowsocks" | "Vmess" => OutboundObservation::Unknown,
        "Unknown" => OutboundObservation::Unknown,
        "Trojan" | "Hysteria2" | "WireGuard" | "Tuic" | "Vless" | "Http" | "Socks5" | "Snell"
        | "Direct" | "Reject" | "Selector" | "URLTest" | "Fallback" | "LoadBalance" | "Relay" => {
            OutboundObservation::Unsupported
        }
        _ => OutboundObservation::Unknown,
    };
    Ok((state, None))
}

fn collection_kind(proxy: &ProxyObservation) -> OutboundObservation<OutboundCollectionKind> {
    let Some(_) = group_observation(proxy) else {
        return OutboundObservation::Unsupported;
    };
    let kind = match proxy.proxy_type() {
        "Selector" => OutboundCollectionKind::ManualSelection,
        "URLTest" => OutboundCollectionKind::AutomaticSelection,
        "Fallback" => OutboundCollectionKind::Failover,
        "LoadBalance" => OutboundCollectionKind::LoadBalancing,
        "Relay" => OutboundCollectionKind::Relay,
        _ => return OutboundObservation::Unknown,
    };
    OutboundObservation::Observed(kind)
}

fn endpoint(proxy: &ProxyObservation) -> OutboundObservation<OutboundEndpoint> {
    let Some((host, port)) = endpoint_parts(proxy) else {
        return OutboundObservation::Unknown;
    };
    if host.trim().is_empty() {
        return OutboundObservation::Unknown;
    }
    OutboundObservation::Observed(OutboundEndpoint {
        host: host.to_owned(),
        port: port
            .map(OutboundObservation::Observed)
            .unwrap_or(OutboundObservation::Unknown),
    })
}

fn endpoint_parts(proxy: &ProxyObservation) -> Option<(&str, Option<u16>)> {
    proxy.server.as_deref().map(|host| (host, proxy.port))
}

fn latency_history(proxy: &ProxyObservation) -> Option<&[ProxyHistory]> {
    proxy.history.as_deref()
}

fn rtt_statistics(history: &[ProxyHistory]) -> OutboundRttStatistics {
    let valid = history
        .iter()
        .map(|sample| sample.delay)
        .filter(|delay| *delay > 0)
        .collect::<Vec<_>>();
    if valid.is_empty() {
        return OutboundRttStatistics::default();
    }
    let sum = valid.iter().map(|delay| u64::from(*delay)).sum::<u64>();
    OutboundRttStatistics {
        min_ms: valid.iter().copied().min(),
        max_ms: valid.iter().copied().max(),
        avg_ms: Some((sum / valid.len() as u64) as u32),
        valid_count: valid.len(),
    }
}

fn probe_availability(proxy: &ProxyObservation) -> OutboundProbeAvailability {
    if group_observation(proxy).is_some() {
        return OutboundProbeAvailability::Unsupported;
    }
    match proxy.proxy_type() {
        "Shadowsocks" | "Vmess" | "Trojan" | "Hysteria2" | "WireGuard" | "Tuic" | "Vless"
        | "Http" | "Socks5" | "Snell" | "Direct" | "Reject" => OutboundProbeAvailability::Supported,
        _ => OutboundProbeAvailability::Unknown,
    }
}

struct GroupObservation<'a> {
    kind: SelectionAvailability,
    members: Option<&'a [String]>,
    selected: Option<&'a str>,
}

fn group_observation(proxy: &ProxyObservation) -> Option<GroupObservation<'_>> {
    let kind = match proxy.proxy_type() {
        "Selector" => SelectionAvailability::Selectable,
        "URLTest" | "Fallback" | "LoadBalance" => SelectionAvailability::Automatic,
        "Relay" => SelectionAvailability::ReadOnly,
        _ => SelectionAvailability::Unknown,
    };
    match proxy.all() {
        Some(members) => Some(GroupObservation {
            kind,
            members: Some(members),
            selected: proxy.now(),
        }),
        None if matches!(
            proxy.proxy_type(),
            "Selector" | "URLTest" | "Fallback" | "LoadBalance" | "Relay"
        ) =>
        {
            Some(GroupObservation {
                kind,
                members: None,
                selected: proxy.now(),
            })
        }
        None => None,
    }
}

fn selection_observation(
    current: Option<&str>,
    members: &[OutboundId],
    member_list_observed: bool,
    availability: SelectionAvailability,
    engine: &EngineId,
) -> OutboundSelectionState {
    if !matches!(
        availability,
        SelectionAvailability::Selectable | SelectionAvailability::Automatic
    ) {
        return OutboundSelectionState::Unknown;
    }
    match current {
        None => OutboundSelectionState::Unknown,
        Some("") => OutboundSelectionState::Empty,
        Some(name) if member_list_observed => {
            let selected = OutboundId::new(engine.clone(), name);
            match selected {
                Ok(selected) if members.contains(&selected) => {
                    OutboundSelectionState::Ready { selected }
                }
                _ => OutboundSelectionState::Unknown,
            }
        }
        Some(_) => OutboundSelectionState::Unknown,
    }
}
