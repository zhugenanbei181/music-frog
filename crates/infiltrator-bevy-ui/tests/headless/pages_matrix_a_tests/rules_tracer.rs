//! Behavior cases for rules tracer.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use infiltrator_bevy_ui::pages::rules_tabs::RulesTabChip;
use infiltrator_bevy_ui::pages::rules_tracer_confirm::TraceConfirmationAction;
use infiltrator_contract::rule_trace_facts::DecisionStageFacts;
use infiltrator_contract::rule_trace_run::{RuleTraceOperationId, RuleTraceRequest};
use infiltrator_contract::rule_tracer::TrafficContextSnapshot;
use tokio::runtime::Builder;

fn open_tracer(app: &mut App) {
    let entity = app
        .world_mut()
        .query::<(Entity, &RulesTabChip)>()
        .iter(app.world())
        .find(|(_, chip)| chip.0 == 3)
        .expect("actual Tracer tab")
        .0;
    app.world_mut().commands().trigger(Activate { entity });
    app.update();
}

#[test]
fn test_rules_tracer_projection_renders_shared_decision_chain() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Rules);
    open_tracer(&mut app);

    // The demo projection carries the shared demo_fixture chain, so the
    // tracer card must replay that exact decision chain — five stages, the
    // matched rule headline and the resolved outbound node.
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(RulesProjection::demo()));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "命中规则 #42：DOMAIN-SUFFIX,github.com,PROXY → PROXY"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "来源 127.0.0.1:58421 · 监听端口 7890"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "#42 · DOMAIN-SUFFIX,github.com,PROXY → PROXY"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "节点 香港 IPLC 01 · 协议 VLESS · Reality"
    ));
}

#[test]
fn test_rules_tracer_source_ip_sandbox_submits_shared_context() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Rules);
    open_tracer(&mut app);

    // The shared snapshot's Inbound stage reflects the simulated source IP;
    // the card must render that exact shared decision-chain line.
    let mut projection = RulesProjection::demo();
    if let Some(chain) = projection.tracer.decision_chain.as_mut() {
        let Some(DecisionStageFacts::Inbound(context)) = chain.nodes[0].facts.as_mut() else {
            panic!("inbound facts");
        };
        context.src_ip = Some("10.20.30.40".to_owned());
    }
    projection.tracer.simulated_context.src_ip = Some("10.20.30.40".to_owned());
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "来源 10.20.30.40:58421 · 监听端口 7890"
    ));

    // Both sandbox inputs mount: the target query and the source-IP field.
    let query_source = {
        let mut fields = app.world_mut().query::<(&TracerQueryField, &Children)>();
        *fields
            .single(app.world())
            .expect("query field wrapper")
            .1
            .iter()
            .next()
            .expect("query text field")
    };
    let src_source = {
        let mut fields = app.world_mut().query::<(&TracerSourceIpField, &Children)>();
        *fields
            .single(app.world())
            .expect("source ip field wrapper")
            .1
            .iter()
            .next()
            .expect("source ip text field")
    };
    app.world_mut()
        .get_mut::<TextField>(query_source)
        .expect("query field state")
        .0
        .apply(TextFieldInput::SetText("google.com".to_owned()));
    app.world_mut()
        .get_mut::<TextField>(src_source)
        .expect("source ip field state")
        .0
        .apply(TextFieldInput::SetText("10.20.30.40".to_owned()));

    // The simulate button submits the shared sandbox context, then the query;
    // no UI-local trace is fabricated.
    let simulate = app
        .world_mut()
        .query_filtered::<Entity, With<SimulateRuleTraceButton>>()
        .single(app.world())
        .expect("simulate button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: simulate });
    app.update();

    let submitted = sink.submitted();
    assert_eq!(submitted.len(), 1);
    let UiCommand::SimulateRuleTrace { operation, request } = &submitted[0] else {
        panic!("Expected one atomic simulation request");
    };
    assert!(operation.0 > 0);
    assert_eq!(request.query, "google.com");
    assert_eq!(request.context.src_ip.as_deref(), Some("10.20.30.40"));
}

