//! Native independent inspection replays shared facts without selecting a node.
use crate::command_execution::drain_command_results;
use crate::pages::proxies::NodeDetailButton;
use crate::pages::proxy_probe::{
    PendingProxyProbe, ProbeInspectedProxy, ProxyProbeCaption, ProxyProbeStatus, finish_probe,
    probe, sync_probe_controls,
};
use crate::route::{ActiveRoute, Route};
use crate::surface::LatestSurfaceSnapshot;
use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Or, QueryData, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::scene::{Scene, SceneList, bsn, bsn_list};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BorderRadius, Display, FlexDirection, GlobalZIndex, JustifyContent, Node, Overflow,
    PositionType, UiRect, percent, px,
};
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::chart::bezier::ScaleMode;
use infiltrator_bevy_widgets::chart::{ChartPlate, chart_scene_with_smooth};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::outbound::details::OutboundEgressSample;
use lurker_core_contract::outbound::details::OutboundProtocolObservation;
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::outbound::inventory::OutboundInventoryState;
use lurker_core_contract::outbound::{
    details::{
        OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState, OutboundHealthObservation,
        OutboundObservation, OutboundRttStatistics, OutboundUdpSupport,
    },
    identity::SnapshotRevision,
};
use lurker_core_contract::page::{PageData, PageStatus};

/// Presentation rows projected from the engine-neutral outbound details snapshot.
/// This belongs to the page because it defines labels, not backend data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProxyDetailField {
    #[default]
    Protocol,
    Server,
    Port,
    Cipher,
    Udp,
    Health,
    Delay,
    RttMin,
    RttMax,
    RttAvg,
    Egress,
    EgressTime,
}

