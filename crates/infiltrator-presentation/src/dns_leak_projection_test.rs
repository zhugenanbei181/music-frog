//! test-intent: behavior
use super::*;
use infiltrator_contract::dns_leak::{DnsLeakObservation, DnsLeakProbeTransport};
#[test]
fn opaque_host_parameters_are_interpolated_once_and_unknown_and_unsupported_never_claim_agreement()
{
    let report = DnsLeakReport::unsupported("host {count} {identity}");
    let display = project_leak(&report, "en-US");
    assert_eq!(display.tone, LeakTone::Neutral);
    assert!(display.conclusion.contains("host {count} {identity}"));
    assert!(display.rows.is_empty());
    assert_eq!(display.listing(), "No DNS leak observations yet");
    let unknown = project_leak(&DnsLeakReport::default(), "en-US");
    assert_eq!(unknown.tone, LeakTone::Neutral);
    assert!(unknown.conclusion.contains("fewer than two"));
    let observed = DnsLeakReport::observed(
        Vec::new(),
        vec![DnsLeakObservation {
            resolver: "system".into(),
            authority: "authority {count}".into(),
            question: "question".into(),
            transport: DnsLeakProbeTransport::System,
            outcome: DnsLeakObservationOutcome::Observed {
                identity: "identity {reason}".into(),
            },
        }],
    );
    let display = project_leak(&observed, "en-US");
    assert_eq!(
        display.rows[0].outcome,
        "observed identity: identity {reason}"
    );
    assert!(display.listing().contains("authority {count}"));
    assert_eq!(display.tone, LeakTone::Neutral);
}
