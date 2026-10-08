//! Custom Node Editor & Universal URI Codec scene component (自定义节点与 URI 编解码).
//!
//! DUAL-05: the card is a projection of the shared
//! `ProtocolStudioSnapshot` published by
//! `infiltrator_application::protocol_codec_application`. The chip row carries
//! the typed Shadowsocks cipher family (05-01), the VLESS REALITY/Vision
//! block (05-02) and the multiplexing parameters (05-11); the footer carries
//! the codec audit and the URI fidelity gaps the application measured.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::business_panel::{BusinessPanelState, PanelKind};
use crate::pages::proxies::ProxiesProjectionUpdated;
use crate::pages::proxies_form::{
    CustomNodeForm, CustomNodeFormError, PendingProtocolImport, PendingProtocolSave,
    ProtocolFieldNode, ProtocolFormText, ProtocolFormTextKind, ProtocolToggle,
    protocol_fields_scene, stage_native_fields, text_field_under,
};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{BackgroundColor, FlexDirection, FlexWrap, Node, UiRect, percent, px};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::protocol_codec_application::ProtocolCodecApplication;
use infiltrator_application::protocol_form::field_projection;
use infiltrator_application::protocol_studio_projection::slot_text;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::{TextField, text_field_with_placeholder_scene};
use infiltrator_contract::protocol_fidelity::ProtocolStudioSnapshot;
use infiltrator_contract::protocol_form::ProtocolStudioSlot;
use infiltrator_shared::locales::{Lang, Localizer, get_system_language};
use std::env;

/// Marker for custom node editor card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CustomNodeEditorRoot;

/// Marker for import URI button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportUriButton;

/// Marker for save custom node button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveCustomNodeButton;

/// DUAL-05: the draft URI input field.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CustomNodeUriField;

/// DUAL-05-09: the dialer hop input field (`dialer-proxy`).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CustomNodeDialerField;

/// DUAL-05-13: the custom-CA path input field (`ca`).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CustomNodeCaField;

/// DUAL-05-09/10: run the shared dialer-chain scan.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScanDialerChainsButton;

/// DUAL-05-13: resolve the CA request against this host's reader.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VerifyCustomNodeCaButton;

/// Which shared action a card button submits. One component keeps the system
/// signature inside the ECS argument budget.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CustomNodeActionButton(pub CustomNodeAction);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CustomNodeAction {
    #[default]
    ImportUri,
    ExportUri,
    SaveDraft,
    ScanDialer,
    VerifyCa,
}

/// Which editable field a text input is. One component keeps the system
/// signature inside the ECS argument budget while the per-field markers stay
/// available to tests and layout code.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CustomNodeInputField(pub CustomNodeInput);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CustomNodeInput {
    #[default]
    Uri,
    Dialer,
    Ca,
}

/// DUAL-05: read-only text slots re-covered from the shared studio snapshot.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CustomNodeText(pub ProtocolStudioSlot);

fn active_language() -> String {
    env::var("INFILTRATOR_LANG").unwrap_or_else(|_| get_system_language())
}

pub(super) fn slot_initial(slot: ProtocolStudioSlot, studio: &ProtocolStudioSnapshot) -> String {
    let language = active_language();
    slot_text(
        slot,
        studio,
        |key| Lang(&language).tr(key).into_owned(),
        !matches!(language.as_str(), "en" | "en-US"),
    )
}

