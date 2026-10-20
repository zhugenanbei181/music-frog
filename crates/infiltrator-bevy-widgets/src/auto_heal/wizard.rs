//! Interactive auto-repair wizard behaviour: the anomaly catalog, the
//! localized fix copy and the wizard state machine.

use super::{AutoFixAction, AutoHealWizardState, DiagnosticAnomaly};
use infiltrator_shared::i18n_interpolator::localize;

impl AutoFixAction {
    fn copy_keys(&self) -> (&'static str, &'static str) {
        match self.anomaly {
            DiagnosticAnomaly::ControllerPortConflict(_) => {
                ("auto_heal_port_title", "auto_heal_port_description")
            }
            DiagnosticAnomaly::TunInterfaceMissing => {
                ("auto_heal_tun_title", "auto_heal_tun_description")
            }
            DiagnosticAnomaly::DnsLeakDetected => {
                ("auto_heal_dns_title", "auto_heal_dns_description")
            }
            DiagnosticAnomaly::SubscriptionExpired => (
                "auto_heal_subscription_title",
                "auto_heal_subscription_description",
            ),
            DiagnosticAnomaly::HighPacketLoss => {
                ("auto_heal_loss_title", "auto_heal_loss_description")
            }
            DiagnosticAnomaly::ZombieProcessDetected => {
                ("auto_heal_zombie_title", "auto_heal_zombie_description")
            }
        }
    }

    pub fn title(&self, locale: &str) -> String {
        let params = match self.anomaly {
            DiagnosticAnomaly::ControllerPortConflict(port) => vec![("port", port.to_string())],
            _ => Vec::new(),
        };
        localize(locale, self.copy_keys().0, &params)
    }

    pub fn description(&self, locale: &str) -> String {
        localize(locale, self.copy_keys().1, &[])
    }
}

impl AutoHealWizardState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_anomaly(&mut self, anomaly: DiagnosticAnomaly) {
        if !self.active_anomalies.contains(&anomaly) {
            self.active_anomalies.push(anomaly);
            let action = AutoFixAction {
                anomaly,
                is_destructive: matches!(anomaly, DiagnosticAnomaly::ZombieProcessDetected),
            };
            self.pending_actions.push(action);
        }
    }

    pub fn clear(&mut self) {
        self.active_anomalies.clear();
        self.pending_actions.clear();
        self.is_repairing = false;
        self.repair_progress_fraction = 0.0;
        self.last_repair_success = None;
    }
}
