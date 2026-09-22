//! Bevy DNS workbench editable fields (DUAL-14-04 / 14-05 / 14-14).
//!
//! The draft is the shared [`DnsWorkbenchForm`]; this module owns only the
//! Bevy widgets (text fields, quick-append chips, the GEOIP trigger toggle and
//! the apply button), the honest local validation line, and the shared patch
//! submission. Both surfaces submit the same patch through
//! `UiCommand::ApplyDnsSettings`.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, PositionType,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, text_field_with_placeholder_scene};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::dns::append_server;
use infiltrator_contract::dns_form::{DnsFormField, DnsFormIssue, DnsWorkbenchForm};

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::dns::{DnsProjection, DnsProjectionUpdated};

/// Shared workbench draft plus the honest local validation state.
#[derive(Resource, Clone, Debug, Default)]
pub struct DnsFormState {
    pub form: DnsWorkbenchForm,
    pub dirty: bool,
    pub issues: Vec<DnsFormIssue>,
    pub submitted: bool,
}

/// Parent marker on the text-field node of one raw workbench field.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DnsEditField(pub DnsFormField);

impl Default for DnsEditField {
    fn default() -> Self {
        Self(DnsFormField::Enable)
    }
}

/// A quick-append template chip for one list field.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DnsEditTemplate {
    pub field: DnsFormField,
    pub server: &'static str,
}

impl Default for DnsEditTemplate {
    fn default() -> Self {
        Self {
            field: DnsFormField::Enable,
            server: "",
        }
    }
}

/// The apply button for the whole workbench edit card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsEditApplyButton;

/// Marker on the local validation / submission status line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsEditStatusLine;

/// The GEOIP fallback trigger toggle, carrying the rendered state.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsEditGeoipToggle(pub bool);

/// Text line showing whether the GEOIP trigger is on.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DnsEditGeoipStatus;

/// Rows of the editable card: field key, placeholder, quick-append templates.
const EDIT_ROWS: [(DnsFormField, &str, &[&str]); 8] = [
    (
        DnsFormField::BootstrapNameserver,
        "223.5.5.5, 119.29.29.29",
        &["223.5.5.5", "119.29.29.29"],
    ),
    (
        DnsFormField::Nameserver,
        "https://dns.google/dns-query, 1.1.1.1",
        &["https://doh.pub/dns-query", "tls://223.5.5.5:853"],
    ),
    (
        DnsFormField::Fallback,
        "https://1.0.0.1/dns-query",
        &["8.8.8.8", "tls://1.0.0.1:853"],
    ),
    (
        DnsFormField::FallbackTriggerIp,
        "240.0.0.0/4, 192.168.0.0/16",
        &["240.0.0.0/4", "192.168.0.0/16"],
    ),
    (
        DnsFormField::FakeIpRange,
        "198.18.0.1/16",
        &["198.18.0.1/16"],
    ),
    (
        DnsFormField::FakeIpFilter,
        "*.lan, localhost.ptlogin2.qq.com",
        &["*.lan", "*.local"],
    ),
    (
        DnsFormField::ProxyServerNameserver,
        "tls://223.5.5.5:853",
        &["tls://223.5.5.5:853", "https://doh.pub/dns-query"],
    ),
    (
        DnsFormField::DirectNameserver,
        "system",
        &["system", "223.5.5.5"],
    ),
];

/// The Bevy workbench edit card (upstream lists, GEOIP trigger, apply).
pub fn dns_edit_card_scene(projection: &DnsProjection, palette: &UiPalette) -> impl Scene + use<> {
    let trigger_count = projection
        .form
        .fallback_policy
        .policy()
        .trigger_ipcidr
        .len();
    let geoip_details = format!("fallback_filter.ipcidr 触发网段 {trigger_count} 条");
    let rows: Vec<Box<dyn Scene>> = EDIT_ROWS
        .iter()
        .map(|(field, placeholder, templates)| {
            Box::new(edit_field_row(
                projection,
                *field,
                placeholder,
                templates,
                palette,
            )) as Box<dyn Scene>
        })
        .collect();

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                Children [
                    (
                        Text({ "上游加密 DNS 配置与回退策略 (DUAL-14-04/05)".to_owned() })
                        TextRole(Role::BodyStrong)
                    ),
                    ( Text({ "DoH / DoT / DoQ · fallback-filter".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                Children [
                    { rows },
                    ( { geoip_toggle_row(projection, geoip_details, palette) } ),
                    ( { edit_apply_row(palette) } ),
                ]
            }),
        ],
        palette,
    )
}

