//! Master-Detail split pane / stacked pane adaptive layout system.
//!
//! On Compact (<600px) viewports: functions as a single-pane stacked view with back button navigation.
//! On Medium/Expanded/Ultra (>=600px) viewports: functions as a dual-pane side-by-side split screen.

use crate::icon::IconId;
use crate::icon_tile::icon_tile_scene;
use crate::palette::UiPalette;
use crate::responsive::MasterDetailMode;
use crate::responsive::ResponsiveContext;
use crate::text::{Role, TextRole};
use crate::theme::space;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, Display, FlexDirection, Node, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};

/// Active view in stacked navigation mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MasterDetailView {
    /// Showing master list.
    #[default]
    Master,
    /// Showing detail view.
    Detail,
}

/// Active selection and view state in master-detail layout.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct MasterDetailState {
    pub selected_item_id: Option<String>,
    pub showing_detail_mobile: bool,
    pub active_view: MasterDetailView,
}

impl MasterDetailState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn select(&mut self, id: impl Into<String>) {
        self.selected_item_id = Some(id.into());
        self.showing_detail_mobile = true;
        self.active_view = MasterDetailView::Detail;
    }

    pub fn back(&mut self) {
        self.active_view = MasterDetailView::Master;
        self.showing_detail_mobile = false;
        self.showing_detail_mobile = false;
    }

    pub fn clear(&mut self) {
        self.selected_item_id = None;
        self.active_view = MasterDetailView::Master;
        self.showing_detail_mobile = false;
        self.showing_detail_mobile = false;
    }
}

/// Marker component for master-detail container.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MasterDetailContainer;

/// Marker component for master pane.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MasterPane;

/// Marker component for detail pane.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DetailPane;

/// Marker component for back button inside detail pane on mobile.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MasterBackButton;

/// Marker for an item button in master list.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct MasterItemButton(pub String);

/// Event when an item in the master pane is selected.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub struct MasterItemSelected(pub String);

/// Event when back button is pressed in detail view on mobile.
#[derive(Event, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MasterNavBack;

/// Declarative constructor for adaptive master-detail layout.
pub fn master_detail_scene(
    master_pane: Box<dyn Scene>,
    detail_pane: Box<dyn Scene>,
    _palette: &UiPalette,
) -> Box<dyn Scene> {
    Box::new(bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Row,
        }
        MasterDetailContainer
        Children [
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    width: px(320.0),
                    flex_shrink: 0.0,
                }
                MasterPane
                Children [
                    { vec![master_pane] },
                ]
            ),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    width: percent(100),
                }
                DetailPane
                Children [
                    { vec![detail_pane] },
                ]
            ),
        ]
    })
}

/// Back button scene for detail views in stacked/compact mode.
pub fn master_back_button_scene(label: &str, palette: &UiPalette) -> Box<dyn Scene> {
    let lbl = label.to_owned();
    Box::new(bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S8),
            padding: UiRect::all(Val::Px(space::S8)),
        }
        BackgroundColor({ palette.surface_elevated })
        Button
        MasterBackButton
        Children [
            ( { icon_tile_scene(IconId::ArrowUp, 20.0, palette) } ),
            ( Text(lbl) TextRole(Role::Body) ),
        ]
    })
}

/// Observer for item activation in the master list.
pub fn on_master_item_button_activated(
    activate: On<Activate>,
    buttons: Query<&MasterItemButton>,
    mut commands: Commands,
) {
    if let Ok(btn) = buttons.get(activate.entity) {
        commands.trigger(MasterItemSelected(btn.0.clone()));
    }
}

/// Observer for selection event.
pub fn on_master_item_selected(
    event: On<MasterItemSelected>,
    mut state: Option<ResMut<MasterDetailState>>,
) {
    if let Some(ref mut state) = state {
        state.select(&event.0);
    }
}

/// Observer for back button activation.
pub fn on_master_back_activated(
    activate: On<Activate>,
    back_buttons: Query<(), With<MasterBackButton>>,
    mut state: Option<ResMut<MasterDetailState>>,
    mut commands: Commands,
) {
    if back_buttons.contains(activate.entity) {
        if let Some(ref mut state) = state {
            state.back();
        }
        commands.trigger(MasterNavBack);
    }
}

/// System to sync master-detail split vs stacked layout based on responsive context.
#[allow(clippy::type_complexity)]
pub fn sync_master_detail_layout(
    ctx: Option<Res<ResponsiveContext>>,
    state: Option<Res<MasterDetailState>>,
    mut masters: Query<
        &mut Node,
        (
            With<MasterPane>,
            Without<DetailPane>,
            Without<MasterBackButton>,
        ),
    >,
    mut details: Query<
        &mut Node,
        (
            With<DetailPane>,
            Without<MasterPane>,
            Without<MasterBackButton>,
        ),
    >,
    mut back_buttons: Query<
        &mut Node,
        (
            With<MasterBackButton>,
            Without<MasterPane>,
            Without<DetailPane>,
        ),
    >,
) {
    let mode = ctx
        .map(|c| c.master_detail_mode())
        .unwrap_or(MasterDetailMode::Split);
    let showing_detail = state
        .as_ref()
        .is_some_and(|s| s.active_view == MasterDetailView::Detail);

    for mut master in &mut masters {
        match mode {
            MasterDetailMode::Split => {
                master.display = Display::Flex;
                master.width = px(320.0);
            }
            MasterDetailMode::Stacked => {
                master.width = percent(100);
                master.display = if showing_detail {
                    Display::None
                } else {
                    Display::Flex
                };
            }
        }
    }

    for mut detail in &mut details {
        match mode {
            MasterDetailMode::Split => {
                detail.display = Display::Flex;
                detail.flex_grow = 1.0;
            }
            MasterDetailMode::Stacked => {
                detail.width = percent(100);
                detail.display = if showing_detail {
                    Display::Flex
                } else {
                    Display::None
                };
            }
        }
    }

    for mut back_btn in &mut back_buttons {
        match mode {
            MasterDetailMode::Split => {
                back_btn.display = Display::None;
            }
            MasterDetailMode::Stacked => {
                back_btn.display = if showing_detail {
                    Display::Flex
                } else {
                    Display::None
                };
            }
        }
    }
}
