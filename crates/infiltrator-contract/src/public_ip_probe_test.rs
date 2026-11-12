use super::*;

#[test]
fn typed_probe_state_rejects_stale_answers_and_duplicate_refresh() {
    let first = stamp(7);
    let state = PublicIpProbeState::default()
        .begin(first)
        .expect("idle starts a probe");
    assert_eq!(state.phase(), PublicIpProbePhase::Probing);
    assert_eq!(
        state.clone().begin(stamp(8)),
        Err(PublicIpProbeError::AlreadyProbing)
    );

    let stale = PublicIpProbeAnswer::Succeeded {
        stamp: stamp(6),
        observation: observation(),
    };
    assert_eq!(
        state.clone().accept(stale),
        Err(PublicIpProbeError::StaleAnswer)
    );

    let answer = PublicIpProbeAnswer::Succeeded {
        stamp: first,
        observation: observation(),
    };
    let settled = state.accept(answer).expect("matching stamp settles");
    assert_eq!(settled.phase(), PublicIpProbePhase::Succeeded);
    assert!(settled.phase().is_terminal());
}

#[test]
fn a_route_move_is_a_different_stamp() {
    let mut direct = stamp(3);
    direct.route = PublicIpProbeRouteStamp::Direct;
    let mut proxied = direct;
    proxied.route = PublicIpProbeRouteStamp::ThroughRuntimeHttpProxy;
    assert_ne!(direct, proxied);

    let state = PublicIpProbeState::Probing { stamp: direct };
    let late = PublicIpProbeAnswer::Failed {
        stamp: proxied,
        reason: "old proxy route".to_owned(),
    };
    assert_eq!(state.accept(late), Err(PublicIpProbeError::StaleAnswer));
}

#[test]
fn a_failed_answer_is_a_terminal_typed_state() {
    let stamp = stamp(5);
    let state = PublicIpProbeState::Probing { stamp };
    let settled = state
        .accept(PublicIpProbeAnswer::Failed {
            stamp,
            reason: "network unreachable".to_owned(),
        })
        .expect("typed failure settles");
    assert_eq!(settled.phase(), PublicIpProbePhase::Failed);
    assert!(settled.phase().is_terminal());
    assert!(matches!(
        settled.accept(PublicIpProbeAnswer::Failed {
            stamp,
            reason: "late duplicate".to_owned(),
        }),
        Err(PublicIpProbeError::NotProbing)
    ));
}

#[test]
fn sealed_state_round_trips_through_serde() {
    let state = PublicIpProbeState::Succeeded {
        stamp: stamp(11),
        observation: observation(),
    };
    let encoded = serde_json::to_string(&state).expect("serialize state");
    assert_eq!(
        serde_json::from_str::<PublicIpProbeState>(&encoded).expect("deserialize state"),
        state
    );
}

fn stamp(request_id: u64) -> PublicIpProbeStamp {
    PublicIpProbeStamp {
        request_id,
        core_generation: 3,
        revision: 42,
        route: PublicIpProbeRouteStamp::Direct,
    }
}

fn observation() -> PublicIpObservation {
    PublicIpObservation {
        ip: "203.0.113.7".to_owned(),
        country: Some("HK".to_owned()),
        region: None,
        city: Some("Hong Kong".to_owned()),
        provider: "api.ipify.org".to_owned(),
        checked_at_epoch_ms: 1_741_300_000_000,
    }
}
