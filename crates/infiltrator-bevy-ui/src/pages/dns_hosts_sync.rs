//! Scoped observation, native input and localized feedback preserve the active editor entities.
use crate::pages::dns_hosts::{
    DnsHostsDomainField, DnsHostsEditorField, DnsHostsEditorState, DnsHostsStatusLine, HostAction,
    HostsEmptyHint, HostsLegacyHint, HostsModalRoot, HostsSummary,
};
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{Has, Or, QueryData, QueryFilter, With, Without};
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::input::{ButtonInput, keyboard::KeyCode};
use bevy::ui::widget::Text;
use bevy::ui::{Display, Node};
use infiltrator_application::dns_hosts_projection::{feedback, legacy_hint, summary};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
pub fn observe(
    latest: Res<LatestSurfaceSnapshot>,
    route: Res<ActiveRoute>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut state: ResMut<DnsHostsEditorState>,
) {
    let previous = (state.editor.address.clone(), state.editor.domain.clone());
    state.editor.observe(&latest.0.dns_hosts);
    state.sync_fields |= previous != (state.editor.address.clone(), state.editor.domain.clone());
    if state.editor.open
        && (route.0 != Some(Route::Dns)
            || keys.is_some_and(|keys| keys.just_pressed(KeyCode::Escape)))
        && state.editor.cancel()
    {
        state.sync_fields = true;
    }
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
                &state.editor.address
            } else {
                &state.editor.domain
            };
            if state.sync_fields {
                if field.0.text() != wanted {
                    field.0.apply(TextFieldInput::SetText(wanted.clone()));
                }
            } else if state.editor.open
                && state.editor.pending.is_none()
                && field.0.text() != wanted
            {
                let value = field.0.text().to_owned();
                if address {
                    state.editor.edit_address(value);
                } else {
                    state.editor.edit_domain(value);
                }
            }
            let disabled = !state.editor.open || state.editor.pending.is_some();
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
        let wanted = if state.editor.open {
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
            state.editor.pending.is_some()
        } else {
            state.editor.pending.is_some()
                || !state.editor.open
                || match action {
                    HostAction::Apply => !state.editor.can_apply(),
                    HostAction::ImportLegacy => {
                        state.editor.editing.is_some()
                            || !state.editor.address.is_empty()
                            || !state.editor.domain.is_empty()
                            || state.editor.importing_legacy
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
            summary(&state.editor, locale.code())
        } else if label.status {
            feedback(&state.editor, locale.code())
        } else {
            legacy_hint(&state.editor, locale.code())
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
                if state.editor.editing.is_some() {
                    "dns_hosts_commit_row"
                } else {
                    "dns_hosts_add"
                }
            }
            HostAction::Apply => {
                if state.editor.pending.is_some() {
                    "dns_hosts_saving"
                } else if state.editor.failure.is_some() {
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
