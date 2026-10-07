//! Preference switches replay actual observations without caching a second fact source.
use super::settings_core::TunStackLabel;
use super::{CloseToTrayToggle, LastSettingsProjection, SystemNotificationsToggle};
use crate::command::CommandSinkHandle;
use accesskit::Toggled;
use bevy::a11y::AccessibilityNode;
use bevy::camera::visibility::Visibility;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{Or, QueryData, QueryFilter, With};
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, JustifyContent, Node, PositionType,
    UiRect, percent, px,
};
use bevy::ui_widgets::Button;
use infiltrator_application::settings_preference_projection::project;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::surface_snapshot::PageStatus;

#[derive(Clone, Copy, Debug, Default)]
pub enum PreferenceKind {
    #[default]
    Tray,
    Notifications,
}
impl PreferenceKind {
    fn label_key(self) -> &'static str {
        match self {
            Self::Tray => "settings_close_to_tray",
            Self::Notifications => "settings_notifications_label",
        }
    }
    fn value(self, last: &LastSettingsProjection) -> Option<bool> {
        last.0.as_ref().and_then(|value| match self {
            Self::Tray => value.close_to_tray,
            Self::Notifications => value.notifications_enabled,
        })
    }
}
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct PreferenceStatus(pub PreferenceKind);
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct PreferenceRow;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct PreferenceTrack(pub PreferenceKind);
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct PreferenceKnob(pub PreferenceKind);

