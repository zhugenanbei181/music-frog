//! Independent native operation surfaces retaining the production form trees.

use crate::pages::profiles::ProfilesProjection;
use crate::pages::profiles_aggregator::profiles_aggregator_scene;
use crate::pages::profiles_diff::snapshot_diff_scene;
use crate::pages::proxies_form::CustomNodeForm;
use crate::route::{ActiveRoute, Route};
use bevy::ecs::prelude::*;
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::*;
use bevy::ui_widgets::ScrollArea;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PanelKind {
    #[default]
    CustomNode,
    Aggregator,
    SnapshotDiff,
}

impl PanelKind {
    fn title_key(self) -> &'static str {
        match self {
            Self::CustomNode => "custom_node_title",
            Self::Aggregator => "aggregator_title",
            Self::SnapshotDiff => "snapshot_diff_title",
        }
    }
    fn route(self) -> Route {
        match self {
            Self::CustomNode => Route::Proxies,
            Self::Aggregator | Self::SnapshotDiff => Route::Profiles,
        }
    }
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BusinessPanelRoot(pub PanelKind);
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BusinessPanelCard(pub PanelKind);
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BusinessPanelScrollArea(pub PanelKind);
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpenBusinessPanel(pub PanelKind);
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CloseBusinessPanel(pub PanelKind);
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BusinessPanelState(pub Option<PanelKind>);

pub fn launcher_scene(kind: PanelKind, palette: &UiPalette) -> impl Scene + use<> {
    let title = LocalizedText::plain(kind.title_key());
    bsn! {
        Node {
            padding: UiRect::axes(px(16.0), px(12.0)),
            min_height: px(palette.control_height_px), align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        Button OpenBusinessPanel(kind)
        Children [ title TextRole(Role::BodyStrong) ]
    }
}

pub fn panel_scene(
    kind: PanelKind,
    content: Box<dyn Scene>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let title = LocalizedText::plain(kind.title_key());
    let close = LocalizedText::plain("modal_cancel");
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0), top: px(0.0), width: percent(100), height: percent(100),
            align_items: AlignItems::Center, justify_content: JustifyContent::Center,
            display: Display::None,
        }
        BusinessPanelRoot(kind) GlobalZIndex(90)
        Children [
            Node {
                position_type: PositionType::Absolute,
                width: percent(100), height: percent(100),
            }
            BackgroundColor({ palette.scrim }) Button CloseBusinessPanel(kind)
            ModalScrim
            --
            Node {
                width: percent(90), max_width: px(960.0), max_height: percent(90),
                min_width: px(0.0), flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(16.0)), row_gap: px(12.0),
                border_radius: BorderRadius::all(px(palette.card_radius_px)),
            }
            BackgroundColor({ palette.surface })
            ModalDialogCard BusinessPanelCard(kind)
            Children [
                Node { width: percent(100), align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween }
                Children [
                    title TextRole(Role::Heading)
                    --
                    Node { padding: UiRect::axes(px(12.0), px(8.0)), min_height: px(palette.control_height_px) }
                    Button CloseBusinessPanel(kind)
                    Children [ close TextRole(Role::Body) ]
                ]
                --
                Node {
                    width: percent(100), min_width: px(0.0), min_height: px(0.0),
                    flex_shrink: 1.0, overflow: Overflow::scroll_y(),
                    flex_direction: FlexDirection::Column,
                }
                ScrollArea BusinessPanelScrollArea(kind)
                Children [ @{ content } ]
            ]
        ]
    }
}

pub fn profiles_panels(
    projection: &ProfilesProjection,
    palette: &UiPalette,
) -> Vec<Box<dyn Scene>> {
    vec![
        Box::new(panel_scene(
            PanelKind::Aggregator,
            Box::new(profiles_aggregator_scene(projection, palette)),
            palette,
        )),
        Box::new(panel_scene(
            PanelKind::SnapshotDiff,
            Box::new(snapshot_diff_scene(projection, palette)),
            palette,
        )),
    ]
}

pub(crate) fn activate_panel(
    activate: On<Activate>,
    open: Query<&OpenBusinessPanel>,
    close: Query<&CloseBusinessPanel>,
    mut state: ResMut<BusinessPanelState>,
    mut form: ResMut<CustomNodeForm>,
    mut roots: Query<(&BusinessPanelRoot, &mut Node)>,
) {
    if let Ok(open) = open.get(activate.entity) {
        if !roots.iter().any(|(root, _)| root.0 == open.0) {
            return;
        }
        if open.0 == PanelKind::CustomNode && form.saving.is_some() {
            return;
        }
        if open.0 == PanelKind::CustomNode && state.0 != Some(open.0) {
            form.begin();
        }
        state.0 = Some(open.0);
    } else if let Ok(close) = close.get(activate.entity) {
        if state.0 != Some(close.0) {
            return;
        }
        state.0 = None;
    } else {
        return;
    }
    for (kind, mut node) in &mut roots {
        node.display = if state.0 == Some(kind.0) {
            Display::Flex
        } else {
            Display::None
        };
    }
}

pub(crate) fn cancel_panel(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    route: Res<ActiveRoute>,
    mut state: ResMut<BusinessPanelState>,
    mut roots: Query<&mut Node, With<BusinessPanelRoot>>,
) {
    if let Some(kind) = state.0
        && (route.0 != Some(kind.route())
            || keys.is_some_and(|keys| keys.just_pressed(KeyCode::Escape)))
    {
        state.0 = None;
        for mut node in &mut roots {
            node.display = Display::None;
        }
    }
}
