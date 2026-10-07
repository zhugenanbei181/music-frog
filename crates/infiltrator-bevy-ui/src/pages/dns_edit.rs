//! Bevy DNS workbench editable fields (DUAL-14-04 / 14-05 / 14-14).
//!
//! The draft is the shared [`DnsWorkbenchForm`]; this module owns only the
//! Bevy widgets (text fields, quick-append chips, the GEOIP trigger toggle and
//! the apply button), the honest local validation line, and the shared patch
//! submission. Both surfaces submit the same patch through
//! `UiCommand::ApplyDnsSettings`.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::dns::{DnsProjection, DnsProjectionUpdated};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut, SystemParam};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, PositionType,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::dns_status_projection::{field_label_key, form_status};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, text_field_with_placeholder_scene};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::dns::{append_server, parse_server_list};
use infiltrator_contract::dns_form::{DnsFormField, DnsFormIssue, DnsWorkbenchForm};
use infiltrator_shared::locales::{Lang, Localizer};

/// Shared workbench draft plus the honest local validation state.
#[derive(Resource, Clone, Debug, Default)]
pub struct DnsFormState {
    pub form: DnsWorkbenchForm,
    pub dirty: bool,
    pub issues: Vec<DnsFormIssue>,
    pub submitted: bool,
    pub host_missing: bool,
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

#[derive(Component, Clone, Copy, Default)]
pub struct DnsEditTriggerCount;

/// Rows of the editable card: field key, placeholder, quick-append templates.
const EDIT_ROWS: [(DnsFormField, &str, &[&str]); 9] = [
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
    (DnsFormField::FallbackGeoipCode, "CN", &[]),
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
    let geoip_details = LocalizedText::new(
        "dns_trigger_count",
        vec![("count", trigger_count.to_string())],
    );
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
                                LocalizedText::plain("dns_upstream_form_title")
                                TextRole(Role::BodyStrong)
                                --
                                Text({ "DoH / DoT / DoQ · fallback-filter".to_owned() }) TextRole(Role::Caption)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S8),
                            }
                            Children [
                                { rows }
                                --
                                @{ geoip_toggle_row(projection, geoip_details, palette) }
                                --
                                @{ edit_apply_row(palette) }
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
    let label = LocalizedText::plain(field_label_key(field));
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
                label TextRole(Role::Caption)
                --
                Node { width: percent(100) }
                DnsEditField(field)
                Children [
                    @{ (text_field_with_placeholder_scene(value, placeholder.to_owned(), palette), bsn! { NativeTextField({100 + field as i32}) }) }
                ]
                --
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S4),
                }
                Children [
                    { chips }
                ]
            ]
    }
}

fn template_chip(
    field: DnsFormField,
    server: &'static str,
    current: &str,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let added = parse_server_list(current)
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
                Text(label) TextRole(Role::Caption) TextColor({ ink })
            ]
    }
}

fn geoip_toggle_row(
    projection: &DnsProjection,
    details: LocalizedText,
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
    let status = geoip_status_label(enabled, UiLocale::default().code());
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
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S2),
                }
                Children [
                    LocalizedText::plain("dns_geoip_fallback_label") TextRole(Role::Body)
                    --
                    details DnsEditTriggerCount TextRole(Role::Caption)
                ]
                --
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    Text(status) DnsEditGeoipStatus TextRole(Role::Caption) TextColor({ status_color })
                    --
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
                        Node {
                            position_type: PositionType::Absolute,
                            left: { knob_left },
                            width: px(16.0),
                            height: px(16.0),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                        }
                        BackgroundColor({ knob })
                    ]
                ]
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
                Text({form_status(&[],false,false,false,UiLocale::default().code())})
                DnsEditStatusLine
                TextRole(Role::Caption)
                TextColor({ palette.ink_dim })
                --
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
                    LocalizedText::plain("dns_apply_form_action") TextRole(Role::BodyStrong)
                ]
            ]
    }
}

