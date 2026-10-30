//! Projections for proxy, observation, traffic, and license data.
//!
//! The proxies page, the logs page and the connections page are rendered
//! from the shared page models and live in their own modules beside this one.

mod connections;
mod logs;
mod proxies;

use super::{THIRD_PARTY_NOTICES, snapshot_language};
use crate::{AppWindow, HomeTrafficPoint, LicenseRow, ObservationRow, TrafficBar, slint_model};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_domain::connection_view::ConnectionView;
use infiltrator_domain::rules::view::RuleView;
use infiltrator_presentation::rules_page_model::RulesPageModel;
use infiltrator_shared::byte_units::format_bytes;
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::page::PageStatus;

/// Projects the proxies page from the shared page model.
///
/// The query fold, the match rule, the member order, the health filter and
/// the group order are the shared reduction; the surface only renders what
/// the model reports.
pub(super) fn project_proxies(window: &AppWindow, snapshot: &SurfaceSnapshot) {
    proxies::project_proxies(window, snapshot);
}

/// Projects the connections page from the shared page model.
///
/// The query fold, the sort key and the row order are the shared
/// reduction; the surface only renders what the model reports.
pub(super) fn project_connections(window: &AppWindow, snapshot: &SurfaceSnapshot) {
    connections::project_connections(window, snapshot);
}

/// Projects the rules page from the shared page model.
///
/// The rule-list query and the match rule belong to the shared page model:
/// the surface asks the model which rules survive the query and renders
/// exactly those rows, so the surface never keeps a private string filter
/// of its own.
pub(super) fn project_rules_and_providers(window: &AppWindow, snapshot: &SurfaceSnapshot) {
    let language = snapshot_language(snapshot);
    let lang = Lang(&language);
    let mut model = RulesPageModel::default();
    model.edit_filter(window.get_rule_search().as_str());
    let (rules, providers) = snapshot
        .pages
        .rules
        .data
        .as_ref()
        .map(|page| {
            let rules = model
                .filtered_indices(&page.rules)
                .into_iter()
                .filter_map(|index| {
                    let rule = &page.rules[index];
                    model.matches_rule(rule).then_some(rule)
                })
                .map(|rule| {
                    // The shared rule view owns the expression a rule is
                    // searched by, so the row shows the very expression the
                    // shared filter matches.
                    let expression = rule.view_rule_search();
                    ObservationRow {
                        id: rule.id.to_string().into(),
                        title: expression.clone().into(),
                        accessible_title: expression.into(),
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
                    }
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

/// Projects the logs page from the shared page model.
///
/// The query fold, the level filter, the page status and the typed search
/// outcome are the shared model's; the surface only renders what the model
/// reports.
pub(super) fn project_logs(window: &AppWindow, snapshot: &SurfaceSnapshot) {
    logs::project_logs(window, snapshot);
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
    let history = &snapshot.traffic_history;
    let today_bars: Vec<TrafficBar> = history
        .today_bars
        .iter()
        .map(|bar| TrafficBar {
            label: bar.label.clone().into(),
            detail_label: bar.detail_label.clone().into(),
            download: format_bytes(bar.download_bytes).into(),
            upload: format_bytes(bar.upload_bytes).into(),
            download_fraction: bar.download_fraction,
            upload_fraction: bar.upload_fraction,
        })
        .collect();
    let week_bars: Vec<TrafficBar> = history
        .week_bars
        .iter()
        .map(|bar| TrafficBar {
            label: bar.label.clone().into(),
            detail_label: bar.detail_label.clone().into(),
            download: format_bytes(bar.download_bytes).into(),
            upload: format_bytes(bar.upload_bytes).into(),
            download_fraction: bar.download_fraction,
            upload_fraction: bar.upload_fraction,
        })
        .collect();
    window.set_traffic_today_bars(slint_model(today_bars));
    window.set_traffic_week_bars(slint_model(week_bars));
    window.set_traffic_today_has_data(history.today_has_data);
    window.set_traffic_week_has_data(history.week_has_data);
    window.set_traffic_today_down(if history.today_has_data {
        format_bytes(history.today_download_bytes).into()
    } else {
        "—".into()
    });
    window.set_traffic_today_up(if history.today_has_data {
        format_bytes(history.today_upload_bytes).into()
    } else {
        "—".into()
    });
    window.set_traffic_week_down(if history.week_has_data {
        format_bytes(history.week_download_bytes).into()
    } else {
        "—".into()
    });
    window.set_traffic_week_up(if history.week_has_data {
        format_bytes(history.week_upload_bytes).into()
    } else {
        "—".into()
    });
    window.set_traffic_week_average_down(if history.week_has_data {
        format_bytes(history.week_average_download_bytes).into()
    } else {
        "—".into()
    });
    window.set_traffic_week_average_up(if history.week_has_data {
        format_bytes(history.week_average_upload_bytes).into()
    } else {
        "—".into()
    });
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
        if let Some(closed) = snapshot
            .closed_connections
            .records
            .iter()
            .find(|c| c.id == connection_id)
        {
            // The closed record is a connection view too, so the drawer reads
            // its facts through the shared accessors. The record carries no
            // process and no endpoint, so those lines stay honestly empty
            // instead of naming a fact the owner never published.
            let rule = closed.view_rule();
            let chains = closed.view_joined_chain();
            let rule_copy = if rule.is_empty() {
                "—".to_owned()
            } else {
                rule.to_owned()
            };
            let chains_copy = if chains.is_empty() {
                "—".to_owned()
            } else {
                chains.to_owned()
            };
            window.set_connection_detail_host(closed.view_host().to_owned().into());
            window.set_connection_detail_id(closed.view_id().to_owned().into());
            window.set_connection_detail_protocol("—".into());
            window.set_connection_detail_process("—".into());
            window.set_connection_detail_source("—".into());
            window.set_connection_detail_destination(closed.view_host().to_owned().into());
            window.set_connection_detail_rule(rule_copy.into());
            window.set_connection_detail_chains(chains_copy.into());
            window.set_connection_detail_providers("—".into());
            window.set_connection_detail_start("—".into());
            let duration_sec = closed.duration_ms / 1000;
            window.set_connection_detail_duration(format!("{}s", duration_sec).into());
            window.set_connection_detail_upload(format_bytes(closed.view_upload_total()).into());
            window
                .set_connection_detail_download(format_bytes(closed.view_download_total()).into());
            window.set_connection_detail_active(false);
            return;
        }
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
    let mut parts = rate.split_whitespace();
    let value = parts.next().unwrap_or("0").to_owned();
    let unit = parts.next().unwrap_or("B/s").to_owned();
    (value, unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_rate_preserves_per_second_unit() {
        assert_eq!(
            split_rate("12.80 MB/s"),
            ("12.80".to_owned(), "MB/s".to_owned())
        );
        assert_eq!(split_rate("0 B/s"), ("0".to_owned(), "B/s".to_owned()));
        assert_eq!(
            split_rate("512 KB/s"),
            ("512".to_owned(), "KB/s".to_owned())
        );
        assert_eq!(split_rate(""), ("0".to_owned(), "B/s".to_owned()));
    }
}
