//! Projections for proxy, observation, traffic, and license data.

use super::{THIRD_PARTY_NOTICES, flag_assets, snapshot_language};
use crate::{
    AppWindow, HomeTrafficPoint, LicenseRow, ObservationRow, ProxyGroup, TrafficBar, slint_model,
};
use infiltrator_contract::active_exit::ActiveExitStatus;
use infiltrator_contract::logs::LogStreamState;
use infiltrator_contract::outbound::details::{
    OutboundCollectionKind, OutboundDetailsState, OutboundObservation,
};
use infiltrator_contract::outbound::identity::OutboundId;
use infiltrator_contract::outbound::inventory::{
    OutboundInventoryState, OutboundSelectionSet, OutboundSelectionState,
};
use infiltrator_contract::outbound::preferences::OutboundSortOrder;
use infiltrator_contract::surface_snapshot::{PageStatus, SurfaceSnapshot};
use infiltrator_shared::byte_units::format_bytes;
use infiltrator_shared::locales::{Lang, Localizer};
use std::cmp::Reverse;
use std::collections::HashMap;

pub(super) fn project_proxies(window: &AppWindow, snapshot: &SurfaceSnapshot) {
    let language = snapshot_language(snapshot);
    let lang = Lang(&language);
    if let Some(preferences) = snapshot.outbound_ui_preferences.data.as_ref() {
        window.set_proxy_search(preferences.search_query.clone().into());
        window.set_proxy_sort_mode(
            match preferences.sort_order {
                OutboundSortOrder::LatencyAscending => "latency_asc",
                OutboundSortOrder::LatencyDescending => "latency_desc",
                OutboundSortOrder::NameAscending => "name_asc",
                OutboundSortOrder::NameDescending => "name_desc",
            }
            .into(),
        );
    }
    let Some(inventory) = snapshot.outbound_inventory.data.as_ref() else {
        window.set_proxy_groups(slint_model(Vec::<ProxyGroup>::new()));
        window.set_proxy_nodes(slint_model(Vec::<ObservationRow>::new()));
        window.set_proxy_current_node_available(false);
        window.set_selected_group("".into());
        return;
    };
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = inventory.inventory()
    else {
        window.set_proxy_groups(slint_model(Vec::<ProxyGroup>::new()));
        window.set_proxy_nodes(slint_model(Vec::<ObservationRow>::new()));
        window.set_proxy_current_node_available(false);
        window.set_selected_group("".into());
        return;
    };
    let names = entries
        .iter()
        .map(|entry| {
            (
                entry.id.value().to_owned(),
                entry
                    .display_name
                    .clone()
                    .unwrap_or_else(|| entry.id.value().to_owned()),
            )
        })
        .collect::<HashMap<_, _>>();
    let details = snapshot
        .outbound_details
        .data
        .as_ref()
        .and_then(|details| match details.state() {
            OutboundDetailsState::Ready { details } => Some(details.as_slice()),
            OutboundDetailsState::Unknown
            | OutboundDetailsState::Empty
            | OutboundDetailsState::Unsupported
            | OutboundDetailsState::Failed { .. } => None,
        })
        .unwrap_or_default();
    let groups = collections
        .iter()
        .map(|collection| {
            let selected_id = match &collection.selection {
                OutboundSelectionState::Ready { selected } => Some(selected.value()),
                OutboundSelectionState::Unknown
                | OutboundSelectionState::Empty
                | OutboundSelectionState::Unsupported
                | OutboundSelectionState::Failed { .. } => None,
            };
            let current = selected_id
                .and_then(|id| names.get(id))
                .map(|name| name.clone())
                .unwrap_or_else(|| "".to_owned());
            let kind = details
                .iter()
                .find(|detail| detail.id == collection.id)
                .and_then(|detail| match &detail.collection_kind {
                    OutboundObservation::Observed(kind) => {
                        Some(collection_kind_label(*kind, &language))
                    }
                    OutboundObservation::Unknown | OutboundObservation::Unsupported => None,
                })
                .unwrap_or_else(|| lang.tr("策略组").into_owned());
            let member_names = collection
                .member_ids
                .iter()
                .filter_map(|id| names.get(id.value()))
                .cloned()
                .collect::<Vec<_>>();
            let active_country = if active_exit_matches(snapshot, &collection.id, selected_id) {
                snapshot
                    .active_exit
                    .country_code
                    .as_deref()
                    .unwrap_or_default()
            } else {
                ""
            };
            ProxyGroup {
                name: collection.id.value().to_owned().into(),
                icon_emoji: "".into(),
                kind: kind.into(),
                current: current.clone().into(),
                current_flag_code: active_country.to_ascii_lowercase().into(),
                current_flag_index: flag_assets::index_for_code(active_country).unwrap_or(-1),
                current_accessible: current.clone().into(),
                nodes: member_names.join("\n").into(),
            }
        })
        .collect::<Vec<_>>();
    let requested_group = window.get_selected_group().to_string();
    let visible_group = if !requested_group.is_empty()
        && collections
            .iter()
            .any(|group| group.id.value() == requested_group)
    {
        Some(requested_group.as_str())
    } else {
        if !requested_group.is_empty() {
            window.set_selected_group("".into());
        }
        selected_group_from_snapshot(snapshot, collections)
    };
    let query = window.get_proxy_search().to_string().trim().to_lowercase();
    let sort_mode = window.get_proxy_sort_mode().to_string();
    let nodes = visible_group
        .and_then(|group_name| {
            collections
                .iter()
                .find(|group| group.id.value() == group_name)
        })
        .map(|group| {
            let selected_id = match &group.selection {
                OutboundSelectionState::Ready { selected } => Some(selected.value()),
                _ => None,
            };
            let mut rows = group
                .member_ids
                .iter()
                .map(|id| {
                    let detail = details.iter().find(|detail| detail.id == *id);
                    let name = names
                        .get(id.value())
                        .cloned()
                        .unwrap_or_else(|| id.value().to_owned());
                    let delay = detail.and_then(|detail| match &detail.current_latency_ms {
                        OutboundObservation::Observed(delay) => Some(*delay),
                        OutboundObservation::Unknown | OutboundObservation::Unsupported => None,
                    });
                    let country = if active_exit_matches(snapshot, &group.id, Some(id.value())) {
                        snapshot
                            .active_exit
                            .country_code
                            .as_deref()
                            .unwrap_or_default()
                    } else {
                        ""
                    };
                    ObservationRow {
                        id: id.value().to_owned().into(),
                        title: name.clone().into(),
                        accessible_title: name.clone().into(),
                        flag_code: country.to_ascii_lowercase().into(),
                        flag_index: flag_assets::index_for_code(country).unwrap_or(-1),
                        delay_ms: delay
                            .map(|value| value.min(i32::MAX as u32) as i32)
                            .unwrap_or(-1),
                        subtitle: detail
                            .and_then(|detail| detail.protocol_display_name.clone())
                            .unwrap_or_else(|| "".to_owned())
                            .into(),
                        trailing: delay
                            .map(|value| format!("{value} ms"))
                            .unwrap_or_else(|| "—".to_owned())
                            .into(),
                        active: selected_id == Some(id.value()),
                    }
                })
                .filter(|row| {
                    query.is_empty()
                        || row.title.to_string().to_lowercase().contains(&query)
                        || row.id.to_string().to_lowercase().contains(&query)
                        || row.subtitle.to_string().to_lowercase().contains(&query)
                })
                .collect::<Vec<_>>();
            match sort_mode.as_str() {
                "latency_asc" => rows.sort_by_key(|row| {
                    if row.delay_ms < 0 {
                        i32::MAX
                    } else {
                        row.delay_ms
                    }
                }),
                "latency_desc" => rows.sort_by_key(|row| {
                    if row.delay_ms < 0 {
                        i32::MAX
                    } else {
                        -row.delay_ms
                    }
                }),
                "name_asc" => rows.sort_by(|left, right| {
                    left.title
                        .to_string()
                        .to_lowercase()
                        .cmp(&right.title.to_string().to_lowercase())
                }),
                "name_desc" => rows.sort_by(|left, right| {
                    right
                        .title
                        .to_string()
                        .to_lowercase()
                        .cmp(&left.title.to_string().to_lowercase())
                }),
                _ => {}
            }
            rows
        })
        .unwrap_or_default();
    let has_nodes = !nodes.is_empty();
    window.set_proxy_groups(slint_model(groups));
    window.set_proxy_nodes(slint_model(nodes));
    window.set_proxy_current_node_available(has_nodes);
}

