//! Scoped native confirmation and exact shared-command terminal correlation.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::dns::ClearDnsCacheButton;
use crate::pages::dns_cache_focus::CacheFocus;
use crate::route::{ActiveRoute, Route, RouteChanged};
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{Display, Node};
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::dns_cache_actions::DnsCacheActions;
use infiltrator_application::dns_cache_projection::project_cache;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::dns_cache::DnsCacheOperationId;
use infiltrator_contract::error::{ErrorCode, Failure};
#[derive(Resource, Default)]
pub struct CacheConfirmation {
    pub model: DnsCacheActions,
    pub request: Option<(RequestId, u64)>,
    pub requested: bool,
    pub restore_focus: Vec<Entity>,
}
#[derive(Event)]
pub struct RequestCacheConfirmation;
#[derive(Component, Clone, Copy, Default)]
pub struct CacheModalRoot;
#[derive(Component, Clone, Copy, Default)]
pub struct CacheModalCard;
#[derive(Component, Clone, Copy, Default)]
#[require(Button, ButtonDisabled)]
pub enum CacheAction {
    #[default]
    Confirm,
    Cancel,
    Retry,
}
#[derive(Component, Clone, Copy, Default)]
pub enum CacheLine {
    #[default]
    Status,
    ReportLabel,
    FakeIp,
    System,
}
pub fn request(
    _: On<RequestCacheConfirmation>,
    mut state: ResMut<CacheConfirmation>,
    mut commands: Commands,
) {
    if state.model.pending.is_none() {
        state.requested = true;
        commands.trigger(RouteChanged(Route::Dns));
    }
}
pub fn activate(
    event: On<Activate>,
    launchers: Query<(), With<ClearDnsCacheButton>>,
    buttons: Query<&CacheAction>,
    route: Res<ActiveRoute>,
    mut state: ResMut<CacheConfirmation>,
    sink: Option<Res<CommandSinkHandle>>,
    mut focus: CacheFocus,
) {
    if route.0 != Some(Route::Dns) || state.model.pending.is_some() {
        return;
    }
    if launchers.contains(event.entity) {
        state.model.show();
        focus.suspend(&mut state);
        return;
    }
    let Ok(action) = buttons.get(event.entity) else {
        return;
    };
    let token = match action {
        CacheAction::Cancel => {
            if state.model.cancel() {
                focus.restore(&mut state);
            }
            return;
        }
        CacheAction::Confirm => state.model.confirm(),
        CacheAction::Retry => state.model.retry(),
    };
    let Ok(token) = token else {
        return;
    };
    if let Some(request) = sink.and_then(|sink| {
        sink.submit_tracked(UiCommand::ClearDnsCache {
            operation: DnsCacheOperationId(token),
        })
    }) {
        state.request = Some((request, token));
    } else {
        state.model.finish(
            token,
            Err(Failure::new(
                ErrorCode::NotReady,
                "DNS cache command service has no terminal response",
                true,
            )),
        );
    }
}
pub fn finish(event: On<CommandExecutedEvent>, mut state: ResMut<CacheConfirmation>) {
    let Some((request, token)) = state.request else {
        return;
    };
    let UiCommand::ClearDnsCache { operation } = event.command else {
        return;
    };
    if event.request_id == request
        && operation == DnsCacheOperationId(token)
        && state.model.finish(token, event.unit_result())
    {
        state.request = None;
    }
}
pub fn observe(
    latest: Res<LatestSurfaceSnapshot>,
    route: Res<ActiveRoute>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut state: ResMut<CacheConfirmation>,
    mut focus: CacheFocus,
) {
    state.model.observe(&latest.0.dns_cache);
    if state.requested && route.0 == Some(Route::Dns) {
        state.requested = false;
        state.model.show();
        focus.suspend(&mut state);
    }
    if state.model.open
        && (route.0 != Some(Route::Dns)
            || keys.is_some_and(|keys| keys.just_pressed(KeyCode::Escape)))
        && state.model.cancel()
    {
        focus.restore(&mut state);
    }
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct CacheControl {
    action: &'static CacheAction,
    node: &'static mut Node,
    disabled: &'static mut ButtonDisabled,
    children: Option<&'static Children>,
    label: Option<&'static mut LocalizedLabel>,
}
pub fn render(
    state: Res<CacheConfirmation>,
    locale: Res<UiLocale>,
    mut roots: Query<&mut Node, With<CacheModalRoot>>,
    mut actions: Query<CacheControl, Without<CacheModalRoot>>,
    mut lines: Query<(&CacheLine, &mut Text, &mut TextColor)>,
    mut copies: Query<&mut LocalizedText>,
    palette: Res<UiPalette>,
) {
    let view = project_cache(&state.model, locale.code());
    for mut root in &mut roots {
        let wanted = if state.model.open {
            Display::Flex
        } else {
            Display::None
        };
        if root.display != wanted {
            root.display = wanted;
        }
    }
    for mut control in &mut actions {
        let action = control.action;
        let node = &mut control.node;
        let disabled = &mut control.disabled;
        let children = control.children;
        let label = control.label;
        if matches!(action, CacheAction::Cancel) {
            let key = if state.model.confirmed {
                "dns_cache_close"
            } else {
                "dns_cache_cancel"
            };
            if let Some(mut label) = label
                && label.0.key != key
            {
                label.0 = LocalizedText::plain(key);
            }
            for child in children.into_iter().flat_map(|children| children.iter()) {
                if let Ok(mut copy) = copies.get_mut(*child)
                    && copy.key != key
                {
                    *copy = LocalizedText::plain(key);
                }
            }
        }
        let shown = match action {
            CacheAction::Confirm => !state.model.confirmed,
            CacheAction::Cancel => true,
            CacheAction::Retry => view.retry,
        };
        let enabled = state.model.open
            && match action {
                CacheAction::Confirm => view.confirm,
                CacheAction::Cancel => view.close,
                CacheAction::Retry => view.retry,
            };
        let display = if shown { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        if disabled.0 != !enabled {
            disabled.0 = !enabled;
        }
    }
    for (line, mut text, mut ink) in &mut lines {
        if matches!(line, CacheLine::Status) {
            let color = if view.error {
                palette.danger
            } else {
                palette.ink
            };
            if ink.0 != color {
                ink.0 = color;
            }
        }
        let value = match line {
            CacheLine::Status => &view.status,
            CacheLine::ReportLabel => &view.results_label,
            CacheLine::FakeIp => &view.fake_ip,
            CacheLine::System => &view.system,
        };
        if &text.0 != value {
            text.0.clone_from(value);
        }
    }
}
