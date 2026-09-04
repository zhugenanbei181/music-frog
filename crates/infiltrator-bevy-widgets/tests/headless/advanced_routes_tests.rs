use bevy::color::Color;
use bevy::math::{Vec2, Vec3};

use infiltrator_bevy_widgets::haptics::{HapticPattern, ProceduralTone};
use infiltrator_bevy_widgets::particle::{GreatCircleArc, ParticleEmitter, TrafficParticle};
use infiltrator_bevy_widgets::shader_fx::{KawasePassMetrics, OklchColor, SdfRoundedBox};
use infiltrator_bevy_widgets::signal_dag::{PluginWidgetAst, ReactiveDag};
use infiltrator_bevy_widgets::tsdb::{
    MultiTierTsdb, TelemetrySample, TimeTravelScrubber, compute_network_health_score,
};

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
