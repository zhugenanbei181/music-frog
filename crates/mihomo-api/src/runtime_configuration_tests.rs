    use super::*;
    use crate::types::{DnsConfig, SnifferConfig, TunConfig};
    use infiltrator_contract::core_identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, EngineId,
    };

    struct FixedOwner(CoreScopeObservation);

    impl CoreScopeSource for FixedOwner {
        fn scope_observation(&self) -> CoreScopeObservation {
            self.0.clone()
        }
    }

    fn test_scope() -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                EngineId::new("mihomo").unwrap(),
                CoreInstanceId::new("primary").unwrap(),
                None,
            ),
            generation: CoreGeneration::new(7),
            session_token: None,
        }
    }

    #[test]
    fn scope_resolver_reads_the_owner_scope() {
        let scope = test_scope();
        let resolver = scope_resolver_from_owner(Arc::new(FixedOwner(
            CoreScopeObservation::Observed(scope.clone()),
        )));

        assert_eq!(resolver().unwrap(), scope);
    }

    #[test]
    fn scope_resolver_rejects_an_unavailable_owner() {
        let resolver =
            scope_resolver_from_owner(Arc::new(FixedOwner(CoreScopeObservation::Unavailable)));

        assert!(matches!(resolver(), Err(PortError::Failed(_))));
    }

    #[test]
    fn mihomo_config_mapping_preserves_absent_vs_explicit_false_empty_and_zero() {
        let missing = ConfigResponse::default();
        assert_eq!(
            tun_configuration(&missing).enabled,
            ObservedValue::Unreported
        );
        assert_eq!(
            ipv6_configuration(&missing).enabled,
            ObservedValue::Unreported
        );
        assert_eq!(
            sniffer_configuration(&missing).enabled,
            ObservedValue::Unreported
        );
        assert_eq!(
            dns_runtime_configuration(&missing).nameserver,
            ObservedValue::Unreported
        );
        assert_eq!(
            lan_security_configuration(&missing).authentication,
            LanAuthenticationState::Unreported
        );
        assert_eq!(
            lan_sharing_configuration(&missing).bind_address,
            ObservedValue::Unreported
        );
        assert_eq!(
            core_log_level_configuration(&missing),
            ObservedValue::Unreported
        );

        let explicit = ConfigResponse {
            log_level: "info".to_owned(),
            allow_lan: false,
            mixed_port: 0,
            port: 0,
            ipv6: Some(false),
            bind_address: Some("*".to_owned()),
            lan_allowed_ips: Some(Vec::new()),
            lan_disallowed_ips: Some(Vec::new()),
            skip_auth_prefixes: Some(Vec::new()),
            authentication: Some(Vec::new()),
            tun: Some(TunConfig {
                enable: Some(false),
                strict_route: Some(false),
                ..Default::default()
            }),
            sniffer: Some(SnifferConfig {
                enable: false,
                ..Default::default()
            }),
            dns: Some(DnsConfig {
                nameserver: Vec::new(),
                fallback: Some(Vec::new()),
                enhanced_mode: "fake-ip".to_owned(),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            tun_configuration(&explicit).enabled,
            ObservedValue::Reported(false)
        );
        assert_eq!(
            tun_configuration(&explicit).strict_route,
            ObservedValue::Reported(false)
        );
        assert_eq!(
            ipv6_configuration(&explicit).enabled,
            ObservedValue::Reported(false)
        );
        assert_eq!(
            sniffer_configuration(&explicit).enabled,
            ObservedValue::Reported(false)
        );
        assert_eq!(
            dns_runtime_configuration(&explicit).nameserver,
            ObservedValue::Reported(Vec::new())
        );
        assert_eq!(
            lan_security_configuration(&explicit).allowed_ips,
            ObservedValue::Reported(Vec::new())
        );
        assert_eq!(
            lan_security_configuration(&explicit).authentication,
            LanAuthenticationState::Disabled
        );
        assert_eq!(
            proxy_ports_configuration(&explicit).http_port,
            ObservedValue::Reported(0)
        );
        assert_eq!(
            proxy_ports_configuration(&explicit).mixed_port,
            ObservedValue::Reported(0)
        );
        assert_eq!(
            core_log_level_configuration(&explicit),
            ObservedValue::Reported(CoreLogLevel::Info)
        );
        let warning = ConfigResponse {
            log_level: "warning".to_owned(),
            ..Default::default()
        };
        assert_eq!(
            core_log_level_configuration(&warning),
            ObservedValue::Reported(CoreLogLevel::Warn)
        );
        let unknown = ConfigResponse {
            log_level: "trace".to_owned(),
            ..Default::default()
        };
        assert_eq!(
            core_log_level_configuration(&unknown),
            ObservedValue::Unreported
        );

        let canonical_stack = ConfigResponse {
            tun: Some(TunConfig {
                stack: Some("System".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            tun_configuration(&canonical_stack).stack,
            ObservedValue::Reported("system".to_owned())
        );
        let unknown_stack = ConfigResponse {
            tun: Some(TunConfig {
                stack: Some("future-stack".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            tun_configuration(&unknown_stack).stack,
            ObservedValue::Reported("future-stack".to_owned())
        );
    }

    #[test]
    fn pac_rule_read_mapping_is_scoped_to_the_dedicated_read_contract() {
        let mapped = pac_rules(vec![Rule {
            rule_type: "DOMAIN-SUFFIX".to_owned(),
            payload: "example.org".to_owned(),
            proxy: "DIRECT".to_owned(),
            ..Default::default()
        }]);
        assert_eq!(
            mapped,
            vec![RuleEntry {
                rule: "DOMAIN-SUFFIX,example.org,DIRECT".to_owned(),
                enabled: true,
            }]
        );
    }
