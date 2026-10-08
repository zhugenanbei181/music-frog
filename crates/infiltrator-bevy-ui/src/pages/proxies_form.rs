//! Native full protocol form. Staged input never writes a profile.
use crate::command::UiCommand;
use crate::command_events::CommandExecutedEvent;
use crate::pages::business_panel::{BusinessPanelRoot, BusinessPanelState, PanelKind};
use crate::pages::proxies_custom::{
    CustomNodeCaField, CustomNodeDialerField, CustomNodeInput, CustomNodeInputField,
    CustomNodeUriField, slot_initial,
};
use bevy::ecs::prelude::*;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::*;
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_application::protocol_codec_application::{
    ProtocolCodecApplication, studio_snapshot,
};
use infiltrator_application::protocol_form::{
    ProtocolInputs, field_projection, special_projection, visible,
};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::state::{TextFieldInput, ValidationStatus};
use infiltrator_bevy_widgets::text_input::text_field_with_placeholder_scene;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::protocol_fidelity::{ProtocolDraft, ProtocolStudioSnapshot};
use infiltrator_contract::protocol_form::{ProtocolField, ProtocolStudioSlot};
use infiltrator_shared::locales::{Lang, Localizer};

#[derive(Clone, Debug)]
pub struct PendingProtocolSave {
    pub request_id: RequestId,
    pub draft: ProtocolDraft,
}
#[derive(Clone, Debug)]
pub struct PendingProtocolImport {
    pub request_id: RequestId,
    pub uri: String,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct CustomNodeForm {
    pub studio: ProtocolStudioSnapshot,
    pub inputs: ProtocolInputs,
    pub reset_fields: bool,
    pub reset_uri: bool,
    pub saving: Option<PendingProtocolSave>,
    pub importing: Option<PendingProtocolImport>,
}
impl CustomNodeForm {
    pub fn begin(&mut self) {
        self.studio = ProtocolCodecApplication::project_draft(ProtocolDraft::new("vless"), None);
        self.inputs = ProtocolInputs::default();
        self.reset_fields = true;
        self.reset_uri = true;
        self.saving = None;
        self.importing = None;
    }
    pub fn load(&mut self, studio: ProtocolStudioSnapshot) {
        self.studio = studio;
        self.inputs = ProtocolInputs::default();
        self.reset_fields = true;
    }
    pub fn stage(&mut self, id: ProtocolField, raw: String) {
        let Some(draft) = self.studio.draft.as_ref() else {
            return;
        };
        match self.inputs.edit(draft, id, raw) {
            Ok(next) => {
                let preview = ProtocolCodecApplication::uri_from_draft(&next).ok();
                let mut studio = ProtocolCodecApplication::project_draft(next, preview);
                studio.dialer = self.studio.dialer.clone();
                studio.ca_trust = self.studio.ca_trust.clone();
                self.studio = studio;
            }
            Err(error) => self.studio.last_error = Some(error.message),
        }
    }
}
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProtocolFieldNode(pub ProtocolField);
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProtocolToggle(pub ProtocolField);
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProtocolFormText(pub ProtocolFormTextKind);
#[derive(Clone, Copy, Debug, Default)]
pub enum ProtocolFormTextKind {
    #[default]
    Error,
    Fact(ProtocolStudioSlot),
    Toggle(ProtocolField),
}
#[derive(Component, Clone, Copy, Default)]
pub struct CustomNodeFormError;

pub fn protocol_fields_scene(
    studio: &ProtocolStudioSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let language = UiLocale::default().code().to_string();
    let lang = Lang(&language);
    let fallback = ProtocolDraft::new("vless");
    let draft = studio.draft.as_ref().unwrap_or(&fallback);
    let fields: Vec<Box<dyn Scene>> = ProtocolField::ALL.iter().copied().map(|id| {
        let field = field_projection(id, draft).unwrap_or_else(|| special_projection(id, draft));
        let label = lang.tr(field.label_key).into_owned();
        let display = if visible(id, draft) { Display::Flex } else { Display::None };
        let mut scene: Box<dyn Scene> = if field.toggle {
            Box::new(bsn! {
                Node { width: percent(100), min_height: px(palette.control_height_px), flex_shrink: 0.0,
                    padding: UiRect::all(px(8.0)), align_items: AlignItems::Center }
                BackgroundColor({ palette.surface_elevated }) Button ProtocolToggle(id)
                Children [ Text({ format!("{label}: {}", field.value) }) ProtocolFormText(ProtocolFormTextKind::Toggle(id)) TextRole(Role::Body) ]
            })
        } else {
            Box::new(bsn! {
                Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(4.0), flex_shrink: 0.0 }
                Children [
                    Text(label) TextRole(Role::Caption)
                    --
                    Node { width: percent(100), flex_shrink: 0.0 }
                    Children [ @{ text_field_with_placeholder_scene(field.value.clone(), String::new(), palette) } ]
                ]
            })
        };
        if id == ProtocolField::Dialer {
            scene = Box::new(bsn! {
                Node { width: percent(100), flex_direction: FlexDirection::Column, flex_shrink: 0.0, row_gap: px(4.0) }
                Children [ Text({ lang.tr("custom_node_dialer_proxy").into_owned() }) TextRole(Role::Caption)
                    --
                    Node { width: percent(100), flex_shrink: 0.0 } CustomNodeDialerField CustomNodeInputField(CustomNodeInput::Dialer)
                    Children [ @{ text_field_with_placeholder_scene(field.value.clone(), String::new(), palette) } ]
                ]
            });
        } else if id == ProtocolField::CaPath {
            scene = Box::new(bsn! {
                Node { width: percent(100), flex_direction: FlexDirection::Column, flex_shrink: 0.0, row_gap: px(4.0) }
                Children [ Text({ lang.tr("custom_node_ca_path").into_owned() }) TextRole(Role::Caption)
                    --
                    Node { width: percent(100), flex_shrink: 0.0 } CustomNodeCaField CustomNodeInputField(CustomNodeInput::Ca)
                    Children [ @{ text_field_with_placeholder_scene(field.value.clone(), String::new(), palette) } ]
                ]
            });
        }
        Box::new(bsn! {
            Node { display, width: percent(100), flex_direction: FlexDirection::Column, flex_shrink: 0.0 }
            ProtocolFieldNode(id)
            Children [ @{ scene } ]
        }) as Box<dyn Scene>
    }).collect();
    // The editor is a full field list with native scrolling supplied by its modal body.
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(10.0), flex_shrink: 0.0 }
        Children [ { fields } ]
    }
}

