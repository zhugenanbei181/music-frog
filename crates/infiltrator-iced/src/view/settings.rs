use crate::state::AppState;
use crate::types::app::{ConfirmAction, ToastStatus};
use crate::types::message::Message;
use crate::types::runtime::RuntimeStatus;
use crate::view::components::{
    BadgeKind, badge, banner_alert, card, form_field_label, form_input_style, form_pick_style,
    form_toggle_row, icon_button, kbd_badge, modern_scrollable, row_card_surface, section_header,
    segmented_control, status_dot, style_accent, style_danger, style_ghost, text_btn,
};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, R_CONTROL, SP_MD, tokens};
use iced::widget::{
    Space, button, column, container, pick_list, progress_bar, row, text, text_input,
};
use iced::{Alignment, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

#[derive(Clone, Copy, PartialEq, Eq)]
struct SettingsChoice {
    value: &'static str,
}

impl std::fmt::Display for SettingsChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.value)
    }
}

const LANGUAGE_OPTIONS: &[SettingsChoice] = &[SettingsChoice { value: "zh-CN" }, SettingsChoice { value: "en-US" }];
const CORE_CHANNEL_OPTIONS: &[SettingsChoice] = &[
    SettingsChoice { value: "stable" },
    SettingsChoice { value: "beta" },
    SettingsChoice { value: "nightly" },
];

fn secondary_text(value: impl Into<String>) -> Element<'static, Message> {
    text(value.into()).size(12).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }).into()
}

fn shell_export_row<'a>(shell_name: &'static str, command: &'static str, is_en: bool) -> Element<'a, Message> {
    let copy_msg = Message::ShowToast(
        if is_en { format!("Copied {shell_name} command to clipboard") } else { format!("已复制 {shell_name} 代理命令到剪贴板") },
        ToastStatus::Success,
    );
    let copy_btn = button(
        row![
            icon_themed(Icon::Copy, 12.0, |t| tokens(t).text_secondary),
            Space::new().width(theme::SP_XS),
            text(if is_en { "Copy" } else { "复制" }).size(11).font(FONT_MEDIUM),
        ].align_y(Alignment::Center),
    )
    .padding([4, 10]).style(style_ghost).on_press(copy_msg);

    container(
        row![
            kbd_badge(shell_name),
            Space::new().width(theme::SP_SM),
            text(command).size(11).font(MONO).width(Length::Fill).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
            Space::new().width(theme::SP_SM),
            copy_btn,
        ].align_y(Alignment::Center).width(Length::Fill),
    )
    .padding([theme::SP_SM, theme::SP_MD]).width(Length::Fill).style(row_card_surface).into()
}

fn shell_export_card<'a>(is_en: bool) -> Element<'a, Message> {
    let bash_cmd = "export http_proxy=http://127.0.0.1:7890 https_proxy=http://127.0.0.1:7890 all_proxy=socks5://127.0.0.1:7891";
    let fish_cmd = "set -gx http_proxy http://127.0.0.1:7890; set -gx https_proxy http://127.0.0.1:7890; set -gx all_proxy socks5://127.0.0.1:7891";
    let pwsh_cmd = "$env:http_proxy=\"http://127.0.0.1:7890\"; $env:https_proxy=\"http://127.0.0.1:7890\"; $env:all_proxy=\"socks5://127.0.0.1:7891\"";
    let cmd_cmd = "set http_proxy=http://127.0.0.1:7890 & set https_proxy=http://127.0.0.1:7890 & set all_proxy=socks5://127.0.0.1:7891";

    card(
        Some(if is_en { "Terminal Proxy Environment" } else { "终端代理环境变量" }.to_string()),
        column![
            secondary_text(if is_en {
                "Quickly copy export commands to configure CLI and terminal sessions to route traffic through the local proxy."
            } else {
                "在终端或命令行会话中快速配置代理环境变量，使 CLI 工具经由本地代理出站。"
            }),
            Space::new().height(theme::SP_XS),
            shell_export_row("Bash / Zsh", bash_cmd, is_en),
            shell_export_row("Fish", fish_cmd, is_en),
            shell_export_row("PowerShell", pwsh_cmd, is_en),
            shell_export_row("Windows CMD", cmd_cmd, is_en),
        ].spacing(theme::SP_SM),
    )
}

