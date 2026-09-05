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
use infiltrator_ports::host_runtime::TunServiceStatus;
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

fn shell_export_row<'a>(shell_name: &'static str, command: &'static str, lang: &Lang<'_>) -> Element<'a, Message> {
    let copy_msg = Message::ShowToast(
        infiltrator_shared::i18n_interpolator::interpolate(&lang.tr("settings_copied_env"), &[("shell_name", shell_name)]),
        ToastStatus::Success,
    );
    let copy_btn = button(
        row![
            icon_themed(Icon::Copy, 12.0, |t| tokens(t).text_secondary),
            Space::new().width(theme::SP_XS),
            text(lang.tr("settings_copy").to_string()).size(11).font(FONT_MEDIUM),
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

fn shell_export_card<'a>(lang: &Lang<'_>) -> Element<'a, Message> {
    let bash_cmd = "export http_proxy=http://127.0.0.1:7890 https_proxy=http://127.0.0.1:7890 all_proxy=socks5://127.0.0.1:7891";
    let fish_cmd = "set -gx http_proxy http://127.0.0.1:7890; set -gx https_proxy http://127.0.0.1:7890; set -gx all_proxy socks5://127.0.0.1:7891";
    let pwsh_cmd = "$env:http_proxy=\"http://127.0.0.1:7890\"; $env:https_proxy=\"http://127.0.0.1:7890\"; $env:all_proxy=\"socks5://127.0.0.1:7891\"";
    let cmd_cmd = "set http_proxy=http://127.0.0.1:7890 & set https_proxy=http://127.0.0.1:7890 & set all_proxy=socks5://127.0.0.1:7891";

    card(
        Some(lang.tr("settings_term_env_title").to_string()),
        column![
            secondary_text(lang.tr("settings_term_env_desc").to_string()),
            Space::new().height(theme::SP_XS),
            shell_export_row("Bash / Zsh", bash_cmd, lang),
            shell_export_row("Fish", fish_cmd, lang),
            shell_export_row("PowerShell", pwsh_cmd, lang),
            shell_export_row("Windows CMD", cmd_cmd, lang),
        ].spacing(theme::SP_SM),
    )
}

fn inbound_port_tile<'a>(running: bool, title: String, badge_label: String, badge_kind: BadgeKind, proto_tag: &'static str, endpoint: &'static str) -> Element<'a, Message> {
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

fn inbounds_card<'a>(state: &AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let running = matches!(state.runtime.status, RuntimeStatus::Running);
    let (status_str, badge_kind) = if running {
        (lang.tr("settings_listening").to_string(), BadgeKind::Success)
    } else {
        (lang.tr("settings_ready").to_string(), BadgeKind::Neutral)
    };

    let mixed_tile = inbound_port_tile(running, lang.tr("settings_mixed_port").to_string(), status_str.clone(), badge_kind, "HTTP / SOCKS5", "127.0.0.1:7890");
    let socks_tile = inbound_port_tile(running, lang.tr("settings_socks_port").to_string(), status_str, badge_kind, "SOCKS5", "127.0.0.1:7891");

    card(
        Some(lang.tr("settings_inbound_title").to_string()),
        column![
            secondary_text(lang.tr("settings_inbound_desc").to_string()),
            Space::new().height(theme::SP_XS),
            row![mixed_tile, socks_tile].spacing(theme::SP_MD),
        ].spacing(theme::SP_SM),
    )
}

fn system_proxy_card<'a>(state: &AppState, lang: &Lang<'a>, _is_en: bool) -> Element<'a, Message> {
    const DEFAULT_BYPASS: &str = "localhost;127.*;10.*;192.168.*;*.lan";
    let proxy_mode_options = vec![lang.tr("settings_mode_manual").to_string(), "PAC".to_string()];

    card(
        Some(lang.tr("system_proxy").to_string()),
        column![
            form_toggle_row(lang.tr("settings_sys_proxy").to_string(), state.runtime.system_proxy_enabled, Message::SetSystemProxy),
            row![
                text(lang.tr("settings_proxy_host").to_string()).size(13).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                Space::new().width(Length::Fill),
                text("127.0.0.1").size(13).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
            ].align_y(Alignment::Center),
            row![
                text(lang.tr("settings_proxy_mode").to_string()).size(13).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                Space::new().width(Length::Fill),
                segmented_control(&proxy_mode_options, 0, |_| Message::Noop),
            ].align_y(Alignment::Center),
            column![
                row![
                    form_field_label(lang.tr("settings_proxy_bypass").to_string()),
                    Space::new().width(Length::Fill),
                    text_btn(lang.tr("settings_add_default_bypass").to_string(), style_ghost, Some(Message::UpdateSystemProxyBypass(DEFAULT_BYPASS.to_string()))),
                ].align_y(Alignment::Center),
                Space::new().height(theme::SP_XS),
                text_input(DEFAULT_BYPASS, &state.shell.system_proxy_bypass).on_input(Message::UpdateSystemProxyBypass).padding([7, 11]).size(12).font(MONO).style(form_input_style),
            ].spacing(theme::SP_XS),
        ].spacing(theme::SP_SM),
    )
}

