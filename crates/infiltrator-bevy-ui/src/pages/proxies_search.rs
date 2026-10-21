//! Native search editing submits shared preferences without rebuilding focused controls.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::proxies::{
    LastProxiesProjection, outbound_ui_preference_request, outbound_ui_preferences,
};
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BorderColor, BorderRadius, Display, FlexDirection, Node, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedPlaceholder, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, text_field_with_placeholder_scene};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::command::RequestId;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::outbound::preferences::OutboundUiPreferenceIntent;

/// One-off layout value (not a ladder rung): the narrowest the proxy search
/// column may shrink before the query field stops growing.
const PROXY_SEARCH_MIN_WIDTH_PX: f32 = 220.0;

#[derive(Component, Clone, Copy, Default)]
pub struct ProxySearchInput;
#[derive(Component, Clone, Copy, Default)]
pub struct ClearProxySearch;
#[derive(Component, Clone, Copy, Default)]
pub struct RetryProxySearch;
#[derive(Component, Clone, Copy, Default)]
pub struct ProxySearchStatus;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ProxySearchPhase {
    #[default]
    Uninitialized,
    Submitted {
        query: String,
    },
    Pending {
        request_id: RequestId,
        query: String,
    },
    Failed {
        query: String,
        failure: Failure,
    },
    RetryRequested {
        query: String,
    },
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxySearchState {
    pub phase: ProxySearchPhase,
}

impl ProxySearchState {
    pub fn failure(&self) -> Option<&Failure> {
        match &self.phase {
            ProxySearchPhase::Failed { failure, .. } => Some(failure),
            ProxySearchPhase::Uninitialized
            | ProxySearchPhase::Submitted { .. }
            | ProxySearchPhase::Pending { .. }
            | ProxySearchPhase::RetryRequested { .. } => None,
        }
    }
}

/// The query field: the search text input and the search's own status line.
pub fn search_input_scene(query: &str, palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node { flex_grow: 1.0, min_width: rem(PROXY_SEARCH_MIN_WIDTH_PX), flex_direction: FlexDirection::Column, row_gap: rem(space::S4) }
        Children [
            Node { width: percent(100), align_items: AlignItems::Center, column_gap: rem(space::S8) }
            Children [
                Node { flex_grow: 1.0, min_width: px(0.0) } ProxySearchInput
                Children [ @{ text_field_with_placeholder_scene(query.into(), String::new(), palette) } ]
            ]
            --
            Text(String::new()) ProxySearchStatus TextRole(Role::Caption)
        ]
    }
}

/// The query field's own actions.
///
/// The group carries its own frame, so the actions read as a group instead of
/// crowding the field; the retry only appears when a search was refused.
pub fn search_actions_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: rem(space::S4),
            padding: UiRect::all(rem(space::S4)),
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            flex_shrink: 0.0,
        }
        ThemeBackgroundColor({ Token::SurfaceElevated })
        BorderColor {
            top: { palette.border },
            right: { palette.border },
            bottom: { palette.border },
            left: { palette.border },
        }
        Children [
            Node {
                min_height: rem(palette.control_height_px),
                padding: UiRect::horizontal(rem(space::S8)),
                align_items: AlignItems::Center,
            }
            Button ClearProxySearch
            Children [ LocalizedText::plain("proxies_search_clear") TextRole(Role::Caption) ]
            --
            Node {
                min_height: rem(palette.control_height_px),
                padding: UiRect::horizontal(rem(space::S8)),
                align_items: AlignItems::Center,
            }
            Button RetryProxySearch
            Children [ LocalizedText::plain("proxies_search_retry") TextRole(Role::Caption) ]
        ]
    }
}