fn inbound_port_tile<'a>(running: bool, title: &'static str, badge_label: &'static str, badge_kind: BadgeKind, proto_tag: &'static str, endpoint: &'static str) -> Element<'a, Message> {
    container(
        column![
            row![
                status_dot(running),
                Space::new().width(theme::SP_SM),
                text(title).size(13).font(FONT_SEMIBOLD).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                Space::new().width(Length::Fill),
                badge(badge_label, badge_kind),
            ].align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            row![
                kbd_badge(proto_tag),
                Space::new().width(theme::SP_SM),
                text(endpoint).size(13).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
            ].align_y(Alignment::Center),
        ].spacing(theme::SP_XS),
    )
    .width(Length::FillPortion(1)).padding(theme::SP_MD).style(row_card_surface).into()
}

fn inbounds_card<'a>(state: &AppState, is_en: bool) -> Element<'a, Message> {
    let running = matches!(state.runtime.status, RuntimeStatus::Running);
    let (status_str, badge_kind) = if running {
        (if is_en { "Listening" } else { "监听中" }, BadgeKind::Success)
    } else {
        (if is_en { "Standby" } else { "待命就绪" }, BadgeKind::Neutral)
    };

    let mixed_tile = inbound_port_tile(running, if is_en { "Mixed Port" } else { "混合代理端口" }, status_str, badge_kind, "HTTP / SOCKS5", "127.0.0.1:7890");
    let socks_tile = inbound_port_tile(running, if is_en { "SOCKS5 Port" } else { "SOCKS5 专用端口" }, status_str, badge_kind, "SOCKS5", "127.0.0.1:7891");

    card(
        Some(if is_en { "Core Inbound & Ports" } else { "核心入站与端口" }.to_string()),
        column![
            secondary_text(if is_en { "Standard local inbound ports for HTTP/HTTPS and SOCKS5 proxy traffic." } else { "核心本地标准入站端口（提供 HTTP/HTTPS 与 SOCKS5 代理接入服务）。" }),
            Space::new().height(theme::SP_XS),
            row![mixed_tile, socks_tile].spacing(theme::SP_MD),
        ].spacing(theme::SP_SM),
    )
}

fn system_proxy_card<'a>(state: &AppState, lang: &Lang<'a>, is_en: bool) -> Element<'a, Message> {
    const DEFAULT_BYPASS: &str = "localhost;127.*;10.*;192.168.*;*.lan";
    let proxy_mode_options = if is_en { vec!["Manual".to_string(), "PAC".to_string()] } else { vec!["手动".to_string(), "PAC".to_string()] };

    card(
        Some(lang.tr("system_proxy").to_string()),
        column![
            form_toggle_row(if is_en { "System Proxy" } else { "系统代理" }, state.runtime.system_proxy_enabled, Message::SetSystemProxy),
            row![
                text(if is_en { "Proxy Host" } else { "代理主机" }).size(13).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                Space::new().width(Length::Fill),
                text("127.0.0.1").size(13).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
            ].align_y(Alignment::Center),
            row![
                text(if is_en { "Proxy Mode" } else { "代理模式" }).size(13).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                Space::new().width(Length::Fill),
                segmented_control(&proxy_mode_options, 0, |_| Message::Noop),
            ].align_y(Alignment::Center),
            column![
                row![
                    form_field_label(lang.tr("settings_proxy_bypass").to_string()),
                    Space::new().width(Length::Fill),
                    text_btn(if is_en { "Add Default Bypass" } else { "添加默认代理绕过" }, style_ghost, Some(Message::UpdateSystemProxyBypass(DEFAULT_BYPASS.to_string()))),
                ].align_y(Alignment::Center),
                Space::new().height(theme::SP_XS),
                text_input(DEFAULT_BYPASS, &state.shell.system_proxy_bypass).on_input(Message::UpdateSystemProxyBypass).padding([7, 11]).size(12).font(MONO).style(form_input_style),
            ].spacing(theme::SP_XS),
        ].spacing(theme::SP_SM),
    )
}