fn edit_field_row(
    projection: &DnsProjection,
    field: DnsFormField,
    placeholder: &'static str,
    templates: &'static [&'static str],
    palette: &UiPalette,
) -> impl Scene + use<> {
    let value = projection.form.raw(field).unwrap_or_default().to_owned();
    let label = field_label(field);
    let chips: Vec<Box<dyn Scene>> = templates
        .iter()
        .map(|server| Box::new(template_chip(field, server, &value, palette)) as Box<dyn Scene>)
        .collect();

    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S4),
        }
        Children [
            ( Text(label) TextRole(Role::Caption) ),
            (
                Node { width: percent(100) }
                DnsEditField(field)
                Children [
                    ( { text_field_with_placeholder_scene(value, placeholder.to_owned(), palette) } ),
                ]
            ),
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S4),
                }
                Children [ { chips } ]
            ),
        ]
    }
}

fn template_chip(
    field: DnsFormField,
    server: &'static str,
    current: &str,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let added = infiltrator_contract::dns::parse_server_list(current)
        .iter()
        .any(|entry| entry.eq_ignore_ascii_case(server));
    let bg = if added {
        palette.surface
    } else {
        palette.surface_elevated
    };
    let ink = if added {
        palette.ink_dim
    } else {
        palette.accent
    };
    let label = format!("+ {server}");

    bsn! {
        Node {
            padding: UiRect::axes(Val::Px(space::S6), Val::Px(space::S2)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            align_items: AlignItems::Center,
        }
        BackgroundColor({ bg })
        DnsEditTemplate { field, server }
        Button
        Children [
            ( Text(label) TextRole(Role::Caption) TextColor({ ink }) ),
        ]
    }
}

fn geoip_toggle_row(
    projection: &DnsProjection,
    details: String,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let enabled = projection.form.fallback_policy.geoip;
    let track = if enabled {
        palette.accent
    } else {
        palette.surface_elevated
    };
    let edge = if enabled {
        palette.accent
    } else {
        palette.border
    };
    let knob = if enabled {
        palette.on_accent
    } else {
        palette.ink_dim
    };
    let knob_left = if enabled { Val::Px(18.0) } else { Val::Px(2.0) };
    let status = geoip_status_label(enabled);
    let status_color = if enabled {
        palette.success
    } else {
        palette.ink_dim
    };

    bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::all(Val::Px(space::S8)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        Children [
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S2),
                }
                Children [
                    ( Text({ "GEOIP 触发回退 (fallback_filter.geoip)".to_owned() }) TextRole(Role::Body) ),
                    ( Text(details) TextRole(Role::Caption) ),
                ]
            ),
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    ( Text(status) DnsEditGeoipStatus TextRole(Role::Caption) TextColor({ status_color }) ),
                    (
                        Node {
                            width: px(38.0),
                            height: px(22.0),
                            border: UiRect::all(Val::Px(palette.hairline_px)),
                            border_radius: BorderRadius::all(Val::Px(11.0)),
                            position_type: PositionType::Relative,
                            align_items: AlignItems::Center,
                        }
                        BackgroundColor({ track })
                        BorderColor {
                            top: edge,
                            right: edge,
                            bottom: edge,
                            left: edge,
                        }
                        DnsEditGeoipToggle(enabled)
                        Button
                        Children [
                            (
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: { knob_left },
                                    width: px(16.0),
                                    height: px(16.0),
                                    border_radius: BorderRadius::all(Val::Px(8.0)),
                                }
                                BackgroundColor({ knob })
                            ),
                        ]
                    ),
                ]
            ),
        ]
    }
}

fn edit_apply_row(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::top(Val::Px(space::S8)),
        }
        Children [
            (
                Text({ "本地校验通过后提交共享 ApplyDnsSettings 补丁".to_owned() })
                DnsEditStatusLine
                TextRole(Role::Caption)
                TextColor({ palette.ink_dim })
            ),
            (
                Node {
                    min_height: px(palette.control_height_px),
                    padding: UiRect::horizontal(Val::Px(space::S12)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                }
                BackgroundColor({ palette.accent })
                DnsEditApplyButton
                Button
                Children [
                    ( Text({ "应用 DNS 表单 (Apply)".to_owned() }) TextRole(Role::BodyStrong) ),
                ]
            ),
        ]
    }
}

