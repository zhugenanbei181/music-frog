use super::*;
use crate::rules::RuleEntry;

#[test]
fn test_generate_pac_basic() {
    let config = PacConfig {
        default_proxy: "127.0.0.1:7890".to_string(),
        rules: vec![
            RuleEntry {
                rule: "DOMAIN,google.com,PROXY".to_string(),
                enabled: true,
            },
            RuleEntry {
                rule: "DOMAIN-SUFFIX,github.com,DIRECT".to_string(),
                enabled: true,
            },
            RuleEntry {
                rule: "DOMAIN-KEYWORD,youtube,127.0.0.1:1080".to_string(),
                enabled: true,
            },
            RuleEntry {
                rule: "MATCH,PROXY".to_string(),
                enabled: true,
            },
        ],
    };

    let pac = PacGenerator::generate_pac(&config);
    assert!(pac.contains("function FindProxyForURL"));
    assert!(pac.contains("if (host === \"google.com\") return \"PROXY 127.0.0.1:7890\";"));
    assert!(pac.contains(
        "if (dnsDomainIs(host, \".github.com\") || host === \"github.com\") return \"DIRECT\";"
    ));
    assert!(pac.contains("if (shExpMatch(host, \"*youtube*\")) return \"PROXY 127.0.0.1:1080\";"));
    assert!(pac.contains("return \"PROXY 127.0.0.1:7890\";"));
    assert!(validate_pac_script(&pac).is_ok());
}

#[test]
fn test_generate_pac_disabled_rule_skipped() {
    let config = PacConfig {
        default_proxy: "127.0.0.1:7890".to_string(),
        rules: vec![
            RuleEntry {
                rule: "DOMAIN,disabled.com,PROXY".to_string(),
                enabled: false,
            },
            RuleEntry {
                rule: "# Comment rule".to_string(),
                enabled: true,
            },
            RuleEntry {
                rule: "// Another comment".to_string(),
                enabled: true,
            },
        ],
    };

    let pac = PacGenerator::generate_pac(&config);
    assert!(!pac.contains("disabled.com"));
    assert!(!pac.contains("Comment rule"));
    assert!(validate_pac_script(&pac).is_ok());
}

#[test]
fn test_builder_pattern() {
    let generator = PacGenerator::new("127.0.0.1:7890")
        .with_socks_target("127.0.0.1:1080")
        .with_bypass_lan(false)
        .with_bypass_domains(vec!["example.com".to_string()])
        .add_bypass_domain("*.test.local")
        .with_custom_rules(vec![
            "if (url.startsWith(\"https://auth\")) return \"DIRECT\";".to_string(),
        ])
        .add_custom_rule("if (host === \"custom.dev\") return \"PROXY 127.0.0.1:9090\";")
        .with_minified(true);

    assert_eq!(generator.proxy_target, "127.0.0.1:7890");
    assert_eq!(generator.socks_target, Some("127.0.0.1:1080".to_string()));
    assert!(!generator.bypass_lan);
    assert_eq!(generator.bypass_domains.len(), 2);
    assert_eq!(generator.custom_rules.len(), 2);
    assert!(generator.minify);

    let script = generator.compile_pac_script(&[]);
    assert!(!script.contains("isPlainHostName"));
    assert!(script.contains("https://auth"));
    assert!(script.contains("custom.dev"));
    assert!(script.contains("example.com"));
    assert!(script.contains("*.test.local"));
    assert!(validate_pac_script(&script).is_ok());
}

#[test]
fn test_proxy_target_formatting() {
    let generator =
        PacGenerator::new("PROXY 127.0.0.1:7890; DIRECT").with_socks_target("127.0.0.1:1080");

    let rules = vec![
        RuleEntry {
            rule: "DOMAIN,proxy1.com,PROXY".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN,proxy2.com,DEFAULT".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN,socks.com,SOCKS".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN,socks5.com,SOCKS5".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN,reject.com,REJECT".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN,drop.com,DROP".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN,custom.com,PROXY 10.0.0.1:8080; SOCKS5 10.0.0.1:1080".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN,scheme.com,socks5://192.168.1.1:1080".to_string(),
            enabled: true,
        },
    ];

    let pac = generator.compile_pac_script(&rules);
    assert!(pac.contains("if (host === \"proxy1.com\") return \"PROXY 127.0.0.1:7890; DIRECT\";"));
    assert!(pac.contains("if (host === \"proxy2.com\") return \"PROXY 127.0.0.1:7890; DIRECT\";"));
    assert!(pac.contains("if (host === \"socks.com\") return \"SOCKS5 127.0.0.1:1080\";"));
    assert!(pac.contains("if (host === \"socks5.com\") return \"SOCKS5 127.0.0.1:1080\";"));
    assert!(pac.contains("if (host === \"reject.com\") return \"PROXY 127.0.0.1:0\";"));
    assert!(pac.contains("if (host === \"drop.com\") return \"PROXY 127.0.0.1:0\";"));
    assert!(pac.contains(
        "if (host === \"custom.com\") return \"PROXY 10.0.0.1:8080; SOCKS5 10.0.0.1:1080\";"
    ));
    assert!(pac.contains("if (host === \"scheme.com\") return \"SOCKS5 192.168.1.1:1080\";"));
    assert!(validate_pac_script(&pac).is_ok());
}

