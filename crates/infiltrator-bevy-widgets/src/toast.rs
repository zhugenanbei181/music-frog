//! Toast notifications: temporary message alerts and auto-dismissing toast stack.
//!
//! **Pure Queue & Timer Core**: [`ToastQueue`], [`ToastMessage`], and [`ToastKind`]
//! manage timed life cycles, capacity limits, dismiss events, and countdown timers.
//! Zero-bevy and 100% headless-testable.
//!
//! **Scene Adapters**: [`toast_item_scene`] and [`toast_stack_scene`] build declarative
//! toast cards with semantic accent borders, icon plates, and dismiss actions.

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::{Message, MessageReader};
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::time::{Time, Virtual};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, PositionType,
    UiRect, Val, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;

use crate::icon::IconId;
use crate::icon_tile::icon_tile_scene;
use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::space;

/// Semantic type of a toast alert.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToastKind {
    #[default]
    Info,
    Success,
    Warning,
    Danger,
}

/// One active toast notification item.
#[derive(Clone, Debug, PartialEq)]
pub struct ToastMessage {
    pub id: u64,
    pub title: Option<String>,
    pub content: String,
    pub kind: ToastKind,
    pub duration_secs: f32,
    pub remaining_secs: f32,
}

impl ToastMessage {
    pub fn new(id: u64, content: impl Into<String>, kind: ToastKind, duration_secs: f32) -> Self {
        Self {
            id,
            title: None,
            content: content.into(),
            kind,
            duration_secs,
            remaining_secs: duration_secs,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
}

/// Pure state manager and queue for toast notifications.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ToastQueue {
    toasts: Vec<ToastMessage>,
    max_capacity: usize,
    next_id: u64,
}

impl ToastQueue {
    /// Create a new toast queue with maximum capacity.
    pub fn new(max_capacity: usize) -> Self {
        Self {
            toasts: Vec::new(),
            max_capacity: max_capacity.max(1),
            next_id: 1,
        }
    }

    /// Number of active toasts.
    pub fn len(&self) -> usize {
        self.toasts.len()
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.toasts.is_empty()
    }

    /// Slice of active toasts.
    pub fn items(&self) -> &[ToastMessage] {
        &self.toasts
    }

    /// Push a new toast message. Evicts oldest if exceeding max capacity. Returns new toast ID.
    pub fn push(&mut self, content: impl Into<String>, kind: ToastKind, duration_secs: f32) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);

        if self.toasts.len() >= self.max_capacity {
            self.toasts.remove(0);
        }

        self.toasts
            .push(ToastMessage::new(id, content, kind, duration_secs));
        id
    }

    /// Dismiss a toast by ID. Returns true if toast was found and removed.
    pub fn dismiss(&mut self, id: u64) -> bool {
        if let Some(pos) = self.toasts.iter().position(|t| t.id == id) {
            self.toasts.remove(pos);
            true
        } else {
            false
        }
    }

    /// Advance remaining time by delta seconds. Removes expired toasts and returns their IDs.
    pub fn tick(&mut self, delta_secs: f32) -> Vec<u64> {
        let mut expired = Vec::new();
        for toast in &mut self.toasts {
            toast.remaining_secs -= delta_secs;
            if toast.remaining_secs <= 0.0 {
                expired.push(toast.id);
            }
        }

        self.toasts.retain(|t| t.remaining_secs > 0.0);
        expired
    }
}

/// Accent color for a given toast kind. Pure function.
pub fn toast_accent_color(kind: ToastKind, palette: &UiPalette) -> Color {
    match kind {
        ToastKind::Info => palette.accent,
        ToastKind::Success => palette.success,
        ToastKind::Warning => palette.warning,
        ToastKind::Danger => palette.danger,
    }
}

/// Marker component on toast stack container.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToastContainer;

/// Marker component on an individual toast card carrying its unique ID.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToastCard(pub u64);

/// Marker component on a toast dismiss button carrying the target toast ID.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToastDismissButton(pub u64);

/// Message to request spawning a toast alert.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct ToastSpawnEvent {
    pub content: String,
    pub kind: ToastKind,
    pub duration_secs: f32,
}

/// Message to dismiss an active toast alert.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToastDismissEvent(pub u64);

/// Construct a single toast card scene.
pub fn toast_item_scene(toast: &ToastMessage, palette: &UiPalette) -> impl Scene + use<> {
    let accent = toast_accent_color(toast.kind, palette);
    let id = toast.id;
    let content_text = toast.content.clone();

    let icon_id = match toast.kind {
        ToastKind::Info => IconId::Globe,
        ToastKind::Success => IconId::Zap,
        ToastKind::Warning => IconId::Activity,
        ToastKind::Danger => IconId::Trash,
    };

    bsn! {
        Node {
            width: px(320.0),
            padding: UiRect::all(Val::Px(space::S12)),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S8),
            border: UiRect::left(Val::Px(4.0)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface })
        BorderColor {
            top: Color::NONE,
            right: Color::NONE,
            bottom: Color::NONE,
            left: accent,
        }
        ToastCard(id)
        Children [
            ( { icon_tile_scene(icon_id, 20.0, palette) } ),
            (
                Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                }
                Children [
                    ( Text(content_text) TextRole(Role::Body) ),
                ]
            ),
            (
                Node {
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                }
                Button
                ToastDismissButton(id)
                Children [
                    ( { icon_tile_scene(IconId::Trash, 16.0, palette) } ),
                ]
            ),
        ]
    }
}

/// Construct the toast stack overlay scene.
pub fn toast_stack_scene(toasts: &[ToastMessage], palette: &UiPalette) -> impl Scene + use<> {
    let toast_nodes: Vec<Box<dyn Scene>> = toasts
        .iter()
        .map(|t| Box::new(toast_item_scene(t, palette)) as Box<dyn Scene>)
        .collect();

    bsn! {
        Node {
            position_type: PositionType::Absolute,
            right: px(space::S16),
            top: px(space::S16),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S8),
        }
        ToastContainer
        Children [
            { toast_nodes },
        ]
    }
}

/// System to advance toast timers and process dismiss events.
pub fn advance_toasts(
    time: Res<Time<Virtual>>,
    mut queue: Option<ResMut<ToastQueue>>,
    mut dismisses: MessageReader<ToastDismissEvent>,
    mut spawns: MessageReader<ToastSpawnEvent>,
) {
    let Some(ref mut q) = queue else { return };

    for spawn in spawns.read() {
        q.push(spawn.content.clone(), spawn.kind, spawn.duration_secs);
    }

    for dismiss in dismisses.read() {
        q.dismiss(dismiss.0);
    }

    let dt = time.delta().as_secs_f32();
    q.tick(dt);
}

/// System to repaint toast card backgrounds from live palette.
pub fn sync_toast_visuals(
    palette: Res<UiPalette>,
    mut cards: Query<(&ToastCard, &mut BackgroundColor), With<ToastCard>>,
) {
    for (_, mut bg) in &mut cards {
        if bg.0 != palette.surface {
            bg.0 = palette.surface;
        }
    }
}