/// Field row labels (bare Chinese follows the Bevy page convention).
fn field_label(field: DnsFormField) -> String {
    match field {
        DnsFormField::BootstrapNameserver => "default_nameserver (bootstrap, 仅纯 IP)".to_owned(),
        DnsFormField::Nameserver => "nameserver (DoH / DoT / DoQ / UDP)".to_owned(),
        DnsFormField::Fallback => "fallback (回退解析服务器)".to_owned(),
        DnsFormField::FallbackTriggerIp => "fallback_filter.ipcidr (GEOIP 触发网段)".to_owned(),
        DnsFormField::FakeIpRange => "fake_ip_range".to_owned(),
        DnsFormField::FakeIpFilter => "fake_ip_filter".to_owned(),
        DnsFormField::ProxyServerNameserver => "proxy_server_nameserver".to_owned(),
        DnsFormField::DirectNameserver => "direct_nameserver".to_owned(),
        other => other.key().to_owned(),
    }
}

/// Honest local issue text for the Bevy status line.
pub fn issue_label(issue: &DnsFormIssue) -> String {
    match issue {
        DnsFormIssue::UnsupportedScheme { field, entry } => {
            format!("{} 上游协议不受支持: {}", field.key(), entry)
        }
        DnsFormIssue::BootstrapNotIp { entry } => {
            format!("bootstrap 解析器必须是纯 IP: {entry}")
        }
        DnsFormIssue::InvalidTriggerCidr { entry } => {
            format!("GEOIP 触发网段不是合法 CIDR: {entry}")
        }
        DnsFormIssue::InvalidGeoipCode { value } => {
            format!("geoip-code 必须是两位国家代码: {value}")
        }
    }
}

/// The status line text for the current draft state.
pub fn dns_edit_status_label(state: &DnsFormState) -> String {
    if let Some(issue) = state.issues.first() {
        return format!("本地校验未通过: {}", issue_label(issue));
    }
    if state.dirty {
        return "有未应用的修改，点击应用提交共享补丁".to_owned();
    }
    if state.submitted {
        return "已提交共享 DNS 工作台补丁".to_owned();
    }
    "表单与当前配置一致".to_owned()
}

fn geoip_status_label(enabled: bool) -> String {
    if enabled {
        "已开启".to_owned()
    } else {
        "已关闭".to_owned()
    }
}

fn read_fields(
    fields: &Query<(&Children, &DnsEditField)>,
    text_fields: &mut Query<&mut TextField>,
    form: &mut DnsWorkbenchForm,
) {
    for (children, marker) in fields.iter() {
        for child in children.iter() {
            if let Ok(field) = text_fields.get_mut(*child) {
                form.set_raw(marker.0, field.0.text().to_owned());
            }
        }
    }
}

fn restamp_fields(
    fields: &Query<(&Children, &DnsEditField)>,
    text_fields: &mut Query<&mut TextField>,
    form: &DnsWorkbenchForm,
) {
    for (children, marker) in fields.iter() {
        let Some(value) = form.raw(marker.0) else {
            continue;
        };
        for child in children.iter() {
            if let Ok(mut field) = text_fields.get_mut(*child)
                && field.0.text() != value
            {
                field.0.apply(TextFieldInput::SetText(value.to_owned()));
            }
        }
    }
}

/// Per-frame honest dirty flag: any field text that differs from the draft.
pub fn sync_dns_edit_dirty(
    fields: Query<(&Children, &DnsEditField)>,
    text_fields: Query<&TextField>,
    mut status_lines: Query<&mut Text, StatusLineFilter>,
    mut state: ResMut<DnsFormState>,
) {
    let mut dirty = false;
    for (children, marker) in fields.iter() {
        let Some(raw) = state.form.raw(marker.0) else {
            continue;
        };
        for child in children.iter() {
            if let Ok(field) = text_fields.get(*child)
                && field.0.text() != raw
            {
                dirty = true;
            }
        }
    }
    state.dirty = dirty;
    let label = dns_edit_status_label(&state);
    for mut text in status_lines.iter_mut() {
        if text.0 != label {
            text.0 = label.clone();
        }
    }
}

type StatusLineFilter = (
    With<DnsEditStatusLine>,
    Without<DnsEditGeoipStatus>,
    Without<DnsEditField>,
);