fn system_integration_card<'a>(state: &'a AppState, lang: &Lang<'a>, _is_en: bool, theme_selector: Element<'a, Message>, selected_language: Option<SettingsChoice>) -> Element<'a, Message> {
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
                text(lang.tr("settings_lang_label").to_string()).size(13).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                Space::new().width(Length::Fill),
                pick_list(LANGUAGE_OPTIONS, selected_language, |choice: SettingsChoice| Message::SetLanguage(choice.value.to_string())).width(Length::Fixed(120.0)).style(form_pick_style),
            ].align_y(Alignment::Center),
            Space::new().height(theme::SP_SM),
            row![
                text_btn(if state.profile.is_saving_app_settings { lang.tr("settings_saving").to_string() } else { lang.tr("settings_save_btn").to_string() }, style_accent, (!state.profile.is_saving_app_settings).then_some(Message::SaveAppSettings)),
                Space::new().width(theme::SP_MD),
                text_btn(if state.shell.is_factory_resetting { lang.tr("settings_reverting").to_string() } else { lang.tr("settings_factory_reset").to_string() }, style_danger, (!state.shell.is_factory_resetting).then_some(Message::RequestConfirmation(ConfirmAction::FactoryReset))),
            ].align_y(Alignment::Center),
        ].spacing(theme::SP_SM),
    )
}

fn tun_card<'a>(state: &'a AppState, lang: &Lang<'a>, _is_en: bool) -> Element<'a, Message> {
    let (tun_status_text, tun_status_kind) = match state.runtime.tun_service_status {
        Some(TunServiceStatus::InstalledAndRunning) => (lang.tr("settings_status_running").to_string(), BadgeKind::Success),
        Some(TunServiceStatus::InstalledStopped) => (lang.tr("settings_status_stopped").to_string(), BadgeKind::Warning),
        Some(TunServiceStatus::MissingPrivilege) => (lang.tr("settings_status_no_perm").to_string(), BadgeKind::Danger),
        Some(TunServiceStatus::Unsupported) => (lang.tr("settings_status_unsupported").to_string(), BadgeKind::Neutral),
        Some(TunServiceStatus::NotInstalled) => (lang.tr("settings_status_uninstalled").to_string(), BadgeKind::Neutral),
        None => (lang.tr("settings_status_undetected").to_string(), BadgeKind::Neutral),
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
                text(lang.tr("settings_tun_service_status").to_string()).size(13).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                Space::new().width(theme::SP_MD),
                badge(tun_status_text, if state.runtime.is_installing_tun_service { BadgeKind::Warning } else { tun_status_kind }),
                Space::new().width(Length::Fill),
                icon_button(Icon::RefreshCw, 14.0, Message::RefreshTunServiceStatus),
                Space::new().width(theme::SP_SM),
                text_btn(if state.runtime.is_installing_tun_service { lang.tr("settings_tun_preparing").to_string() } else { lang.tr("settings_tun_prepare_btn").to_string() }, style_ghost, (!state.runtime.is_installing_tun_service).then_some(Message::InstallTunService)),
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
            form_toggle_row(lang.tr("settings_dns_hijack").to_string(), dns_hijack_active, |on| Message::UpdateTunFormDnsHijack(if on { "any:53".to_string() } else { String::new() })),
        ].spacing(theme::SP_SM),
    )
}

fn kernel_management_card<'a>(state: &'a AppState, lang: &Lang<'a>, _is_en: bool, selected_core_channel: Option<SettingsChoice>) -> Element<'a, Message> {
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
                                secondary_text(format!("{} {}/s", lang.tr("settings_speed"), state.runtime.download_stats.as_ref().map(|s| crate::utils::format_bytes(s.speed_bytes)).unwrap_or_else(|| "—".to_string()))),
                            ].spacing(theme::SP_XS),
                            Space::new().width(theme::SP_SM),
                            text_btn(lang.tr("btn_cancel").to_string(), style_ghost, Some(Message::CancelCoreDownload)),
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