#[test]
fn test_bypass_lan_behavior() {
    let gen_with_lan = PacGenerator::new("127.0.0.1:7890").with_bypass_lan(true);
    let pac_lan = gen_with_lan.compile_pac_script(&[]);
    assert!(pac_lan.contains("isPlainHostName(host)"));
    assert!(pac_lan.contains("isInNet(dnsResolve(host), \"10.0.0.0\", \"255.0.0.0\")"));
    assert!(pac_lan.contains("isInNet(dnsResolve(host), \"172.16.0.0\", \"255.240.0.0\")"));
    assert!(pac_lan.contains("isInNet(dnsResolve(host), \"192.168.0.0\", \"255.255.0.0\")"));
    assert!(pac_lan.contains("isInNet(dnsResolve(host), \"127.0.0.0\", \"255.0.0.0\")"));
    assert!(pac_lan.contains("isInNet(dnsResolve(host), \"169.254.0.0\", \"255.255.0.0\")"));
    assert!(pac_lan.contains("shExpMatch(host, \"*.local\")"));

    let gen_without_lan = PacGenerator::new("127.0.0.1:7890").with_bypass_lan(false);
    let pac_no_lan = gen_without_lan.compile_pac_script(&[]);
    assert!(!pac_no_lan.contains("isPlainHostName"));
    assert!(!pac_no_lan.contains("10.0.0.0"));
    assert!(validate_pac_script(&pac_lan).is_ok());
    assert!(validate_pac_script(&pac_no_lan).is_ok());
}

#[test]
fn test_all_rule_types() {
    let generator = PacGenerator::new("127.0.0.1:7890");
    let rules = vec![
        RuleEntry {
            rule: "DOMAIN,exact.com,PROXY".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN-SUFFIX,.suffix.com,DIRECT".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN-KEYWORD,keyword,PROXY".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN-WILDCARD,*.wild.*,PROXY".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN-REGEX,^api-.*\\.service\\.org$,DIRECT".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "URL-REGEX,^https?://cdn\\.example\\.com/.*,DIRECT".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "IP-CIDR,1.1.1.1/32,PROXY,no-resolve".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "IP-CIDR,192.168.50.0/24,DIRECT".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "IP-CIDR,8.8.8.8,PROXY".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "FINAL,DIRECT".to_string(),
            enabled: true,
        },
    ];

    let pac = generator.compile_pac_script(&rules);
    assert!(pac.contains("if (host === \"exact.com\") return \"PROXY 127.0.0.1:7890\";"));
    assert!(pac.contains(
        "if (dnsDomainIs(host, \".suffix.com\") || host === \"suffix.com\") return \"DIRECT\";"
    ));
    assert!(pac.contains("if (shExpMatch(host, \"*keyword*\")) return \"PROXY 127.0.0.1:7890\";"));
    assert!(pac.contains("if (shExpMatch(host, \"*.wild.*\")) return \"PROXY 127.0.0.1:7890\";"));
    assert!(pac.contains("if (/^api-.*\\.service\\.org$/i.test(host)) return \"DIRECT\";"));
    assert!(
        pac.contains("if (/^https?:\\/\\/cdn\\.example\\.com\\/.*/i.test(url)) return \"DIRECT\";")
    );
    assert!(pac.contains("if (isInNet(dnsResolve(host), \"1.1.1.1\", \"255.255.255.255\")) return \"PROXY 127.0.0.1:7890\";"));
    assert!(pac.contains(
        "if (isInNet(dnsResolve(host), \"192.168.50.0\", \"255.255.255.0\")) return \"DIRECT\";"
    ));
    assert!(pac.contains("if (isInNet(dnsResolve(host), \"8.8.8.8\", \"255.255.255.255\")) return \"PROXY 127.0.0.1:7890\";"));
    assert!(pac.contains("return \"DIRECT\";"));
    assert!(validate_pac_script(&pac).is_ok());
}

#[test]
fn test_cidr_to_netmask_calculation() {
    assert_eq!(cidr_to_netmask(0), Some("0.0.0.0".to_string()));
    assert_eq!(cidr_to_netmask(8), Some("255.0.0.0".to_string()));
    assert_eq!(cidr_to_netmask(12), Some("255.240.0.0".to_string()));
    assert_eq!(cidr_to_netmask(16), Some("255.255.0.0".to_string()));
    assert_eq!(cidr_to_netmask(24), Some("255.255.255.0".to_string()));
    assert_eq!(cidr_to_netmask(32), Some("255.255.255.255".to_string()));
    assert_eq!(cidr_to_netmask(33), None);
}