/// Activation seam: quick-append chips, the GEOIP toggle and apply.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn on_dns_edit_activated(
    activate: On<Activate>,
    templates: Query<&DnsEditTemplate>,
    geoip_toggles: Query<&DnsEditGeoipToggle>,
    apply_buttons: Query<(), With<DnsEditApplyButton>>,
    fields: Query<(&Children, &DnsEditField)>,
    mut text_fields: Query<&mut TextField>,
    mut geoip_status: Query<&mut Text, (With<DnsEditGeoipStatus>, Without<DnsEditStatusLine>)>,
    mut status_lines: Query<&mut Text, StatusLineFilter>,
    mut state: Option<ResMut<DnsFormState>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(state) = state.as_deref_mut() else {
        return;
    };
    if let Ok(template) = templates.get(activate.entity) {
        let current = state
            .form
            .raw(template.field)
            .unwrap_or_default()
            .to_owned();
        let next = append_server(&current, template.server);
        state.form.set_raw(template.field, next);
        state.submitted = false;
        restamp_fields(&fields, &mut text_fields, &state.form);
        restamp_status(state, &mut status_lines);
        return;
    }
    if let Ok(toggle) = geoip_toggles.get(activate.entity) {
        let enabled = !toggle.0;
        state.form.fallback_policy.geoip = enabled;
        state.submitted = false;
        for mut text in &mut geoip_status {
            text.0 = geoip_status_label(enabled);
        }
        restamp_status(state, &mut status_lines);
        return;
    }
    if apply_buttons.get(activate.entity).is_err() {
        return;
    }
    read_fields(&fields, &mut text_fields, &mut state.form);
    state.issues = state.form.validate();
    if state.issues.is_empty() {
        if let Some(handle) = handle {
            handle.submit(UiCommand::ApplyDnsSettings {
                patch: state.form.patch(),
            });
        }
        state.submitted = true;
    } else {
        state.submitted = false;
    }
    restamp_status(state, &mut status_lines);
}

/// Projection seam: re-seed the draft while the user has no pending edits.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn apply_dns_edit_projection(
    update: On<DnsProjectionUpdated>,
    fields: Query<(&Children, &DnsEditField)>,
    mut text_fields: Query<&mut TextField>,
    mut geoip_toggles: Query<(
        &mut BackgroundColor,
        &mut BorderColor,
        &mut DnsEditGeoipToggle,
    )>,
    mut geoip_status: Query<&mut Text, (With<DnsEditGeoipStatus>, Without<DnsEditStatusLine>)>,
    mut status_lines: Query<&mut Text, StatusLineFilter>,
    palette: Res<UiPalette>,
    mut state: Option<ResMut<DnsFormState>>,
) {
    let Some(state) = state.as_deref_mut() else {
        return;
    };
    if !state.dirty {
        state.form = update.0.form.clone();
        state.issues.clear();
        state.submitted = false;
        restamp_fields(&fields, &mut text_fields, &state.form);
    }
    let enabled = state.form.fallback_policy.geoip;
    for (mut background, mut border, mut toggle) in &mut geoip_toggles {
        toggle.0 = enabled;
        background.0 = if enabled {
            palette.accent
        } else {
            palette.surface_elevated
        };
        let edge = if enabled {
            palette.accent
        } else {
            palette.border
        };
        border.top = edge;
        border.right = edge;
        border.bottom = edge;
        border.left = edge;
    }
    for mut text in &mut geoip_status {
        text.0 = geoip_status_label(enabled);
    }
    restamp_status(state, &mut status_lines);
}

fn restamp_status<F: bevy::ecs::query::QueryFilter>(
    state: &DnsFormState,
    lines: &mut Query<&mut Text, F>,
) {
    let label = dns_edit_status_label(state);
    for mut text in lines.iter_mut() {
        text.0 = label.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_line_reports_issues_dirty_and_submitted() {
        let mut state = DnsFormState::default();
        assert_eq!(dns_edit_status_label(&state), "表单与当前配置一致");
        state.dirty = true;
        assert!(dns_edit_status_label(&state).contains("未应用"));
        state.dirty = false;
        state.submitted = true;
        assert!(dns_edit_status_label(&state).contains("已提交"));
        state.submitted = false;
        state.form.nameserver = "ftp://dns.example".to_owned();
        state.issues = state.form.validate();
        assert!(dns_edit_status_label(&state).contains("本地校验未通过"));
    }

    #[test]
    fn template_chip_append_stays_unique() {
        let next = append_server("1.1.1.1", "8.8.8.8");
        assert_eq!(next, "1.1.1.1, 8.8.8.8");
        assert_eq!(append_server(&next, "8.8.8.8"), next);
    }

    #[test]
    fn edit_rows_cover_the_ten_editable_fields() {
        let covered: Vec<DnsFormField> = EDIT_ROWS.iter().map(|(field, ..)| *field).collect();
        assert_eq!(covered.len(), 8);
        for field in [
            DnsFormField::BootstrapNameserver,
            DnsFormField::Nameserver,
            DnsFormField::Fallback,
            DnsFormField::FallbackTriggerIp,
            DnsFormField::FakeIpRange,
            DnsFormField::FakeIpFilter,
            DnsFormField::ProxyServerNameserver,
            DnsFormField::DirectNameserver,
        ] {
            assert!(covered.contains(&field), "{field:?} row missing");
        }
    }
}
