//! Independent native operation surfaces retaining the production form trees.

use crate::pages::profiles::ProfilesProjection;
use crate::pages::profiles_aggregator::profiles_aggregator_scene;
use crate::pages::profiles_diff::snapshot_diff_scene;
use crate::pages::proxies_form::CustomNodeForm;
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::prelude::*;
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::input_focus::{AcquireFocus, FocusCause, InputFocus};
use bevy::scene::{Scene, bsn, on};
use bevy::ui::prelude::*;
use bevy::ui_widgets::ScrollArea;
use bevy::ui_widgets::{Activate, Button, ModalDialog, ModalDialogBarrier, RequestClose};
use bevy::window::PrimaryWindow;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use lurker_core_contract::page::PageStatus;

/// One-off layout value (not a ladder rung): the widest the business panel
/// dialog card may grow on a wide viewport.
const BUSINESS_PANEL_MAX_WIDTH_PX: f32 = 960.0;

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
            padding: UiRect::axes(rem(space::S16), rem(space::S12)),
            min_height: rem(palette.control_height_px), align_items: AlignItems::Center,
            border_radius: BorderRadius::all(rem(palette.control_radius_px)),
        }
        ThemeBackgroundColor({ Token::SurfaceElevated })
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
        // The panel is a modal dialog on Bevy's official behaviour layer: the
        // barrier owns the click-outside close and the Escape interception, the
        // card owns the focus containment, and every close request travels as
        // the official `RequestClose`.
        //
        // The barrier is the panel root — the only common ancestor of the
        // scrim and the card — because the official Escape interception
        // reaches the barrier through the focused entity and the official
        // `RequestClose` reaches it through the closing control, both of
        // which live under the card.
        ModalDialogBarrier
        on(move |close: On<RequestClose>, closes: Query<(Entity, &CloseBusinessPanel), Without<ModalScrim>>, mut commands: Commands| {
            // A close request that names this panel's own close control is
            // that control's business (it already asked to close); every other
            // close request — the barrier's click-outside and the Escape
            // interception — runs the panel's own close path.
            for (entity, marker) in &closes {
                if marker.0 == kind && entity != close.original_event_target() {
                    commands.trigger(Activate { entity });
                }
            }
        })
        Children [
            Node {
                position_type: PositionType::Absolute,
                width: percent(100), height: percent(100),
            }
            ThemeBackgroundColor({ Token::Scrim })
            CloseBusinessPanel(kind)
            ModalScrim
            --
            Node {
                width: percent(90), max_width: rem(BUSINESS_PANEL_MAX_WIDTH_PX), max_height: percent(90),
                min_width: px(0.0), flex_direction: FlexDirection::Column,
                padding: UiRect::all(rem(space::S16)), row_gap: rem(space::S12),
                // The business panel is the contract's panel-style dialog, so
                // it keeps the card rung; only a centered dialog takes the
                // modal rung.
                border_radius: BorderRadius::all(rem(palette.card_radius_px)),
            }
            ThemeBackgroundColor({ Token::Surface })
            ModalDialogCard BusinessPanelCard(kind)
            ModalDialog
            Children [
                Node { width: percent(100), align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween }
                Children [
                    title TextRole(Role::Heading)
                    --
                    Node { padding: UiRect::axes(rem(space::S12), rem(space::S8)), min_height: rem(palette.control_height_px) }
                    Button CloseBusinessPanel(kind)
                    // The dismiss control is the dialog's focusable control:
                    // the official `TabIndex` marks it for Bevy's focus
                    // scheme, so the focus hand-off can move the input focus
                    // into the panel.
                    TabIndex(0)
                    // The close control asks the official layer to close; the
                    // barrier observer runs the panel's own close path.
                    on(|activate: On<Activate>, mut commands: Commands| {
                        commands.trigger(RequestClose { source: activate.entity });
                    })
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
    close: Query<(Entity, &CloseBusinessPanel), Without<ModalScrim>>,
    mut state: ResMut<BusinessPanelState>,
    mut form: ResMut<CustomNodeForm>,
    latest: Option<Res<LatestSurfaceSnapshot>>,
    mut roots: Query<(&BusinessPanelRoot, &mut Node)>,
    mut commands: Commands,
    mut focus: ResMut<InputFocus>,
    windows: Query<Entity, With<PrimaryWindow>>,
) {
    if let Ok(open) = open.get(activate.entity) {
        if !roots.iter().any(|(root, _)| root.0 == open.0) {
            return;
        }
        if open.0 == PanelKind::CustomNode && form.operation.is_busy() {
            return;
        }
        if open.0 == PanelKind::CustomNode && state.0 != Some(open.0) {
            let studio = latest
                .as_ref()
                .and_then(|snapshot| match &snapshot.0.protocol_studio.status {
                    PageStatus::Ready | PageStatus::Empty => {
                        snapshot.0.protocol_studio.data.as_ref()
                    }
                    PageStatus::Loading
                    | PageStatus::Unavailable { .. }
                    | PageStatus::Failed { .. } => None,
                })
                .cloned()
                .unwrap_or_else(|| form.studio.clone());
            form.begin(studio);
        }
        state.0 = Some(open.0);
        // The panel is presented: the official focus hand-off moves the input
        // focus into the dialog, so Bevy's own `ModalDialogBarrier` owns the
        // Escape interception.
        if let Some(control) = dismiss_control(&close, open.0) {
            hand_off_focus(&mut commands, &mut focus, &windows, control);
        }
    } else if let Ok((_, closing)) = close.get(activate.entity) {
        if state.0 != Some(closing.0) {
            return;
        }
        if closing.0 == PanelKind::CustomNode {
            form.operation.cancel();
        }
        state.0 = None;
        // The panel is closed: the focus leaves the dialog again.
        release_focus(&mut focus, dismiss_control(&close, closing.0));
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

/// The panel's dismiss control: the one close control that is not the scrim.
fn dismiss_control(
    close: &Query<(Entity, &CloseBusinessPanel), Without<ModalScrim>>,
    kind: PanelKind,
) -> Option<Entity> {
    close
        .iter()
        .find(|(_, marker)| marker.0 == kind)
        .map(|(entity, _)| entity)
}

/// The official focus hand-off.
///
/// The dialog's dismiss control is the dialog's focusable control — the
/// official `TabIndex` marks it for Bevy's focus scheme — and the official
/// `AcquireFocus` request asks that scheme to move the input focus into the
/// dialog. The official focus resource carries the result, so the page and
/// the official layer read the same fact. While the focus lives inside the
/// barrier, Bevy's own `ModalDialogBarrier` owns the Escape interception.
fn hand_off_focus(
    commands: &mut Commands,
    focus: &mut InputFocus,
    windows: &Query<Entity, With<PrimaryWindow>>,
    control: Entity,
) {
    commands
        .entity(control)
        .entry::<TabIndex>()
        .or_insert(TabIndex(0));
    if let Ok(window) = windows.single() {
        commands.trigger(AcquireFocus {
            focused_entity: control,
            window,
        });
    }
    focus.set(control, FocusCause::Navigated);
}

/// Release the input focus the hand-off moved into the dialog.
fn release_focus(focus: &mut InputFocus, control: Option<Entity>) {
    if let Some(control) = control
        && focus.get() == Some(control)
    {
        focus.clear();
    }
}

pub(crate) fn cancel_panel(
    route: Res<ActiveRoute>,
    mut state: ResMut<BusinessPanelState>,
    mut form: ResMut<CustomNodeForm>,
    mut roots: Query<&mut Node, With<BusinessPanelRoot>>,
    mut focus: ResMut<InputFocus>,
    close: Query<(Entity, &CloseBusinessPanel), Without<ModalScrim>>,
) {
    if let Some(kind) = state.0
        && route.0 != Some(kind.route())
    {
        if kind == PanelKind::CustomNode {
            form.operation.cancel();
        }
        state.0 = None;
        for mut node in &mut roots {
            node.display = Display::None;
        }
        // The panel is closed: the focus leaves the dialog again.
        release_focus(&mut focus, dismiss_control(&close, kind));
    }
}