/// Find the actual TextField under a field's declarative wrapper.
pub fn text_field_under(
    root: Entity,
    children: &Query<&Children>,
    fields: &Query<&TextField>,
) -> Option<Entity> {
    if fields.get(root).is_ok() {
        return Some(root);
    }
    children
        .get(root)
        .ok()?
        .iter()
        .find_map(|child| text_field_under(child, children, fields))
}

pub(crate) fn sync_protocol_form(
    panel: Res<BusinessPanelState>,
    mut form: ResMut<CustomNodeForm>,
    mut wrappers: Query<(&ProtocolFieldNode, &Children, &mut Node)>,
    children: Query<&Children>,
    uris: Query<&Children, With<CustomNodeUriField>>,
    mut fields: Query<&mut TextField>,
    mut texts: Query<(&mut Text, &ProtocolFormText)>,
) {
    let active = panel.0 == Some(PanelKind::CustomNode) && form.saving.is_none();
    let reset = form.reset_fields;
    for (id, nodes, mut node) in &mut wrappers {
        let Some(draft) = form.studio.draft.as_ref() else {
            continue;
        };
        node.display = if visible(id.0, draft) {
            Display::Flex
        } else {
            Display::None
        };
        let projected =
            field_projection(id.0, draft).unwrap_or_else(|| special_projection(id.0, draft));
        if projected.toggle {
            continue;
        }
        // The mutable query is intentionally separate from hierarchy traversal.
        let mut stack: Vec<Entity> = nodes.iter().collect();
        while let Some(entity) = stack.pop() {
            if let Ok(mut field) = fields.get_mut(entity) {
                if reset {
                    field
                        .0
                        .apply(TextFieldInput::SetText(form.inputs.value(&projected)));
                } else if active && visible(id.0, form.studio.draft.as_ref().expect("draft")) {
                    let raw = field.0.text().to_owned();
                    if raw != form.inputs.value(&projected) {
                        form.stage(id.0, raw);
                    }
                }
                field
                    .0
                    .set_validation(if form.inputs.errors.contains_key(&id.0) {
                        ValidationStatus::Error
                    } else {
                        ValidationStatus::Normal
                    });
                break;
            }
            if let Ok(nested) = children.get(entity) {
                stack.extend(nested.iter());
            }
        }
    }
    if form.reset_uri {
        for children in &uris {
            for entity in children {
                if let Ok(mut field) = fields.get_mut(*entity) {
                    field.0.apply(TextFieldInput::Clear);
                }
            }
        }
        form.reset_uri = false;
    }
    form.reset_fields = false;
    let language = UiLocale::default().code().to_string();
    let lang = Lang(&language);
    for (mut text, kind) in &mut texts {
        match kind.0 {
            ProtocolFormTextKind::Fact(slot) => text.0 = slot_initial(slot, &form.studio),
            ProtocolFormTextKind::Error => {
                text.0 = form.studio.last_error.clone().unwrap_or_default()
            }
            ProtocolFormTextKind::Toggle(id) => {
                if let Some(draft) = &form.studio.draft {
                    let field = field_projection(id, draft).expect("typed toggle");
                    text.0 = format!("{}: {}", lang.tr(field.label_key), field.value);
                }
            }
        }
    }
}

