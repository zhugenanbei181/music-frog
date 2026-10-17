//! One localized replay of host network facts; missing values never become observations.
use infiltrator_contract::network_roaming::{
    NetworkRoamingEvent, NetworkRoamingSnapshot, NetworkRoamingStatus,
};
use infiltrator_contract::system_proxy::{
    SystemProxyOwnership, SystemProxyRecoverySnapshot, SystemProxyRecoveryStatus,
    SystemProxySnapshot, SystemProxyStatus,
};
use lurker_core_contract::redact::redact_line;
use lurker_ui_presentation::copy::TextCatalog;
use lurker_ui_presentation::copy::interpolate;

fn reason_copy(catalog: &dyn TextCatalog, key: &str, reason: &str) -> String {
    interpolate(
        catalog.text("network_status_reason").as_ref(),
        &[
            ("status", catalog.text(key).as_ref()),
            ("reason", &redact_line(reason, &[])),
        ],
    )
}
pub fn roaming_status(status: &NetworkRoamingStatus, catalog: &dyn TextCatalog) -> String {
    let key = match status {
        NetworkRoamingStatus::Unknown => "net_roam_status_unknown",
        NetworkRoamingStatus::Stable => "net_roam_status_stable",
        NetworkRoamingStatus::Recovering => "net_roam_status_recovering",
        NetworkRoamingStatus::Degraded { reason } => {
            return reason_copy(catalog, "net_roam_status_degraded", reason);
        }
        NetworkRoamingStatus::Unsupported { reason } => {
            return reason_copy(catalog, "net_roam_status_unsupported", reason);
        }
        NetworkRoamingStatus::Failed { failure } => {
            return reason_copy(catalog, "net_roam_status_failed", &failure.message);
        }
    };
    catalog.text(key).into_owned()
}
pub fn roaming_event(event: &NetworkRoamingEvent, catalog: &dyn TextCatalog) -> String {
    match event {
        NetworkRoamingEvent::InitialObservation {
            interface,
            gateway_ip,
        } => format!(
            "{} {} / {}",
            catalog.text("net_roam_event_initial"),
            interface.as_deref().unwrap_or("—"),
            gateway_ip.as_deref().unwrap_or("—")
        ),
        NetworkRoamingEvent::GatewayChanged {
            old_interface,
            new_interface,
            old_gateway_ip,
            new_gateway_ip,
        } => format!(
            "{} {} → {} ({} → {})",
            catalog.text("net_roam_event_gateway_changed"),
            old_interface.as_deref().unwrap_or("—"),
            new_interface.as_deref().unwrap_or("—"),
            old_gateway_ip.as_deref().unwrap_or("—"),
            new_gateway_ip.as_deref().unwrap_or("—")
        ),
        NetworkRoamingEvent::InterfaceAddressChanged { interface } => {
            reason_copy(catalog, "net_roam_event_address_changed", interface)
        }
        NetworkRoamingEvent::RoutesRepaired {
            physical_interface,
            tun_interface,
            detail,
        } => {
            format!(
                "{} · {physical_interface} → {tun_interface} · {}",
                catalog.text("net_roam_event_routes_repaired"),
                redact_line(detail, &[])
            )
        }
        NetworkRoamingEvent::RepairSkipped { reason } => {
            reason_copy(catalog, "net_roam_event_repair_skipped", reason)
        }
        NetworkRoamingEvent::RepairFailed { failure } => {
            reason_copy(catalog, "net_roam_event_repair_failed", &failure.message)
        }
    }
}
pub fn roaming_interfaces(snapshot: &NetworkRoamingSnapshot, catalog: &dyn TextCatalog) -> String {
    if snapshot.interfaces.is_empty() {
        return catalog.text("net_roam_interfaces_unobserved").into_owned();
    }
    snapshot
        .interfaces
        .iter()
        .take(12)
        .map(|interface| {
            let state = if interface.is_up { "up" } else { "down" };
            format!(
                "{} [{state}] gw={}",
                interface.name,
                interface.gateway_ip.as_deref().unwrap_or("—")
            )
        })
        .collect::<Vec<_>>()
        .join(" · ")
}
pub fn roaming_mtu(snapshot: &NetworkRoamingSnapshot, catalog: &dyn TextCatalog) -> String {
    let physical = snapshot
        .physical_mtu
        .map_or_else(|| "—".to_owned(), |value| value.to_string());
    let tun = snapshot
        .recommended_tun_mtu
        .map_or_else(|| "—".to_owned(), |value| value.to_string());
    let mss = snapshot
        .tcp_mss
        .map_or_else(|| "—".to_owned(), |value| value.to_string());
    interpolate(
        catalog.text("net_roam_mtu_detail").as_ref(),
        &[("physical", &physical), ("tun", &tun), ("mss", &mss)],
    )
}
pub fn roaming_route(snapshot: &NetworkRoamingSnapshot, catalog: &dyn TextCatalog) -> String {
    let active = snapshot.active_interface.as_deref().unwrap_or("—");
    let gateway = snapshot.default_gateway.as_deref().unwrap_or("—");
    let event = snapshot
        .last_event
        .as_ref()
        .map(|event| roaming_event(event, catalog))
        .unwrap_or_else(|| catalog.text("net_roam_no_event").into_owned());
    let mtu = roaming_mtu(snapshot, catalog);
    interpolate(
        catalog.text("net_roam_route_detail").as_ref(),
        &[
            ("active", active),
            ("gateway", gateway),
            ("mtu", &mtu),
            ("event", &event),
        ],
    )
}
pub fn system_proxy_status(
    snapshot: &SystemProxySnapshot,
    recovery: &SystemProxyRecoverySnapshot,
    catalog: &dyn TextCatalog,
) -> String {
    let recovery_key = match &recovery.status {
        SystemProxyRecoveryStatus::Restored { .. } => Some("system_proxy_recovery_restored"),
        SystemProxyRecoveryStatus::SkippedExternal { .. } => Some("system_proxy_recovery_external"),
        SystemProxyRecoveryStatus::Failed { failure, .. } => {
            return reason_copy(catalog, "system_proxy_recovery_failed", &failure.message);
        }
        SystemProxyRecoveryStatus::SkippedLiveOwner { .. } => {
            Some("system_proxy_recovery_live_owner")
        }
        SystemProxyRecoveryStatus::Unknown | SystemProxyRecoveryStatus::NotNeeded => None,
    };
    if let Some(key) = recovery_key {
        return catalog.text(key).into_owned();
    }
    let key = match &snapshot.status {
        SystemProxyStatus::Unknown => "system_proxy_status_unknown",
        SystemProxyStatus::Disabled => "system_proxy_status_disabled",
        SystemProxyStatus::Enabled => match snapshot.ownership {
            SystemProxyOwnership::Repaired => "system_proxy_status_repaired",
            SystemProxyOwnership::Owned => {
                let status = catalog.text("system_proxy_status_owned");
                return snapshot.endpoint.as_ref().map_or_else(
                    || status.clone().into_owned(),
                    |endpoint| format!("{status} · {endpoint}"),
                );
            }
            _ => "system_proxy_status_enabled",
        },
        SystemProxyStatus::Unsupported { failure } => {
            return reason_copy(catalog, "system_proxy_status_unsupported", &failure.message);
        }
        SystemProxyStatus::Failed { failure } => {
            return reason_copy(catalog, "system_proxy_status_failed", &failure.message);
        }
    };
    catalog.text(key).into_owned()
}

#[cfg(test)]
#[path = "network_status_projection_tests.rs"]
mod tests;