fn active_exit_matches(
    snapshot: &SurfaceSnapshot,
    group: &OutboundId,
    member_id: Option<&str>,
) -> bool {
    let Some(member_id) = member_id else {
        return false;
    };
    matches!(
        &snapshot.active_exit.status,
        ActiveExitStatus::Ready {
            group_id,
            outbound_id,
        } if group_id == group && outbound_id.value() == member_id
    )
}

pub(super) fn project_connections(window: &AppWindow, snapshot: &SurfaceSnapshot) {
    let query = window
        .get_connection_search()
        .to_string()
        .trim()
        .to_lowercase();
    let sort_mode = window.get_connection_sort().to_string();
    let rows = snapshot
        .pages
        .connections
        .data
        .as_ref()
        .map(|page| {
            let mut connections = page.connections.iter().collect::<Vec<_>>();
            match sort_mode.as_str() {
                "upload" => connections.sort_by_key(|connection| Reverse(connection.upload_total)),
                "download" => {
                    connections.sort_by_key(|connection| Reverse(connection.download_total))
                }
                "host" => connections.sort_by(|left, right| left.host.cmp(&right.host)),
                "time" => connections.sort_by(|left, right| right.start.cmp(&left.start)),
                _ => {}
            }
            connections
                .into_iter()
                .map(|connection| ObservationRow {
                    id: connection.id.clone().into(),
                    title: connection.host.clone().into(),
                    accessible_title: connection.host.clone().into(),
                    flag_code: "".into(),
                    flag_index: -1,
                    delay_ms: -1,
                    subtitle: format!("{} · {}", connection.network, connection.process).into(),
                    trailing: format!(
                        "↑ {} · ↓ {}",
                        format_bytes(connection.upload_total),
                        format_bytes(connection.download_total)
                    )
                    .into(),
                    active: true,
                })
                .filter(|row| {
                    query.is_empty()
                        || row.title.to_string().to_lowercase().contains(&query)
                        || row.subtitle.to_string().to_lowercase().contains(&query)
                        || row.id.to_string().to_lowercase().contains(&query)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    window.set_connections(slint_model(rows));
}

pub(super) fn project_rules_and_providers(window: &AppWindow, snapshot: &SurfaceSnapshot) {
    let language = snapshot_language(snapshot);
    let lang = Lang(&language);
    let query = window.get_rule_search().to_string().trim().to_lowercase();
    let (rules, providers) = snapshot
        .pages
        .rules
        .data
        .as_ref()
        .map(|page| {
            let rules = page
                .rules
                .iter()
                .filter(|rule| {
                    query.is_empty()
                        || rule.raw.to_lowercase().contains(&query)
                        || rule.proxy.to_lowercase().contains(&query)
                        || rule.rule_type.to_lowercase().contains(&query)
                })
                .map(|rule| ObservationRow {
                    id: rule.id.to_string().into(),
                    title: rule.raw.clone().into(),
                    accessible_title: rule.raw.clone().into(),
                    flag_code: "".into(),
                    flag_index: -1,
                    delay_ms: -1,
                    subtitle: format!("{} → {}", rule.rule_type, rule.proxy).into(),
                    trailing: rule
                        .hit_count
                        .map(|count| count.to_string())
                        .unwrap_or_default()
                        .into(),
                    active: rule.is_enabled,
                })
                .collect::<Vec<_>>();
            let providers = page
                .providers
                .iter()
                .map(|provider| ObservationRow {
                    id: provider.name.clone().into(),
                    title: provider.name.clone().into(),
                    accessible_title: provider.name.clone().into(),
                    flag_code: "".into(),
                    flag_index: -1,
                    delay_ms: -1,
                    subtitle: provider.behavior.clone().into(),
                    trailing: format!("{}{}", provider.rule_count, lang.tr("条规则")).into(),
                    active: false,
                })
                .collect::<Vec<_>>();
            (rules, providers)
        })
        .unwrap_or_default();
    window.set_rules(slint_model(rules));
    window.set_providers(slint_model(providers));
}

pub(super) fn project_logs(window: &AppWindow, snapshot: &SurfaceSnapshot) {
    let language = snapshot_language(snapshot);
    let lang = Lang(&language);
    let query = window.get_log_search().to_string().trim().to_lowercase();
    let selected_level = window.get_log_level().to_string();
    let rows = snapshot
        .pages
        .logs
        .data
        .as_ref()
        .map(|page| {
            page.entries
                .iter()
                .filter(|entry| {
                    let level_match = selected_level == "all"
                        || log_level_key(&selected_level) == log_level_key(&entry.level);
                    let text = format!("{} {} {}", entry.message, entry.tag, entry.level);
                    level_match && (query.is_empty() || text.to_lowercase().contains(&query))
                })
                .map(|entry| ObservationRow {
                    id: entry.id.to_string().into(),
                    title: entry.message.clone().into(),
                    accessible_title: entry.message.clone().into(),
                    flag_code: "".into(),
                    flag_index: -1,
                    delay_ms: -1,
                    subtitle: format!("{} · {}", entry.timestamp, entry.level).into(),
                    trailing: entry.tag.clone().into(),
                    active: false,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let stream_status = log_stream_status(
        snapshot.pages.logs.data.as_ref().map(|page| &page.stream),
        &lang,
    );
    let owner_level = snapshot
        .pages
        .logs
        .data
        .as_ref()
        .and_then(|page| page.active_level.as_deref())
        .map(log_level_key)
        .unwrap_or("all");
    window.set_log_level(owner_level.into());
    window.set_logs(slint_model(rows));
    window.set_log_stream_status(stream_status.into());
}

fn log_stream_status(stream: Option<&LogStreamState>, lang: &Lang<'_>) -> String {
    match stream {
        Some(LogStreamState::Live) => lang.tr("已连接").into_owned(),
        Some(LogStreamState::Reconnecting(_)) => lang.tr("正在重连").into_owned(),
        Some(LogStreamState::Connecting) => lang.tr("正在连接").into_owned(),
        Some(LogStreamState::Failed(_)) => lang.tr("读取失败").into_owned(),
        Some(LogStreamState::Unsupported(_)) => lang.tr("日志流不支持").into_owned(),
        Some(LogStreamState::Cancelled) => lang.tr("日志流已取消").into_owned(),
        Some(LogStreamState::Idle) | None => lang.tr("未运行").into_owned(),
    }
}

fn log_level_key(value: &str) -> &str {
    match value.to_ascii_lowercase().as_str() {
        "dbg" | "debug" => "debug",
        "inf" | "info" => "info",
        "warn" | "warning" | "wrn" => "warning",
        "err" | "error" | "fatal" => "error",
        _ => value,
    }
}

#[cfg(test)]
mod tests {
    use super::log_stream_status;
    use infiltrator_contract::error::Failure;
    use infiltrator_contract::logs::LogStreamState;
    use infiltrator_shared::locales::Lang;

    #[test]
    fn unsupported_and_cancelled_log_streams_project_localized_terminal_copy() {
        let unsupported = LogStreamState::Unsupported(Failure::unsupported("not available"));
        assert_eq!(
            log_stream_status(Some(&unsupported), &Lang("zh-CN")),
            "日志流不支持"
        );
        assert_eq!(
            log_stream_status(Some(&unsupported), &Lang("en-US")),
            "Log stream is unsupported"
        );

        assert_eq!(
            log_stream_status(Some(&LogStreamState::Cancelled), &Lang("zh-CN")),
            "日志流已取消"
        );
        assert_eq!(
            log_stream_status(Some(&LogStreamState::Cancelled), &Lang("en-US")),
            "Log stream was cancelled"
        );
    }
}

pub(super) fn project_traffic(window: &AppWindow, snapshot: &SurfaceSnapshot) {
    let language = snapshot_language(snapshot);
    let lang = Lang(&language);
    let values = snapshot
        .traffic_waveform
        .samples
        .iter()
        .map(|sample| (sample.download_bps.max(0.0), sample.upload_bps.max(0.0)))
        .collect::<Vec<_>>();
    let peak = values
        .iter()
        .flat_map(|(download, upload)| [*download, *upload])
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let points = values
        .iter()
        .map(|(download, upload)| HomeTrafficPoint {
            download_fraction: (download / peak).clamp(0.0, 1.0) as f32,
            upload_fraction: (upload / peak).clamp(0.0, 1.0) as f32,
        })
        .collect::<Vec<_>>();
    let samples = values
        .iter()
        .enumerate()
        .map(|(index, (download, upload))| ObservationRow {
            id: index.to_string().into(),
            title: format!("{download:.0} bps").into(),
            accessible_title: format!("{download:.0} bps").into(),
            flag_code: "".into(),
            flag_index: -1,
            delay_ms: -1,
            subtitle: format!("{} / {}", lang.tr("下载"), lang.tr("上传")).into(),
            trailing: format!("{upload:.0} bps").into(),
            active: false,
        })
        .collect::<Vec<_>>();
    window.set_home_traffic_points(slint_model(points));
    window.set_home_traffic_has_data(snapshot.traffic_waveform.is_drawable());
    window.set_traffic_samples(slint_model(samples));
    window.set_traffic_today_bars(slint_model(Vec::<TrafficBar>::new()));
    window.set_traffic_week_bars(slint_model(Vec::<TrafficBar>::new()));
    window.set_traffic_today_has_data(false);
    window.set_traffic_week_has_data(false);
    window.set_traffic_today_down("—".into());
    window.set_traffic_today_up("—".into());
    window.set_traffic_week_down("—".into());
    window.set_traffic_week_up("—".into());
    window.set_traffic_week_average_down("—".into());
    window.set_traffic_week_average_up("—".into());
}

pub(super) fn license_rows(query: &str, language: &str) -> Vec<LicenseRow> {
    let lang = Lang(language);
    let query = query.trim().to_lowercase();
    let name = lang.tr("第三方依赖许可证").into_owned();
    let identifier = lang.tr("随应用离线提供").into_owned();
    if !query.is_empty()
        && !name.to_lowercase().contains(&query)
        && !identifier.to_lowercase().contains(&query)
        && !THIRD_PARTY_NOTICES.to_lowercase().contains(&query)
    {
        return Vec::new();
    }
    vec![LicenseRow {
        id: "product.third-party-notices".into(),
        name: name.into(),
        version: env!("CARGO_PKG_VERSION").into(),
        identifier: identifier.into(),
    }]
}

pub(super) fn project_license_catalog(window: &AppWindow, language: &str) {
    window.set_license_catalog_error("".into());
    window.set_license_rows(slint_model(license_rows(
        window.get_license_query().as_str(),
        language,
    )));
    window.set_license_rows_revision(window.get_license_rows_revision() + 1);
}

pub(crate) fn project_connection_detail(
    window: &AppWindow,
    snapshot: &SurfaceSnapshot,
    connection_id: &str,
) {
    if connection_id.is_empty() {
        return;
    }
    let connection = snapshot.pages.connections.data.as_ref().and_then(|page| {
        page.connections
            .iter()
            .find(|connection| connection.id == connection_id)
    });
    let Some(connection) = connection else {
        window.set_connection_detail_host("".into());
        window.set_connection_detail_active(false);
        return;
    };
    let source = endpoint(&connection.source_ip, &connection.source_port);
    let destination = if connection.destination_host.is_empty() {
        endpoint(&connection.destination_ip, &connection.destination_port)
    } else {
        endpoint(&connection.destination_host, &connection.destination_port)
    };
    let rule = if connection.rule_payload.is_empty() {
        connection.rule.clone()
    } else {
        connection.rule_payload.clone()
    };
    let chains = if connection.chains.is_empty() {
        connection.chain.clone()
    } else {
        connection.chains.join(" → ")
    };
    window.set_connection_detail_host(connection.host.clone().into());
    window.set_connection_detail_id(connection.id.clone().into());
    window.set_connection_detail_protocol(connection.network.clone().into());
    window.set_connection_detail_process(connection.process.clone().into());
    window.set_connection_detail_source(source.into());
    window.set_connection_detail_destination(destination.into());
    window.set_connection_detail_rule(if rule.is_empty() { "—" } else { &rule }.into());
    window.set_connection_detail_chains(if chains.is_empty() { "—" } else { &chains }.into());
    window.set_connection_detail_providers("—".into());
    window.set_connection_detail_start(
        if connection.start.is_empty() {
            "—"
        } else {
            &connection.start
        }
        .into(),
    );
    window.set_connection_detail_duration("—".into());
    window.set_connection_detail_upload(format_bytes(connection.upload_total).into());
    window.set_connection_detail_download(format_bytes(connection.download_total).into());
    window.set_connection_detail_active(true);
}

fn endpoint(host: &str, port: &str) -> String {
    match (host.trim(), port.trim()) {
        ("", _) => "—".to_owned(),
        (host, "") => host.to_owned(),
        (host, port) => format!("{host}:{port}"),
    }
}

pub(super) fn page_failure(window: &AppWindow, snapshot: &SurfaceSnapshot) -> Option<String> {
    let page = window.get_page().to_string();
    let status = match page.as_str() {
        "Config" | "ProfileSettings" => &snapshot.pages.profiles.status,
        "Proxy" => &snapshot.outbound_inventory.status,
        "Connections" | "ConnectionDetail" => &snapshot.pages.connections.status,
        "Rules" | "Providers" => &snapshot.pages.rules.status,
        "Logs" => &snapshot.pages.logs.status,
        "Traffic" => &snapshot.pages.connections.status,
        "Settings" => &snapshot.pages.settings.status,
        "Overrides" => {
            let language = snapshot_language(snapshot);
            return Some(
                Lang(&language)
                    .tr("命名 Override 尚未接入共享配置 owner")
                    .into_owned(),
            );
        }
        _ => return None,
    };
    status_failure(status)
}

fn status_failure(status: &PageStatus) -> Option<String> {
    match status {
        PageStatus::Unavailable { failure } | PageStatus::Failed { failure } => {
            Some(failure.message.clone())
        }
        PageStatus::Loading | PageStatus::Ready | PageStatus::Empty => None,
    }
}

pub(super) fn split_rate(rate: &str) -> (String, String) {
    let trimmed = rate.trim().trim_end_matches("/s").trim();
    let mut parts = trimmed.split_whitespace();
    let value = parts.next().unwrap_or("0").to_owned();
    let unit = parts.next().unwrap_or("B").to_owned();
    (value, unit)
}

fn collection_kind_label(kind: OutboundCollectionKind, language: &str) -> String {
    let lang = Lang(language);
    match kind {
        OutboundCollectionKind::ManualSelection => lang.tr("手动选择").into_owned(),
        OutboundCollectionKind::AutomaticSelection => lang.tr("自动选择").into_owned(),
        OutboundCollectionKind::Failover => lang.tr("故障转移").into_owned(),
        OutboundCollectionKind::LoadBalancing => lang.tr("负载均衡").into_owned(),
        OutboundCollectionKind::Relay => lang.tr("链式中继").into_owned(),
    }
}

fn selected_group_from_snapshot<'a>(
    snapshot: &'a SurfaceSnapshot,
    collections: &'a [OutboundSelectionSet],
) -> Option<&'a str> {
    snapshot
        .active_exit
        .group
        .as_deref()
        .or_else(|| collections.first().map(|group| group.id.value()))
}
