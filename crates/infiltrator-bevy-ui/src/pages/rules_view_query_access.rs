//! Scoped native component access for rules view systems.

use super::{RulesListScrollArea, RulesPageIndicator, RulesWindowRows};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, SystemParam};
use bevy::ui::prelude::{ComputedNode, ScrollPosition};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::localization::LocalizedText;

#[derive(SystemParam)]
pub struct RuleWindowTargets<'w, 's> {
    pub(super) scroll_areas: Query<
        'w,
        's,
        (&'static mut ScrollPosition, Option<&'static ComputedNode>),
        With<RulesListScrollArea>,
    >,
    pub(super) containers: Query<'w, 's, Entity, With<RulesWindowRows>>,
    pub(super) indicator:
        Query<'w, 's, (&'static mut Text, &'static mut LocalizedText), With<RulesPageIndicator>>,
}
