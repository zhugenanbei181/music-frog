//! MRS rule-provider detail panel for the Rules page providers tab: parsed
//! header metadata (behavior / rule count / version) per provider, with the
//! failure reason when a cache file is missing or not MRS.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::options::MrsProviderDetail;
use crate::view::components::{
    BadgeKind, badge, card, chip, row_card_surface, style_ghost,
};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};
use std::collections::HashMap;

/// Format the rule behavior identifier into capitalized display string.
pub fn format_behavior_name(behavior: &str) -> &'static str {
    match behavior.to_ascii_lowercase().as_str() {
        "domain" => "Domain",
        "ipcidr" | "ip-cidr" => "IP-CIDR",
        "classical" => "Classical",
        _ => "Domain",
    }
}

/// Format the combined transport format and rule behavior (e.g. `HTTP::Domain`).
pub fn format_vehicle_behavior(vehicle: Option<&str>, behavior: &str) -> String {
    let behavior_name = if behavior.is_empty() {
        "Domain"
    } else {
        format_behavior_name(behavior)
    };
    let vehicle_name = vehicle.filter(|v| !v.is_empty()).unwrap_or("HTTP");
    format!("{vehicle_name}::{behavior_name}")
}

/// Format rule count badge label (e.g. `179 rules`).
pub fn format_rule_count(count: u32) -> String {
    format!("{count} rules")
}

fn meta_text(value: String) -> text::Text<'static> {
    text(value)
        .size(11)
        .font(MONO)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        })
}

fn detail_row<'a>(
    lang: &Lang<'a>,
    detail: &MrsProviderDetail,
    rule_provider: Option<&mihomo_api::types::RuleProvider>,
) -> Element<'a, Message> {
    let vehicle_type = rule_provider.map(|rp| rp.vehicle_type.as_str());
    let behavior_raw = if !detail.behavior.is_empty() {
        detail.behavior.as_str()
    } else if let Some(rp) = rule_provider {
        rp.behavior.as_str()
    } else if let Some(meta) = &detail.metadata {
        meta.behavior.as_str()
    } else {
        "domain"
    };

    let behavior_format_str = format_vehicle_behavior(vehicle_type, behavior_raw);
    let rule_count_opt = detail
        .metadata
        .as_ref()
        .map(|m| m.rule_count)
        .or_else(|| rule_provider.map(|rp| rp.rule_count));

    let count_badge = match rule_count_opt {
        Some(count) => badge(format_rule_count(count), BadgeKind::Accent),
        None => badge(lang.tr("mrs_badge_missing").to_string(), BadgeKind::Neutral),
    };

    let badge_row = row![
        text(detail.name.clone())
            .size(13)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().width(theme::SP_SM),
        count_badge,
        Space::new().width(theme::SP_SM),
        chip(behavior_format_str),
        Space::new().width(theme::SP_XS),
        chip("MrsRule"),
    ]
    .align_y(Alignment::Center);

    let mut info_col = column![badge_row].spacing(3);

    if let Some(meta) = &detail.metadata {
        info_col = info_col.push(meta_text(format!(
            "v{} · {} · {}",
            meta.version,
            crate::utils::format_bytes(meta.payload_size as u64),
            meta.description
        )));
        if let Some(file) = &detail.file {
            info_col = info_col.push(meta_text(file.display().to_string()));
        }
    } else if let Some(error) = detail.errors.first() {
        info_col = info_col.push(meta_text(error.clone()));
    }

    let unpack_btn = button(
        row![
            icon_themed(Icon::Zap, 12.0, |t: &Theme| tokens(t).text_secondary),
            Space::new().width(4.0),
            text("Unpack").size(11).font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_ghost)
    .on_press(Message::UnpackRuleProvider(detail.name.clone()));

    let update_btn = button(
        row![
            icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).text_secondary),
            Space::new().width(4.0),
            text(lang.tr("btn_update").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_ghost)
    .on_press(Message::UpdateRuleProvider(detail.name.clone()));

    let actions = row![unpack_btn, Space::new().width(theme::SP_XS), update_btn]
        .align_y(Alignment::Center);

    let row_content = row![
        container(icon_themed(Icon::Shield, 16.0, |t: &Theme| tokens(t).accent))
            .width(32)
            .height(32)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|t: &Theme| {
                let tk = tokens(t);
                container::Style {
                    background: Some(tk.accent_soft.into()),
                    border: Border {
                        radius: border::Radius::from(theme::R_CONTROL),
                        width: 1.0,
                        color: Color {
                            a: 0.20,
                            ..tk.accent
                        },
                    },
                    ..Default::default()
                }
            }),
        Space::new().width(theme::SP_MD),
        info_col.width(Length::Fill),
        Space::new().width(theme::SP_MD),
        actions,
    ]
    .align_y(Alignment::Center);

    container(row_content)
        .padding([10, 14])
        .width(Length::Fill)
        .style(row_card_surface)
        .into()
}

