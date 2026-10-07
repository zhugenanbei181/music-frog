//! Scoped native component access for profiles import systems.

use super::{SaveUserAgentButton, SubscriptionInsecureToggle, SubscriptionUserAgentField};
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, SystemParam};
use bevy::ui::Checked;
use infiltrator_bevy_widgets::text_input::TextField;

#[derive(SystemParam)]
pub struct SubscriptionFetchControls<'w, 's> {
    pub(super) buttons: Query<'w, 's, (), With<SaveUserAgentButton>>,
    pub(super) fields: Query<'w, 's, &'static Children, With<SubscriptionUserAgentField>>,
    pub(super) text_fields: Query<'w, 's, &'static TextField>,
    pub(super) toggles: Query<'w, 's, &'static Children, With<SubscriptionInsecureToggle>>,
    pub(super) checkboxes: Query<'w, 's, &'static Checked>,
}
