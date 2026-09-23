use super::*;
use infiltrator_contract::dns_leak::{
    DnsLeakEchoRecord, DnsLeakObservation, DnsLeakObservationOutcome, DnsLeakProbeSource,
    DnsLeakProbeTransport, DnsLeakReport,
};

// ===========================================================================
// DUAL-14-08: DNS leak cross-source panel
// ===========================================================================

fn leak_observation(authority: &str, identity: &str) -> DnsLeakObservation {
    DnsLeakObservation {
        resolver: "1.1.1.1".to_owned(),
        authority: authority.to_owned(),
        question: format!("l7f3.{authority}"),
        transport: DnsLeakProbeTransport::Udp,
        outcome: DnsLeakObservationOutcome::Observed {
            identity: identity.to_owned(),
        },
    }
}

#[test]
fn test_dns_leak_panel_renders_the_shared_cross_source_report() {
    let zh = Lang("zh-CN");
    let en = Lang("en-US");

    // A host without a fact source renders the typed unsupported copy and no
    // observation row: no country, no ISP, no verdict.
    let unsupported = DnsLeakReport::unsupported("no echo authority is configured");
    let (unsupported_zh, _) = leak_conclusion_copy(&unsupported, &zh);
    assert!(
        unsupported_zh.contains("宿主未提供泄漏探测事实源"),
        "{unsupported_zh}"
    );
    let (unsupported_en, _) = leak_conclusion_copy(&unsupported, &en);
    assert!(
        unsupported_en.contains("no DNS leak fact source"),
        "{unsupported_en}"
    );
    assert!(leak_observation_lines(&unsupported, &zh).is_empty());

    // Nothing reported yet is not a verdict either: the default report stays
    // unknown and must never claim the host has no fact source.
    let pending = DnsLeakReport::default();
    let (pending_zh, _) = leak_conclusion_copy(&pending, &zh);
    assert!(pending_zh.contains("尚无交叉结论"), "{pending_zh}");
    assert!(!pending_zh.contains("宿主未提供"), "{pending_zh}");

    // A divergent shared report lists every observed fact, localized.
    let divergent = DnsLeakReport::observed(
        vec![
            DnsLeakProbeSource::new("1.1.1.1", "a.echo.example.org"),
            DnsLeakProbeSource::new("1.1.1.1", "b.echo.example.org"),
        ],
        vec![
            leak_observation("a.echo.example.org", "203.0.113.9"),
            leak_observation("b.echo.example.org", "198.51.100.7"),
        ],
    );
    let (divergent_zh, _) = leak_conclusion_copy(&divergent, &zh);
    assert!(
        divergent_zh.contains("2 个不同解析器身份"),
        "{divergent_zh}"
    );
    assert!(divergent_zh.contains("仅列事实"), "{divergent_zh}");
    let (divergent_en, _) = leak_conclusion_copy(&divergent, &en);
    assert!(divergent_en.contains("2 distinct resolver identities"));
    assert_eq!(leak_observation_lines(&divergent, &zh).len(), 2);
    assert_eq!(leak_observation_lines(&divergent, &en).len(), 2);

    // An agreeing report is the only shape that claims agreement.
    let consistent = DnsLeakReport::observed(
        vec![DnsLeakProbeSource::new("1.1.1.1", "a.echo.example.org")],
        vec![
            leak_observation("a.echo.example.org", "203.0.113.9"),
            DnsLeakObservation {
                resolver: "1.1.1.1".to_owned(),
                authority: "b.echo.example.org".to_owned(),
                question: "l7f4.b.echo.example.org".to_owned(),
                transport: DnsLeakProbeTransport::Udp,
                outcome: DnsLeakObservationOutcome::Observed {
                    identity: "203.0.113.9".to_owned(),
                },
            },
        ],
    );
    let (consistent_zh, _) = leak_conclusion_copy(&consistent, &zh);
    assert!(consistent_zh.contains("2 个来源观测到同一解析器身份 203.0.113.9"));

    // Both locales render the panel from the shared report.
    let (mut state, _) = AppState::new();
    state.editor.dns_leak = divergent;
    let _ = leak_panel(&state, &zh);
    let _ = leak_panel(&state, &en);
    state.editor.dns_leak = unsupported;
    let _ = leak_panel(&state, &zh);
    assert!(!state.diag.is_probing_dns_leak);

    // The probe message reaches the shared handler; without a host runtime it
    // is a no-op instead of a fabricated report.
    let _ = state.update(Message::RunDnsLeakProbe);
    assert!(!state.diag.is_probing_dns_leak);
}

