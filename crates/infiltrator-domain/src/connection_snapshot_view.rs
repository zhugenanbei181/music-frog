//! Shared connection snapshot implements the domain reduction seam.
use crate::connection_view::{ConnectionView, bare_host};
use infiltrator_contract::surface_snapshot::ConnectionSnapshot;

impl ConnectionView for ConnectionSnapshot {
    fn view_id(&self) -> &str {
        &self.id
    }

    fn view_host(&self) -> &str {
        if self.destination_host.is_empty() {
            bare_host(&self.host)
        } else {
            &self.destination_host
        }
    }

    fn view_destination_port(&self) -> &str {
        &self.destination_port
    }
    fn view_rule(&self) -> &str {
        &self.rule
    }

    fn view_start(&self) -> &str {
        &self.start
    }

    fn view_destination_ip(&self) -> &str {
        &self.destination_ip
    }

    fn view_process_path(&self) -> &str {
        &self.process
    }

    fn view_upload_total(&self) -> u64 {
        self.upload_total
    }

    fn view_download_total(&self) -> u64 {
        self.download_total
    }

    fn view_upload_rate_bps(&self) -> f64 {
        self.upload_bps
    }

    fn view_download_rate_bps(&self) -> f64 {
        self.download_bps
    }

    fn view_chain(&self) -> &[String] {
        &self.chains
    }

    fn view_joined_chain(&self) -> &str {
        &self.chain
    }

    fn view_search_terms(&self) -> Vec<&str> {
        let mut terms = vec![
            self.id.as_str(),
            self.view_host(),
            self.process.as_str(),
            self.source_ip.as_str(),
            self.destination_ip.as_str(),
            self.source_port.as_str(),
            self.destination_port.as_str(),
            self.network.as_str(),
            self.rule.as_str(),
            self.rule_payload.as_str(),
        ];
        terms.extend(self.chains.iter().map(String::as_str));
        terms
    }
}
