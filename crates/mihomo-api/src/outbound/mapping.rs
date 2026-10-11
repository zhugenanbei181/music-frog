use super::*;

pub(super) fn map_inventory(
    proxies: &HashMap<String, Proxy>,
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
    proxies: &HashMap<String, Proxy>,
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
            let (protocol, protocol_display_name) = if protocol_name.trim().is_empty()
                || protocol_name == "Unknown"
                || matches!(proxy, Proxy::Unknown)
            {
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
    proxy: &Proxy,
    engine: &EngineId,
) -> Result<(OutboundObservation<OutboundCipherId>, Option<String>), Failure> {
    let cipher = match proxy {
        Proxy::Observed(observation) => observation.cipher.as_deref(),
        Proxy::Shadowsocks(value) => Some(value.cipher.as_str()),
        Proxy::Vmess(value) => Some(value.cipher.as_str()),
        _ => None,
    };
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

fn collection_kind(proxy: &Proxy) -> OutboundObservation<OutboundCollectionKind> {
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

fn endpoint(proxy: &Proxy) -> OutboundObservation<OutboundEndpoint> {
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

fn endpoint_parts(proxy: &Proxy) -> Option<(&str, Option<u16>)> {
    match proxy {
        Proxy::Observed(observation) => observation
            .server
            .as_deref()
            .map(|host| (host, observation.port)),
        Proxy::Shadowsocks(value) => Some((&value.server, Some(value.port))),
        Proxy::Vmess(value) => Some((&value.server, Some(value.port))),
        Proxy::Trojan(value) => Some((&value.server, Some(value.port))),
        Proxy::Hysteria2(value) => Some((&value.server, Some(value.port))),
        Proxy::WireGuard(value) => Some((&value.server, Some(value.port))),
        Proxy::Tuic(value) => Some((&value.server, Some(value.port))),
        Proxy::Vless(value) => Some((&value.server, Some(value.port))),
        Proxy::Http(value) => Some((&value.server, Some(value.port))),
        Proxy::Socks5(value) => Some((&value.server, Some(value.port))),
        Proxy::Snell(value) => Some((&value.server, Some(value.port))),
        Proxy::Direct(_)
        | Proxy::Reject(_)
        | Proxy::Selector(_)
        | Proxy::URLTest(_)
        | Proxy::Fallback(_)
        | Proxy::LoadBalance(_)
        | Proxy::Relay(_)
        | Proxy::Unknown => None,
    }
}

fn latency_history(proxy: &Proxy) -> Option<&[ProxyHistory]> {
    match proxy {
        Proxy::Observed(observation) => observation.history.as_deref(),
        Proxy::Shadowsocks(value) => Some(&value.base.history),
        Proxy::Vmess(value) => Some(&value.base.history),
        Proxy::Trojan(value) => Some(&value.base.history),
        Proxy::Hysteria2(value) => Some(&value.base.history),
        Proxy::WireGuard(value) => Some(&value.base.history),
        Proxy::Tuic(value) => Some(&value.base.history),
        Proxy::Vless(value) => Some(&value.base.history),
        Proxy::Http(value) => Some(&value.base.history),
        Proxy::Socks5(value) => Some(&value.base.history),
        Proxy::Snell(value) => Some(&value.base.history),
        Proxy::Direct(value) => Some(&value.base.history),
        Proxy::Reject(value) => Some(&value.base.history),
        Proxy::Selector(value)
        | Proxy::URLTest(value)
        | Proxy::Fallback(value)
        | Proxy::LoadBalance(value)
        | Proxy::Relay(value) => Some(&value.history),
        Proxy::Unknown => None,
    }
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

fn probe_availability(proxy: &Proxy) -> OutboundProbeAvailability {
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

fn group_observation(proxy: &Proxy) -> Option<GroupObservation<'_>> {
    match proxy {
        Proxy::Selector(group) => Some(GroupObservation {
            kind: SelectionAvailability::Selectable,
            members: Some(&group.all),
            selected: Some(&group.now),
        }),
        Proxy::URLTest(group) => Some(GroupObservation {
            kind: SelectionAvailability::Automatic,
            members: Some(&group.all),
            selected: Some(&group.now),
        }),
        Proxy::Fallback(group) => Some(GroupObservation {
            kind: SelectionAvailability::Automatic,
            members: Some(&group.all),
            selected: Some(&group.now),
        }),
        Proxy::LoadBalance(group) => Some(GroupObservation {
            kind: SelectionAvailability::Automatic,
            members: Some(&group.all),
            selected: Some(&group.now),
        }),
        Proxy::Relay(group) => Some(GroupObservation {
            kind: SelectionAvailability::ReadOnly,
            members: Some(&group.all),
            selected: Some(&group.now),
        }),
        Proxy::Observed(observation) => {
            let kind = match observation.proxy_type.as_str() {
                "Selector" => SelectionAvailability::Selectable,
                "URLTest" => SelectionAvailability::Automatic,
                "Fallback" => SelectionAvailability::Automatic,
                "LoadBalance" => SelectionAvailability::Automatic,
                "Relay" => SelectionAvailability::ReadOnly,
                _ => SelectionAvailability::Unknown,
            };
            observation.all.as_ref().map_or_else(
                || {
                    matches!(
                        observation.proxy_type.as_str(),
                        "Selector" | "URLTest" | "Fallback" | "LoadBalance" | "Relay"
                    )
                    .then_some(GroupObservation {
                        kind,
                        members: None,
                        selected: observation.now.as_deref(),
                    })
                },
                |members| {
                    Some(GroupObservation {
                        kind,
                        members: Some(members.as_slice()),
                        selected: observation.now.as_deref(),
                    })
                },
            )
        }
        Proxy::Shadowsocks(_)
        | Proxy::Vmess(_)
        | Proxy::Trojan(_)
        | Proxy::Hysteria2(_)
        | Proxy::WireGuard(_)
        | Proxy::Tuic(_)
        | Proxy::Vless(_)
        | Proxy::Http(_)
        | Proxy::Socks5(_)
        | Proxy::Snell(_)
        | Proxy::Direct(_)
        | Proxy::Reject(_)
        | Proxy::Unknown => None,
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
