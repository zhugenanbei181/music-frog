//! Scoped native component access for connections drawer systems.

use super::{
    ConnDrawerField, ConnDrawerHopSlot, ConnDrawerHopText, ConnDrawerRuleDraft,
    DrawerAddRuleButton, DrawerCloseConnectionButton,
};
use crate::pages::connections::ConnInspectButton;
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::system::{Query, SystemParam};
use bevy::ui::prelude::Node;
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::drawer::DrawerCloseButton;

#[derive(QueryFilter)]
pub struct OnConnectionsDrawerActivatedFieldsFilter {
    without_conn_drawer_hop_text: Without<ConnDrawerHopText>,
    without_conn_drawer_rule_draft: Without<ConnDrawerRuleDraft>,
}

#[derive(QueryFilter)]
pub struct OnConnectionsDrawerActivatedHopsFilter {
    with_conn_drawer_hop_text: With<ConnDrawerHopText>,
    without_conn_drawer_field: Without<ConnDrawerField>,
    without_conn_drawer_rule_draft: Without<ConnDrawerRuleDraft>,
}

#[derive(QueryFilter)]
pub struct OnConnectionsDrawerActivatedHopSlotsFilter {
    with_conn_drawer_hop_slot: With<ConnDrawerHopSlot>,
    without_conn_drawer_field: Without<ConnDrawerField>,
    without_conn_drawer_rule_draft: Without<ConnDrawerRuleDraft>,
}

#[derive(QueryFilter)]
pub struct OnConnectionsDrawerActivatedDraftLinesFilter {
    with_conn_drawer_rule_draft: With<ConnDrawerRuleDraft>,
    without_conn_drawer_field: Without<ConnDrawerField>,
    without_conn_drawer_hop_text: Without<ConnDrawerHopText>,
}

#[derive(SystemParam)]
pub struct ConnectionDrawerControls<'w, 's> {
    pub(super) inspect_buttons: Query<'w, 's, &'static ConnInspectButton>,
    pub(super) add_rule_buttons: Query<'w, 's, (), With<DrawerAddRuleButton>>,
    pub(super) close_connection_buttons: Query<'w, 's, (), With<DrawerCloseConnectionButton>>,
    pub(super) close_buttons: Query<'w, 's, (), With<DrawerCloseButton>>,
    pub(super) fields: Query<
        'w,
        's,
        (&'static mut Text, &'static ConnDrawerField),
        OnConnectionsDrawerActivatedFieldsFilter,
    >,
    pub(super) hops: Query<
        'w,
        's,
        (&'static mut Text, &'static ConnDrawerHopText),
        OnConnectionsDrawerActivatedHopsFilter,
    >,
    pub(super) hop_slots: Query<
        'w,
        's,
        (&'static mut Node, &'static ConnDrawerHopSlot),
        OnConnectionsDrawerActivatedHopSlotsFilter,
    >,
    pub(super) draft_lines:
        Query<'w, 's, &'static mut Text, OnConnectionsDrawerActivatedDraftLinesFilter>,
}