#[test]
fn test_proxy_override_generation() {
    let generator = PacGenerator::new("127.0.0.1:7890")
        .with_bypass_lan(true)
        .with_bypass_domains(vec!["*.internal.net".to_string(), "corp.local".to_string()]);

    let override_str = generator.generate_proxy_override();
    assert!(override_str.contains("localhost"));
    assert!(override_str.contains("127.*"));
    assert!(override_str.contains("10.*"));
    assert!(override_str.contains("172.16.*"));
    assert!(override_str.contains("192.168.*"));
    assert!(override_str.contains("*.lan"));
    assert!(override_str.contains("*.local"));
    assert!(override_str.contains("*.internal.net"));
    assert!(override_str.contains("corp.local"));
    assert!(override_str.contains("<local>"));
    assert_eq!(override_str, generator.proxy_override());

    let gen_no_lan = PacGenerator::new("127.0.0.1:7890")
        .with_bypass_lan(false)
        .with_bypass_domains(vec!["custom.domain".to_string()]);
    assert_eq!(gen_no_lan.generate_proxy_override(), "custom.domain");
}

#[test]
fn test_minify_pac_script() {
    let generator = PacGenerator::new("127.0.0.1:7890");
    let rules = vec![RuleEntry {
        rule: "DOMAIN,google.com,PROXY 127.0.0.1:7890; DIRECT".to_string(),
        enabled: true,
    }];

    let minified = generator.compile_pac_script_minified(&rules);
    assert!(!minified.contains('\n'));
    assert!(minified.contains("function FindProxyForURL(url,host){"));
    assert!(minified.contains("\"PROXY 127.0.0.1:7890; DIRECT\""));
    assert!(validate_pac_script(&minified).is_ok());

    let custom_js = r#"
    // This is a comment
    /* Multi line
       comment */
    function FindProxyForURL(url, host) {
        if (isPlainHostName(host)) {
            return "DIRECT";
        }
        return "PROXY 127.0.0.1:7890";
    }
    "#;
    let min = minify_pac_script(custom_js);
    assert!(!min.contains("comment"));
    assert!(min.contains("function FindProxyForURL(url,host){"));
    assert!(validate_pac_script(&min).is_ok());
}

#[test]
fn test_validate_pac_script_errors() {
    assert_eq!(
        validate_pac_script(""),
        Err(PacValidationError::EmptyScript)
    );
    assert_eq!(
        validate_pac_script("   \n\t  "),
        Err(PacValidationError::EmptyScript)
    );
    assert_eq!(
        validate_pac_script("function other(url, host) { return 'DIRECT'; }"),
        Err(PacValidationError::MissingFindProxyForURL)
    );
    assert_eq!(
        validate_pac_script("function FindProxyForURL(url, host) { let x = 1; }"),
        Err(PacValidationError::MissingReturnStatement)
    );
    assert_eq!(
        validate_pac_script("function FindProxyForURL(url, host) { return 'DIRECT';"),
        Err(PacValidationError::UnbalancedBraces { open: 1, close: 0 })
    );
    assert_eq!(
        validate_pac_script("function FindProxyForURL(url, host { return 'DIRECT'; }"),
        Err(PacValidationError::UnbalancedParentheses { open: 1, close: 0 })
    );
    assert_eq!(
        validate_pac_script("function FindProxyForURL(url, host) { return 'DIRECT; }"),
        Err(PacValidationError::UnterminatedStringLiteral)
    );
    assert_eq!(
        validate_pac_script("function FindProxyForURL(url, host) { /* unclosed return 'DIRECT'; }"),
        Err(PacValidationError::UnterminatedComment)
    );
}

#[test]
fn test_validation_error_display() {
    assert_eq!(
        format!("{}", PacValidationError::EmptyScript),
        "PAC script is empty"
    );
    assert_eq!(
        format!("{}", PacValidationError::MissingFindProxyForURL),
        "PAC script is missing 'FindProxyForURL' function declaration"
    );
    assert_eq!(
        format!("{}", PacValidationError::MissingReturnStatement),
        "PAC script does not contain any 'return' statement"
    );
    assert_eq!(
        format!(
            "{}",
            PacValidationError::UnbalancedBraces { open: 2, close: 1 }
        ),
        "Unbalanced braces: 2 opened, 1 closed"
    );
    assert_eq!(
        format!(
            "{}",
            PacValidationError::UnbalancedParentheses { open: 2, close: 1 }
        ),
        "Unbalanced parentheses: 2 opened, 1 closed"
    );
    assert_eq!(
        format!(
            "{}",
            PacValidationError::UnbalancedBrackets { open: 2, close: 1 }
        ),
        "Unbalanced brackets: 2 opened, 1 closed"
    );
    assert_eq!(
        format!("{}", PacValidationError::UnterminatedStringLiteral),
        "PAC script contains an unterminated string literal"
    );
    assert_eq!(
        format!("{}", PacValidationError::UnterminatedComment),
        "PAC script contains an unterminated block comment"
    );
    assert_eq!(
        format!(
            "{}",
            PacValidationError::InvalidSyntax("bad token".to_string())
        ),
        "PAC script syntax error: bad token"
    );
}
