//! Scenario-specific observation systems expose only their declared native UI facts.
use super::geometry::CaptureGeometry;
use super::{InteractionCapture, ObservedInteraction};
use crate::command_palette::{CommandPaletteCard, CommandPaletteOverlayRoot, CommandPaletteState};
use crate::pages::business_panel::{
    BusinessPanelCard, BusinessPanelRoot, BusinessPanelState, PanelKind,
};
use crate::pages::connections_confirm::{CloseAllConfirmationCard, CloseAllConfirmationRoot};
use crate::pages::connections_drawer::{ConnectionDrawerLayer, ConnectionsDrawerState};
use crate::pages::connections_view::ConnectionsCloseAllState;
use crate::pages::overview::LastOverviewProjection;
use crate::pages::overview_speedtest::OverviewSpeedtestDetailBodyText;
use crate::pages::proxies_custom::CustomNodeText;
use crate::pages::proxies_form::CustomNodeForm;
use crate::pages::proxy_inspection::{
    ProxyInspectionCard, ProxyInspectionChart, ProxyInspectionRoot, ProxyInspectionState,
};
use bevy::ecs::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::ui::widget::Text;
use infiltrator_application::proxy_inspection_fixtures::INSPECTION_NODE;
use infiltrator_application::speedtest_detail_projection::{listing, project_details};
use infiltrator_bevy_widgets::adaptive_modal::{AdaptiveModalRoot, ModalCard, ModalState};
use infiltrator_bevy_widgets::chart::ChartPlate;
use infiltrator_bevy_widgets::drawer::DrawerPanel;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::protocol_form::ProtocolStudioSlot;
use infiltrator_shared::locales::{Lang, Localizer};

