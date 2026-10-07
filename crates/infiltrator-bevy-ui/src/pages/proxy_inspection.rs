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
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::scene::{Scene, bsn};
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BackgroundColor, BorderRadius, Display, FlexDirection, GlobalZIndex,
    JustifyContent, Node, Overflow, PositionType, UiRect, percent, px,
};
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_application::proxy_inspection_projection::{
    field_value, history_listing, history_plot, lookup_inspection,
};
use infiltrator_application::proxy_inspection_reader::ProxyInspectionReadState;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::chart::bezier::ScaleMode;
use infiltrator_bevy_widgets::chart::{ChartPlate, chart_scene_with_smooth};
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_contract::error::Failure;
use infiltrator_contract::proxy_inspection::ProxyDetailField;
use infiltrator_shared::locales::{Lang, Localizer};

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyInspectionState {
    pub selected: Option<String>,
    pub pending: Option<PendingProxyProbe>,
    pub failure: Option<Failure>,
    pub read: ProxyInspectionReadState,
}
impl ProxyInspectionState {
    fn dismiss(&mut self) {
        self.selected = None;
        self.pending = None;
        self.failure = None;
        self.read = Default::default();
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
    if latest
        .0
        .pages
        .proxies
        .data
        .as_ref()
        .and_then(|page| lookup_inspection(page, &button.node_name))
        .is_some()
        && state.selected.as_deref() != Some(button.node_name.as_str())
    {
        state.dismiss();
        state.selected = Some(button.node_name.clone());
        let selected = state.selected.clone();
        state
            .read
            .observe(selected.as_deref(), &latest.0.pages.proxies);
    }
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
    let selected = state.selected.clone();
    if state
        .read
        .observe(selected.as_deref(), &latest.0.pages.proxies)
    {
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
    locale: Res<UiLocale>,
    mut roots: Query<&mut Node, With<ProxyInspectionRoot>>,
    mut texts: Query<DetailTexts, DetailTextFilter>,
    mut charts: Query<&mut ChartPlate, With<ProxyInspectionChart>>,
) {
    let detail = state.read.detail.as_ref();
    for mut root in &mut roots {
        let display = if state.selected.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        if root.display != display {
            root.display = display;
        }
    }
    let Some(detail) = detail else { return };
    let tr = |key: &str| Lang(locale.code()).tr(key).into_owned();
    for mut row in &mut texts {
        let value = if row.title.is_some() {
            detail.name.clone()
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

pub fn inspection_scene(palette: &UiPalette) -> impl Scene + use<> {
    let fields: Vec<Box<dyn Scene>> = ProxyDetailField::ALL
        .into_iter()
        .map(|field| {
            Box::new(bsn! {
                Node { width:percent(100), column_gap:px(12.0), align_items:AlignItems::Start }
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
    let semantic = accesskit::Node::new(accesskit::Role::Dialog);
    bsn! {
        Node { position_type:PositionType::Absolute, left:px(0.0), top:px(0.0), width:percent(100), height:percent(100), align_items:AlignItems::Center, justify_content:JustifyContent::Center, display:Display::None }
        ProxyInspectionRoot GlobalZIndex(95)
        Children [
            Node { position_type:PositionType::Absolute, width:percent(100),height:percent(100) }
            BackgroundColor({palette.scrim}) ModalScrim Button CloseProxyInspection
            --
            Node { width:percent(92),max_width:px(650.0),max_height:percent(90), min_width:px(0.0), flex_direction:FlexDirection::Column, row_gap:px(12.0),padding:UiRect::all(px(16.0)),border_radius:BorderRadius::all(px(palette.card_radius_px)) }
            BackgroundColor({palette.surface}) ProxyInspectionCard ModalDialogCard
            AccessibilityNode(semantic) LocalizedLabel::plain("proxy_inspection_title")
            Children [
                LocalizedText::plain("proxy_inspection_title") TextRole(Role::Heading)
                --
                Text::new("") ProxyInspectionTitle TextRole(Role::BodyStrong)
                --
                LocalizedText::new("common_message",vec![("message",String::new())]) ProxyProbeStatus TextRole(Role::Caption)
                --
                Node { width:percent(100),min_width:px(0.0),min_height:px(0.0),flex_shrink:1.0,flex_direction:FlexDirection::Column,row_gap:px(12.0),overflow:Overflow::scroll_y() }
                ScrollArea ProxyInspectionScrollArea
                Children [
                    LocalizedText::plain("proxy_inspection_history") TextRole(Role::BodyStrong)
                    --
                    @{ chart_scene_with_smooth(Vec::new(),Vec::new(),560.0,90.0,false) } ProxyInspectionChart
                    --
                    {fields}
                    --
                    LocalizedText::plain("proxy_inspection_timing_unavailable") TextRole(Role::Caption)
                    --
                    Text::new("") ProxyHistoryText TextRole(Role::Body)
                ]
                --
                Node { width:percent(100),column_gap:px(12.0),align_items:AlignItems::Center,flex_shrink:0.0 }
                Children [
                    Node { flex_grow:1.0,min_width:px(0.0),min_height:px(palette.control_height_px),padding:UiRect::axes(px(12.0),px(8.0)),border_radius:BorderRadius::all(px(palette.control_radius_px)) }
                    BackgroundColor({palette.surface_elevated}) Button CloseProxyInspection
                    Children [ LocalizedText::plain("modal_close") TextRole(Role::BodyStrong) ]
                    --
                    Node { flex_grow:1.0,min_width:px(0.0),min_height:px(palette.control_height_px),padding:UiRect::axes(px(12.0),px(8.0)),border_radius:BorderRadius::all(px(palette.control_radius_px)) }
                    BackgroundColor({palette.accent_container}) Button ButtonDisabled(true) ProbeInspectedProxy
                    Children [ LocalizedText::plain("modal_speed_test_now") ProxyProbeCaption TextRole(Role::BodyStrong) ]
                ]
            ]
        ]
    }
}
