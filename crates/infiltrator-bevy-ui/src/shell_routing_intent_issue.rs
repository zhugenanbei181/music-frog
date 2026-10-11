//! Persistent routing failure controls; lifecycle and controller read status stay independent.
use crate::app::RoutingIntentActionState;
use crate::localized_widgets::localized_pill_scene;
use crate::route::{Route, RouteChanged};
use crate::shell_routing_intent::{RoutingIntentActivation, begin_request};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, With, Without};
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::feathers::theme::ThemeTextColor;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    BackgroundColor, BorderColor, Display, FlexDirection, FlexWrap, Node, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_application::routing_intent_projection::routing_intent_failure_copy;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

#[derive(Component, Clone, Copy, Default)]
pub struct RoutingIntentIssueRoot;
#[derive(Component, Clone, Copy, Default)]
pub struct RoutingIntentIssueText;
#[derive(Component, Clone, Copy, Default)]
pub struct RetryRoutingIntentChange;
#[derive(Component, Clone, Copy, Default)]
pub struct DismissRoutingIntentIssue;
#[derive(Component, Clone, Copy, Default)]
pub struct RoutingIntentSettingsGuide;

pub fn scene(palette: &UiPalette) -> impl Scene + use<> {
    let semantic = accesskit::Node::new(accesskit::Role::Alert);
    let edge = palette.danger;
    bsn! {
        Node { display: Display::None, width: percent(100), min_width: px(0.0),
            flex_direction: FlexDirection::Column, row_gap: rem(space::S4), padding: UiRect::all(rem(space::S8)),
            border: UiRect::all(Val::Px(palette.hairline_px)) }
        RoutingIntentIssueRoot ThemeBackgroundColor({ Token::SurfaceElevated })
        BorderColor::all(edge)
        Children [
            Text({ String::new() }) TextRole(Role::Body) RoutingIntentIssueText
            ThemeTextColor({ Token::Ink }) AccessibilityNode(semantic)
            --
            Node { flex_wrap: FlexWrap::Wrap, column_gap: rem(space::S8), row_gap: rem(space::S4) }
            Children [
                @{ localized_pill_scene(LocalizedText::plain("routing_intent_retry"), false, palette) }
                RetryRoutingIntentChange ButtonDisabled(true)
                --
                @{ localized_pill_scene(LocalizedText::plain("routing_intent_dismiss"), false, palette) }
                DismissRoutingIntentIssue
                --
                @{ localized_pill_scene(LocalizedText::plain("routing_intent_settings"), false, palette) }
                RoutingIntentSettingsGuide
            ]
        ]
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct IssueText {
    text: &'static mut Text,
    color: &'static mut TextColor,
    semantic: &'static mut AccessibilityNode,
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct IssueCard {
    node: &'static mut Node,
    background: &'static mut BackgroundColor,
    border: &'static mut BorderColor,
}

pub fn sync(
    state: Res<RoutingIntentActionState>,
    locale: Res<UiLocale>,
    palette: Res<UiPalette>,
    mut roots: Query<IssueCard, With<RoutingIntentIssueRoot>>,
    mut texts: Query<IssueText, With<RoutingIntentIssueText>>,
    mut retry: Query<&mut ButtonDisabled, With<RetryRoutingIntentChange>>,
    mut guides: Query<
        &mut Node,
        (
            With<RoutingIntentSettingsGuide>,
            Without<RoutingIntentIssueRoot>,
        ),
    >,
) {
    for mut root in &mut roots {
        root.background.0 = palette.surface_elevated;
        *root.border = BorderColor::all(palette.danger);
        root.node.display = if state.0.failure.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut text in &mut texts {
        let caption = routing_intent_failure_copy(&state.0, locale.code());
        text.semantic.0.set_label(caption.clone());
        text.text.0 = caption;
        text.color.0 = palette.ink;
    }
    for mut disabled in &mut retry {
        disabled.0 = state.0.retry_target().is_none() || state.0.pending.is_some();
    }
    for mut guide in &mut guides {
        guide.display = if state.0.needs_controller_settings() {
            Display::Flex
        } else {
            Display::None
        };
    }
}

pub fn settings(
    activate: On<Activate>,
    controls: Query<(), With<RoutingIntentSettingsGuide>>,
    state: Res<RoutingIntentActionState>,
    mut commands: Commands,
) {
    if controls.contains(activate.entity) && state.0.needs_controller_settings() {
        commands.trigger(RouteChanged(Route::Settings));
    }
}

pub fn retry(
    activate: On<Activate>,
    buttons: Query<(), With<RetryRoutingIntentChange>>,
    state: RoutingIntentActivation,
) {
    if buttons.contains(activate.entity)
        && let Some(target) = state.actions.0.retry_target()
    {
        begin_request(target, state);
    }
}

pub fn dismiss(
    activate: On<Activate>,
    buttons: Query<(), With<DismissRoutingIntentIssue>>,
    mut state: ResMut<RoutingIntentActionState>,
) {
    if buttons.contains(activate.entity) {
        state.0.dismiss_failure();
    }
}