/// Custom Node Editor scene.
pub fn custom_node_scene(
    studio: &ProtocolStudioSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let language = active_language();
    let lang = Lang(&language);
    let slots: Vec<Box<dyn Scene>> = ProtocolStudioSlot::ALL.iter().filter(|slot| **slot != ProtocolStudioSlot::UriPreview).map(|slot| {
        Box::new(bsn! {
            Node { width: percent(100), flex_shrink: 0.0 }
            Children [ Text({ slot_initial(*slot, studio) }) CustomNodeText({ *slot }) ProtocolFormText({ ProtocolFormTextKind::Fact(*slot) }) TextRole(Role::Caption) ]
        }) as Box<dyn Scene>
    }).collect();
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(12.0), flex_shrink: 0.0 }
        CustomNodeEditorRoot
        Children [
            Node { width: percent(100), flex_shrink: 0.0 }
            Children [ Text({ studio.last_error.clone().unwrap_or_default() }) CustomNodeFormError ProtocolFormText(ProtocolFormTextKind::Error) TextRole(Role::Body) ]
            --
            Node { width: percent(100), flex_shrink: 0.0 }
            CustomNodeUriField CustomNodeInputField(CustomNodeInput::Uri)
            Children [ @{ text_field_with_placeholder_scene(String::new(), lang.tr("custom_node_uri_placeholder").into_owned(), palette) } ]
            --
            Node { width: percent(100), flex_wrap: FlexWrap::Wrap, column_gap: px(8.0), row_gap: px(8.0), flex_shrink: 0.0 }
            Children [
                Node { min_height: px(palette.control_height_px), padding: UiRect::all(px(8.0)) }
                BackgroundColor({ palette.accent }) Button ImportUriButton CustomNodeActionButton(CustomNodeAction::ImportUri)
                Children [ Text({ lang.tr("custom_node_btn_import_uri").into_owned() }) TextRole(Role::Body) ]
                --
                Node { min_height: px(palette.control_height_px), padding: UiRect::all(px(8.0)) }
                BackgroundColor({ palette.surface_elevated }) Button CustomNodeActionButton(CustomNodeAction::ExportUri)
                Children [ Text({ lang.tr("custom_node_btn_export_uri").into_owned() }) TextRole(Role::Body) ]
                --
                Node { min_height: px(palette.control_height_px), padding: UiRect::all(px(8.0)) }
                BackgroundColor({ palette.success }) Button SaveCustomNodeButton CustomNodeActionButton(CustomNodeAction::SaveDraft)
                Children [ Text({ lang.tr("btn_save").into_owned() }) TextRole(Role::BodyStrong) ]
            ]
            --
            Node { width: percent(100), flex_shrink: 0.0 }
            Children [ Text({ slot_initial(ProtocolStudioSlot::UriPreview, studio) }) CustomNodeText(ProtocolStudioSlot::UriPreview) ProtocolFormText(ProtocolFormTextKind::Fact(ProtocolStudioSlot::UriPreview)) TextRole(Role::Caption) ]
            --
            @{ protocol_fields_scene(studio, palette) }
            --
            { slots }
            --
            Node { width: percent(100), flex_wrap: FlexWrap::Wrap, column_gap: px(8.0), row_gap: px(8.0), flex_shrink: 0.0 }
            Children [
                Node { min_height: px(palette.control_height_px), padding: UiRect::all(px(8.0)) }
                Button ScanDialerChainsButton CustomNodeActionButton(CustomNodeAction::ScanDialer)
                Children [ Text({ lang.tr("custom_node_dialer_scan").into_owned() }) TextRole(Role::Body) ]
                --
                Node { min_height: px(palette.control_height_px), padding: UiRect::all(px(8.0)) }
                Button VerifyCustomNodeCaButton CustomNodeActionButton(CustomNodeAction::VerifyCa)
                Children [ Text({ lang.tr("custom_node_ca_verify").into_owned() }) TextRole(Role::Body) ]
            ]
        ]
    }
}