/// Fold current native text immediately at button activation, including the last input frame.
pub fn stage_native_fields(
    form: &mut CustomNodeForm,
    wrappers: &Query<(Entity, &ProtocolFieldNode)>,
    children: &Query<&Children>,
    fields: &Query<&TextField>,
) {
    for (entity, id) in wrappers {
        let Some(draft) = &form.studio.draft else {
            return;
        };
        if !visible(id.0, draft) {
            continue;
        }
        if let Some(field) = text_field_under(entity, children, fields) {
            let raw = fields
                .get(field)
                .expect("resolved native field")
                .0
                .text()
                .to_owned();
            let projected =
                field_projection(id.0, draft).unwrap_or_else(|| special_projection(id.0, draft));
            if raw != form.inputs.value(&projected) {
                form.stage(id.0, raw);
            }
        }
    }
}

/// A projection refresh never substitutes for a terminal application acknowledgment.
pub(crate) fn finish_custom_node(
    result: On<CommandExecutedEvent>,
    mut form: ResMut<CustomNodeForm>,
    mut panel: ResMut<BusinessPanelState>,
    mut roots: Query<(&BusinessPanelRoot, &mut Node)>,
) {
    match &result.command {
        UiCommand::SaveCustomNodeDraft { draft }
            if form.saving.as_ref().is_some_and(|pending| {
                pending.request_id == result.request_id && &pending.draft == draft.as_ref()
            }) =>
        {
            form.saving = None;
            if result.result.is_ok() {
                if panel.0 == Some(PanelKind::CustomNode) {
                    panel.0 = None;
                    for (root, mut node) in &mut roots {
                        if root.0 == PanelKind::CustomNode {
                            node.display = Display::None;
                        }
                    }
                }
            } else {
                form.studio.last_error = result.error_message().map(str::to_owned);
            }
        }
        UiCommand::ImportCustomNodeUri { uri }
            if panel.0 == Some(PanelKind::CustomNode)
                && form.importing.as_ref().is_some_and(|pending| {
                    pending.request_id == result.request_id && &pending.uri == uri
                }) =>
        {
            form.importing = None;
            if result.result.is_ok() {
                if let Some(studio) = studio_snapshot() {
                    form.load(studio);
                }
            } else {
                form.studio.last_error = result.error_message().map(str::to_owned);
            }
        }
        UiCommand::ScanCustomNodeDialer | UiCommand::VerifyCustomNodeCa { .. }
            if panel.0 == Some(PanelKind::CustomNode) =>
        {
            if let Some(studio) = studio_snapshot() {
                form.studio.dialer = studio.dialer;
                form.studio.ca_trust = studio.ca_trust;
            }
            if !result.result.is_ok() {
                form.studio.last_error = result.error_message().map(str::to_owned);
            }
        }
        _ => {}
    }
}
