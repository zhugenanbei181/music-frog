use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::color::Color;
use bevy::math::{Vec2, Vec3};
use infiltrator_bevy_widgets::haptics::{
    AdsrEnvelope, AudioHapticOutput, AudioHapticsSettings, EmittedFeedback, FeedbackDispatcher,
    FeedbackIntent, FeedbackTriggerEvent, HapticPattern, MAX_TONE_FRAMES, PcmBuffer,
    ProceduralTone,
};
use infiltrator_bevy_widgets::particle::{GreatCircleArc, ParticleEmitter, TrafficParticle};
use infiltrator_bevy_widgets::reactive::{
    ReactiveDagResource, ReactiveEvaluationStats, ReactiveGraphPlugin, SignalInput, SignalOutput,
};
use infiltrator_bevy_widgets::shader_fx::{KawasePassMetrics, OklchColor, SdfRoundedBox};
use infiltrator_bevy_widgets::signal_dag::{PluginWidgetAst, ReactiveDag};
use infiltrator_bevy_widgets::tsdb::{
    MultiTierTsdb, TelemetrySample, TimeTravelScrubber, compute_network_health_score,
};
use std::sync::Mutex;

#[test]
fn test_shader_fx_sdf_rounded_box_and_oklch_ladder() {
    let box_sdf = SdfRoundedBox::new(Vec2::new(100.0, 50.0), 8.0, 1.0);
    assert!(box_sdf.distance_at(Vec2::ZERO) < 0.0);
    assert!(box_sdf.distance_at(Vec2::new(60.0, 0.0)) > 0.0);
    assert_eq!(box_sdf.coverage_at(Vec2::ZERO, 1.0), 1.0);

    let passes = KawasePassMetrics::compute_passes(3);
    assert_eq!(passes.len(), 3);
    assert_eq!(passes[0].downscale_factor, 1.0);
    assert_eq!(passes[2].downscale_factor, 4.0);

    let oklch = OklchColor::new(0.6, 0.2, 240.0);
    let ladder = oklch.generate_tonal_ladder(5);
    assert_eq!(ladder.len(), 5);
}

#[test]
fn test_traffic_particle_and_great_circle_navigation() {
    let mut emitter = ParticleEmitter::new(100);
    let p = TrafficParticle::new(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), 1.0, Color::WHITE);
    assert!(emitter.emit(p));
    assert_eq!(emitter.count(), 1);

    emitter.update(0.5);
    assert_eq!(emitter.count(), 1);
    assert!(emitter.particles[0].position.x > 0.0);

    emitter.update(0.6); // Exceeds lifetime 1.0s
    assert_eq!(emitter.count(), 0);

    // Great circle arc
    let arc = GreatCircleArc::new(Vec2::new(39.9, 116.4), Vec2::new(37.7, -122.4), 100.0);
    let p_mid = arc.sample_point(0.5, 0.2);
    assert!(p_mid.length() > 100.0); // Elevated altitude
}

#[test]
fn test_multitier_tsdb_and_time_travel_scrubber() {
    let mut tsdb = MultiTierTsdb::new(60);
    for i in 0..25 {
        tsdb.push(TelemetrySample {
            timestamp_sec: 1000 + i,
            upload_bytes: 1024,
            download_bytes: 4096,
            active_connections: 5,
            latency_ms: 35.0,
        });
    }

    assert_eq!(tsdb.tier_1s.len(), 25);
    assert_eq!(tsdb.tier_10s.len(), 2); // 25 / 10 = 2 aggregated chunks

    let query_sample = tsdb.query_at(1015).expect("sample found");
    assert_eq!(query_sample.timestamp_sec, 1015);

    let mut scrubber = TimeTravelScrubber::new(1000, 2000);
    scrubber.scrub_to_fraction(0.5);
    assert_eq!(scrubber.current_timestamp, 1500);
    assert_eq!(scrubber.fraction(), 0.5);

    let score = compute_network_health_score(20.0, 0.0, 2.0);
    assert!(score > 90.0);
}

