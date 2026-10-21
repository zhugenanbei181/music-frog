//! BANDROID-026.2: 50-row scrollable list and three-button/menu acceptance page.
//!
//! Validates:
//! - a 50-row list inside the official `ScrollArea`, revealed through the
//!   official `ScrollIntoView` event
//! - three primary navigation buttons (Top / Reveal / Bottom) that ask the
//!   official mechanism to reveal their row
//! - action menu popup with selection commands
//! - integration with BANDROID-025 shared acceptance scenario ID (`SCN-10-ACCEPTANCE-LIST`)
//! - diagnostic tracking via `TouchLayerDiagnosticEntry` and `TouchLayerDiagnosticCategory`
//!
//! **One offset owner**: the official viewport owns the scroll offset. The
//! page keeps no offset of its own; the typed [`VirtualListState`] mirrors
//! the viewport so the model and the rendered list can never disagree.

use crate::gesture::TouchLayerDiagnosticEntry;
use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BorderRadius, FlexDirection, JustifyContent, Node, Overflow, ScrollPosition,
    UiRect, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ScrollArea, ScrollAreaPlugin};
use infiltrator_bevy_widgets::list::scrollbar::scroll_item_into_view;
use infiltrator_bevy_widgets::list::{ScrollOffsetOutcome, SelectionOutcome, VirtualListState};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_shared::i18n_interpolator::localize;

/// Scenario identifier shared with Compose and automated test walkthroughs (BANDROID-025/026).
pub const ACCEPTANCE_LIST_SCENARIO_ID: &str = "SCN-10-ACCEPTANCE-LIST";

/// Total rows in the acceptance list dataset.
pub const ACCEPTANCE_ROW_COUNT: usize = 50;
/// Fixed uniform row height in pixels.
pub const ACCEPTANCE_ROW_HEIGHT_PX: f32 = 44.0;
/// Viewport height in pixels.
pub const ACCEPTANCE_VIEWPORT_HEIGHT_PX: f32 = 440.0;

/// The first row of the acceptance dataset. Revealing it brings the list back
/// to its top, because the official mechanism puts a row that lies above the
/// viewport at the viewport's top edge.
pub const ACCEPTANCE_FIRST_ROW: usize = 0;
/// The row the reveal affordance asks the official mechanism to reveal.
pub const ACCEPTANCE_REVEAL_ROW: usize = ACCEPTANCE_ROW_COUNT / 2;
/// The last row of the acceptance dataset. Revealing it brings the list to its
/// bottom, because the official mechanism puts a row that lies below the
/// viewport at the viewport's bottom edge.
pub const ACCEPTANCE_LAST_ROW: usize = ACCEPTANCE_ROW_COUNT - 1;

/// Item data in the acceptance list dataset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptanceItem {
    pub index: usize,
    pub title: String,
    pub subtitle: String,
    pub selected: bool,
}

/// Menu choice in the acceptance action menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcceptanceMenuChoice {
    SelectFirst,
    SelectMiddle,
    SelectLast,
    ClearSelection,
}

/// The acceptance page's own reveal outcome: what the page asked the official
/// scroll-into-view mechanism to reveal.
///
/// The official mechanism REVEALS a row — it scrolls the viewport the minimum
/// distance that brings the row into view. It never centres a row, so the
/// page's affordance says "reveal" and never promises a centred row.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcceptanceRevealOutcome {
    /// The requested row is outside the acceptance dataset.
    IndexOutOfRange { index: usize, item_count: usize },
    /// The requested row is not mounted, so there is no row entity the
    /// official mechanism could reveal.
    RowNotMounted { index: usize },
    /// The official scroll-into-view event was sent for the mounted row. The
    /// official observer measures the row against the viewport and decides how
    /// far the viewport scrolls.
    RevealRequested { index: usize },
}

