//! SHOT-OV-07: Overview banner five-state matrix behavior tests.
//!
//! Validates:
//! - State 1: Demo mode (Demo origin) -> "演示模式" / "Demo Mode", neutral accent dot, disabled start/stop.
//! - State 2: Missing host (Failed) -> "运行出错" / "Failed", danger dot and fill.
//! - State 3: Core stopped -> "已停止" / "Stopped", ink_dim dot, start action.
//! - State 4: VPN unauthorized / untaken -> "VPN 未授权" / "内核运行 · 未接管", warning dot (never green!), stop action.
//! - State 5: Real running -> "运行中" / "Running", success green dot, stop action.

use super::*;
use infiltrator_bevy_ui::pages::overview::{
    banner_state_text, card_fill, state_ink, status_dot_color,
};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_contract::error::Failure;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::system_toggle::{SystemToggleSnapshot, SystemToggleState};

fn make_projection(
    origin: OverviewOrigin,
    lifecycle: CoreLifecycle,
    proxy_enabled: bool,
    tun_state: SystemToggleState,
) -> OverviewProjection {
    let mut projection = DemoOverviewSource::running().current();
    projection.origin = origin;
    projection.lifecycle = lifecycle;
    projection.system_toggles = SystemToggleSnapshot {
        system_proxy: SystemToggleState::from_enabled(proxy_enabled),
        tun: tun_state,
        revision: 1,
    };
    projection
}

#[test]
fn five_states_banner_verdict_matrix() {
    let palette = UiPalette::new(&Theme::dark());

    // 1. Demo Mode
    let demo = make_projection(
        OverviewOrigin::Demo,
        CoreLifecycle::Running,
        true,
        SystemToggleState::Enabled,
    );
    assert_eq!(banner_state_text(&demo, "zh-CN"), "演示模式");
    assert_eq!(banner_state_text(&demo, "en-US"), "Demo Mode");
    assert_eq!(
        status_dot_color(Some(&demo), &palette),
        palette.accent,
        "demo dot must use neutral accent, never green success"
    );
    assert_ne!(
        status_dot_color(Some(&demo), &palette),
        palette.success,
        "demo dot must never be green"
    );
    assert_eq!(
        card_fill(Some(&demo), &demo.lifecycle, &palette),
        palette.accent_container
    );
    assert_eq!(
        state_ink(Some(&demo), &demo.lifecycle, &palette),
        palette.ink
    );

    // 2. Missing Host / Failed
    let missing = make_projection(
        OverviewOrigin::LiveCore,
        CoreLifecycle::Failed,
        false,
        SystemToggleState::Disabled,
    );
    assert_eq!(banner_state_text(&missing, "zh-CN"), "运行出错");
    assert_eq!(banner_state_text(&missing, "en-US"), "Runtime Error");
    assert_eq!(status_dot_color(Some(&missing), &palette), palette.danger);
    assert_eq!(
        card_fill(Some(&missing), &missing.lifecycle, &palette),
        palette.danger
    );
    assert_eq!(
        state_ink(Some(&missing), &missing.lifecycle, &palette),
        palette.on_accent
    );

    // 3. Core Stopped
    let stopped = make_projection(
        OverviewOrigin::LiveCore,
        CoreLifecycle::Stopped,
        false,
        SystemToggleState::Disabled,
    );
    assert_eq!(banner_state_text(&stopped, "zh-CN"), "已停止");
    assert_eq!(banner_state_text(&stopped, "en-US"), "Stopped");
    assert_eq!(status_dot_color(Some(&stopped), &palette), palette.ink_dim);
    assert_eq!(
        card_fill(Some(&stopped), &stopped.lifecycle, &palette),
        palette.accent_container
    );
    assert_eq!(
        state_ink(Some(&stopped), &stopped.lifecycle, &palette),
        palette.ink_dim
    );

    // 4. VPN Unauthorized / Untaken (Core running, but takeover failed/unauthorized)
    let unauthorized = make_projection(
        OverviewOrigin::LiveCore,
        CoreLifecycle::Running,
        false,
        SystemToggleState::Failed {
            failure: Failure::unsupported("VPN permission denied"),
        },
    );
    assert_eq!(banner_state_text(&unauthorized, "zh-CN"), "VPN 未授权");
    assert_eq!(
        banner_state_text(&unauthorized, "en-US"),
        "VPN Unauthorized"
    );
    assert_eq!(
        status_dot_color(Some(&unauthorized), &palette),
        palette.warning,
        "unauthorized dot must be warning, never green"
    );
    assert_ne!(
        status_dot_color(Some(&unauthorized), &palette),
        palette.success
    );
    assert_eq!(
        card_fill(Some(&unauthorized), &unauthorized.lifecycle, &palette),
        palette.accent_container
    );
    assert_eq!(
        state_ink(Some(&unauthorized), &unauthorized.lifecycle, &palette),
        palette.warning
    );

    // 4b. Core Running Untaken (neither proxy nor TUN enabled)
    let untaken = make_projection(
        OverviewOrigin::LiveCore,
        CoreLifecycle::Running,
        false,
        SystemToggleState::Disabled,
    );
    assert_eq!(banner_state_text(&untaken, "zh-CN"), "内核运行 · 未接管");
    assert_eq!(
        banner_state_text(&untaken, "en-US"),
        "Core Running · Untaken"
    );
    assert_eq!(
        status_dot_color(Some(&untaken), &palette),
        palette.warning,
        "untaken dot must be warning, never green"
    );
    assert_ne!(status_dot_color(Some(&untaken), &palette), palette.success);

    // 5. Real Running (Core running AND proxy/VPN active)
    let real_running = make_projection(
        OverviewOrigin::LiveCore,
        CoreLifecycle::Running,
        true,
        SystemToggleState::Enabled,
    );
    assert_eq!(banner_state_text(&real_running, "zh-CN"), "运行中");
    assert_eq!(banner_state_text(&real_running, "en-US"), "Running");
    assert_eq!(
        status_dot_color(Some(&real_running), &palette),
        palette.success,
        "real running must show genuine green dot"
    );
    assert_eq!(
        card_fill(Some(&real_running), &real_running.lifecycle, &palette),
        palette.accent_container
    );
    assert_eq!(
        state_ink(Some(&real_running), &real_running.lifecycle, &palette),
        palette.ink
    );
}

#[test]
fn demo_mode_disables_lifecycle_control_action() {
    let mut app = mounted_default();
    let world = app.world_mut();
    let mut query = world.query::<(&CoreControlButton, &ButtonDisabled)>();
    let (_, disabled) = query.iter(world).next().expect("CoreControlButton mounted");
    assert!(disabled.0, "Demo mode must keep start/stop button disabled");
}