fn system_integration_card<'a>(state: &'a AppState, lang: &Lang<'a>, is_en: bool, theme_selector: Element<'a, Message>, selected_language: Option<SettingsChoice>) -> Element<'a, Message> {
    card(
        Some(lang.tr("settings_system_integration").to_string()),
        column![
            form_toggle_row(lang.tr("autostart").to_string(), state.runtime.autostart_enabled, Message::SetAutostart),
            form_toggle_row(lang.tr("settings_close_to_tray").to_string(), state.shell.close_to_tray, Message::UpdateCloseToTray),
            form_toggle_row(lang.tr("settings_notifications").to_string(), state.shell.notifications_enabled, Message::UpdateNotificationsEnabled),
            Space::new().height(theme::SP_XS),
            row![
                text(lang.tr("theme").to_string()).size(13).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                Space::new().width(theme::SP_MD),
                theme_selector,
            ].align_y(Alignment::Center),
            row![
                text(if is_en { "Language" } else { "语言 / Language" }).size(13).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                Space::new().width(Length::Fill),
                pick_list(LANGUAGE_OPTIONS, selected_language, |choice: SettingsChoice| Message::SetLanguage(choice.value.to_string())).width(Length::Fixed(120.0)).style(form_pick_style),
            ].align_y(Alignment::Center),
            Space::new().height(theme::SP_SM),
            row![
                text_btn(if state.profile.is_saving_app_settings { if is_en { "Saving..." } else { "保存中..." }.to_string() } else if is_en { "Save Settings".to_string() } else { "保存设置".to_string() }, style_accent, (!state.profile.is_saving_app_settings).then_some(Message::SaveAppSettings)),
                Space::new().width(theme::SP_MD),
                text_btn(if state.shell.is_factory_resetting { if is_en { "Resetting..." } else { "恢复中..." }.to_string() } else { lang.tr("settings_factory_reset").to_string() }, style_danger, (!state.shell.is_factory_resetting).then_some(Message::RequestConfirmation(ConfirmAction::FactoryReset))),
            ].align_y(Alignment::Center),
        ].spacing(theme::SP_SM),
    )
}