/// State resource for the 50-row acceptance page.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct AcceptanceListState {
    pub scenario_id: &'static str,
    pub list: VirtualListState,
    pub items: Vec<AcceptanceItem>,
    pub menu_open: bool,
    /// The last typed fact of the model's mirrored scroll offset.
    pub last_scroll_offset_outcome: Option<ScrollOffsetOutcome>,
    /// The last typed fact of the official scroll-into-view mechanism.
    pub last_reveal_outcome: Option<AcceptanceRevealOutcome>,
    pub last_selection_outcome: Option<SelectionOutcome>,
    pub last_diagnostic: Option<TouchLayerDiagnosticEntry>,
    pub diagnostic_log: Vec<TouchLayerDiagnosticEntry>,
}

impl Default for AcceptanceListState {
    fn default() -> Self {
        let items: Vec<AcceptanceItem> = (0..ACCEPTANCE_ROW_COUNT)
            .map(|i| AcceptanceItem {
                index: i,
                title: format!("Acceptance Item #{:02}", i + 1),
                subtitle: format!("Row {} · Scenario {}", i, ACCEPTANCE_LIST_SCENARIO_ID),
                selected: false,
            })
            .collect();
        let list = VirtualListState::new(
            ACCEPTANCE_ROW_COUNT,
            ACCEPTANCE_ROW_HEIGHT_PX,
            ACCEPTANCE_VIEWPORT_HEIGHT_PX,
        );
        Self {
            scenario_id: ACCEPTANCE_LIST_SCENARIO_ID,
            list,
            items,
            menu_open: false,
            last_scroll_offset_outcome: None,
            last_reveal_outcome: None,
            last_selection_outcome: None,
            last_diagnostic: None,
            diagnostic_log: Vec::new(),
        }
    }
}

impl AcceptanceListState {
    /// Reveal one row through the official scroll-into-view mechanism.
    ///
    /// The official observer measures the row against the viewport and moves
    /// the viewport the minimum distance that brings the row into view. The
    /// page keeps no scroll offset of its own: the viewport owns it, and
    /// [`sync_acceptance_scroll_offset`] mirrors it into the typed model.
    ///
    /// `row` is the mounted row entity, or `None` when the requested index has
    /// no mounted row for the official mechanism to reveal.
    pub fn reveal_row(
        &mut self,
        commands: &mut Commands,
        index: usize,
        row: Option<Entity>,
    ) -> AcceptanceRevealOutcome {
        let outcome = match row {
            Some(entity) => {
                scroll_item_into_view(commands, entity);
                AcceptanceRevealOutcome::RevealRequested { index }
            }
            None if index >= self.items.len() => AcceptanceRevealOutcome::IndexOutOfRange {
                index,
                item_count: self.items.len(),
            },
            None => AcceptanceRevealOutcome::RowNotMounted { index },
        };
        self.last_reveal_outcome = Some(outcome);
        outcome
    }

    /// Select one row index.
    pub fn select_item(&mut self, index: usize) -> SelectionOutcome {
        let outcome = self.list.select(Some(index));
        if let SelectionOutcome::Changed { selected_index, .. } = outcome {
            for item in &mut self.items {
                item.selected = Some(item.index) == selected_index;
            }
        }
        self.last_selection_outcome = Some(outcome);
        outcome
    }

    /// Clear all selections.
    pub fn clear_selection(&mut self) -> SelectionOutcome {
        let outcome = self.list.select(None);
        for item in &mut self.items {
            item.selected = false;
        }
        self.last_selection_outcome = Some(outcome);
        outcome
    }

    /// Toggle action menu open state.
    pub fn toggle_menu(&mut self) -> bool {
        self.menu_open = !self.menu_open;
        self.menu_open
    }

    /// Apply an action menu choice.
    ///
    /// The menu chooses a row; it never scrolls. Scrolling is the job of the
    /// official reveal mechanism, so the menu's choices and the page's scroll
    /// affordances cannot disagree about where the list is.
    pub fn apply_menu_choice(&mut self, choice: AcceptanceMenuChoice) {
        match choice {
            AcceptanceMenuChoice::SelectFirst => {
                let _ = self.select_item(ACCEPTANCE_FIRST_ROW);
            }
            AcceptanceMenuChoice::SelectMiddle => {
                let _ = self.select_item(ACCEPTANCE_REVEAL_ROW);
            }
            AcceptanceMenuChoice::SelectLast => {
                let _ = self.select_item(ACCEPTANCE_LAST_ROW);
            }
            AcceptanceMenuChoice::ClearSelection => {
                let _ = self.clear_selection();
            }
        }
        self.menu_open = false;
    }