/// Replay only the shared fold; queued is not an applied acknowledgement.
pub fn dns_edit_status_label(state: &DnsFormState, code: &str) -> String {
    form_status(
        &state.issues,
        state.dirty,
        state.submitted,
        state.host_missing,
        code,
    )
}
fn geoip_status_label(enabled: bool, code: &str) -> String {
    Lang(code)
        .tr(if enabled {
            "dns_switch_enabled"
        } else {
            "dns_switch_disabled"
        })
        .into_owned()
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
#[derive(SystemParam)]
pub struct DnsEditView<'w, 's> {
    fields: Query<'w, 's, (&'static Children, &'static DnsEditField)>,
    text_fields: Query<'w, 's, &'static TextField>,
    status_lines: Query<'w, 's, &'static mut Text, StatusLineFilter>,
    state: ResMut<'w, DnsFormState>,
    locale: Res<'w, UiLocale>,
    geoip_status: Query<'w, 's, &'static mut Text, GeoipStatusFilter>,
    details: Query<'w, 's, &'static mut LocalizedText, With<DnsEditTriggerCount>>,
    apply: Query<'w, 's, &'static mut ButtonDisabled, With<DnsEditApplyButton>>,
}
pub fn sync_dns_edit_dirty(surface: DnsEditView) {
    let DnsEditView {
        fields,
        text_fields,
        mut status_lines,
        mut state,
        locale,
        mut geoip_status,
        mut details,
        mut apply,
    } = surface;
    let composing = fields
        .iter()
        .flat_map(|(children, _)| children.iter())
        .any(|child| {
            text_fields
                .get(*child)
                .is_ok_and(|field| field.0.is_in_ime_transaction())
        });
    for mut disabled in &mut apply {
        if disabled.0 != composing {
            disabled.0 = composing;
        }
    }
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
    let value = geoip_status_label(state.form.fallback_policy.geoip, locale.code());
    for mut text in &mut geoip_status {
        if text.0 != value {
            text.0 = value.clone();
        }
    }
    let count = state
        .form
        .fallback_policy
        .policy()
        .trigger_ipcidr
        .len()
        .to_string();
    for mut copy in &mut details {
        if copy.params != [("count", count.clone())] {
            copy.params = vec![("count", count.clone())];
        }
    }

    let label = dns_edit_status_label(&state, locale.code());
    for mut text in status_lines.iter_mut() {
        if text.0 != label {
            text.0 = label.clone();
        }
    }
}

#[derive(QueryFilter)]
pub struct StatusLineFilter {
    status: With<DnsEditStatusLine>,
    no_geoip: Without<DnsEditGeoipStatus>,
    no_field: Without<DnsEditField>,
}
#[derive(QueryFilter)]
pub struct GeoipStatusFilter {
    geoip: With<DnsEditGeoipStatus>,
    no_status: Without<DnsEditStatusLine>,
}

/// Activation seam: quick-append chips, the GEOIP toggle and apply.
#[derive(SystemParam)]
pub struct DnsEditInteraction<'w, 's> {
    templates: Query<'w, 's, &'static DnsEditTemplate>,
    geoip_toggles: Query<'w, 's, &'static DnsEditGeoipToggle>,
    apply_buttons: Query<'w, 's, (), With<DnsEditApplyButton>>,
    fields: Query<'w, 's, (&'static Children, &'static DnsEditField)>,
    text_fields: Query<'w, 's, &'static mut TextField>,
    geoip_status: Query<'w, 's, &'static mut Text, GeoipStatusFilter>,
    status_lines: Query<'w, 's, &'static mut Text, StatusLineFilter>,
    state: ResMut<'w, DnsFormState>,
    handle: Option<Res<'w, CommandSinkHandle>>,
    locale: Res<'w, UiLocale>,
}
pub(crate) fn on_dns_edit_activated(activate: On<Activate>, surface: DnsEditInteraction) {
    let DnsEditInteraction {
        templates,
        geoip_toggles,
        apply_buttons,
        fields,
        mut text_fields,
        mut geoip_status,
        mut status_lines,
        mut state,
        handle,
        locale,
    } = surface;
    let state = &mut *state;
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
        restamp_status(state, &mut status_lines, locale.code());
        return;
    }
    if let Ok(toggle) = geoip_toggles.get(activate.entity) {
        let enabled = !toggle.0;
        state.form.fallback_policy.geoip = enabled;
        state.submitted = false;
        for mut text in &mut geoip_status {
            text.0 = geoip_status_label(enabled, locale.code());
        }
        restamp_status(state, &mut status_lines, locale.code());
        return;
    }
    if apply_buttons.get(activate.entity).is_err() {
        return;
    }
    if fields
        .iter()
        .flat_map(|(children, _)| children.iter())
        .any(|child| {
            text_fields
                .get(*child)
                .is_ok_and(|field| field.0.is_in_ime_transaction())
        })
    {
        return;
    }
    read_fields(&fields, &mut text_fields, &mut state.form);
    state.issues = state.form.validate();
    if state.issues.is_empty() {
        if let Some(handle) = handle.as_ref() {
            handle.submit(UiCommand::ApplyDnsSettings {
                patch: state.form.patch(),
            });
        }
        state.host_missing = handle.is_none();
        state.submitted = handle.is_some();
    } else {
        state.submitted = false;
    }
    restamp_status(state, &mut status_lines, locale.code());
}

