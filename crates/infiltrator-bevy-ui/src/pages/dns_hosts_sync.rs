//! Scoped observation, native input and localized feedback preserve the active editor entities.
use crate::pages::dns_hosts::{
    DnsHostsDomainField, DnsHostsEditorField, DnsHostsEditorState, DnsHostsStatusLine, HostAction,
    HostsEmptyHint, HostsLegacyHint, HostsModalCard, HostsModalRoot, HostsSummary,
};
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::query::{Has, Or, QueryData, QueryFilter, With, Without};
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::input_focus::{AcquireFocus, FocusCause, InputFocus};
use bevy::ui::widget::Text;
use bevy::ui::{Display, Node};
use bevy::window::PrimaryWindow;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::modal::ModalScrim;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_presentation::dns_hosts_projection::{feedback, legacy_hint, summary};
use infiltrator_shared::locales::Lang;
#[derive(SystemParam)]
pub struct DnsHostsObserveParams<'w, 's> {
    latest: Res<'w, LatestSurfaceSnapshot>,
    route: Res<'w, ActiveRoute>,
    state: ResMut<'w, DnsHostsEditorState>,
    parents: Query<'w, 's, &'static ChildOf>,
    cards: Query<'w, 's, Entity, With<HostsModalCard>>,
    actions: Query<'w, 's, (Entity, &'static HostAction), Without<ModalScrim>>,
    commands: Commands<'w, 's>,
    input_focus: ResMut<'w, InputFocus>,
    windows: Query<'w, 's, Entity, With<PrimaryWindow>>,
}

pub fn observe(params: DnsHostsObserveParams) {
    let DnsHostsObserveParams {
        latest,
        route,
        mut state,
        parents,
        cards,
        actions,
        mut commands,
        mut input_focus,
        windows,
    } = params;
    let previous = (
        state.editor.address().to_owned(),
        state.editor.domain().to_owned(),
    );
    state.editor.observe(&latest.0.dns_hosts);
    state.sync_fields |= previous
        != (
            state.editor.address().to_owned(),
            state.editor.domain().to_owned(),
        );
    // The Escape interception belongs to Bevy's official dialog layer: the
    // barrier reports the close request and the modal's own close path runs
    // from it. This system only owns the page navigation.
    if state.editor.is_open() && route.0 != Some(Route::Dns) && state.editor.cancel() {
        state.sync_fields = true;
    }
    hand_off_focus(
        &mut commands,
        &mut input_focus,
        &windows,
        &parents,
        &cards,
        &actions,
        state.editor.is_open(),
    );
}

/// The official focus hand-off.
///
/// While the hosts modal is presented the input focus lives inside the
/// dialog, so Bevy's own `ModalDialogBarrier` owns the Escape interception;
/// when the modal closes the focus is released again. The dialog's dismiss
/// control is its focusable control — the official `TabIndex` marks it for
/// Bevy's focus scheme — and the official `AcquireFocus` request asks that
/// scheme to move the input focus there.
fn hand_off_focus(
    commands: &mut Commands,
    focus: &mut InputFocus,
    windows: &Query<Entity, With<PrimaryWindow>>,
    parents: &Query<&ChildOf>,
    cards: &Query<Entity, With<HostsModalCard>>,
    actions: &Query<(Entity, &HostAction), Without<ModalScrim>>,
    presented: bool,
) {
    let Ok(card) = cards.single() else {
        return;
    };
    let inside = focus
        .get()
        .is_some_and(|entity| inside_dialog(parents, entity, card));
    if presented {
        if inside {
            return;
        }
        let Some(control) = dismiss_control(actions) else {
            return;
        };
        commands
            .entity(control)
            .entry::<TabIndex>()
            .or_insert(TabIndex(0));
        if let Ok(window) = windows.single() {
            commands.trigger(AcquireFocus {
                focused_entity: control,
                window,
            });
        }
        focus.set(control, FocusCause::Navigated);
    } else if inside {
        focus.clear();
    }
}

/// Whether the entity is the dialog card or lives inside it.
fn inside_dialog(parents: &Query<&ChildOf>, entity: Entity, card: Entity) -> bool {
    let mut current = Some(entity);
    while let Some(node) = current {
        if node == card {
            return true;
        }
        current = parents.get(node).ok().map(|parent| parent.parent());
    }
    false
}

/// The modal's dismiss control: the one cancel control that is not the scrim.
fn dismiss_control(actions: &Query<(Entity, &HostAction), Without<ModalScrim>>) -> Option<Entity> {
    actions
        .iter()
        .find(|(_, action)| matches!(action, HostAction::Cancel))
        .map(|(entity, _)| entity)
}
#[derive(QueryFilter)]
pub struct HostFieldWrappers {
    input: Or<(With<DnsHostsEditorField>, With<DnsHostsDomainField>)>,
}
pub fn inputs(
    mut state: ResMut<DnsHostsEditorState>,
    wrappers: Query<(&Children, Has<DnsHostsEditorField>), HostFieldWrappers>,
    mut fields: Query<(&mut TextField, &mut TextFieldFocused)>,
) {
    for (children, address) in &wrappers {
        for child in children.iter() {
            let Ok((mut field, mut focus)) = fields.get_mut(*child) else {
                continue;
            };
            let wanted = if address {
                state.editor.address()
            } else {
                state.editor.domain()
            };
            if state.sync_fields {
                if field.0.text() != wanted {
                    field.0.apply(TextFieldInput::SetText(wanted.to_owned()));
                }
            } else if state.editor.is_open()
                && !state.editor.is_pending()
                && field.0.text() != wanted
            {
                let value = field.0.text().to_owned();
                if address {
                    state.editor.edit_address(value);
                } else {
                    state.editor.edit_domain(value);
                }
            }
            let disabled = !state.editor.is_open() || state.editor.is_pending();
            if field.0.is_disabled() != disabled {
                field.0.set_disabled(disabled);
            }
            if disabled && focus.0 {
                focus.0 = false;
            }
        }
    }
    state.sync_fields = false;
}
pub fn controls(
    state: Res<DnsHostsEditorState>,
    mut roots: Query<&mut Node, With<HostsModalRoot>>,
    mut empty: Query<&mut Node, (With<HostsEmptyHint>, Without<HostsModalRoot>)>,
    mut buttons: Query<(&HostAction, &mut ButtonDisabled)>,
) {
    for mut root in &mut roots {
        let wanted = if state.editor.is_open() {
            Display::Flex
        } else {
            Display::None
        };
        if root.display != wanted {
            root.display = wanted;
        }
    }
    for mut hint in &mut empty {
        let wanted = if state.editor.rows.is_empty() {
            Display::Flex
        } else {
            Display::None
        };
        if hint.display != wanted {
            hint.display = wanted;
        }
    }
    for (action, mut disabled) in &mut buttons {
        let wanted = if matches!(action, HostAction::Open) {
            state.editor.is_pending()
        } else {
            state.editor.is_pending()
                || !state.editor.is_open()
                || match action {
                    HostAction::Apply => !state.editor.can_apply(),
                    HostAction::ImportLegacy => {
                        state.editor.is_editing()
                            || !state.editor.address().is_empty()
                            || !state.editor.domain().is_empty()
                            || state.editor.is_importing_legacy()
                            || !state
                                .editor
                                .applied
                                .as_ref()
                                .is_some_and(|profile| !profile.legacy_entries.is_empty())
                    }
                    _ => false,
                }
        };
        if disabled.0 != wanted {
            disabled.0 = wanted;
        }
    }
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct HostCopy {
    text: &'static mut Text,
    node: &'static mut Node,
    summary: Has<HostsSummary>,
    status: Has<DnsHostsStatusLine>,
    legacy: Has<HostsLegacyHint>,
}
#[derive(QueryFilter)]
pub struct HostCopyFilter {
    copy: Or<(
        With<HostsSummary>,
        With<DnsHostsStatusLine>,
        With<HostsLegacyHint>,
    )>,
}
pub fn copy(
    state: Res<DnsHostsEditorState>,
    locale: Res<UiLocale>,
    mut labels: Query<HostCopy, HostCopyFilter>,
    mut buttons: Query<(&HostAction, &Children, &mut LocalizedLabel)>,
    mut copies: Query<&mut LocalizedText>,
) {
    for mut label in &mut labels {
        let value = if label.summary {
            summary(&state.editor, &Lang(locale.code()))
        } else if label.status {
            feedback(&state.editor, &Lang(locale.code()))
        } else {
            legacy_hint(&state.editor, &Lang(locale.code()))
        };
        if label.text.0 != value {
            label.text.0 = value.clone();
        }
        if label.legacy {
            let display = if value.is_empty() {
                Display::None
            } else {
                Display::Flex
            };
            if label.node.display != display {
                label.node.display = display;
            }
        }
    }
    for (action, children, mut label) in &mut buttons {
        let key = match action {
            HostAction::CommitRow => {
                if state.editor.is_editing() {
                    "dns_hosts_commit_row"
                } else {
                    "dns_hosts_add"
                }
            }
            HostAction::Apply => {
                if state.editor.is_pending() {
                    "dns_hosts_saving"
                } else if state.editor.failure().is_some() {
                    "dns_hosts_retry"
                } else {
                    "dns_hosts_apply"
                }
            }
            _ => continue,
        };
        if label.0.key != key {
            label.0 = LocalizedText::plain(key);
        }
        for child in children.iter() {
            if let Ok(mut copy) = copies.get_mut(*child)
                && copy.key != key
            {
                *copy = LocalizedText::plain(key);
            }
        }
    }
}