#[derive(SystemParam)]
pub struct ObservationMarker<'w> {
    feature: Res<'w, InteractionCapture>,
    observed: ResMut<'w, ObservedInteraction>,
}
impl ObservationMarker<'_> {
    fn publish(&mut self, bounds: Option<[f32; 4]>) {
        self.observed.0 = bounds;
    }
}
#[derive(SystemParam)]
pub struct CloseAllSurface<'w, 's> {
    state: Res<'w, ConnectionsCloseAllState>,
    roots: Query<'w, 's, Entity, With<CloseAllConfirmationRoot>>,
    cards: Query<'w, 's, Entity, With<CloseAllConfirmationCard>>,
}
pub fn close_all_observation(
    surface: CloseAllSurface,
    mut geometry: CaptureGeometry,
    mut marker: ObservationMarker,
) {
    let bounds = (|| {
        if !marker.feature.activated
            || !surface.state.armed
            || !geometry.visible(surface.roots.iter().next()?)
        {
            return None;
        }
        geometry.bounds(surface.cards.iter().next()?, "close all confirmation")
    })();
    marker.publish(bounds);
}
#[derive(SystemParam)]
pub struct DrawerSurface<'w, 's> {
    state: Res<'w, ConnectionsDrawerState>,
    roots: Query<'w, 's, Entity, With<ConnectionDrawerLayer>>,
    panels: Query<'w, 's, Entity, With<DrawerPanel>>,
}
pub fn drawer_observation(
    surface: DrawerSurface,
    mut geometry: CaptureGeometry,
    mut marker: ObservationMarker,
) {
    let bounds = (|| {
        if !marker.feature.activated
            || !surface.state.open
            || surface.state.selected != Some(0)
            || !geometry.visible(surface.roots.iter().next()?)
        {
            return None;
        }
        geometry.bounds(surface.panels.iter().next()?, "connection details")
    })();
    marker.publish(bounds);
}
#[derive(SystemParam)]
pub struct SpeedtestSurface<'w, 's> {
    modal: Res<'w, ModalState>,
    latest: Res<'w, LastOverviewProjection>,
    roots: Query<'w, 's, Entity, With<AdaptiveModalRoot>>,
    cards: Query<'w, 's, Entity, With<ModalCard>>,
    details: Query<'w, 's, &'static Text, With<OverviewSpeedtestDetailBodyText>>,
}
pub fn speedtest_observation(
    surface: SpeedtestSurface,
    mut geometry: CaptureGeometry,
    mut marker: ObservationMarker,
) {
    let bounds = (|| {
        if !marker.feature.activated
            || !surface.modal.is_open
            || !geometry.visible(surface.roots.iter().next()?)
        {
            return None;
        }
        let snapshot = &surface.latest.0.as_ref()?.speedtest;
        let language = UiLocale::default().code().to_string();
        let expected = listing(&project_details(snapshot), &|key| {
            Lang(&language).tr(key).into_owned()
        });
        if snapshot.node_results.is_empty()
            || !surface.details.iter().any(|text| text.0 == expected)
        {
            return None;
        }
        geometry.bounds(surface.cards.iter().next()?, "speedtest details")
    })();
    marker.publish(bounds);
}
#[derive(SystemParam)]
pub struct PaletteSurface<'w, 's> {
    state: Res<'w, CommandPaletteState>,
    roots: Query<'w, 's, Entity, With<CommandPaletteOverlayRoot>>,
    cards: Query<'w, 's, Entity, With<CommandPaletteCard>>,
}
pub fn palette_observation(
    surface: PaletteSurface,
    mut geometry: CaptureGeometry,
    mut marker: ObservationMarker,
) {
    let bounds = (|| {
        if !marker.feature.activated
            || !surface.state.is_open
            || surface.state.query != "dns"
            || surface.state.filtered_indices.is_empty()
            || !geometry.visible(surface.roots.iter().next()?)
        {
            return None;
        }
        geometry.bounds(surface.cards.iter().next()?, "command palette")
    })();
    marker.publish(bounds);
}
#[derive(SystemParam)]
pub struct ProtocolSurface<'w, 's> {
    panel: Res<'w, BusinessPanelState>,
    form: Res<'w, CustomNodeForm>,
    roots: Query<'w, 's, Entity, With<BusinessPanelRoot>>,
    cards: Query<'w, 's, Entity, With<BusinessPanelCard>>,
    preview: Query<'w, 's, (&'static Text, &'static CustomNodeText)>,
}
pub fn protocol_observation(
    surface: ProtocolSurface,
    mut geometry: CaptureGeometry,
    mut marker: ObservationMarker,
) {
    let bounds = (|| {
        if !marker.feature.activated
            || surface.panel.0 != Some(PanelKind::CustomNode)
            || !geometry.visible(surface.roots.iter().next()?)
            || surface.form.studio.draft.is_none()
            || surface.form.reset_fields
        {
            return None;
        }
        if marker.feature.feature == FeatureId::ProxiesUriImportPreview {
            if surface.form.importing.is_some()
                || surface.form.studio.draft.as_ref()?.name != "Imported-Node"
            {
                return None;
            }
            let preview = surface.form.studio.uri_preview.as_ref()?;
            if !surface.preview.iter().any(|(text, marker)| {
                marker.0 == ProtocolStudioSlot::UriPreview && &text.0 == preview
            }) {
                return None;
            }
        }
        geometry.bounds(surface.cards.iter().next()?, "protocol editor")
    })();
    marker.publish(bounds);
}
#[derive(SystemParam)]
pub struct InspectionSurface<'w, 's> {
    state: Res<'w, ProxyInspectionState>,
    roots: Query<'w, 's, Entity, With<ProxyInspectionRoot>>,
    cards: Query<'w, 's, Entity, With<ProxyInspectionCard>>,
    charts: Query<'w, 's, (Entity, &'static ChartPlate), With<ProxyInspectionChart>>,
}
pub fn inspection_observation(
    surface: InspectionSurface,
    mut geometry: CaptureGeometry,
    mut marker: ObservationMarker,
) {
    let bounds = (|| {
        if !marker.feature.activated
            || surface.state.selected.as_deref() != Some(INSPECTION_NODE)
            || !geometry.visible(surface.roots.iter().next()?)
        {
            return None;
        }
        let (entity, samples) = surface.charts.iter().next()?;
        if samples.0.up.len() != 3
            || samples.0.up[0] != 18.0
            || !samples.0.up[1].is_nan()
            || samples.0.up[2] != 42.0
        {
            return None;
        }
        let card = geometry.bounds(surface.cards.iter().next()?, "proxy inspection")?;
        let chart = geometry.bounds(entity, "proxy observation history")?;
        if chart[0] < card[0]
            || chart[1] < card[1]
            || chart[0] + chart[2] > card[0] + card[2]
            || chart[1] + chart[3] > card[1] + card[3]
        {
            return None;
        }
        Some(card)
    })();
    marker.publish(bounds);
}