#[test]
fn test_haptics_and_procedural_tone_generator() {
    assert_eq!(HapticPattern::LightTick.duration_ms(), 15);
    assert_eq!(HapticPattern::HeavyThud.duration_ms(), 60);

    let tone = ProceduralTone::click();
    assert_eq!(tone.frequency_hz, 880.0);
    let sample = tone.sample_at(0.005);
    assert!(sample.abs() <= 1.0);

    let chime = ProceduralTone::success_chime();
    assert_eq!(chime.frequency_hz, 523.25);
}

#[test]
fn test_adsr_envelope_shape() {
    let env = AdsrEnvelope::new(0.01, 0.02, 0.5, 0.03);
    // Before the note starts the envelope is silent.
    assert_eq!(env.evaluate(-0.1, 0.1), 0.0);
    assert_eq!(env.evaluate(0.0, 0.1), 0.0);
    // Attack ramps linearly to 1.0.
    assert!((env.evaluate(0.005, 0.1) - 0.5).abs() < 1e-4);
    assert_eq!(env.evaluate(0.01, 0.1), 1.0);
    // Decay falls from 1.0 toward the sustain level.
    assert!((env.evaluate(0.02, 0.1) - 0.75).abs() < 1e-4);
    assert!((env.evaluate(0.03, 0.1) - 0.5).abs() < 1e-4);
    // Sustain holds flat until the note duration.
    assert_eq!(env.evaluate(0.08, 0.1), 0.5);
    // Release ramps down to silence.
    assert!((env.evaluate(0.115, 0.1) - 0.25).abs() < 1e-4);
    assert_eq!(env.evaluate(0.14, 0.1), 0.0);
}

#[test]
fn test_pcm_frame_count_and_duration_math() {
    let tone = ProceduralTone {
        frequency_hz: 440.0,
        duration_secs: 0.1,
        envelope: AdsrEnvelope::new(0.005, 0.01, 0.5, 0.0),
    };
    assert!((tone.total_duration_secs() - 0.1).abs() < 1e-6);
    assert_eq!(tone.frame_count(1_000), 100);
    assert_eq!(tone.frame_count(0), 0);

    let pcm = tone.render_pcm(1_000);
    assert_eq!(pcm.len(), 100);
    assert_eq!(pcm.sample_rate_hz, 1_000);
    assert!((pcm.duration_secs() - 0.1).abs() < 1e-3);
    assert!(
        pcm.frames
            .iter()
            .all(|sample| (-1.0..=1.0).contains(sample))
    );

    // A release tail extends the rendered buffer.
    let tail = ProceduralTone {
        frequency_hz: 440.0,
        duration_secs: 0.1,
        envelope: AdsrEnvelope::new(0.005, 0.01, 0.5, 0.05),
    };
    assert_eq!(tail.frame_count(1_000), 150);
    assert_eq!(tail.render_pcm(1_000).len(), 150);

    // Synthesis stays bounded even for an absurd duration.
    let huge = ProceduralTone {
        frequency_hz: 440.0,
        duration_secs: 10_000.0,
        envelope: AdsrEnvelope::new(0.0, 0.0, 1.0, 0.0),
    };
    assert_eq!(huge.frame_count(48_000), MAX_TONE_FRAMES);
    assert!(huge.render_pcm(48_000).len() <= MAX_TONE_FRAMES);

    // A zero sample rate is a typed empty buffer, never a divide-by-zero.
    let silent = tone.render_pcm(0);
    assert_eq!(silent.sample_rate_hz, 0);
    assert!(silent.is_empty());
    assert_eq!(silent.duration_secs(), 0.0);
}

#[derive(Default)]
struct RecordingOutput {
    vibrations: Mutex<Vec<u32>>,
    tones: Mutex<Vec<usize>>,
}

