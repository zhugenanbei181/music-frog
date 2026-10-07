//! Scoped native component access for dns systems.

use super::{
    DnsEnhancedModePill, DnsFilterModePill, DnsLine, DnsSwitchButton, DnsSwitchKnob,
    DnsSwitchTrack, TestDnsLatencyButton, TestStunProbeButton,
};
use crate::pages::dns_servers::{DnsServerAddress, DnsServerLatency, DnsServerProto};
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::system::{Query, SystemParam};
use bevy::text::TextColor;
use bevy::ui::BorderColor;
use bevy::ui::prelude::{BackgroundColor, Node};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::localization::LocalizedText;

#[derive(SystemParam)]
pub struct DnsActionControls<'w, 's> {
    pub(super) test_buttons: Query<'w, 's, (), With<TestDnsLatencyButton>>,
    pub(super) stun_buttons: Query<'w, 's, (), With<TestStunProbeButton>>,
    pub(super) switch_buttons: Query<'w, 's, &'static DnsSwitchButton>,
    pub(super) enhanced_pills: Query<'w, 's, &'static DnsEnhancedModePill>,
    pub(super) filter_pills: Query<'w, 's, &'static DnsFilterModePill>,
}

#[derive(QueryFilter)]
pub struct ApplyDnsProjectionLinesFilter {
    with_dns_line: With<DnsLine>,
    without_dns_server_address: Without<DnsServerAddress>,
    without_dns_server_proto: Without<DnsServerProto>,
    without_dns_server_latency: Without<DnsServerLatency>,
}

#[derive(QueryFilter)]
pub struct ApplyDnsProjectionAddressesFilter {
    with_dns_server_address: With<DnsServerAddress>,
    without_dns_line: Without<DnsLine>,
    without_dns_server_proto: Without<DnsServerProto>,
    without_dns_server_latency: Without<DnsServerLatency>,
}

#[derive(QueryFilter)]
pub struct ApplyDnsProjectionProtocolsFilter {
    with_dns_server_proto: With<DnsServerProto>,
    without_dns_line: Without<DnsLine>,
    without_dns_server_address: Without<DnsServerAddress>,
    without_dns_server_latency: Without<DnsServerLatency>,
}

#[derive(QueryFilter)]
pub struct ApplyDnsProjectionLatenciesFilter {
    with_dns_server_latency: With<DnsServerLatency>,
    without_dns_line: Without<DnsLine>,
    without_dns_server_address: Without<DnsServerAddress>,
    without_dns_server_proto: Without<DnsServerProto>,
}

#[derive(QueryFilter)]
pub struct ApplyDnsProjectionSwitchTracksFilter {
    with_dns_switch_track: With<DnsSwitchTrack>,
    without_dns_switch_knob: Without<DnsSwitchKnob>,
    without_dns_enhanced_mode_pill: Without<DnsEnhancedModePill>,
    without_dns_filter_mode_pill: Without<DnsFilterModePill>,
}

#[derive(QueryFilter)]
pub struct ApplyDnsProjectionSwitchKnobsFilter {
    with_dns_switch_knob: With<DnsSwitchKnob>,
    without_dns_switch_track: Without<DnsSwitchTrack>,
    without_dns_enhanced_mode_pill: Without<DnsEnhancedModePill>,
    without_dns_filter_mode_pill: Without<DnsFilterModePill>,
}

#[derive(QueryFilter)]
pub struct ApplyDnsProjectionEnhancedPillsFilter {
    with_dns_enhanced_mode_pill: With<DnsEnhancedModePill>,
    without_dns_switch_track: Without<DnsSwitchTrack>,
    without_dns_switch_knob: Without<DnsSwitchKnob>,
    without_dns_filter_mode_pill: Without<DnsFilterModePill>,
}

#[derive(QueryFilter)]
pub struct ApplyDnsProjectionFilterPillsFilter {
    with_dns_filter_mode_pill: With<DnsFilterModePill>,
    without_dns_switch_track: Without<DnsSwitchTrack>,
    without_dns_switch_knob: Without<DnsSwitchKnob>,
    without_dns_enhanced_mode_pill: Without<DnsEnhancedModePill>,
}

#[derive(SystemParam)]
pub struct DnsProjectionTargets<'w, 's> {
    pub(super) lines: Query<
        'w,
        's,
        (&'static mut Text, &'static mut TextColor, &'static DnsLine),
        ApplyDnsProjectionLinesFilter,
    >,
    pub(super) addresses: Query<
        'w,
        's,
        (&'static mut Text, &'static DnsServerAddress),
        ApplyDnsProjectionAddressesFilter,
    >,
    pub(super) protocols: Query<
        'w,
        's,
        (&'static mut Text, &'static DnsServerProto),
        ApplyDnsProjectionProtocolsFilter,
    >,
    pub(super) latencies: Query<
        'w,
        's,
        (
            &'static mut Text,
            &'static mut TextColor,
            &'static DnsServerLatency,
            &'static mut LocalizedText,
        ),
        ApplyDnsProjectionLatenciesFilter,
    >,
    pub(super) switch_tracks: Query<
        'w,
        's,
        (
            &'static mut BackgroundColor,
            &'static mut BorderColor,
            &'static mut DnsSwitchButton,
            &'static DnsSwitchTrack,
        ),
        ApplyDnsProjectionSwitchTracksFilter,
    >,
    pub(super) switch_knobs: Query<
        'w,
        's,
        (
            &'static mut BackgroundColor,
            &'static mut Node,
            &'static DnsSwitchKnob,
        ),
        ApplyDnsProjectionSwitchKnobsFilter,
    >,
    pub(super) enhanced_pills: Query<
        'w,
        's,
        (&'static mut BackgroundColor, &'static DnsEnhancedModePill),
        ApplyDnsProjectionEnhancedPillsFilter,
    >,
    pub(super) filter_pills: Query<
        'w,
        's,
        (&'static mut BackgroundColor, &'static DnsFilterModePill),
        ApplyDnsProjectionFilterPillsFilter,
    >,
}
