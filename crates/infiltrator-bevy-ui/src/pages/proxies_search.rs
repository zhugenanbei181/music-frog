//! Native search editing submits shared preferences without rebuilding focused controls.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::proxies::LastProxiesProjection;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{AlignItems, FlexDirection, Node, Val, percent, px};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::localization::{LocalizedPlaceholder, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, text_field_with_placeholder_scene};
use infiltrator_contract::command::RequestId;
use infiltrator_contract::error::{ErrorCode, Failure};

#[derive(Component, Clone, Copy, Default)]
pub struct ProxySearchInput;
#[derive(Component, Clone, Copy, Default)]
pub struct ClearProxySearch;
#[derive(Component, Clone, Copy, Default)]
pub struct RetryProxySearch;
#[derive(Component, Clone, Copy, Default)]
pub struct ProxySearchStatus;

#[derive(Resource, Default)]
pub struct ProxySearchState {
    pub submitted: Option<String>,
    pub pending: Option<(RequestId, String)>,
    pub failure: Option<Failure>,
    pub force_retry: bool,
}

pub fn search_input_scene(query: &str, palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node { flex_grow: 1.0, min_width: px(220.0), flex_direction: FlexDirection::Column }
        Children [
            Node { width: percent(100), align_items: AlignItems::Center, column_gap: Val::Px(8.0) }
            Children [
                Node { flex_grow: 1.0, min_width: px(0.0) } ProxySearchInput
                Children [ @{ text_field_with_placeholder_scene(query.into(), String::new(), palette) } ]
                --
                Node { min_height: px(palette.control_height_px), align_items: AlignItems::Center }
                Button ClearProxySearch
                Children [ LocalizedText::plain("proxies_search_clear") TextRole(Role::Caption) ]
                --
                Node { min_height: px(palette.control_height_px), align_items: AlignItems::Center }
                Button RetryProxySearch
                Children [ LocalizedText::plain("proxies_search_retry") TextRole(Role::Caption) ]
            ]
            --
            Text(String::new()) ProxySearchStatus TextRole(Role::Caption)
        ]
    }
}

/// Text edits alone are commands; caret, selection and IME preedit changes never resubmit a query.
pub fn sync_search_input(
    wrappers: Query<&Children, With<ProxySearchInput>>,
    fields: Query<(Entity, &TextField, Option<&LocalizedPlaceholder>)>,
    mut state: ResMut<ProxySearchState>,
    last: Option<Res<LastProxiesProjection>>,
    sink: Option<Res<CommandSinkHandle>>,
    mut commands: Commands,
) {
    for children in &wrappers {
        for child in children {
            let Ok((entity, field, placeholder)) = fields.get(*child) else {
                continue;
            };
            if placeholder.is_none() {
                commands
                    .entity(entity)
                    .insert(LocalizedPlaceholder::plain("proxies_search_placeholder"));
            }
            let query = field.0.text();
            if state.submitted.as_deref() == Some(query) && !state.force_retry {
                continue;
            }
            let observed = last
                .as_ref()
                .and_then(|last| last.0.as_ref())
                .map(|projection| projection.search_query.as_str());
            if !state.force_retry
                && state.submitted.is_none()
                && (observed == Some(query) || (observed.is_none() && query.is_empty()))
            {
                state.submitted = Some(query.into());
                continue;
            }
            if state.pending.is_some() {
                continue;
            }
            state.force_retry = false;
            state.submitted = Some(query.into());
            state.failure = None;
            state.pending = sink
                .as_ref()
                .and_then(|sink| {
                    sink.submit_tracked(UiCommand::SetProxySearchQuery {
                        query: query.into(),
                    })
                })
                .map(|id| (id, query.into()));
            if state.pending.is_none() {
                state.failure = Some(Failure::new(
                    ErrorCode::NotReady,
                    "search command service supplies no terminal acknowledgment",
                    true,
                ));
            }
        }
    }
}

pub fn on_clear_search(
    activated: On<Activate>,
    buttons: Query<(), With<ClearProxySearch>>,
    wrappers: Query<&Children, With<ProxySearchInput>>,
    mut fields: Query<&mut TextField>,
) {
    if buttons.get(activated.entity).is_err() {
        return;
    }
    for children in &wrappers {
        for child in children {
            if let Ok(mut field) = fields.get_mut(*child) {
                field.0.apply(TextFieldInput::Clear);
            }
        }
    }
}

pub fn finish_search(result: On<CommandExecutedEvent>, mut state: ResMut<ProxySearchState>) {
    let Some((id, query)) = &state.pending else {
        return;
    };
    if *id != result.request_id
        || !matches!(&result.command, UiCommand::SetProxySearchQuery { query: submitted } if submitted == query)
    {
        return;
    }
    state.pending = None;
    state.failure = result.result.as_ref().err().cloned();
}

pub fn sync_search_status(
    state: Res<ProxySearchState>,
    last: Option<Res<LastProxiesProjection>>,
    locale: Res<UiLocale>,
    mut labels: Query<&mut Text, With<ProxySearchStatus>>,
) {
    let copy = if let Some(failure) = &state.failure {
        LocalizedText::new(
            "proxies_search_failed",
            vec![("reason", failure.message.clone())],
        )
    } else if last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .is_some_and(|projection| {
            !projection.search_query.trim().is_empty() && projection.total_nodes() == 0
        })
    {
        LocalizedText::plain("proxies_search_empty")
    } else {
        LocalizedText::plain("proxies_search_hint")
    };
    for mut text in &mut labels {
        text.0 = copy.render(&locale);
    }
}

pub fn on_retry_search(
    activated: On<Activate>,
    buttons: Query<(), With<RetryProxySearch>>,
    mut state: ResMut<ProxySearchState>,
) {
    if buttons.get(activated.entity).is_ok() && state.pending.is_none() && state.failure.is_some() {
        state.force_retry = true;
    }
}