fn tun_card<'a>(state: &'a AppState, lang: &Lang<'a>, is_en: bool) -> Element<'a, Message> {
    let (tun_status_text, tun_status_kind) = match state.runtime.tun_service_status {
        Some(infiltrator_desktop::tun_service::ServiceModeStatus::InstalledAndRunning) => (if is_en { "Running" } else { "运行中" }.to_string(), BadgeKind::Success),
        Some(infiltrator_desktop::tun_service::ServiceModeStatus::InstalledStopped) => (if is_en { "Stopped" } else { "已停止" }.to_string(), BadgeKind::Warning),
        Some(infiltrator_desktop::tun_service::ServiceModeStatus::MissingPrivilege) => (if is_en { "Missing Privilege" } else { "缺少权限" }.to_string(), BadgeKind::Danger),
        Some(infiltrator_desktop::tun_service::ServiceModeStatus::Unsupported) => (if is_en { "Unsupported" } else { "不支持" }.to_string(), BadgeKind::Neutral),
        Some(infiltrator_desktop::tun_service::ServiceModeStatus::NotInstalled) => (if is_en { "Not Installed" } else { "未安装" }.to_string(), BadgeKind::Neutral),
        None => (if is_en { "Unchecked" } else { "未检测" }.to_string(), BadgeKind::Neutral),
    };

    let stack_options = vec!["gVisor".to_string(), "Mixed".to_string(), "System".to_string()];
    let current_stack_index = if state.editor.tun_stack.eq_ignore_ascii_case("mixed") || state.editor.tun_form.stack.eq_ignore_ascii_case("mixed") {
        1
    } else if state.editor.tun_stack.eq_ignore_ascii_case("system") || state.editor.tun_form.stack.eq_ignore_ascii_case("system") {
        2
    } else {
        0
    };
    let tun_stack_selector = segmented_control(&stack_options, current_stack_index, |index| {
        let stack_str = match index { 1 => "mixed", 2 => "system", _ => "gvisor" };
        Message::SetTunStack(stack_str.to_string())
    });

    let dns_hijack_active = !state.editor.tun_form.dns_hijack.trim().is_empty();

    card(
        Some(lang.tr("tun_mode").to_string()),
        column![
            row![
                text(if is_en { "TUN Service Status" } else { "TUN 服务状态" }).size(13).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                Space::new().width(theme::SP_MD),
                badge(tun_status_text, if state.runtime.is_installing_tun_service { BadgeKind::Warning } else { tun_status_kind }),
                Space::new().width(Length::Fill),
                icon_button(Icon::RefreshCw, 14.0, Message::RefreshTunServiceStatus),
                Space::new().width(theme::SP_SM),
                text_btn(if state.runtime.is_installing_tun_service { if is_en { "Preparing..." } else { "准备中..." }.to_string() } else if is_en { "Prepare TUN Service".to_string() } else { "准备 TUN 服务".to_string() }, style_ghost, (!state.runtime.is_installing_tun_service).then_some(Message::InstallTunService)),
            ].align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            row![
                text(lang.tr("tun_stack").to_string()).size(13).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                Space::new().width(Length::Fill),
                tun_stack_selector,
            ].align_y(Alignment::Center),
            row![
                text("MTU").size(13).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                Space::new().width(Length::Fill),
                text_input("1500", &state.editor.tun_form.mtu).on_input(Message::UpdateTunFormMtu).width(Length::Fixed(120.0)).padding([6, 10]).size(12).font(MONO).style(form_input_style),
            ].align_y(Alignment::Center),
            form_toggle_row(lang.tr("tun_auto_route").to_string(), state.editor.tun_auto_route || state.editor.tun_form.auto_route, Message::SetTunAutoRoute),
            form_toggle_row(lang.tr("tun_strict_route").to_string(), state.editor.tun_strict_route || state.editor.tun_form.strict_route, Message::SetTunStrictRoute),
            form_toggle_row(if is_en { "DNS Hijack" } else { "DNS 劫持 (DNS Hijack)" }.to_string(), dns_hijack_active, |on| Message::UpdateTunFormDnsHijack(if on { "any:53".to_string() } else { String::new() })),
        ].spacing(theme::SP_SM),
    )
}

