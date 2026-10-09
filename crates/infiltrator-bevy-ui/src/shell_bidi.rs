//! Shell-level bidirectional layout direction (BEVY-035).
//!
//! The widget layer owns the mirroring math ([`LayoutDirection`] and its
//! `mirror_*` transforms); this module owns the product wiring: the direction
//! resource plus a system that mirrors the shell root's rail side and row
//! alignment in place for RTL. LTR resolves every mirror to its base value, so
//! the left-to-right shell is byte-for-byte unaffected.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Query, Res};
use bevy::ui::prelude::{AlignItems, FlexDirection, JustifyContent, Node};
use infiltrator_bevy_widgets::bidi::LayoutDirection;

/// Marker on the shell body row: the flex row that places the rail beside the
/// content column. Its flex direction mirrors to `RowReverse` for RTL.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShellDirectionRoot;

/// Marker on a shell row whose inline-axis alignment mirrors for RTL.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShellDirectionRow;

/// The LTR base the body row mirrors from (rail first, content second).
const BASE_ROOT_FLEX: FlexDirection = FlexDirection::Row;
/// The LTR base main-axis distribution of a direction row.
const BASE_ROW_JUSTIFY: JustifyContent = JustifyContent::FlexStart;
/// The LTR base cross-axis alignment of a direction row.
const BASE_ROW_ALIGN: AlignItems = AlignItems::Center;

/// Mirror the shell root's rail side and row alignment for the live direction.
pub fn sync_shell_direction(
    direction: Res<LayoutDirection>,
    mut roots: Query<&mut Node, With<ShellDirectionRoot>>,
    mut rows: Query<&mut Node, (With<ShellDirectionRow>, Without<ShellDirectionRoot>)>,
) {
    let root_flex = direction.mirror_flex_direction(BASE_ROOT_FLEX);
    for mut node in &mut roots {
        if node.flex_direction != root_flex {
            node.flex_direction = root_flex;
        }
    }

    let row_justify = direction.mirror_justify_content(BASE_ROW_JUSTIFY);
    let row_align = direction.mirror_align_items(BASE_ROW_ALIGN);
    for mut node in &mut rows {
        if node.justify_content != row_justify {
            node.justify_content = row_justify;
        }
        if node.align_items != row_align {
            node.align_items = row_align;
        }
    }
}

/// Install the shared direction resource and the shell mirroring system.
pub struct ShellBidiPlugin;

impl Plugin for ShellBidiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LayoutDirection>();
        app.add_systems(Update, sync_shell_direction);
    }
}