fn hotkeys_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let mut rows = column![].spacing(theme::SP_SM);
    for binding in &state.shell.hotkeys_config {
        let binding_id = binding.id.clone();
        let binding_id_toggle = binding.id.clone();
        let action_name = lang.tr(binding.action_title_key);

        let row_item = row![
            column![
                text(action_name.to_string()).size(13).font(FONT_SEMIBOLD),
                secondary_text(format!("Global hotkey: {}", binding.combo)),
            ].width(Length::Fill),
            text_input("e.g. Ctrl+Alt+P", &binding.combo)
                .on_input(move |combo| Message::UpdateHotkeyCombo { id: binding_id.clone(), combo })
                .padding([4, 8])
                .size(11)
                .font(MONO)
                .width(110)
                .style(form_input_style),
            Space::new().width(theme::SP_SM),
            button(text(if binding.enabled { "Active" } else { "Off" }).size(11))
                .padding([4, 8])
                .style(if binding.enabled { style_accent } else { style_ghost })
                .on_press(Message::ToggleHotkeyEnabled(binding_id_toggle)),
        ].align_y(Alignment::Center);

        rows = rows.push(row_item);
    }

    card(
        Some(lang.tr("hotkey_manager_title").to_string()),
        column![
            secondary_text(lang.tr("hotkey_manager_desc")),
            Space::new().height(theme::SP_XS),
            rows,
        ].spacing(theme::SP_SM)
    )
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let is_en = state.shell.lang.starts_with("en");
    let selected_language = LANGUAGE_OPTIONS.iter().find(|option| option.value == state.shell.lang).copied();
    let selected_core_channel = CORE_CHANNEL_OPTIONS.iter().find(|option| option.value == state.runtime.core_channel).copied();
    let tun_service_ready = state.shell.is_admin || matches!(state.runtime.tun_service_status, Some(TunServiceStatus::InstalledAndRunning));

    let header = text(lang.tr("nav_settings").to_string()).size(24).font(FONT_SEMIBOLD).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) });

    let uac_banner = if !tun_service_ready {
        let uac_title = lang.tr("admin_status").to_string();
        let uac_desc = if cfg!(windows) {
            lang.tr("settings_uac_desc").to_string()
        } else if is_en {
            "Configuring platform permissions is required before enabling TUN mode; restart or prepare permissions below.".to_string()
        } else {
            lang.tr("settings_tun_perm_hint").to_string()
        };
        let uac_btn_label = if cfg!(windows) { lang.tr("settings_uac_request").to_string() } else if is_en { "Prepare TUN Privilege".to_string() } else { lang.tr("settings_tun_prepare_perm_btn").to_string() };
        let action_btn = text_btn(uac_btn_label, style_accent, Some(Message::RequestAdminPrivilege));
        Some(banner_alert(BadgeKind::Warning, uac_title, uac_desc, Some(action_btn)))
    } else {
        None
    };

    let theme_labels = vec![lang.tr("theme_light").to_string(), lang.tr("theme_dark").to_string(), lang.tr("theme_forest").to_string(), "AMOLED".to_string()];
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
        .push(crate::view::pac_card::pac_card(state, &lang))
        .push(Space::new().height(10))
        .push(system_section)
        .push(Space::new().height(10))
        .push(inbounds_card(state, &lang))
        .push(Space::new().height(10))
        .push(crate::view::lan_sharing_card::lan_sharing_card(state, &lang))
        .push(Space::new().height(10))
        .push(shell_export_card(&lang))
        .push(Space::new().height(10))
        .push(tun_section)
        .push(Space::new().height(10))
        .push(crate::view::net_roam_card::net_roam_card(state, &lang))
        .push(Space::new().height(10))
        .push(sniffer_section)
        .push(Space::new().height(10))
        .push(editor_section)
        .push(Space::new().height(10))
        .push(admin_section)
        .push(Space::new().height(10))
        .push(crate::view::web_dash_card::web_dash_card(state, &lang))
        .push(Space::new().height(10))
        .push(crate::view::apply_guard_card::apply_guard_card(state, &lang))
        .push(Space::new().height(10))
        .push(crate::view::doctor::section(state))
        .push(Space::new().height(10))
        .push(hotkeys_card(state, &lang))
        .push(Space::new().height(10))
        .push(crate::view::geodata_card::geodata_card(state, &lang))
        .push(Space::new().height(10))
        .push(crate::view::uwp_card::uwp_card(state, &lang))
        .push(Space::new().height(10))
        .push(kernel_management_card(state, &lang, is_en, selected_core_channel))
        .push(Space::new().height(40));

    modern_scrollable(content).height(Length::Fill).into()
}

#[cfg(test)]
#[path = "../../tests/gui/view_settings_tests.rs"]
mod tests;