impl AudioHapticOutput for RecordingOutput {
    fn vibrate(&self, duration_ms: u32) {
        self.vibrations.lock().unwrap().push(duration_ms);
    }

    fn play_tone(&self, pcm: &PcmBuffer) {
        self.tones.lock().unwrap().push(pcm.len());
    }
}

#[test]
fn test_global_mute_blocks_all_output() {
    let output = RecordingOutput::default();
    let intent = FeedbackIntent::for_pattern(HapticPattern::SuccessPulse);

    // Unmuted: both channels fire and reach the port.
    let dispatcher =
        FeedbackDispatcher::new(&output, AudioHapticsSettings::default()).with_sample_rate(1_000);
    let emitted = dispatcher.dispatch(intent);
    assert_eq!(
        emitted,
        EmittedFeedback {
            haptic: true,
            audio: true
        }
    );
    assert_eq!(*output.vibrations.lock().unwrap(), vec![80]);
    assert_eq!(output.tones.lock().unwrap().len(), 1);

    // The global gate overrides still-enabled per-channel flags.
    let mut settings = AudioHapticsSettings::default();
    settings.set_muted(true);
    assert!(settings.is_muted());
    assert!(!settings.allows_haptics());
    assert!(!settings.allows_sound());

    let dispatcher = FeedbackDispatcher::new(&output, settings).with_sample_rate(1_000);
    let emitted = dispatcher.dispatch(intent);
    assert!(!emitted.haptic && !emitted.audio);
    // Nothing new reached the port.
    assert_eq!(output.vibrations.lock().unwrap().len(), 1);
    assert_eq!(output.tones.lock().unwrap().len(), 1);
}

#[test]
fn test_combined_feedback_intent_mapping() {
    let success = FeedbackIntent::for_pattern(HapticPattern::SuccessPulse);
    assert_eq!(success.duration_ms(), 80);
    assert!(success.tone.is_some());

    let thud = FeedbackIntent::for_pattern(HapticPattern::HeavyThud);
    assert!(thud.tone.is_none());

    let explicit = FeedbackIntent::with_tone(HapticPattern::MediumClick, ProceduralTone::click());
    assert_eq!(explicit.duration_ms(), 30);
    assert!(explicit.tone.is_some());

    let haptic_only = FeedbackIntent::haptic_only(HapticPattern::MediumClick);
    assert!(haptic_only.tone.is_none());

    // The legacy event maps onto the same combined intent.
    let silent_event = FeedbackTriggerEvent {
        pattern: HapticPattern::WarningDoublePulse,
        play_sound: false,
    };
    let silent_intent = silent_event.to_intent();
    assert_eq!(silent_intent.pattern, HapticPattern::WarningDoublePulse);
    assert!(silent_intent.tone.is_none());

    let audible_event = FeedbackTriggerEvent {
        pattern: HapticPattern::WarningDoublePulse,
        play_sound: true,
    };
    assert!(audible_event.to_intent().tone.is_some());
}

#[test]
fn test_reactive_signal_dag_and_plugin_ast_sanitizer() {
    let mut dag = ReactiveDag::new();
    let s1 = dag.create_signal(10);
    let s2 = dag.create_signal(20);
    let d1 = dag.create_derived(&[s1, s2], 30);

    let dirty = dag.update_signal(s1, 15);
    assert_eq!(dirty, vec![d1]);
    assert_eq!(dag.get_value(s1), Some(15));

    // Plugin AST Sanitization
    let safe_ast = PluginWidgetAst::Container {
        direction_column: true,
        children: vec![
            PluginWidgetAst::Label {
                text: "Safe Label".to_string(),
                is_bold: true,
            },
            PluginWidgetAst::StatCard {
                title: "Ping".to_string(),
                value: "12ms".to_string(),
            },
        ],
    };
    assert!(safe_ast.validate_and_sanitize(10));
    assert!(!safe_ast.validate_and_sanitize(0)); // Exceeds max depth 0
}

