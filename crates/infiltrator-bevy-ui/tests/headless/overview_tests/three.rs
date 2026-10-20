//! Behavior cases for three.
//! test-intent: behavior

use super::*;

/// Running / Stopped / Unavailable each render a visible projection:
/// state word, state ink token, failure copy and banner fill — all from
/// the same injected seam.
#[test]
fn three_state_projections_are_visible() {
    let palette = UiPalette::new(&Theme::dark());
    for source in [
        DemoOverviewSource::running(),
        DemoOverviewSource::stopped(),
        DemoOverviewSource::unavailable(),
    ] {
        let projection = source.current();
        let state = projection.run_state.clone();
        let mut app = mounted_app_with(source);
        let world = app.world_mut();

        let (_, state_text, ink) = line(world, OverviewLineKind::State);
        assert_eq!(
            state_text,
            match state {
                OverviewRunState::Running => "演示模式",
                OverviewRunState::Stopped => "已停止",
                OverviewRunState::Unavailable { .. } => "运行出错",
            }
        );
        let expected_ink = match state {
            OverviewRunState::Running => palette.ink,
            OverviewRunState::Stopped => palette.ink_dim,
            OverviewRunState::Unavailable { .. } => palette.on_accent,
        };
        assert_eq!(ink.0, expected_ink, "{state:?} ink is the token one");

        let (_, failure_text, _) = line(world, OverviewLineKind::Failure);
        match state {
            OverviewRunState::Unavailable { .. } => {
                assert_eq!(failure_text, DEMO_REASON, "the reason is visible");
            }
            _ => assert_eq!(failure_text, "", "no fabricated status"),
        }

        let (_, fill, stored) = card(world);
        let expected_fill = if matches!(state, OverviewRunState::Unavailable { .. }) {
            palette.danger
        } else {
            palette.accent_container
        };
        assert_eq!(
            fill, expected_fill,
            "{state:?} banner fill is the token one"
        );
        assert_eq!(
            stored, projection.lifecycle,
            "the banner stores its actual lifecycle"
        );

        let (_, upload, _) = line(world, OverviewLineKind::Upload);
        let (_, connections) = chip_value(world, OverviewChipKind::Connections);
        let (_, memory) = chip_value(world, OverviewChipKind::Memory);
        if matches!(state, OverviewRunState::Running) {
            assert_eq!(upload, "↑ 1.40 MB/s");
            assert_eq!(connections, "12");
            assert_eq!(memory, "96.00 MB");
        } else {
            assert_eq!(
                upload, "↑ 未观测",
                "missing traffic is an absent observation"
            );
            assert_eq!(connections, "0");
            assert_eq!(memory, "—", "no memory reading is stated as absent");
        }
    }
}
