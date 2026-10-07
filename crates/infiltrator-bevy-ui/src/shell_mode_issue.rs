//! Persistent mode failure controls; lifecycle and controller read status stay independent.
use crate::app::ModeActionState;
use crate::localized_widgets::localized_pill_scene;
use crate::route::{Route, RouteChanged};
use crate::shell_modes::{ModeActivation, begin_request};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, With, Without};
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    BackgroundColor, BorderColor, Display, FlexDirection, FlexWrap, Node, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_application::proxy_mode_projection::mode_failure_copy;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

#[derive(Component, Clone, Copy, Default)]
pub struct ModeIssueRoot;
#[derive(Component, Clone, Copy, Default)]
pub struct ModeIssueText;
#[derive(Component, Clone, Copy, Default)]
pub struct RetryModeChange;
#[derive(Component, Clone, Copy, Default)]
pub struct DismissModeIssue;
#[derive(Component, Clone, Copy, Default)]
pub struct ModeSettingsGuide;

pub fn scene(palette: &UiPalette) -> impl Scene + use<> {
    let semantic = accesskit::Node::new(accesskit::Role::Alert);
    let edge = palette.danger;
    bsn! {
        Node { display: Display::None, width: percent(100), min_width: px(0.0),
            flex_direction: FlexDirection::Column, row_gap: px(space::S4), padding: UiRect::all(Val::Px(space::S8)),
            border: UiRect::all(Val::Px(palette.hairline_px)) }
        ModeIssueRoot BackgroundColor({ palette.surface_elevated })
        BorderColor::all(edge)
        Children [
            Text({ String::new() }) TextRole(Role::Body) ModeIssueText
            TextColor({ palette.ink }) AccessibilityNode(semantic)
            --
            Node { flex_wrap: FlexWrap::Wrap, column_gap: px(space::S8), row_gap: px(space::S4) }
            Children [
                @{ localized_pill_scene(LocalizedText::plain("proxy_mode_retry"), false, palette) }
                RetryModeChange ButtonDisabled(true)
                --
                @{ localized_pill_scene(LocalizedText::plain("proxy_mode_dismiss"), false, palette) }
                DismissModeIssue
                --
                @{ localized_pill_scene(LocalizedText::plain("proxy_mode_settings"), false, palette) }
                ModeSettingsGuide
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
    state: Res<ModeActionState>,
    locale: Res<UiLocale>,
    palette: Res<UiPalette>,
    mut roots: Query<IssueCard, With<ModeIssueRoot>>,
    mut texts: Query<IssueText, With<ModeIssueText>>,
    mut retry: Query<&mut ButtonDisabled, With<RetryModeChange>>,
    mut guides: Query<&mut Node, (With<ModeSettingsGuide>, Without<ModeIssueRoot>)>,
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
        let caption = mode_failure_copy(&state.0, locale.code());
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
    controls: Query<(), With<ModeSettingsGuide>>,
    state: Res<ModeActionState>,
    mut commands: Commands,
) {
    if controls.contains(activate.entity) && state.0.needs_controller_settings() {
        commands.trigger(RouteChanged(Route::Settings));
    }
}

pub fn retry(
    activate: On<Activate>,
    buttons: Query<(), With<RetryModeChange>>,
    state: ModeActivation,
) {
    if buttons.contains(activate.entity)
        && let Some(target) = state.actions.0.retry_target()
    {
        begin_request(target, state);
    }
}

pub fn dismiss(
    activate: On<Activate>,
    buttons: Query<(), With<DismissModeIssue>>,
    mut state: ResMut<ModeActionState>,
) {
    if buttons.contains(activate.entity) {
        state.0.dismiss_failure();
    }
}