fn reactive_bridge_app(dag: ReactiveDag) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(ReactiveDagResource::from(dag));
    app.add_plugins(ReactiveGraphPlugin);
    app
}

#[test]
fn test_reactive_dag_ecs_change_recomputes_only_the_dependent_subtree() {
    let mut dag = ReactiveDag::new();
    let source_a = dag.create_signal(1);
    let source_b = dag.create_signal(1);
    let branch_a = dag
        .create_computed(&[source_a], |values| values[0] + 10)
        .unwrap();
    let branch_b = dag
        .create_computed(&[source_b], |values| values[0] + 20)
        .unwrap();
    let leaf = dag
        .create_computed(&[branch_a], |values| values[0] * 2)
        .unwrap();

    let mut app = reactive_bridge_app(dag);
    let input_a = app.world_mut().spawn(SignalInput::new(source_a, 1)).id();
    app.world_mut().spawn(SignalInput::new(source_b, 1));
    let output_leaf = app.world_mut().spawn(SignalOutput::new(leaf)).id();
    let output_b = app.world_mut().spawn(SignalOutput::new(branch_b)).id();

    app.update();
    assert_eq!(
        app.world().get::<SignalOutput>(output_leaf).unwrap().value,
        22
    );
    assert_eq!(app.world().get::<SignalOutput>(output_b).unwrap().value, 21);

    app.world_mut()
        .get_mut::<SignalInput>(input_a)
        .unwrap()
        .value = 5;
    app.update();

    let stats = app.world().resource::<ReactiveEvaluationStats>();
    assert_eq!(stats.last_recomputed, vec![branch_a, leaf]);
    assert!(!stats.last_recomputed.contains(&branch_b));
    assert_eq!(
        app.world().get::<SignalOutput>(output_leaf).unwrap().value,
        30
    );
    assert_eq!(app.world().get::<SignalOutput>(output_b).unwrap().value, 21);
}

#[test]
fn test_reactive_dag_ecs_idle_frame_recomputes_nothing() {
    let mut dag = ReactiveDag::new();
    let source = dag.create_signal(1);
    let derived = dag
        .create_computed(&[source], |values| values[0] + 1)
        .unwrap();

    let mut app = reactive_bridge_app(dag);
    app.world_mut().spawn(SignalInput::new(source, 1));
    let output = app.world_mut().spawn(SignalOutput::new(derived)).id();

    app.update();
    let baseline = app
        .world()
        .resource::<ReactiveEvaluationStats>()
        .total_recomputed;
    assert!(baseline >= 1);

    app.update();
    let stats = app.world().resource::<ReactiveEvaluationStats>();
    assert!(stats.last_recomputed.is_empty());
    assert_eq!(stats.total_recomputed, baseline);
    assert_eq!(app.world().get::<SignalOutput>(output).unwrap().value, 2);
}

#[test]
fn test_reactive_dag_ecs_hundred_cycle_churn_stays_bounded() {
    let mut dag = ReactiveDag::new();
    let hot = dag.create_signal(0);
    let mut upstream = hot;
    for _ in 0..100 {
        upstream = dag
            .create_computed(&[upstream], |values| values[0] + 1)
            .unwrap();
    }
    let head = upstream;

    let mut app = reactive_bridge_app(dag);
    let input = app.world_mut().spawn(SignalInput::new(hot, 0)).id();
    let output = app.world_mut().spawn(SignalOutput::new(head)).id();

    for tick in 1..=100i64 {
        app.world_mut().get_mut::<SignalInput>(input).unwrap().value = tick;
        app.update();
        let stats = app.world().resource::<ReactiveEvaluationStats>();
        assert_eq!(stats.last_recomputed.len(), 100);
    }

    let stats = app.world().resource::<ReactiveEvaluationStats>();
    assert_eq!(stats.total_recomputed, 10_000);
    assert_eq!(app.world().get::<SignalOutput>(output).unwrap().value, 200);
}