pub fn preference_row_scene(
    kind: PreferenceKind,
    value: Option<bool>,
    status: &PageStatus,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let enabled = value == Some(true);
    let observed = project(value, status, UiLocale::default().code());
    let copy = LocalizedText::new(observed.key, observed.params);
    let status_color = if enabled {
        palette.ink
    } else {
        palette.ink_dim
    };
    let fill = if enabled {
        palette.accent
    } else {
        palette.surface_elevated
    };
    let knob = if enabled {
        palette.on_accent
    } else {
        palette.ink_dim
    };
    let edge = if enabled {
        palette.accent
    } else {
        palette.border
    };
    let action: Box<dyn Scene> = match kind {
        PreferenceKind::Tray => Box::new(bsn! { CloseToTrayToggle }),
        PreferenceKind::Notifications => Box::new(bsn! { SystemNotificationsToggle }),
    };
    let visibility = if value.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let label = LocalizedLabel::plain(kind.label_key());
    let mut semantic = accesskit::Node::new(accesskit::Role::Switch);
    semantic.set_label(label.0.render(&UiLocale::default()));
    if let Some(value) = value {
        semantic.set_toggled(if value { Toggled::True } else { Toggled::False });
    }
    bsn! {
        Node {
            width: percent(100), align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::axes(px(space::S8), px(space::S6)),
            border_radius: BorderRadius::all(px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated }) PreferenceRow
        Children [
            LocalizedText::plain(kind.label_key()) TextRole(Role::Body)
            --
            Node { align_items: AlignItems::Center, column_gap: px(space::S8) }
            Children [
                LocalizedText { key: { copy.key }, params: { copy.params } }
                PreferenceStatus(kind) TextRole(Role::Caption) TextColor(status_color)
                --
                Node {
                    width: px(38.0), height: px(22.0), border: UiRect::all(px(palette.hairline_px)),
                    border_radius: BorderRadius::all(px(11.0)), position_type: PositionType::Relative,
                    align_items: AlignItems::Center,
                }
                BackgroundColor(fill) BorderColor::all(edge)
                PreferenceTrack(kind) Button ButtonDisabled({ !observed.editable })
                AccessibilityNode(semantic) label @{ action }
                Children [
                    Node {
                        position_type: PositionType::Absolute,
                        left: { if enabled { px(18.0) } else { px(2.0) } },
                        width: px(16.0), height: px(16.0), border_radius: BorderRadius::all(px(8.0)),
                    }
                    BackgroundColor(knob) PreferenceKnob(kind)
                    visibility
                ]
            ]
        ]
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct PreferenceView {
    status: Option<&'static PreferenceStatus>,
    track: Option<&'static PreferenceTrack>,
    knob: Option<&'static PreferenceKnob>,
    text: Option<&'static mut Text>,
    copy: Option<&'static mut LocalizedText>,
    ink: Option<&'static mut TextColor>,
    fill: Option<&'static mut BackgroundColor>,
    border: Option<&'static mut BorderColor>,
    node: Option<&'static mut Node>,
    visibility: Option<&'static mut Visibility>,
    disabled: Option<&'static mut ButtonDisabled>,
    semantic: Option<&'static mut AccessibilityNode>,
}
#[derive(QueryFilter)]
pub struct PreferenceFilter {
    native: Or<(
        With<PreferenceStatus>,
        With<PreferenceTrack>,
        With<PreferenceKnob>,
    )>,
}

pub fn replay(
    last: Res<LastSettingsProjection>,
    locale: Res<UiLocale>,
    palette: Res<UiPalette>,
    handle: Option<Res<CommandSinkHandle>>,
    mut views: Query<PreferenceView, PreferenceFilter>,
) {
    let status = last
        .0
        .as_ref()
        .map(|value| &value.preference_status)
        .unwrap_or(&PageStatus::Loading);
    for mut view in &mut views {
        let kind = view
            .status
            .map(|kind| kind.0)
            .or(view.track.map(|kind| kind.0))
            .or(view.knob.map(|kind| kind.0))
            .expect("a preference view has its role");
        let value = kind.value(&last);
        let enabled = value == Some(true);
        let observed = project(value, status, locale.code());
        if view.status.is_some() {
            if let (Some(copy), Some(text)) = (&mut view.copy, &mut view.text) {
                let next = LocalizedText::new(observed.key, observed.params);
                if **copy != next {
                    **copy = next;
                }
                let next = copy.render(&locale);
                if text.0 != next {
                    text.0 = next;
                }
            }
            if let Some(ink) = &mut view.ink {
                ink.0 = if enabled {
                    palette.ink
                } else {
                    palette.ink_dim
                };
            }
        } else if view.track.is_some() {
            if let Some(disabled) = &mut view.disabled {
                disabled.0 = !observed.editable || handle.is_none();
            }
            if let Some(fill) = &mut view.fill {
                fill.0 = if enabled {
                    palette.accent
                } else {
                    palette.surface_elevated
                };
            }
            if let Some(border) = &mut view.border {
                border.set_all(if enabled {
                    palette.accent
                } else {
                    palette.border
                });
            }
            if let Some(semantic) = &mut view.semantic {
                if let Some(value) = value {
                    semantic
                        .0
                        .set_toggled(if value { Toggled::True } else { Toggled::False });
                } else {
                    semantic.0.clear_toggled();
                }
            }
        } else if view.knob.is_some() {
            if let Some(node) = &mut view.node {
                node.left = if enabled { px(18.0) } else { px(2.0) };
            }
            if let Some(fill) = &mut view.fill {
                fill.0 = if enabled {
                    palette.on_accent
                } else {
                    palette.ink_dim
                };
            }
            if let Some(visibility) = &mut view.visibility {
                **visibility = if value.is_some() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

pub fn replay_stack_labels(locale: Res<UiLocale>, mut labels: Query<(&TunStackLabel, &mut Text)>) {
    for (stack, mut text) in &mut labels {
        let next = if stack.0.is_live_supported() {
            stack.0.label().into()
        } else {
            locale.text("settings_lwip_reference")
        };
        if text.0 != next {
            text.0 = next;
        }
    }
}

pub fn replay_background(
    palette: Res<UiPalette>,
    mut rows: Query<&mut BackgroundColor, With<PreferenceRow>>,
) {
    for mut fill in &mut rows {
        if fill.0 != palette.surface_elevated {
            fill.0 = palette.surface_elevated;
        }
    }
}
