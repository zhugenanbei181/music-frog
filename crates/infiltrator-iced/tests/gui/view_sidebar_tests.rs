use super::*;

#[test]
fn test_mode_ids() {
    let (mut state, _) = AppState::new();
    state.runtime.script_block_present = false;
    assert_eq!(mode_ids(&state), vec!["rule", "global", "direct"]);

    state.runtime.script_block_present = true;
    assert_eq!(
        mode_ids(&state),
        vec!["rule", "global", "direct", "script"]
    );
}

#[test]
fn test_short_label() {
    assert_eq!(short_label("系统代理 (System Proxy)"), "系统代理");
    assert_eq!(short_label("TUN 模式 (TUN Mode)"), "TUN 模式");
    assert_eq!(short_label("Simple"), "Simple");
}

#[test]
fn test_format_gb() {
    assert_eq!(format_gb(500 * 1024 * 1024), "500 MB");
    assert_eq!(format_gb(1024 * 1024 * 1024), "1.00 GB");
    assert_eq!(format_gb(2560 * 1024 * 1024), "2.50 GB");
}

#[test]
fn test_sidebar_render_smoke() {
    let (state, _) = AppState::new();
    let _elem = sidebar(&state);

    let (mut state2, _) = AppState::new();
    state2.runtime.script_block_present = true;
    state2.runtime.system_proxy_enabled = true;
    state2.runtime.tun_enabled = Some(true);
    state2.shell.current_route = Route::Proxies;
    let _elem_active = sidebar(&state2);
}

#[test]
fn test_sidebar_render_with_traffic_and_samples() {
    let (mut state, _) = AppState::new();
    state.diag.traffic = Some(infiltrator_domain::runtime::TrafficData {
        up: 1024 * 50,
        down: 1024 * 1024 * 2,
    });
    state.diag.traffic_history.push_back((100, 200));
    state.diag.traffic_history.push_back((300, 800));
    state.diag.traffic_history.push_back((500, 1200));

    let _elem = sidebar(&state);
}

#[test]
fn test_sidebar_rail_render_smoke() {
    let (state, _) = AppState::new();
    let _rail = sidebar_rail(&state);
    assert_eq!(RAIL_WIDTH, 64.0);
}
