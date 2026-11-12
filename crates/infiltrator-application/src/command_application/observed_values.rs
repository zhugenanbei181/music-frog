//! Pure helpers for runtime-configuration observations.

use infiltrator_ports::runtime_configuration::ObservedValue;

pub(super) fn reported_port(value: &ObservedValue<u16>) -> u16 {
    match value {
        ObservedValue::Unreported => 0,
        ObservedValue::Reported(port) => *port,
    }
}
