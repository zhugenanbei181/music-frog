use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::network_roaming::{
    NetworkInterfaceSnapshot, NetworkRoamingRepairRequest,
};
use infiltrator_domain::network_roaming::NetworkRoamingDecision;
use infiltrator_ports::runtime_configuration::{ObservedValue, TunConfiguration};

#[derive(Clone, Copy)]
pub(super) enum ReconcileIntent {
    Observe,
    ForceRepair,
}

pub(super) enum TunConfigurationObservation {
    NotComposed,
    Failed(Failure),
    Reported(TunConfiguration),
}

#[derive(Clone, Copy)]
enum ObservedBoolean {
    Unreported,
    Disabled,
    Enabled,
}

pub(super) enum RouteRepairPlan {
    NotRequested,
    Skipped(String),
    Degraded(Failure),
    Failed(Failure),
    Repair(NetworkRoamingRepairRequest),
}

pub(super) fn plan_route_repair(
    intent: ReconcileIntent,
    decision: &NetworkRoamingDecision,
    selected: Option<&NetworkInterfaceSnapshot>,
    configuration: &TunConfigurationObservation,
    tun_interface: Option<&str>,
) -> RouteRepairPlan {
    match (intent, decision) {
        (
            ReconcileIntent::Observe,
            NetworkRoamingDecision::InitialObservation { .. }
            | NetworkRoamingDecision::Stable { .. },
        ) => RouteRepairPlan::NotRequested,
        (
            ReconcileIntent::Observe,
            NetworkRoamingDecision::InterfaceAddressChanged { .. }
            | NetworkRoamingDecision::GatewayChanged { .. },
        ) => plan_automatic_route_repair(selected, configuration, tun_interface),
        (ReconcileIntent::ForceRepair, _) => {
            plan_forced_route_repair(selected, configuration, tun_interface)
        }
    }
}

fn plan_automatic_route_repair(
    selected: Option<&NetworkInterfaceSnapshot>,
    configuration: &TunConfigurationObservation,
    tun_interface: Option<&str>,
) -> RouteRepairPlan {
    let Some(selected) = selected else {
        return RouteRepairPlan::Skipped(
            "no live physical egress interface is available".to_owned(),
        );
    };
    let configuration = match configuration {
        TunConfigurationObservation::NotComposed => {
            return RouteRepairPlan::Skipped(
                "Mihomo gateway is not composed for network route repair".to_owned(),
            );
        }
        TunConfigurationObservation::Failed(failure) => {
            return RouteRepairPlan::Degraded(failure.clone());
        }
        TunConfigurationObservation::Reported(configuration) => configuration,
    };
    match observed_boolean(&configuration.enabled) {
        ObservedBoolean::Unreported => RouteRepairPlan::Degraded(Failure::new(
            ErrorCode::NotReady,
            "TUN enable state was not reported",
            true,
        )),
        ObservedBoolean::Disabled => {
            RouteRepairPlan::Skipped("TUN is not enabled; no route table needs repair".to_owned())
        }
        ObservedBoolean::Enabled => match observed_boolean(&configuration.auto_route) {
            ObservedBoolean::Unreported => RouteRepairPlan::Degraded(Failure::new(
                ErrorCode::NotReady,
                "TUN auto-route state was not reported",
                true,
            )),
            ObservedBoolean::Disabled => RouteRepairPlan::Skipped(
                "TUN auto-route is disabled; refusing to mutate host routes".to_owned(),
            ),
            ObservedBoolean::Enabled => match tun_interface {
                None => RouteRepairPlan::Degraded(Failure::new(
                    ErrorCode::NotReady,
                    "active TUN route was configured but no TUN interface was observed",
                    true,
                )),
                Some(tun_interface) => match observed_boolean(&configuration.strict_route) {
                    ObservedBoolean::Unreported => RouteRepairPlan::Failed(Failure::new(
                        ErrorCode::NotReady,
                        "TUN strict-route state was not reported",
                        true,
                    )),
                    ObservedBoolean::Disabled => {
                        RouteRepairPlan::Repair(repair_request(selected, tun_interface, false))
                    }
                    ObservedBoolean::Enabled => {
                        RouteRepairPlan::Repair(repair_request(selected, tun_interface, true))
                    }
                },
            },
        },
    }
}

fn plan_forced_route_repair(
    selected: Option<&NetworkInterfaceSnapshot>,
    configuration: &TunConfigurationObservation,
    tun_interface: Option<&str>,
) -> RouteRepairPlan {
    let Some(selected) = selected else {
        return RouteRepairPlan::Failed(Failure::new(
            ErrorCode::NotReady,
            "no live physical egress interface is available",
            true,
        ));
    };
    let configuration = match configuration {
        TunConfigurationObservation::NotComposed => {
            return RouteRepairPlan::Failed(Failure::unsupported(
                "Mihomo gateway is not composed for network route repair",
            ));
        }
        TunConfigurationObservation::Failed(failure) => {
            return RouteRepairPlan::Failed(failure.clone());
        }
        TunConfigurationObservation::Reported(configuration) => configuration,
    };
    match observed_boolean(&configuration.enabled) {
        ObservedBoolean::Unreported => RouteRepairPlan::Failed(Failure::new(
            ErrorCode::NotReady,
            "TUN enable state was not reported",
            true,
        )),
        ObservedBoolean::Disabled => RouteRepairPlan::Failed(Failure::new(
            ErrorCode::NotReady,
            "TUN is not enabled; no route table needs repair",
            false,
        )),
        ObservedBoolean::Enabled => match observed_boolean(&configuration.auto_route) {
            ObservedBoolean::Unreported => RouteRepairPlan::Failed(Failure::new(
                ErrorCode::NotReady,
                "TUN auto-route state was not reported",
                true,
            )),
            ObservedBoolean::Disabled => RouteRepairPlan::Failed(Failure::new(
                ErrorCode::InvalidState,
                "TUN auto-route is disabled; refusing to mutate host routes",
                false,
            )),
            ObservedBoolean::Enabled => match tun_interface {
                None => RouteRepairPlan::Failed(Failure::new(
                    ErrorCode::NotReady,
                    "active TUN interface was not observed",
                    true,
                )),
                Some(tun_interface) => match observed_boolean(&configuration.strict_route) {
                    ObservedBoolean::Unreported => RouteRepairPlan::Failed(Failure::new(
                        ErrorCode::NotReady,
                        "TUN strict-route state was not reported",
                        true,
                    )),
                    ObservedBoolean::Disabled => {
                        RouteRepairPlan::Repair(repair_request(selected, tun_interface, false))
                    }
                    ObservedBoolean::Enabled => {
                        RouteRepairPlan::Repair(repair_request(selected, tun_interface, true))
                    }
                },
            },
        },
    }
}

fn repair_request(
    selected: &NetworkInterfaceSnapshot,
    tun_interface: &str,
    strict_route: bool,
) -> NetworkRoamingRepairRequest {
    NetworkRoamingRepairRequest {
        physical_interface: selected.name.clone(),
        gateway_ip: selected.gateway_ip.clone(),
        tun_interface: tun_interface.to_owned(),
        strict_route,
    }
}

fn observed_boolean(value: &ObservedValue<bool>) -> ObservedBoolean {
    match value {
        ObservedValue::Unreported => ObservedBoolean::Unreported,
        ObservedValue::Reported(false) => ObservedBoolean::Disabled,
        ObservedValue::Reported(true) => ObservedBoolean::Enabled,
    }
}