    /// Record a diagnostic entry conforming to the 025 diagnostic category scheme.
    pub fn record_diagnostic(&mut self, entry: TouchLayerDiagnosticEntry) {
        self.last_diagnostic = Some(entry.clone());
        self.diagnostic_log.push(entry);
        if self.diagnostic_log.len() > 100 {
            self.diagnostic_log.remove(0);
        }
    }
}

// Markers for scene queries
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct AcceptanceListPageRoot;

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct AcceptanceScrollToTopButton;

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct AcceptanceScrollToRevealButton;

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct AcceptanceScrollToBottomButton;

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct AcceptanceMenuButton;

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct AcceptanceRowItem(pub usize);

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct AcceptanceRowsContainer;

/// Build one row scene.
pub fn acceptance_row_scene(item: &AcceptanceItem) -> impl Scene + use<> {
    let title = item.title.clone();
    let subtitle = item.subtitle.clone();
    let index = item.index;
    bsn! {
        Node {
            width: percent(100),
            height: px(ACCEPTANCE_ROW_HEIGHT_PX),
            padding: UiRect::axes(rem(space::S12), px(0.0)),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
        }
        AcceptanceRowItem({ index })
        Children [
            Text({ title }) TextRole(Role::Body)
            --
            Text({ subtitle }) TextRole(Role::Caption)
        ]
    }
}

/// Build the acceptance page scene.
pub fn acceptance_list_scene(
    state: &AcceptanceListState,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let viewport_str = ACCEPTANCE_VIEWPORT_HEIGHT_PX.to_string();
    let row_str = ACCEPTANCE_ROW_HEIGHT_PX.to_string();
    let title_text = localize(
        "zh-CN",
        "acceptance_list_title",
        &[("scenario", state.scenario_id.to_string())],
    );
    let desc_text = localize(
        "zh-CN",
        "acceptance_list_desc",
        &[("viewport", viewport_str), ("row", row_str)],
    );
    let btn_top_text = localize("zh-CN", "acceptance_btn_top", &[]);
    let btn_reveal_text = localize("zh-CN", "acceptance_btn_reveal", &[]);
    let btn_bottom_text = localize("zh-CN", "acceptance_btn_bottom", &[]);
    let btn_menu_text = localize("zh-CN", "acceptance_btn_menu", &[]);
    let radius = rem(palette.control_radius_px);
    // The whole dataset is mounted: the official scroll-into-view mechanism
    // needs a mounted row entity to reveal, and the official scrollbar needs
    // the real content height to size its thumb.
    let row_scenes: Vec<Box<dyn Scene>> = state
        .items
        .iter()
        .map(|item| Box::new(acceptance_row_scene(item)) as Box<dyn Scene>)
        .collect();

    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: rem(space::S12),
            padding: UiRect::all(rem(space::S16)),
        }
        AcceptanceListPageRoot
        Children [
            // Title & Scenario banner
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S4),
            }
            Children [
                Text({ title_text }) TextRole(Role::Heading)
                --
                Text({ desc_text }) TextRole(Role::Caption)
            ]
            --
            // Three navigation buttons + Menu button
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: rem(space::S8),
                align_items: AlignItems::Center,
            }
            Children [
                // Button 1: reveal the first row (the list's top)
                Node {
                    padding: UiRect::axes(rem(space::S12), rem(space::S8)),
                    border_radius: BorderRadius::all(radius),
                }
                AcceptanceScrollToTopButton Button
                Children [ Text({ btn_top_text }) TextRole(Role::BodyStrong) ]
                --
                // Button 2: reveal the middle row
                Node {
                    padding: UiRect::axes(rem(space::S12), rem(space::S8)),
                    border_radius: BorderRadius::all(radius),
                }
                AcceptanceScrollToRevealButton Button
                Children [ Text({ btn_reveal_text }) TextRole(Role::BodyStrong) ]
                --
                // Button 3: reveal the last row (the list's bottom)
                Node {
                    padding: UiRect::axes(rem(space::S12), rem(space::S8)),
                    border_radius: BorderRadius::all(radius),
                }
                AcceptanceScrollToBottomButton Button
                Children [ Text({ btn_bottom_text }) TextRole(Role::BodyStrong) ]
                --
                // Action Menu button
                Node {
                    padding: UiRect::axes(rem(space::S12), rem(space::S8)),
                    border_radius: BorderRadius::all(radius),
                }
                AcceptanceMenuButton Button
                Children [ Text({ btn_menu_text }) TextRole(Role::BodyStrong) ]
            ]
            --
            // The official scroll area over the mounted rows
            Node {
                width: percent(100),
                height: px(ACCEPTANCE_VIEWPORT_HEIGHT_PX),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
            }
            ScrollArea
            AcceptanceRowsContainer
            Children [
                { row_scenes }
            ]
        ]
    }
}

