use std::{rc::Rc, time::{Duration, Instant}};
use slint::{ComponentHandle, SharedString};
use slint::platform::{Platform, PlatformError, WindowAdapter};
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};

struct SnapshotPlatform { window: Rc<MinimalSoftwareWindow>, start: Instant }
impl Platform for SnapshotPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }
    fn duration_since_start(&self) -> std::time::Duration { self.start.elapsed() }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let page = args.get(1).map(String::as_str).unwrap_or("Proxy");
    let width: u32 = args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(320);
    let height: u32 = args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(720);
    let output = args.get(4).map(String::as_str).unwrap_or("target/ui-preview/systemfont.png");
    let variant = args.get(5).map(String::as_str).unwrap_or("");

    let english_variant = variant.contains("english");
    let _ = slint::select_bundled_translation("zh-CN");

    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(SnapshotPlatform { window: window.clone(), start: Instant::now() }))?;
    let ui = lumen::AppWindow::new()?;
    if english_variant {
        let _ = slint::select_bundled_translation("en");
    }
    ui.global::<lumen::Palette>().set_color_scheme(slint::language::ColorScheme::Light);
    ui.window().set_size(slint::LogicalSize::new(width as f32, height as f32));
    ui.set_compact_layout(true);
    ui.set_android_platform(true);
    let dark_variant = variant == "detail-dark" || variant == "add-profile-dark" || variant == "layout-debug-sheet" || variant.starts_with("app-") && variant.contains("dark");
    ui.set_application_dark_mode(dark_variant);
    ui.set_theme_mode(if dark_variant { "dark" } else { "light" }.into());
    ui.set_page(SharedString::from(page));
    if english_variant {
        ui.set_application_language("en".into());
        ui.set_current_translated_language("en".into());
        ui.set_system_language("en".into());
    }
    let config_sheet_debug = matches!(variant,
        "config-sheet-debug" | "config-action-debug" | "config-share-debug" | "config-delete-debug" | "config-settings-debug" | "config-settings-overrides-debug");
    let proxy_debug_variant = matches!(variant, "proxy-ping-states" | "proxy-ping-loading");
    let debug_variant = variant == "layout-debug" || variant == "layout-debug-sheet" || variant == "home-stopped-debug" || config_sheet_debug || proxy_debug_variant || variant.starts_with("app-") && variant.contains("debug");
    if debug_variant {
        ui.set_layout_debug_tools_enabled(true);
        ui.set_layout_debug_bounds(true);
    }
    ui.set_runtime_status("Running".into());
    ui.set_runtime_detail("核心已就绪 · VPN 已连接".into());
    ui.set_active_profile("家庭订阅 / 海外节点池".into());
    ui.set_has_current_profile(true);
    ui.set_network_mode("VPN".into());
    ui.set_network_mode_status("已连接".into());
    ui.set_route_mode("Rule".into());
    ui.set_stat_down("18.42 MB/s".into());
    ui.set_stat_up("2.72 MB/s".into());
    ui.set_stat_down_value("18.42".into());
    ui.set_stat_down_unit("MB".into());
    ui.set_stat_up_compact("2.72 MB".into());
    ui.set_home_current_node("美国洛杉矶节点".into());
    ui.set_home_current_accessible("🇺🇸 美国洛杉矶节点".into());
    ui.set_home_current_flag_code("us".into());
    ui.set_home_current_flag_index(240);
    ui.set_home_current_delay("38 ms".into());
    ui.set_home_exit_ip("69.63.***.121".into());
    ui.set_home_exit_country_code("US".into());
    ui.set_home_exit_flag_index(240);
    ui.set_home_exit_status("已更新".into());
    ui.set_content_preview_profile("家庭订阅 · 海外线路配置".into());
    ui.set_content_preview(
        "mode: rule\nmixed-port: 7890\nallow-lan: false\nlog-level: info\nexternal-controller: 127.0.0.1:9090\nsecret: ••••••\nproxy-groups:\n  - name: 节点选择\n    type: select\n    proxies:\n      - 🇺🇸 美国洛杉矶节点\n      - 🇯🇵 日本大阪节点\nproxies:\n  - name: 🇺🇸 美国洛杉矶节点\n    type: socks5\n    server: 203.0.113.8\n    port: 443\n    password: ••••••\n".into(),
    );
    let home_traffic_points: Vec<lumen::HomeTrafficPoint> = (0..48)
        .map(|sample| lumen::HomeTrafficPoint {
            download_fraction: (((sample * 17) % 31) + 3) as f32 / 34.0,
            upload_fraction: (((sample * 11) % 23) + 2) as f32 / 34.0,
        })
        .collect();
    ui.set_home_traffic_points(home_traffic_points.as_slice().into());
    ui.set_home_traffic_has_data(true);
    ui.set_stat_connections("128".into());
    ui.set_stat_total_down("28.6 GiB".into());
    ui.set_stat_total_up("4.1 GiB".into());
    ui.set_traffic_today_has_data(true);
    ui.set_traffic_today_down("3.4 GiB".into());
    ui.set_traffic_today_up("612 MiB".into());
    let traffic_bars: Vec<lumen::TrafficBar> = (0..24)
        .map(|hour| lumen::TrafficBar {
            label: format!("{hour:02}").into(),
            detail_label: format!("{hour:02}:00").into(),
            download: "".into(),
            upload: "".into(),
            download_fraction: (((hour * 7) % 17) + 2) as f32 / 18.0,
            upload_fraction: (((hour * 5) % 13) + 1) as f32 / 18.0,
        })
        .collect();
    ui.set_traffic_today_bars(traffic_bars.as_slice().into());
    ui.set_profiles([lumen::ProfileCard {
        id: "demo-profile".into(), name: "家庭订阅 · 海外线路配置".into(), kind: "订阅链接".into(),
        location: "订阅源 / household".into(), updated: "刚刚更新".into(), current: true,
        can_move_up: false, can_move_down: true,
    }].as_slice().into());
    ui.set_selected_group(if variant == "groups" { "" } else { "自动选择 · 长名称宽度检查" }.into());
    ui.set_proxy_groups([
        lumen::ProxyGroup { name: "🚀 节点选择 · 长名称宽度检查".into(), icon_emoji: "".into(), kind: "Selector".into(), current: "美国洛杉矶节点".into(), current_flag_code: "us".into(), current_flag_index: 240, current_accessible: "🇺🇸 美国洛杉矶节点".into(), nodes: "🇺🇸 美国洛杉矶节点、🇯🇵 日本大阪节点".into() },
        lumen::ProxyGroup { name: "AI 服务".into(), icon_emoji: "💬".into(), kind: "Selector".into(), current: "日本大阪节点".into(), current_flag_code: "jp".into(), current_flag_index: 119, current_accessible: "🇯🇵 日本大阪节点".into(), nodes: "🇺🇸 美国洛杉矶节点、🇯🇵 日本大阪节点".into() },
    ].as_slice().into());
    let proxy_nodes = if variant == "proxy-ping-states" {
        vec![
            lumen::ObservationRow { id: "🇺🇸 洛杉矶 未测".into(), title: "洛杉矶 未测".into(), accessible_title: "🇺🇸 洛杉矶 未测".into(), flag_code: "us".into(), flag_index: 240, delay_ms: -1, subtitle: "VLESS".into(), trailing: "Ping".into(), active: false },
            lumen::ObservationRow { id: "🇯🇵 大阪 120ms".into(), title: "大阪 120ms".into(), accessible_title: "🇯🇵 大阪 120ms".into(), flag_code: "jp".into(), flag_index: 119, delay_ms: 120, subtitle: "SS".into(), trailing: "120 ms".into(), active: true },
            lumen::ObservationRow { id: "🇸🇬 新加坡 667ms".into(), title: "新加坡 667ms".into(), accessible_title: "🇸🇬 新加坡 667ms".into(), flag_code: "sg".into(), flag_index: 203, delay_ms: 667, subtitle: "Trojan".into(), trailing: "667 ms".into(), active: false },
            lumen::ObservationRow { id: "🇩🇪 法兰克福 930ms".into(), title: "法兰克福 930ms".into(), accessible_title: "🇩🇪 法兰克福 930ms".into(), flag_code: "de".into(), flag_index: 58, delay_ms: 930, subtitle: "VLESS".into(), trailing: "930 ms".into(), active: false },
            lumen::ObservationRow { id: "🇭🇰 香港 超时".into(), title: "香港 超时".into(), accessible_title: "🇭🇰 香港 超时".into(), flag_code: "hk".into(), flag_index: 99, delay_ms: 0, subtitle: "SS".into(), trailing: "超时".into(), active: false },
        ]
    } else {
        vec![
            lumen::ObservationRow { id: "🇺🇸 美国洛杉矶 · 专线节点 01".into(), title: "美国洛杉矶 · 专线节点 01".into(), accessible_title: "🇺🇸 美国洛杉矶 · 专线节点 01".into(), flag_code: "us".into(), flag_index: 240, delay_ms: 38, subtitle: "VLESS".into(), trailing: "38 ms".into(), active: true },
            lumen::ObservationRow { id: "🇯🇵 日本大阪 · 高速线路节点 02".into(), title: "日本大阪 · 高速线路节点 02".into(), accessible_title: "🇯🇵 日本大阪 · 高速线路节点 02".into(), flag_code: "jp".into(), flag_index: 119, delay_ms: 52, subtitle: "SS".into(), trailing: "52 ms".into(), active: false },
            lumen::ObservationRow { id: "🇸🇬 新加坡 · 长名称和混排 ABC-01".into(), title: "新加坡 · 长名称和混排 ABC-01".into(), accessible_title: "🇸🇬 新加坡 · 长名称和混排 ABC-01".into(), flag_code: "sg".into(), flag_index: 203, delay_ms: 78, subtitle: "Trojan".into(), trailing: "78 ms".into(), active: false },
        ]
    };
    let loading_node_id = "🇺🇸 美国洛杉矶 · 专线节点 01";
    ui.set_proxy_nodes(proxy_nodes.as_slice().into());
    ui.set_proxy_current_node_available(proxy_nodes.iter().any(|node| node.active));
    if variant == "proxy-ping-loading" {
        ui.set_proxy_testing_node(loading_node_id.into());
    }
    if variant == "detail-dark" {
        ui.global::<lumen::Palette>().set_color_scheme(slint::language::ColorScheme::Dark);
    }
    if dark_variant {
        ui.global::<lumen::Palette>().set_color_scheme(slint::language::ColorScheme::Dark);
    }
    // Running alone cannot exercise the distinct HomeActionButton component.
    if variant.starts_with("home-stopped") || variant == "home-empty" {
        ui.set_runtime_status("Stopped".into());
        ui.set_home_current_flag_code("".into());
        ui.set_home_current_flag_index(-1);
        ui.set_home_current_accessible("未知".into());
        ui.set_home_traffic_has_data(false);
        ui.set_home_traffic_points(Vec::<lumen::HomeTrafficPoint>::new().as_slice().into());
        ui.set_home_exit_ip("—".into());
        ui.set_home_exit_country_code("".into());
        ui.set_home_exit_status("".into());
        ui.set_traffic_today_has_data(false);
        if variant == "home-empty" {
            ui.set_profiles(Vec::<lumen::ProfileCard>::new().as_slice().into());
            ui.set_has_current_profile(false);
            ui.set_active_profile("未选择配置".into());
        }
    }
    ui.show()?;
    if variant.starts_with("app-section-") {
        let section = variant.trim_start_matches("app-section-").split('-').next().unwrap_or("0").parse::<i32>()?;
        ui.set_app_settings_preview_section(section);
    }
    if variant.starts_with("app-") {
        ui.set_refresh_current_profile_on_start(true);
        if variant.contains("user-agent") {
            ui.set_app_settings_preview_section(5);
            ui.set_user_agent_draft(if variant.contains("error") { "invalid header".into() } else { "".into() });
            ui.set_user_agent_error(if variant.contains("error") { "User-Agent 格式无效；请删除换行或控制字符".into() } else { "".into() });
            ui.set_user_agent_dialog_open(true);
        } else if variant.contains("theme-color") {
            ui.set_app_settings_preview_section(1);
            ui.set_theme_color_dialog_open(true);
        } else if variant.contains("page-scale") {
            ui.set_app_settings_preview_section(1);
            ui.set_page_scale_dialog_open(true);
        }
        for _ in 0..32 {
            std::thread::sleep(Duration::from_millis(10));
            slint::platform::update_timers_and_animations();
        }
    }
    if variant == "config-action-debug" {
        ui.set_profile_action_id("demo-profile".into());
        ui.set_profile_action_open(true);
    } else if variant == "config-share-debug" {
        ui.set_pending_profile_export_name("家庭订阅 · 海外线路配置".into());
        ui.set_pending_profile_export_id("demo-profile".into());
    } else if variant == "config-delete-debug" {
        ui.set_pending_delete_name("家庭订阅 · 海外线路配置".into());
        ui.set_pending_delete_id("demo-profile".into());
    } else if variant == "config-settings-debug" || variant == "config-settings-overrides-debug" {
        ui.set_profile_settings_name("家庭订阅 · 海外线路配置".into());
        ui.set_profile_settings_kind("订阅链接".into());
        ui.set_profile_settings_location("https://example.invalid/config.yaml".into());
        ui.set_profile_settings_id("demo-profile".into());
        for _ in 0..8 {
            std::thread::sleep(Duration::from_millis(10));
            slint::platform::update_timers_and_animations();
        }
        if variant == "config-settings-overrides-debug" {
            ui.set_profile_settings_type_index(1);
        }
    }
    let sheet_variant = variant.starts_with("add-profile-") || variant == "layout-debug-sheet" || variant == "config-sheet-debug";
    if sheet_variant {
        ui.set_profile_add_open(true);
        for _ in 0..8 {
            std::thread::sleep(Duration::from_millis(10));
            slint::platform::update_timers_and_animations();
        }
    }
    if variant == "proxy-ping-loading" {
        for _ in 0..4 {
            std::thread::sleep(Duration::from_millis(80));
            slint::platform::update_timers_and_animations();
        }
    }
    let snapshot = window.take_snapshot()?;
    let image = image::RgbaImage::from_raw(snapshot.width(), snapshot.height(), snapshot.as_bytes().to_vec())
        .ok_or_else(|| anyhow::anyhow!("snapshot pixel buffer size mismatch"))?;
    image.save(output)?;
    println!("Rendered {page} at {width}x{height} logical px to {output}");
    Ok(())
}