/// DUAL-05-14: the import button reads the typed URI from the shared field and
/// submits the decode intent; the save button submits the *shared draft* it
/// received from the application. Neither button fabricates a node locally.
///
/// DUAL-05-09/13: the scan button first applies the dialer hop field to the
/// shared draft (typed field edit) and then runs the shared analyzer; the
/// verify button applies the CA path field and resolves the trust request
/// against this host's reader.
#[derive(SystemParam)]
pub(crate) struct CustomNodeControls<'w, 's> {
    buttons: Query<'w, 's, &'static CustomNodeActionButton>,
    wrappers: Query<'w, 's, (Entity, &'static ProtocolFieldNode)>,
    fields: Query<'w, 's, (&'static CustomNodeInputField, &'static Children)>,
    text_fields: Query<'w, 's, &'static TextField>,
    toggles: Query<'w, 's, &'static ProtocolToggle>,
    children: Query<'w, 's, &'static Children>,
}

pub(crate) fn on_custom_node_action_activated(
    activate: On<Activate>,
    controls: CustomNodeControls,
    mut form: ResMut<CustomNodeForm>,
    panel: Res<BusinessPanelState>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    if panel.0 != Some(PanelKind::CustomNode) || form.saving.is_some() {
        return;
    }
    if let Ok(toggle) = controls.toggles.get(activate.entity) {
        if let Some(draft) = &form.studio.draft {
            let projected = field_projection(toggle.0, draft).expect("toggle field");
            let raw = if projected.value == "true" {
                "false"
            } else {
                "true"
            };
            form.stage(toggle.0, raw.to_owned());
        }
        return;
    }
    let Some(handle) = handle else {
        return;
    };
    let Some(action) = controls
        .buttons
        .get(activate.entity)
        .ok()
        .map(|button| button.0)
    else {
        return;
    };
    if form.reset_fields && action != CustomNodeAction::ImportUri {
        return;
    }
    if !form.reset_fields {
        stage_native_fields(
            &mut form,
            &controls.wrappers,
            &controls.children,
            &controls.text_fields,
        );
    }
    let field_text = |wanted: CustomNodeInput| -> String {
        controls
            .fields
            .iter()
            .find(|(kind, _)| kind.0 == wanted)
            .map(|(_, children)| children)
            .into_iter()
            .flat_map(|children| children.iter())
            .find_map(|child| text_field_under(child, &controls.children, &controls.text_fields))
            .and_then(|entity| controls.text_fields.get(entity).ok())
            .map(|field| field.0.text())
            .unwrap_or_default()
            .trim()
            .to_owned()
    };
    match action {
        CustomNodeAction::ImportUri => {
            let uri = field_text(CustomNodeInput::Uri);
            if !uri.is_empty() && form.importing.is_none() {
                let command = UiCommand::ImportCustomNodeUri { uri: uri.clone() };
                if let Some(request_id) = handle.submit_tracked(command) {
                    form.importing = Some(PendingProtocolImport { request_id, uri });
                } else {
                    form.studio.last_error = Some(
                        Lang(&active_language())
                            .tr("protocol_form_no_feedback")
                            .into_owned(),
                    );
                }
            }
        }
        CustomNodeAction::ExportUri => {
            if let Some(draft) = &form.studio.draft {
                match ProtocolCodecApplication::uri_from_draft(draft) {
                    Ok(uri) => form.studio.uri_preview = Some(uri),
                    Err(error) => form.studio.last_error = Some(error.message),
                }
            }
        }
        CustomNodeAction::ScanDialer => {
            let Some(draft) = form.studio.draft.clone() else {
                return;
            };
            handle.submit(UiCommand::PrepareCustomNodeDraft {
                draft: Box::new(draft),
            });
            handle.submit(UiCommand::ScanCustomNodeDialer);
        }
        CustomNodeAction::VerifyCa => {
            let Some(draft) = form.studio.draft.as_ref() else {
                return;
            };
            handle.submit(UiCommand::VerifyCustomNodeCa {
                trust: Box::new(draft.params.tls_trust.clone()),
            });
        }
        CustomNodeAction::SaveDraft => {
            if form.saving.is_some() {
                return;
            }
            let draft = form.studio.draft.clone();
            let Some(draft) = draft else {
                return;
            };
            if !form.inputs.errors.is_empty() {
                return;
            }
            if !draft.report().is_valid() {
                form.studio.last_error = Some(draft.report().issue_lines().join("; "));
                return;
            }
            let command = UiCommand::SaveCustomNodeDraft {
                draft: Box::new(draft.clone()),
            };
            if let Some(request_id) = handle.submit_tracked(command) {
                form.saving = Some(PendingProtocolSave { request_id, draft });
            } else {
                form.studio.last_error = Some(
                    Lang(&active_language())
                        .tr("protocol_form_no_feedback")
                        .into_owned(),
                );
            }
        }
    }
}

/// Re-cover every custom-node text slot from the shared studio snapshot. Runs
/// as an observer on the same projection event so it never fights the page
/// restamp queries.
pub(crate) fn sync_custom_node_studio(
    update: On<ProxiesProjectionUpdated>,
    mut texts: Query<(&mut Text, &CustomNodeText)>,
    mut form: ResMut<CustomNodeForm>,
    panel: Res<BusinessPanelState>,
) {
    let studio = &update.0.custom_node;
    if panel.0 != Some(PanelKind::CustomNode) {
        form.load(studio.clone());
    } else {
        form.studio.dialer = studio.dialer.clone();
        form.studio.ca_trust = studio.ca_trust.clone();
    }
    for (mut text, slot) in &mut texts {
        text.0 = slot_initial(slot.0, studio);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_initial_reports_shared_facts_and_honest_empty_states() {
        let empty = ProtocolStudioSnapshot::default();
        let language = active_language();
        let lang = Lang(&language);
        assert_eq!(
            slot_initial(ProtocolStudioSlot::Chips, &empty),
            lang.tr("protocol_form_no_draft").as_ref()
        );
        assert_eq!(
            slot_initial(ProtocolStudioSlot::UriPreview, &empty),
            lang.tr("protocol_form_no_preview").as_ref()
        );
        assert_eq!(
            slot_initial(ProtocolStudioSlot::Audit, &empty),
            lang.tr("protocol_form_no_codec").as_ref()
        );
        assert_eq!(
            slot_initial(ProtocolStudioSlot::Gaps, &empty),
            lang.tr("protocol_form_no_gaps").as_ref()
        );
    }
}
