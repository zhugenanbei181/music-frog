//! Kernel attribution is folded once, preserving never queried versus no record.
use infiltrator_contract::rule_override::{CreateRuleOverrideIntent, DEFAULT_OVERRIDE_TARGETS};
use infiltrator_domain::connection_view::{
    ConnectionView, DestinationAsnFact, DestinationGeoFact, destination_asn_fact,
    destination_geo_fact, quick_rule_spec,
};

pub fn kernel_asn_label(raw: &str, translate: &impl Fn(&str) -> String) -> String {
    match destination_asn_fact(raw) {
        DestinationAsnFact::NotEvaluated => translate("conn_drawer_kernel_not_evaluated"),
        DestinationAsnFact::NoResult => translate("conn_drawer_kernel_no_result"),
        DestinationAsnFact::Reported(value) => value,
    }
}

pub fn kernel_geo_label(codes: Option<&[String]>, translate: &impl Fn(&str) -> String) -> String {
    match destination_geo_fact(codes) {
        DestinationGeoFact::NotEvaluated => translate("conn_drawer_kernel_not_evaluated"),
        DestinationGeoFact::NoResult => translate("conn_drawer_kernel_no_result"),
        DestinationGeoFact::Codes(codes) => codes.join(", "),
    }
}

/// Build a typed `CreateRuleOverrideIntent` from a connection and a designated outbound target (FLPARITY-028).
/// Defaults target to "DIRECT" if unspecified.
pub fn connection_rule_override_intent<C: ConnectionView>(
    conn: &C,
    target: Option<&str>,
) -> Option<CreateRuleOverrideIntent> {
    let desired_target = target.unwrap_or("DIRECT");
    let spec = quick_rule_spec(conn, desired_target);
    spec.to_override_intent()
}

/// Available default targets for quick rule override popup / drawer actions.
pub fn quick_rule_target_options() -> &'static [&'static str] {
    &DEFAULT_OVERRIDE_TARGETS
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockConn {
        host: String,
        dest_ip: String,
    }

    impl ConnectionView for MockConn {
        fn view_id(&self) -> &str {
            "mock-1"
        }
        fn view_host(&self) -> &str {
            &self.host
        }
        fn view_destination_ip(&self) -> &str {
            &self.dest_ip
        }
        fn view_process_path(&self) -> &str {
            "/usr/bin/curl"
        }
        fn view_upload_total(&self) -> u64 {
            100
        }
        fn view_download_total(&self) -> u64 {
            200
        }
    }

    #[test]
    fn connection_override_intent_derivation() {
        let conn_domain = MockConn {
            host: "api.anthropic.com:443".to_owned(),
            dest_ip: "104.18.2.1".to_owned(),
        };
        let intent = connection_rule_override_intent(&conn_domain, Some("PROXY")).unwrap();
        assert_eq!(intent.rule_type, "DOMAIN-SUFFIX");
        assert_eq!(intent.payload, "api.anthropic.com");
        assert_eq!(intent.target, "PROXY");
        assert_eq!(intent.rule_line(), "DOMAIN-SUFFIX,api.anthropic.com,PROXY");

        let conn_ip = MockConn {
            host: "".to_owned(),
            dest_ip: "1.1.1.1".to_owned(),
        };
        let intent_ip = connection_rule_override_intent(&conn_ip, None).unwrap();
        assert_eq!(intent_ip.rule_type, "IP-CIDR");
        assert_eq!(intent_ip.payload, "1.1.1.1/32");
        assert_eq!(intent_ip.target, "DIRECT");
    }
}
