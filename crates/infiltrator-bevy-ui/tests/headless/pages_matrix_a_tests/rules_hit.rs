//! Behavior cases for rules hit.
//! test-intent: behavior

use super::*;
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::rules_statistics::{RulesStatisticsState, StatisticsLine};
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_composition::capture_fixtures::list_document;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_presentation::rule_statistics_workbench::StatisticsResetPhase;
use lurker_core_contract::error::ErrorCode;

/// The statistics card publishes the audit's facts as their own metric lines,
/// so the assertion reads the card's line and not one joined string.
fn statistics_line(app: &mut App, wanted: fn(&StatisticsLine) -> bool) -> String {
    app.world_mut()
        .query::<(&StatisticsLine, &Text)>()
        .iter(app.world())
        .find(|(line, _)| wanted(line))
        .map(|(_, text)| text.0.clone())
        .expect("the statistics card owns the line")
}

#[test]
fn test_rules_hit_audit_projection_and_clear_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    app.insert_resource(UiLocale::new("en-US"));
    let (root, _) = navigate_to(&mut app, Route::Rules);

    let dead = RuleDeadEntry {
        rule_index: None,
        rule_raw: "MATCH,DIRECT".to_owned(),
        hit_count: 0,
        reason: RuleDeadReason::ZeroHits,
        shadowed_by: None,
        detail: None,
        last_hit_secs: None,
    };
    let overlap = RuleDeadEntry {
        rule_index: None,
        rule_raw: "IP-CIDR,10.1.2.0/24,PROXY".to_owned(),
        hit_count: 0,
        reason: RuleDeadReason::Shadowed,
        shadowed_by: Some("IP-CIDR,10.0.0.0/8,DIRECT".to_owned()),
        detail: Some("IP CIDR is shadowed by an earlier broader IP-CIDR rule".to_owned()),
        last_hit_secs: None,
    };

    let mut projection = RulesProjection::demo();
    projection.hit_audit = Some(RuleHitAuditSnapshot {
        revision: 1,
        source: Some(RuleSourceIdentity {
            profile: "fixture.yaml".into(),
            document_hash: "explicit-fixture-source".into(),
        }),
        total_hits: 1287,
        tracked_rules: 42,
        top_hits: Vec::new(),
        dead_rules: vec![dead, overlap.clone()],
        cidr_overlaps: vec![overlap],
        last_hit_rule: Some("DOMAIN-SUFFIX,google.com,PROXY".to_owned()),
        last_hit_secs: Some(1_700_000_012),
        can_clear: true,
        trace_count: 128,
        avg_match_latency_us: Some(18.5),
        last_match_latency_us: Some(14),
    });
    // The demo MATCH rule is the last entry; flag it shadowed so the row label
    // must render the shared shadow fact.
    if let Some(rule) = projection.rules.last_mut() {
        rule.is_shadowed = true;
        rule.shadow_reason = Some("unreachable after MATCH".to_owned());
    }

    let mut observed = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    observed.revision += 1;
    let page = observed.pages.rules.data.as_mut().unwrap();
    let mut document = list_document(
        projection
            .rules
            .iter()
            .map(|row| RuleEntry {
                rule: row.raw.clone(),
                enabled: row.is_enabled,
            })
            .collect(),
    );
    document.source = projection
        .hit_audit
        .as_ref()
        .unwrap()
        .source
        .clone()
        .unwrap();
    page.document = Some(document);
    page.hit_audit = projection.hit_audit.clone();
    app.world_mut().trigger(SurfaceSnapshotUpdated(observed));
    app.update();
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();

    // The audit's facts ride their own metric lines: the hits, the
    // zero-hit/shadowed count, the CIDR overlaps and the match latency are
    // four published numbers, not one joined sentence.
    assert_eq!(
        statistics_line(&mut app, |line| matches!(line, StatisticsLine::Total)),
        "1287"
    );
    assert_eq!(
        statistics_line(&mut app, |line| matches!(line, StatisticsLine::Dead)),
        "2"
    );
    assert_eq!(
        statistics_line(&mut app, |line| matches!(line, StatisticsLine::Cidr)),
        "1"
    );
    assert_eq!(
        statistics_line(&mut app, |line| matches!(line, StatisticsLine::Latency)),
        "18.5µs"
    );
    assert!(subtree_has_text(
        app.world(),
        root,
        "56 local trace hits · Shadowed"
    ));

    // The clear button submits the shared reset intent (no UI-local reset).
    let clear_entity = {
        let world = app.world_mut();
        let mut buttons = world.query::<(Entity, &ClearRuleHitCountersButton)>();
        buttons
            .iter(world)
            .next()
            .expect("clear hit counters button mounted")
            .0
    };
    app.world_mut().commands().trigger(Activate {
        entity: clear_entity,
    });
    app.update();

    assert!(
        sink.submitted()
            .iter()
            .any(|command| matches!(command, UiCommand::ClearRuleHitCounters { expected_source } if expected_source.profile == "fixture.yaml" && expected_source.document_hash == "explicit-fixture-source"))
    );
    let (request_id, request) = app
        .world()
        .resource::<RulesStatisticsState>()
        .request
        .clone()
        .unwrap();
    assert!(
        app.world()
            .resource::<RulesStatisticsState>()
            .model
            .reset_request()
            .is_some()
    );
    app.world_mut().trigger(CommandExecutedEvent {
        command: UiCommand::ClearRuleHitCounters {
            expected_source: request.source,
        },
        request_id,
        result: Ok(CommandOutput::Unit),
    });
    app.update();
    let statistics = &app.world().resource::<RulesStatisticsState>().model;
    assert_eq!(statistics.audit.as_ref().unwrap().total_hits, 1287);
    assert_eq!(
        statistics.reset_failure().unwrap().code,
        ErrorCode::InvalidState
    );
    assert!(statistics.reset_request().is_none());
    assert!(!matches!(
        statistics.reset_phase(),
        StatisticsResetPhase::AwaitingReadback { .. }
    ));
    assert!(
        statistics.reset_failure().is_some(),
        "a mock without a typed terminal cannot publish a reset success"
    );
}