#[test]
fn test_rules_tracer_override_submits_and_consumes_shared_result() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Rules);
    open_tracer(&mut app);

    // The demo fixture matched rule #42 (0-based index 41) with target PROXY,
    // so the shared `can_reverse_apply` fact mounts the chooser.
    let mut rules: Vec<RuleEntry> = (0..42)
        .map(|index| RuleEntry {
            rule: format!("DOMAIN-SUFFIX,rule{index}.com,PROXY"),
            enabled: true,
        })
        .collect();
    rules[41] = RuleEntry {
        rule: "DOMAIN-SUFFIX,github.com,PROXY".to_owned(),
        enabled: true,
    };
    let mut projection = RulesProjection::demo();
    projection.tracer.source = Some(tracer_test_workspace(&rules).source);
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection.clone()));
    app.update();

    let override_source = {
        let mut fields = app
            .world_mut()
            .query::<(&TracerOverrideTargetField, &Children)>();
        *fields
            .single(app.world())
            .expect("override target field wrapper")
            .1
            .iter()
            .next()
            .expect("override text field")
    };
    // The field seeds from the shared suggested target; the user overrides it.
    assert_eq!(
        app.world()
            .get::<TextField>(override_source)
            .expect("override field state")
            .0
            .text(),
        "DIRECT"
    );
    app.world_mut()
        .get_mut::<TextField>(override_source)
        .expect("override field state")
        .0
        .apply(TextFieldInput::SetText("REJECT".to_owned()));

    let button = app
        .world_mut()
        .query_filtered::<Entity, With<ApplyTracerRuleOverrideButton>>()
        .single(app.world())
        .expect("apply override button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    assert!(
        sink.submitted().is_empty(),
        "first click only opens confirmation"
    );
    let confirm = app
        .world_mut()
        .query::<(Entity, &TraceConfirmationAction)>()
        .iter(app.world())
        .find(|(_, action)| matches!(action, TraceConfirmationAction::Apply))
        .expect("confirm button")
        .0;
    app.world_mut()
        .commands()
        .trigger(Activate { entity: confirm });
    app.update();
    let expected = UiCommand::ApplyTracerRuleOverride {
        request: TracerRuleOverride {
            rule_index: 41,
            rule_table: None,
            new_target: "REJECT".to_owned(),
            expected_source: projection.tracer.source.clone().expect("source identity"),
            expected_rule: "DOMAIN-SUFFIX,github.com,PROXY".to_owned(),
        },
    };
    assert_eq!(sink.submitted().last(), Some(&expected));

    // Drive the exact submitted intent through the shared application with a
    // composed override port and consume the one typed result.
    let intent = expected.to_intent().expect("override intent");
    let request = match intent {
        CommandIntent::ApplyTracerRuleOverride { request } => request,
        other => panic!("unexpected intent: {other:?}"),
    };
    let port = Arc::new(FakeRuleOverridePort {
        rules: Mutex::new(rules),
    });
    let application = RuleTracerApplication::new();
    application.set_override_port(port);
    let result = Builder::new_current_thread()
        .build()
        .expect("test runtime")
        .block_on(application.apply_override(&request));
    assert!(result.is_applied());
    assert_eq!(
        result.updated_rule_raw.as_deref(),
        Some("DOMAIN-SUFFIX,github.com,REJECT")
    );

    // Replay a fresh simulation read from the same persisted host, not a manually patched chain.
    Builder::new_current_thread()
        .build()
        .expect("test runtime")
        .block_on(application.simulate(
            RuleTraceOperationId(1),
            RuleTraceRequest {
                query: "github.com".into(),
                context: TrafficContextSnapshot::default(),
            },
            None,
        ))
        .expect("post-apply simulation");
    projection.tracer = application
        .execution()
        .report
        .expect("new source-bound report");
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "DOMAIN-SUFFIX,github.com,REJECT → REJECT"
    ));
}