impl ProxyDetailField {
    pub const ALL: [Self; 12] = [
        Self::Protocol,
        Self::Server,
        Self::Port,
        Self::Cipher,
        Self::Udp,
        Self::Health,
        Self::Delay,
        Self::RttMin,
        Self::RttMax,
        Self::RttAvg,
        Self::Egress,
        Self::EgressTime,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Self::Protocol => "proxy_inspection_protocol",
            Self::Server => "modal_server_addr",
            Self::Port => "modal_port",
            Self::Cipher => "modal_cipher",
            Self::Udp => "modal_udp",
            Self::Health => "proxy_inspection_health",
            Self::Delay => "proxy_inspection_delay",
            Self::RttMin => "proxy_inspection_rtt_min",
            Self::RttMax => "proxy_inspection_rtt_max",
            Self::RttAvg => "proxy_inspection_rtt_avg",
            Self::Egress => "proxy_inspection_egress",
            Self::EgressTime => "proxy_inspection_egress_time",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ProxyProbePhase {
    #[default]
    Idle,
    Pending(PendingProxyProbe),
    Failed(Failure),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum InspectionScope {
    #[default]
    NoActiveCore,
    Active(CoreScope),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ProxyInspectionReadPhase {
    #[default]
    Loading,
    Ready,
    Failed(Failure),
    Unsupported(Failure),
}

/// Bevy-local read retention for the typed engine-neutral details page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyInspectionReadState {
    pub phase: ProxyInspectionReadPhase,
    pub detail_revision: Option<SnapshotRevision>,
    pub detail: Option<OutboundDetail>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProxyInspectionObservation {
    Retained,
    Removed,
    ScopeChanged,
}

impl ProxyInspectionReadState {
    /// Keep the last detail through read failures; only a successful complete
    /// typed details observation can prove that an outbound was removed.
    pub fn observe(
        &mut self,
        selected: &OutboundId,
        observed_scope: &mut InspectionScope,
        scope: Option<&CoreScope>,
        page: &PageData<OutboundDetailsSnapshot>,
    ) -> ProxyInspectionObservation {
        let next_scope = scope
            .cloned()
            .map_or(InspectionScope::NoActiveCore, InspectionScope::Active);
        if *observed_scope != next_scope {
            let changed_active_scope = matches!(&*observed_scope, InspectionScope::Active(_));
            *observed_scope = next_scope;
            self.detail = None;
            self.detail_revision = None;
            self.phase = ProxyInspectionReadPhase::Loading;
            return if changed_active_scope {
                ProxyInspectionObservation::ScopeChanged
            } else {
                ProxyInspectionObservation::Retained
            };
        }
        let InspectionScope::Active(scope) = &*observed_scope else {
            self.phase = ProxyInspectionReadPhase::Failed(Failure::new(
                ErrorCode::NotReady,
                "active core scope is not observed",
                true,
            ));
            return ProxyInspectionObservation::Retained;
        };
        match &page.status {
            PageStatus::Ready | PageStatus::Empty => {
                let Some(snapshot) = page.data.as_ref() else {
                    self.phase = ProxyInspectionReadPhase::Failed(Failure::new(
                        ErrorCode::Internal,
                        "successful outbound details page has no observation data",
                        true,
                    ));
                    return ProxyInspectionObservation::Retained;
                };
                if snapshot.scope() != scope {
                    self.phase = ProxyInspectionReadPhase::Failed(Failure::new(
                        ErrorCode::NotReady,
                        "outbound details belong to a different active core scope",
                        true,
                    ));
                    return ProxyInspectionObservation::Retained;
                }
                match snapshot.state() {
                    OutboundDetailsState::Ready { .. } => {
                        if let Some(detail) = snapshot.detail(selected) {
                            self.detail = Some(detail.clone());
                            self.detail_revision = Some(snapshot.detail_revision());
                            self.phase = ProxyInspectionReadPhase::Ready;
                            ProxyInspectionObservation::Retained
                        } else {
                            self.detail = None;
                            self.detail_revision = Some(snapshot.detail_revision());
                            self.phase = ProxyInspectionReadPhase::Ready;
                            ProxyInspectionObservation::Removed
                        }
                    }
                    OutboundDetailsState::Empty => {
                        self.detail = None;
                        self.detail_revision = Some(snapshot.detail_revision());
                        self.phase = ProxyInspectionReadPhase::Ready;
                        ProxyInspectionObservation::Removed
                    }
                    OutboundDetailsState::Unknown => {
                        self.phase = ProxyInspectionReadPhase::Loading;
                        ProxyInspectionObservation::Retained
                    }
                    OutboundDetailsState::Unsupported => {
                        self.phase = ProxyInspectionReadPhase::Unsupported(Failure::new(
                            ErrorCode::Unsupported,
                            "outbound detail observations are unsupported",
                            false,
                        ));
                        ProxyInspectionObservation::Retained
                    }
                    OutboundDetailsState::Failed { failure } => {
                        self.phase = ProxyInspectionReadPhase::Failed(failure.clone());
                        ProxyInspectionObservation::Retained
                    }
                }
            }
            PageStatus::Loading => {
                self.phase = ProxyInspectionReadPhase::Loading;
                ProxyInspectionObservation::Retained
            }
            PageStatus::Failed { failure } | PageStatus::Unavailable { failure } => {
                self.phase = ProxyInspectionReadPhase::Failed(failure.clone());
                ProxyInspectionObservation::Retained
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyInspectionSession {
    pub selected: OutboundId,
    pub scope: InspectionScope,
    pub probe: ProxyProbePhase,
    pub read: ProxyInspectionReadState,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ProxyInspectionPhase {
    #[default]
    Closed,
    Open(ProxyInspectionSession),
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyInspectionState {
    pub phase: ProxyInspectionPhase,
}
impl ProxyInspectionState {
    fn dismiss(&mut self) {
        self.phase = ProxyInspectionPhase::Closed;
    }
}
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProxyInspectionRoot;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProxyInspectionCard;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CloseProxyInspection;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProxyInspectionTitle;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProxyDetailValue(pub ProxyDetailField);
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProxyHistoryText;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProxyInspectionChart;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProxyInspectionScrollArea;

pub struct ProxyInspectionPlugin;
impl Plugin for ProxyInspectionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProxyInspectionState>();
        app.add_observer(activate_inspection);
        app.add_observer(probe);
        app.add_observer(finish_probe);
        app.add_systems(
            Update,
            (dismiss_inspection, sync_inspection, sync_probe_controls)
                .chain()
                .after(drain_command_results),
        );
    }
}
fn activate_inspection(
    activated: On<Activate>,
    details: Query<&NodeDetailButton>,
    close: Query<&CloseProxyInspection>,
    latest: Res<LatestSurfaceSnapshot>,
    mut state: ResMut<ProxyInspectionState>,
) {
    if close.contains(activated.entity) {
        state.dismiss();
        return;
    }
    let Ok(button) = details.get(activated.entity) else {
        return;
    };
    let Some(outbound_id) = button.outbound_id.as_ref() else {
        return;
    };
    if matches!(
        &state.phase,
        ProxyInspectionPhase::Open(session) if &session.selected == outbound_id
    ) {
        return;
    }
    let active_scope = latest.0.core.scope();
    let mut session = ProxyInspectionSession {
        selected: outbound_id.clone(),
        scope: active_scope
            .clone()
            .map_or(InspectionScope::NoActiveCore, InspectionScope::Active),
        probe: ProxyProbePhase::Idle,
        read: ProxyInspectionReadState::default(),
    };
    session.read.observe(
        &session.selected,
        &mut session.scope,
        active_scope.as_ref(),
        &latest.0.outbound_details,
    );
    state.phase = ProxyInspectionPhase::Open(session);
}
fn dismiss_inspection(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    route: Res<ActiveRoute>,
    latest: Res<LatestSurfaceSnapshot>,
    mut state: ResMut<ProxyInspectionState>,
) {
    if route.0 != Some(Route::Proxies)
        || keys.is_some_and(|keys| keys.just_pressed(KeyCode::Escape))
    {
        state.dismiss();
        return;
    }
    let outcome = match &mut state.phase {
        ProxyInspectionPhase::Closed => return,
        ProxyInspectionPhase::Open(session) => session.read.observe(
            &session.selected,
            &mut session.scope,
            latest.0.core.scope().as_ref(),
            &latest.0.outbound_details,
        ),
    };
    if matches!(
        outcome,
        ProxyInspectionObservation::Removed | ProxyInspectionObservation::ScopeChanged
    ) {
        state.dismiss();
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct DetailTexts {
    text: &'static mut Text,
    title: Option<&'static ProxyInspectionTitle>,
    value: Option<&'static ProxyDetailValue>,
    history: Option<&'static ProxyHistoryText>,
}
type DetailTextFilter = Or<(
    With<ProxyInspectionTitle>,
    With<ProxyDetailValue>,
    With<ProxyHistoryText>,
)>;
fn sync_inspection(
    state: Res<ProxyInspectionState>,
    latest: Res<LatestSurfaceSnapshot>,
    locale: Res<UiLocale>,
    mut roots: Query<&mut Node, With<ProxyInspectionRoot>>,
    mut texts: Query<DetailTexts, DetailTextFilter>,
    mut charts: Query<&mut ChartPlate, With<ProxyInspectionChart>>,
) {
    let (selected, detail) = match &state.phase {
        ProxyInspectionPhase::Closed => (None, None),
        ProxyInspectionPhase::Open(session) => {
            (Some(&session.selected), session.read.detail.as_ref())
        }
    };
    for mut root in &mut roots {
        let display = if selected.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        if root.display != display {
            root.display = display;
        }
    }
    let tr = |key: &str| Lang(locale.code()).tr(key).into_owned();
    for mut row in &mut texts {
        let value = if row.title.is_some() {
            selected
                .as_ref()
                .and_then(|selected| {
                    latest
                        .0
                        .outbound_inventory
                        .data
                        .as_ref()
                        .and_then(|inventory| match inventory.inventory() {
                            OutboundInventoryState::Ready { entries, .. } => entries
                                .iter()
                                .find(|entry| &entry.id == *selected)
                                .and_then(|entry| entry.display_name.clone()),
                            _ => None,
                        })
                })
                .unwrap_or_else(|| selected.map(|id| id.value().to_owned()).unwrap_or_default())
        } else if let Some(field) = row.value {
            field_value(detail, field.0, &tr)
        } else if row.history.is_some() {
            history_listing(detail, &tr)
        } else {
            continue;
        };
        if row.text.0 != value {
            row.text.0 = value;
        }
    }
    let plot = history_plot(detail);
    for mut chart in &mut charts {
        let equal = chart.0.up.len() == plot.samples.len()
            && chart
                .0
                .up
                .iter()
                .zip(&plot.samples)
                .all(|(left, right)| left == right || (left.is_nan() && right.is_nan()));
        if !equal {
            chart.0.up = plot.samples.clone();
        }
        let scale = ScaleMode::Fixed(plot.ceiling_ms);
        if chart.0.scale_mode != scale {
            chart.0.scale_mode = scale;
        }
    }
}

struct ProxyHistoryPlot {
    samples: Vec<f32>,
    ceiling_ms: f32,
}

fn history_plot(detail: Option<&OutboundDetail>) -> ProxyHistoryPlot {
    let samples: Vec<_> = detail
        .and_then(|detail| match &detail.latency_history {
            OutboundObservation::Observed(samples) => Some(samples.as_slice()),
            OutboundObservation::Unknown | OutboundObservation::Unsupported => None,
        })
        .unwrap_or_default()
        .iter()
        .map(|sample| {
            if sample.delay_ms == 0 {
                f32::NAN
            } else {
                sample.delay_ms as f32
            }
        })
        .collect();
    let ceiling_ms = samples
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .fold(1.0, f32::max);
    ProxyHistoryPlot {
        samples,
        ceiling_ms,
    }
}

fn field_value(
    detail: Option<&OutboundDetail>,
    field: ProxyDetailField,
    tr: &impl Fn(&str) -> String,
) -> String {
    let unknown = || tr("proxy_inspection_unknown");
    let millis = |value: Option<u32>| {
        value
            .map(|value| format!("{value} ms"))
            .unwrap_or_else(unknown)
    };
    let Some(detail) = detail else {
        return match field {
            ProxyDetailField::Delay => tr("proxy_inspection_unmeasured"),
            _ => unknown(),
        };
    };
    match field {
        ProxyDetailField::Protocol => match &detail.protocol {
            OutboundProtocolObservation::Reported(protocol) => detail
                .protocol_display_name
                .clone()
                .unwrap_or_else(|| protocol.value().to_owned()),
            OutboundProtocolObservation::Unknown | OutboundProtocolObservation::Unsupported => {
                unknown()
            }
        },
        ProxyDetailField::Server => match &detail.endpoint {
            OutboundObservation::Observed(endpoint) => endpoint.host.clone(),
            OutboundObservation::Unknown | OutboundObservation::Unsupported => unknown(),
        },
        ProxyDetailField::Port => match &detail.endpoint {
            OutboundObservation::Observed(endpoint) => match endpoint.port {
                OutboundObservation::Observed(port) => port.to_string(),
                OutboundObservation::Unknown | OutboundObservation::Unsupported => unknown(),
            },
            OutboundObservation::Unknown | OutboundObservation::Unsupported => unknown(),
        },
        ProxyDetailField::Cipher => match &detail.cipher {
            OutboundObservation::Observed(cipher) => detail
                .cipher_display_name
                .clone()
                .unwrap_or_else(|| cipher.value().to_owned()),
            OutboundObservation::Unknown | OutboundObservation::Unsupported => unknown(),
        },
        ProxyDetailField::Udp => match detail.udp {
            OutboundUdpSupport::Supported => tr("proxy_inspection_supported"),
            OutboundUdpSupport::Unsupported => tr("proxy_inspection_disabled"),
            OutboundUdpSupport::Unknown => unknown(),
        },
        ProxyDetailField::Health => match detail.health {
            OutboundHealthObservation::Healthy => tr("proxy_inspection_alive"),
            OutboundHealthObservation::Unhealthy => tr("proxy_inspection_dead"),
            OutboundHealthObservation::Unknown | OutboundHealthObservation::Unsupported => {
                unknown()
            }
        },
        ProxyDetailField::Delay => match detail.current_latency_ms {
            OutboundObservation::Observed(0) => tr("proxy_inspection_unconfirmed_zero"),
            OutboundObservation::Observed(ms) => format!("{ms} ms"),
            OutboundObservation::Unknown => tr("proxy_inspection_unmeasured"),
            OutboundObservation::Unsupported => unknown(),
        },
        ProxyDetailField::RttMin => millis(rtt(detail).and_then(|rtt| rtt.min_ms)),
        ProxyDetailField::RttMax => millis(rtt(detail).and_then(|rtt| rtt.max_ms)),
        ProxyDetailField::RttAvg => millis(rtt(detail).and_then(|rtt| rtt.avg_ms)),
        ProxyDetailField::Egress => latest_egress(detail)
            .map(|sample| match &sample.country {
                Some(country) => format!("{} ({country})", sample.ip),
                None => sample.ip.clone(),
            })
            .unwrap_or_else(unknown),
        ProxyDetailField::EgressTime => latest_egress(detail)
            .and_then(|sample| sample.observed_at_epoch_ms)
            .map(|epoch| format!("{epoch} ms"))
            .unwrap_or_else(unknown),
    }
}

fn rtt(detail: &OutboundDetail) -> Option<&OutboundRttStatistics> {
    match &detail.rtt {
        OutboundObservation::Observed(stats) => Some(stats),
        OutboundObservation::Unknown | OutboundObservation::Unsupported => None,
    }
}

fn latest_egress(detail: &OutboundDetail) -> Option<&OutboundEgressSample> {
    match &detail.historical_egress {
        OutboundObservation::Observed(samples) => samples.last(),
        OutboundObservation::Unknown | OutboundObservation::Unsupported => None,
    }
}

fn history_listing(detail: Option<&OutboundDetail>, tr: &impl Fn(&str) -> String) -> String {
    let Some(samples) = detail.and_then(|detail| match &detail.latency_history {
        OutboundObservation::Observed(samples) => Some(samples.as_slice()),
        OutboundObservation::Unknown | OutboundObservation::Unsupported => None,
    }) else {
        return tr("proxy_inspection_history_empty");
    };
    if samples.is_empty() {
        return tr("proxy_inspection_history_empty");
    }
    let mut lines = vec![format!(
        "{}: {}/{}",
        tr("proxy_inspection_history"),
        samples.len(),
        samples.len()
    )];
    for sample in samples {
        let time = if sample.observed_at.is_empty() {
            tr("proxy_inspection_unknown")
        } else {
            sample.observed_at.clone()
        };
        let delay = if sample.delay_ms == 0 {
            tr("proxy_inspection_unconfirmed_zero")
        } else {
            format!("{} ms", sample.delay_ms)
        };
        lines.push(format!("{time} · {delay}"));
    }
    lines.join("\n")
}

/// The detail field rows as one scene list: a repeated group, so it is
/// authored as a [`SceneList`] instead of an inline `Children [...]` enumeration.
fn inspection_field_scenes() -> impl SceneList + use<> {
    let fields: Vec<Box<dyn Scene>> = ProxyDetailField::ALL
        .into_iter()
        .map(|field| {
            Box::new(bsn! {
                Node { width:percent(100), column_gap:rem(space::S12), align_items:AlignItems::Start }
                Children [
                    Node { width:percent(35), min_width:px(0.0) }
                    Children [ LocalizedText::plain(field.key()) TextRole(Role::Caption) ]
                    --
                    Node { width:percent(65), min_width:px(0.0) }
                    Children [ Text::new("") ProxyDetailValue(field) TextRole(Role::Body) ]
                ]
            }) as Box<dyn Scene>
        })
        .collect();
    bsn_list! { { fields } }
}

pub fn inspection_scene(palette: &UiPalette) -> impl Scene + use<> {
    let semantic = accesskit::Node::new(accesskit::Role::Dialog);
    bsn! {
        Node { position_type:PositionType::Absolute, left:px(0.0), top:px(0.0), width:percent(100), height:percent(100), align_items:AlignItems::Center, justify_content:JustifyContent::Center, display:Display::None }
        ProxyInspectionRoot GlobalZIndex(95)
        Children [
            Node { position_type:PositionType::Absolute, width:percent(100),height:percent(100) }
            ThemeBackgroundColor({Token::Scrim}) ModalScrim Button CloseProxyInspection
            --
            Node { width:percent(92),max_width:rem(650.0),max_height:percent(90), min_width:px(0.0), flex_direction:FlexDirection::Column, row_gap:rem(space::S12),padding:UiRect::all(rem(space::S16)),border_radius:BorderRadius::all(rem(palette.card_radius_px)) }
            ThemeBackgroundColor({Token::Surface}) ProxyInspectionCard ModalDialogCard
            AccessibilityNode(semantic) LocalizedLabel::plain("proxy_inspection_title")
            Children [
                LocalizedText::plain("proxy_inspection_title") TextRole(Role::Heading)
                --
                Text::new("") ProxyInspectionTitle TextRole(Role::BodyStrong)
                --
                LocalizedText::new("common_message",vec![("message",String::new())]) ProxyProbeStatus TextRole(Role::Caption)
                --
                Node { width:percent(100),min_width:px(0.0),min_height:px(0.0),flex_shrink:1.0,flex_direction:FlexDirection::Column,row_gap:rem(space::S12),overflow:Overflow::scroll_y() }
                ScrollArea ProxyInspectionScrollArea
                Children [
                    LocalizedText::plain("proxy_inspection_history") TextRole(Role::BodyStrong)
                    --
                    @{ chart_scene_with_smooth(Vec::new(),Vec::new(),560.0,90.0,false) } ProxyInspectionChart
                    --
                    { inspection_field_scenes() }
                    --
                    LocalizedText::plain("proxy_inspection_timing_unavailable") TextRole(Role::Caption)
                    --
                    Text::new("") ProxyHistoryText TextRole(Role::Body)
                ]
                --
                Node { width:percent(100),column_gap:rem(space::S12),align_items:AlignItems::Center,flex_shrink:0.0 }
                Children [
                    Node { flex_grow:1.0,min_width:px(0.0),min_height:rem(palette.control_height_px),padding:UiRect::axes(rem(space::S12),rem(space::S8)),border_radius:BorderRadius::all(rem(palette.control_radius_px)) }
                    ThemeBackgroundColor({Token::SurfaceElevated}) Button CloseProxyInspection
                    Children [ LocalizedText::plain("modal_close") TextRole(Role::BodyStrong) ]
                    --
                    Node { flex_grow:1.0,min_width:px(0.0),min_height:rem(palette.control_height_px),padding:UiRect::axes(rem(space::S12),rem(space::S8)),border_radius:BorderRadius::all(rem(palette.control_radius_px)) }
                    ThemeBackgroundColor({Token::AccentContainer}) Button ButtonDisabled(true) ProbeInspectedProxy
                    Children [ LocalizedText::plain("modal_speed_test_now") ProxyProbeCaption TextRole(Role::BodyStrong) ]
                ]
            ]
        ]
    }
}