/// Rendered inside the providers tab, below the provider lists. Scan results
/// refresh whenever the live provider list reloads (`ProvidersLoaded`).
pub fn mrs_card(state: &AppState) -> Option<Element<'_, Message>> {
    if state.editor.mrs_details.is_empty() && !state.editor.is_scanning_mrs {
        return None;
    }
    let lang = Lang(&state.shell.lang);
    let mut body = column![].spacing(theme::SP_SM);
    if state.editor.is_scanning_mrs {
        body = body.push(
            text(lang.tr("mrs_scanning").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        );
    }
    let rp_map: HashMap<&str, &mihomo_api::types::RuleProvider> = state
        .editor
        .rule_providers
        .iter()
        .map(|rp| (rp.name.as_str(), rp))
        .collect();

    for detail in &state.editor.mrs_details {
        let rp = rp_map.get(detail.name.as_str()).copied();
        body = body.push(detail_row(&lang, detail, rp));
    }
    Some(card(Some(lang.tr("mrs_panel_title").to_string()), body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_behavior_name() {
        assert_eq!(format_behavior_name("domain"), "Domain");
        assert_eq!(format_behavior_name("ipcidr"), "IP-CIDR");
        assert_eq!(format_behavior_name("IP-CIDR"), "IP-CIDR");
        assert_eq!(format_behavior_name("classical"), "Classical");
        assert_eq!(format_behavior_name("other"), "Domain");
    }

    #[test]
    fn test_format_vehicle_behavior() {
        assert_eq!(
            format_vehicle_behavior(Some("HTTP"), "domain"),
            "HTTP::Domain"
        );
        assert_eq!(
            format_vehicle_behavior(Some("File"), "ipcidr"),
            "File::IP-CIDR"
        );
        assert_eq!(
            format_vehicle_behavior(None, "classical"),
            "HTTP::Classical"
        );
    }

    #[test]
    fn test_format_rule_count() {
        assert_eq!(format_rule_count(179), "179 rules");
        assert_eq!(format_rule_count(52345), "52345 rules");
    }

    #[test]
    fn test_mrs_card_empty_and_populated() {
        let (mut state, _) = AppState::new();
        assert!(mrs_card(&state).is_none());

        state.editor.mrs_details.push(MrsProviderDetail {
            name: "XiaoHongShu".into(),
            behavior: "domain".into(),
            file: None,
            metadata: Some(infiltrator_domain::mrs::MrsMetadata {
                behavior: infiltrator_domain::mrs::Behavior::Domain,
                rule_count: 179,
                version: 1,
                payload_size: 4096,
                description: "XiaoHongShu ruleset".into(),
            }),
            errors: Vec::new(),
        });
        state.editor.mrs_details.push(MrsProviderDetail {
            name: "blizzard".into(),
            behavior: "ipcidr".into(),
            file: None,
            metadata: None,
            errors: vec!["Cache missing".into()],
        });
        state.editor.mrs_details.push(MrsProviderDetail {
            name: "category-ai-chat".into(),
            behavior: "classical".into(),
            file: None,
            metadata: None,
            errors: Vec::new(),
        });

        assert!(mrs_card(&state).is_some());
    }

    #[test]
    fn test_detail_row_render() {
        let lang = Lang("en");
        let detail = MrsProviderDetail {
            name: "test-mrs".into(),
            behavior: "domain".into(),
            file: None,
            metadata: Some(infiltrator_domain::mrs::MrsMetadata {
                behavior: infiltrator_domain::mrs::Behavior::Domain,
                rule_count: 350,
                version: 1,
                payload_size: 2048,
                description: "Test ruleset".into(),
            }),
            errors: Vec::new(),
        };
        let _element = detail_row(&lang, &detail, None);
    }
}