/// Projection seam: re-seed the draft while the user has no pending edits.
#[derive(SystemParam)]
pub struct DnsEditReplay<'w, 's> {
    fields: Query<'w, 's, (&'static Children, &'static DnsEditField)>,
    text_fields: Query<'w, 's, &'static mut TextField>,
    geoip_toggles: Query<
        'w,
        's,
        (
            &'static mut BackgroundColor,
            &'static mut BorderColor,
            &'static mut DnsEditGeoipToggle,
        ),
    >,
    geoip_status: Query<'w, 's, &'static mut Text, GeoipStatusFilter>,
    status_lines: Query<'w, 's, &'static mut Text, StatusLineFilter>,
    palette: Res<'w, UiPalette>,
    state: ResMut<'w, DnsFormState>,
    locale: Res<'w, UiLocale>,
}
pub(crate) fn apply_dns_edit_projection(update: On<DnsProjectionUpdated>, surface: DnsEditReplay) {
    let DnsEditReplay {
        fields,
        mut text_fields,
        mut geoip_toggles,
        mut geoip_status,
        mut status_lines,
        palette,
        mut state,
        locale,
    } = surface;
    let state = &mut *state;
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
        text.0 = geoip_status_label(enabled, locale.code());
    }
    restamp_status(state, &mut status_lines, locale.code());
}

fn restamp_status<F: QueryFilter>(
    state: &DnsFormState,
    lines: &mut Query<&mut Text, F>,
    code: &str,
) {
    let label = dns_edit_status_label(state, code);
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
        assert_eq!(dns_edit_status_label(&state, "zh-CN"), "表单与当前配置一致");
        state.dirty = true;
        assert!(dns_edit_status_label(&state, "zh-CN").contains("未应用"));
        state.dirty = false;
        state.submitted = true;
        assert!(dns_edit_status_label(&state, "zh-CN").contains("已提交"));
        state.submitted = false;
        state.form.nameserver = "ftp://dns.example".to_owned();
        state.issues = state.form.validate();
        assert!(dns_edit_status_label(&state, "zh-CN").contains("本地校验未通过"));
    }

    #[test]
    fn template_chip_append_stays_unique() {
        let next = append_server("1.1.1.1", "8.8.8.8");
        assert_eq!(next, "1.1.1.1, 8.8.8.8");
        assert_eq!(append_server(&next, "8.8.8.8"), next);
    }

    #[test]
    fn edit_rows_cover_every_shared_raw_field() {
        let covered: Vec<DnsFormField> = EDIT_ROWS.iter().map(|(field, ..)| *field).collect();
        let form = DnsWorkbenchForm::default();
        let expected: Vec<_> = DnsFormField::ALL
            .into_iter()
            .filter(|field| form.raw(*field).is_some())
            .collect();
        assert_eq!(covered.len(), expected.len());
        for field in expected {
            assert!(covered.contains(&field), "{field:?} row missing");
        }
    }
}