/// The acceptance page's scroll affordances: each one asks the official
/// scroll-into-view mechanism to reveal its row.
///
/// The official mechanism decides how far the viewport scrolls — it reveals
/// the row, it never centres it.
pub fn on_acceptance_scroll_activated(
    activate: On<Activate>,
    top: Query<(), With<AcceptanceScrollToTopButton>>,
    reveal: Query<(), With<AcceptanceScrollToRevealButton>>,
    bottom: Query<(), With<AcceptanceScrollToBottomButton>>,
    rows: Query<(Entity, &AcceptanceRowItem), With<AcceptanceRowItem>>,
    mut state: ResMut<AcceptanceListState>,
    mut commands: Commands,
) {
    let index = if top.contains(activate.entity) {
        ACCEPTANCE_FIRST_ROW
    } else if reveal.contains(activate.entity) {
        ACCEPTANCE_REVEAL_ROW
    } else if bottom.contains(activate.entity) {
        ACCEPTANCE_LAST_ROW
    } else {
        return;
    };
    let row = rows
        .iter()
        .find(|(_, item)| item.0 == index)
        .map(|(entity, _)| entity);
    // The typed fact of the request is recorded in the page's state.
    let _outcome = state.reveal_row(&mut commands, index, row);
}

/// Mirror the official viewport's scroll offset into the typed
/// [`VirtualListState`], so the acceptance model and the rendered list can
/// never disagree about where the list is.
///
/// The viewport owns the offset; this is the model's read-back of it, never a
/// second offset of the page's own.
pub fn sync_acceptance_scroll_offset(
    viewports: Query<&ScrollPosition, With<AcceptanceRowsContainer>>,
    state: Option<ResMut<AcceptanceListState>>,
) {
    let Some(mut state) = state else {
        return;
    };
    let Some(position) = viewports.iter().next() else {
        return;
    };
    let outcome = state.list.set_scroll_offset(position.0.y.max(0.0));
    if matches!(
        outcome,
        ScrollOffsetOutcome::Changed { .. } | ScrollOffsetOutcome::Clamped { .. }
    ) {
        state.last_scroll_offset_outcome = Some(outcome);
    }
}

pub struct AcceptanceListPlugin;

impl Plugin for AcceptanceListPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AcceptanceListState>();
        // The official scroll-into-view observer is the mechanism the page's
        // reveal affordance uses, so the page owns it when it is mounted
        // without the shared widget layer.
        if !app.is_plugin_added::<ScrollAreaPlugin>() {
            app.add_plugins(ScrollAreaPlugin);
        }
        app.add_observer(on_acceptance_scroll_activated);
        app.add_systems(Update, sync_acceptance_scroll_offset);
    }
}