#[test]
fn test_dns_leak_panel_renders_the_configured_txt_sources() {
    let zh = Lang("zh-CN");
    let en = Lang("en-US");

    // The two real default TXT authorities, declared with their extraction
    // rules, render their observed resolver identity through the shared panel.
    let report = DnsLeakReport::observed(
        vec![
            DnsLeakProbeSource::exact(
                "system",
                "whoami.ds.akahelp.net",
                DnsLeakEchoRecord::TxtKeyedValue {
                    key: "ip".to_owned(),
                },
            ),
            DnsLeakProbeSource::exact(
                "system",
                "o-o.myaddr.l.google.com",
                DnsLeakEchoRecord::TxtFirstIpAddress,
            ),
        ],
        vec![
            DnsLeakObservation {
                resolver: "system".to_owned(),
                authority: "whoami.ds.akahelp.net".to_owned(),
                question: "whoami.ds.akahelp.net".to_owned(),
                transport: DnsLeakProbeTransport::System,
                outcome: DnsLeakObservationOutcome::Observed {
                    identity: "203.0.113.9".to_owned(),
                },
            },
            DnsLeakObservation {
                resolver: "system".to_owned(),
                authority: "o-o.myaddr.l.google.com".to_owned(),
                question: "o-o.myaddr.l.google.com".to_owned(),
                transport: DnsLeakProbeTransport::System,
                outcome: DnsLeakObservationOutcome::Observed {
                    identity: "203.0.113.9".to_owned(),
                },
            },
        ],
    );
    assert_eq!(report.sources.len(), 2);
    let lines = leak_observation_lines(&report, &zh);
    assert_eq!(lines.len(), 2);
    assert!(
        report
            .sources
            .iter()
            .any(|source| source.authority == "whoami.ds.akahelp.net")
    );
    assert!(
        report
            .sources
            .iter()
            .any(|source| source.authority == "o-o.myaddr.l.google.com")
    );

    // The two agreeing TXT authorities are the only shape that claims
    // agreement, localized in both locales.
    let (label_zh, _) = leak_conclusion_copy(&report, &zh);
    assert!(
        label_zh.contains("2 个来源观测到同一解析器身份 203.0.113.9"),
        "{label_zh}"
    );
    let (label_en, _) = leak_conclusion_copy(&report, &en);
    assert!(label_en.contains("2 sources observed"), "{label_en}");

    let (mut state, _) = AppState::new();
    state.editor.dns_leak = report;
    let _ = leak_panel(&state, &zh);
    let _ = leak_panel(&state, &en);
}

#[test]
fn test_dns_leak_observation_outcome_copy_stays_typed() {
    let zh = Lang("zh-CN");
    let en = Lang("en-US");
    let report = DnsLeakReport::observed(
        vec![DnsLeakProbeSource::new("1.1.1.1", "a.echo.example.org")],
        vec![
            DnsLeakObservation {
                resolver: "1.1.1.1".to_owned(),
                authority: "a.echo.example.org".to_owned(),
                question: "l1.a.echo.example.org".to_owned(),
                transport: DnsLeakProbeTransport::Udp,
                outcome: DnsLeakObservationOutcome::TimedOut,
            },
            DnsLeakObservation {
                resolver: "tls://1.0.0.1:853".to_owned(),
                authority: "a.echo.example.org".to_owned(),
                question: "l2.a.echo.example.org".to_owned(),
                transport: DnsLeakProbeTransport::Undrivable {
                    reason: "DNS over TLS is not probed by this host".to_owned(),
                },
                outcome: DnsLeakObservationOutcome::NotProbed {
                    reason: "DNS over TLS is not probed by this host".to_owned(),
                },
            },
        ],
    );
    let zh_lines = leak_observation_lines(&report, &zh);
    let en_lines = leak_observation_lines(&report, &en);
    assert_eq!(zh_lines.len(), 2);
    assert_eq!(en_lines.len(), 2);
    // Two sources, no observation: a typed failure, not an agreement.
    let (label, _) = leak_conclusion_copy(&report, &zh);
    assert!(label.contains("探测源全部失败"), "{label}");
}