/// Text edits alone are commands; caret, selection and IME preedit changes never resubmit a query.
pub fn sync_search_input(
    wrappers: Query<&Children, With<ProxySearchInput>>,
    fields: Query<(Entity, &TextField, Option<&LocalizedPlaceholder>)>,
    mut state: ResMut<ProxySearchState>,
    last: Option<Res<LastProxiesProjection>>,
    latest: Res<LatestSurfaceSnapshot>,
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
            if matches!(&state.phase, ProxySearchPhase::Uninitialized) {
                let observed = last
                    .as_ref()
                    .and_then(|last| last.0.as_ref())
                    .map(|projection| projection.search_query.as_str());
                if observed == Some(query) || (observed.is_none() && query.is_empty()) {
                    state.phase = ProxySearchPhase::Submitted {
                        query: query.into(),
                    };
                    continue;
                }
            }
            match &state.phase {
                ProxySearchPhase::Submitted { query: submitted } if submitted == query => continue,
                ProxySearchPhase::Pending { .. } => continue,
                ProxySearchPhase::Failed { query: failed, .. } if failed == query => continue,
                ProxySearchPhase::RetryRequested { .. }
                | ProxySearchPhase::Submitted { .. }
                | ProxySearchPhase::Failed { .. }
                | ProxySearchPhase::Uninitialized => {}
            }
            let request = outbound_ui_preferences(&latest.0).and_then(|_| {
                outbound_ui_preference_request(
                    &latest.0,
                    OutboundUiPreferenceIntent::SetSearchQuery(query.into()),
                )
            });
            state.phase = request
                .zip(sink.as_ref())
                .and_then(|(request, sink)| {
                    sink.submit_tracked(UiCommand::ApplyOutboundUiPreference { request })
                })
                .map(|request_id| ProxySearchPhase::Pending {
                    request_id,
                    query: query.into(),
                })
                .unwrap_or_else(|| ProxySearchPhase::Failed {
                    query: query.into(),
                    failure: Failure::new(
                    ErrorCode::NotReady,
                    "outbound search preferences have no terminal command service or accepted observation",
                    true,
                    ),
                });
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
    let (request_id, query) = match &state.phase {
        ProxySearchPhase::Pending { request_id, query } => (*request_id, query.clone()),
        ProxySearchPhase::Uninitialized
        | ProxySearchPhase::Submitted { .. }
        | ProxySearchPhase::Failed { .. }
        | ProxySearchPhase::RetryRequested { .. } => return,
    };
    if request_id != result.request_id
        || !matches!(
            &result.command,
            UiCommand::ApplyOutboundUiPreference { request }
                if request.intent() == &OutboundUiPreferenceIntent::SetSearchQuery(query.clone())
        )
    {
        return;
    }
    state.phase = match result.result.as_ref() {
        Ok(_) => ProxySearchPhase::Submitted { query },
        Err(failure) => ProxySearchPhase::Failed {
            query,
            failure: failure.clone(),
        },
    };
}

pub fn sync_search_status(
    state: Res<ProxySearchState>,
    last: Option<Res<LastProxiesProjection>>,
    locale: Res<UiLocale>,
    mut labels: Query<&mut Text, With<ProxySearchStatus>>,
    mut retries: Query<&mut Node, With<RetryProxySearch>>,
) {
    let empty_result = last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .is_some_and(|projection| {
            !projection.search_query.trim().is_empty() && projection.total_nodes() == 0
        });
    let copy = match &state.phase {
        ProxySearchPhase::Failed { failure, .. } => LocalizedText::new(
            "proxies_search_failed",
            vec![("reason", failure.message.clone())],
        ),
        ProxySearchPhase::Uninitialized
        | ProxySearchPhase::Submitted { .. }
        | ProxySearchPhase::Pending { .. }
        | ProxySearchPhase::RetryRequested { .. }
            if empty_result =>
        {
            LocalizedText::plain("proxies_search_empty")
        }
        ProxySearchPhase::Uninitialized
        | ProxySearchPhase::Submitted { .. }
        | ProxySearchPhase::Pending { .. }
        | ProxySearchPhase::RetryRequested { .. } => LocalizedText::plain("proxies_search_hint"),
    };
    for mut text in &mut labels {
        text.0 = copy.render(&locale);
    }
    // The retry only exists while a search was refused: a search that never
    // failed has nothing to retry.
    let display = if matches!(state.phase, ProxySearchPhase::Failed { .. }) {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in &mut retries {
        if node.display != display {
            node.display = display;
        }
    }
}

pub fn on_retry_search(
    activated: On<Activate>,
    buttons: Query<(), With<RetryProxySearch>>,
    mut state: ResMut<ProxySearchState>,
) {
    match (buttons.get(activated.entity), state.phase.clone()) {
        (Ok(()), ProxySearchPhase::Failed { query, .. }) => {
            state.phase = ProxySearchPhase::RetryRequested { query };
        }
        (Ok(()), ProxySearchPhase::Uninitialized)
        | (Ok(()), ProxySearchPhase::Submitted { .. })
        | (Ok(()), ProxySearchPhase::Pending { .. })
        | (Ok(()), ProxySearchPhase::RetryRequested { .. })
        | (Err(_), _) => {}
    }
}