fn kernel_management_card<'a>(state: &'a AppState, lang: &Lang<'a>, is_en: bool, selected_core_channel: Option<SettingsChoice>) -> Element<'a, Message> {
    let mut kernel_rows = column![].spacing(theme::SP_SM);

    if let Some(latest) = &state.runtime.latest_core_version {
        kernel_rows = kernel_rows.push(
            container(
                row![
                    text(if state.runtime.is_downloading_core { format!("{} {:.0}%", lang.tr("settings_downloading"), state.runtime.download_progress * 100.0) } else { format!("{} {}", lang.tr("settings_available"), latest) }).size(13).width(Length::Fill),
                    if state.runtime.is_downloading_core {
                        Element::from(row![
                            column![
                                progress_bar(0.0..=1.0, state.runtime.download_progress).length(Length::Fixed(180.0)),
                                secondary_text(format!("{} {}/s", if is_en { "Speed" } else { "速度" }, state.runtime.download_stats.as_ref().map(|s| crate::utils::format_bytes(s.speed_bytes)).unwrap_or_else(|| "—".to_string()))),
                            ].spacing(theme::SP_XS),
                            Space::new().width(theme::SP_SM),
                            text_btn(if is_en { "Cancel" } else { "取消" }, style_ghost, Some(Message::CancelCoreDownload)),
                        ].align_y(Alignment::Center))
                    } else {
                        Element::from(text_btn(lang.tr("settings_download"), style_accent, Some(Message::DownloadCore(latest.clone()))))
                    },
                ].align_y(Alignment::Center),
            )
            .padding(theme::SP_MD).width(Length::Fill).style(|t: &Theme| {
                let tk = tokens(t);
                container::Style { background: Some(Color { a: 0.10, ..tk.success }.into()), border: border::Border { radius: border::Radius::from(R_CONTROL), ..Default::default() }, ..Default::default() }
            }),
        );
    }

    if state.runtime.installed_kernels.is_empty() {
        kernel_rows = kernel_rows.push(secondary_text(lang.tr("settings_no_kernels")));
    } else {
        for kernel in &state.runtime.installed_kernels {
            kernel_rows = kernel_rows.push(
                container(
                    row![
                        column![
                            text(&kernel.version).size(13).font(FONT_SEMIBOLD).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                            if kernel.is_default { badge(lang.tr("active_tag").trim().to_string(), BadgeKind::Success) } else { Space::new().width(0).height(0).into() },
                        ].spacing(2).width(Length::Fill),
                        if !kernel.is_default {
                            Element::from(row![
                                text_btn(lang.tr("settings_set_default"), style_ghost, Some(Message::SetDefaultKernel(kernel.version.clone()))),
                                Space::new().width(theme::SP_SM),
                                icon_button(Icon::Trash2, 14.0, Message::RequestConfirmation(ConfirmAction::DeleteKernel(kernel.version.clone()))),
                            ].align_y(Alignment::Center))
                        } else {
                            Element::from(secondary_text(lang.tr("settings_installed")))
                        },
                    ].align_y(Alignment::Center),
                ).padding([theme::SP_SM, SP_MD]).width(Length::Fill).style(row_card_surface),
            );
        }
    }

    card(
        None,
        column![
            section_header(
                lang.tr("settings_kernel_mgmt").as_ref(),
                Some(
                    row![
                        pick_list(CORE_CHANNEL_OPTIONS, selected_core_channel, |choice: SettingsChoice| Message::SetCoreChannel(choice.value.to_string())).width(Length::Fixed(110.0)).style(form_pick_style),
                        Space::new().width(theme::SP_SM),
                        if state.runtime.is_checking_update {
                            text(lang.tr("settings_checking").to_string()).size(12).style(|t: &Theme| text::Style { color: Some(tokens(t).text_tertiary) }).into()
                        } else {
                            icon_button(Icon::RefreshCw, 14.0, Message::CheckCoreUpdate)
                        },
                    ].align_y(Alignment::Center).into(),
                ),
            ),
            Space::new().height(theme::SP_MD),
            kernel_rows,
        ],
    )
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let is_en = state.shell.lang.starts_with("en");
    let selected_language = LANGUAGE_OPTIONS.iter().find(|option| option.value == state.shell.lang).copied();
    let selected_core_channel = CORE_CHANNEL_OPTIONS.iter().find(|option| option.value == state.runtime.core_channel).copied();
    let tun_service_ready = state.shell.is_admin || matches!(state.runtime.tun_service_status, Some(infiltrator_desktop::tun_service::ServiceModeStatus::InstalledAndRunning));

    let header = text(lang.tr("nav_settings").to_string()).size(24).font(FONT_SEMIBOLD).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) });

    let uac_banner = if !tun_service_ready {
        let uac_title = lang.tr("admin_status").to_string();
        let uac_desc = if cfg!(windows) {
            lang.tr("settings_uac_desc").to_string()
        } else if is_en {
            "Configuring platform permissions is required before enabling TUN mode; restart or prepare permissions below.".to_string()
        } else {
            "启用 TUN 前需要为 mihomo 配置平台权限；完成后请重新开启 TUN。".to_string()
        };
        let uac_btn_label = if cfg!(windows) { lang.tr("settings_uac_request").to_string() } else if is_en { "Prepare TUN Privilege".to_string() } else { "准备 TUN 权限".to_string() };
        let action_btn = text_btn(uac_btn_label, style_accent, Some(Message::RequestAdminPrivilege));
        Some(banner_alert(BadgeKind::Warning, uac_title, uac_desc, Some(action_btn)))
    } else {
        None
    };

    let theme_labels = if is_en { vec!["Light".to_string(), "Dark".to_string(), "Forest".to_string(), "AMOLED".to_string()] } else { vec!["浅色模式".to_string(), "深色模式".to_string(), "护眼森林".to_string(), "AMOLED".to_string()] };
    let current_theme_index = if crate::view::theme::is_amoled(&state.shell.theme) { 3 } else if crate::view::theme::is_forest(&state.shell.theme) { 2 } else if state.shell.theme == Theme::Light { 0 } else { 1 };
    let theme_selector = segmented_control(&theme_labels, current_theme_index, |index| {
        let name = match index { 0 => "light", 2 => "forest", 3 => "amoled", _ => "dark" };
        Message::SetTheme(name.to_string())
    });

    let system_proxy_section = system_proxy_card(state, &lang, is_en);
    let system_section = system_integration_card(state, &lang, is_en, theme_selector, selected_language);
    let tun_section = tun_card(state, &lang, is_en);

    let sniffer_section = card(
        Some(lang.tr("settings_sniffer").to_string()),
        column![
            secondary_text(lang.tr("settings_sniffer_desc")),
            form_toggle_row(lang.tr("settings_sniffer").to_string(), state.editor.sniffer_enabled, Message::SetSnifferEnabled),
        ].spacing(theme::SP_SM),
    );

    let editor_section = card(
        Some("External Editor".to_string()),
        column![
            secondary_text("Set a preferred editor executable path (optional)."),
            text_input("e.g. C:\\Program Files\\Sublime Text\\subl.exe", &state.editor.editor_path_setting).on_input(Message::UpdateEditorPathSetting).padding([8, 12]).size(13).style(form_input_style),
            row![
                if state.profile.is_saving_app_settings { text_btn("Saving...", style_ghost, None) } else { text_btn("Save Editor Path", style_ghost, Some(Message::SaveAppSettings)) },
                Space::new().width(theme::SP_SM),
                text_btn("Reset", style_ghost, Some(Message::UpdateEditorPathSetting(String::new()))),
            ].align_y(Alignment::Center),
        ].spacing(theme::SP_SM),
    );

    let admin_running = state.shell.admin_server.is_running();
    let admin_section = card(
        Some(lang.tr("settings_admin_web").to_string()),
        column![
            secondary_text(lang.tr("settings_admin_desc")),
            form_toggle_row(lang.tr("settings_admin_enable").to_string(), state.shell.admin_enabled, Message::SetAdminEnabled),
            row![
                text(lang.tr("settings_admin_port").to_string()).size(13).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                Space::new().width(theme::SP_MD),
                text_input("25210", &state.shell.admin_port_input).on_input(Message::UpdateAdminPort).width(Length::Fixed(120.0)).padding([8, 12]).size(13).font(MONO).style(form_input_style),
                Space::new().width(theme::SP_MD),
                text_btn(lang.tr("settings_admin_apply"), style_ghost, Some(Message::ApplyAdminSettings)),
                Space::new().width(Length::Fill),
            ].align_y(Alignment::Center),
            row![
                status_dot(admin_running),
                Space::new().width(theme::SP_SM),
                text(if admin_running { lang.tr("settings_admin_running") } else { lang.tr("settings_admin_stopped") }).size(12).style(move |t: &Theme| text::Style { color: Some(if admin_running { tokens(t).success } else { tokens(t).text_secondary }) }),
                Space::new().width(theme::SP_MD),
                if admin_running {
                    text(state.shell.admin_server.url().unwrap_or_default()).size(12).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }).into()
                } else {
                    Element::from(Space::new().width(Length::Shrink))
                },
            ].align_y(Alignment::Center),
        ].spacing(theme::SP_SM),
    );

    let mut content = column![header, Space::new().height(theme::SP_LG)].spacing(10);
    if let Some(banner) = uac_banner { content = content.push(banner).push(Space::new().height(10)); }

    content = content
        .push(system_proxy_section)
        .push(Space::new().height(10))
        .push(system_section)
        .push(Space::new().height(10))
        .push(inbounds_card(state, is_en))
        .push(Space::new().height(10))
        .push(shell_export_card(is_en))
        .push(Space::new().height(10))
        .push(tun_section)
        .push(Space::new().height(10))
        .push(sniffer_section)
        .push(Space::new().height(10))
        .push(editor_section)
        .push(Space::new().height(10))
        .push(admin_section)
        .push(Space::new().height(10))
        .push(crate::view::doctor::section(state))
        .push(Space::new().height(10))
        .push(kernel_management_card(state, &lang, is_en, selected_core_channel))
        .push(Space::new().height(40));

    modern_scrollable(content).height(Length::Fill).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_choice_display() {
        assert_eq!(format!("{}", SettingsChoice { value: "zh-CN" }), "zh-CN");
        assert_eq!(format!("{}", SettingsChoice { value: "stable" }), "stable");
    }

    #[test]
    fn test_secondary_text_widget() {
        let _elem: Element<'_, Message> = secondary_text("Helper description");
    }

    #[test]
    fn test_theme_segmented_control_options() {
        let options_en = vec!["Light".to_string(), "Dark".to_string(), "Forest".to_string(), "AMOLED".to_string()];
        let _ctrl_light = segmented_control(&options_en, 0, |_| Message::ToggleTheme);
        let _ctrl_dark = segmented_control(&options_en, 1, |_| Message::ToggleTheme);
        let _ctrl_forest = segmented_control(&options_en, 2, |_| Message::ToggleTheme);
        let _ctrl_amoled = segmented_control(&options_en, 3, |_| Message::ToggleTheme);
    }

    #[test]
    fn test_shell_export_row() {
        let _row: Element<'_, Message> = shell_export_row("Bash", "export http_proxy=...", true);
        let _card: Element<'_, Message> = shell_export_card(false);
    }

    #[test]
    fn test_inbounds_card() {
        let (state, _) = AppState::new();
        let _card: Element<'_, Message> = inbounds_card(&state, false);
    }

    #[test]
    fn test_system_proxy_card() {
        let (state, _) = AppState::new();
        let lang = Lang(&state.shell.lang);
        let _card_zh: Element<'_, Message> = system_proxy_card(&state, &lang, false);
        let _card_en: Element<'_, Message> = system_proxy_card(&state, &lang, true);
    }

    #[test]
    fn test_tun_card() {
        let (state1, _) = AppState::new();
        let lang1 = Lang(&state1.shell.lang);
        let _card_default: Element<'_, Message> = tun_card(&state1, &lang1, false);

        let (mut state2, _) = AppState::new();
        state2.editor.tun_stack = "mixed".to_string();
        state2.editor.tun_form.dns_hijack = "any:53".to_string();
        let lang2 = Lang(&state2.shell.lang);
        let _card_mixed: Element<'_, Message> = tun_card(&state2, &lang2, true);

        let (mut state3, _) = AppState::new();
        state3.editor.tun_stack = "system".to_string();
        let lang3 = Lang(&state3.shell.lang);
        let _card_system: Element<'_, Message> = tun_card(&state3, &lang3, false);
    }

    #[test]
    fn test_settings_view_render() {
        let (state1, _) = AppState::new();
        let _view_zh: Element<'_, Message> = view(&state1);

        let (mut state2, _) = AppState::new();
        state2.shell.lang = "en-US".to_string();
        let _view_en: Element<'_, Message> = view(&state2);
    }
}
