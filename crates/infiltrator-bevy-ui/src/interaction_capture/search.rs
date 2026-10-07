//! Search capture edits the native field, executes its shared command and observes actual highlight ink.
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::scroll::request_scroll;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::pages::proxies::ProxiesScrollArea;
use crate::pages::proxies::{NodeNameText, ProxyNodeButton};
use crate::pages::proxies_search::{ProxySearchInput, ProxySearchState};
use crate::route::{ActiveRoute, Route};
use crate::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::proxy_preferences_application::ProxyPreferencesApplication;
use infiltrator_application::proxy_projection::{project_groups_snapshot, project_proxy_groups};
use infiltrator_application::proxy_search_fixtures::{SEARCH_QUERY, observed_proxies};
use infiltrator_application::proxy_search_projection::project_name_runs;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface_snapshot::ProxyGroupSnapshot;
use std::sync::Arc;

#[derive(Resource, Default)]
struct SearchCapture {
    preferences: ProxyPreferencesApplication,
    groups: Vec<ProxyGroupSnapshot>,
    stage: u8,
}
pub fn install(app: &mut App) {
    let preferences = ProxyPreferencesApplication::new();
    let (groups, _) = project_groups_snapshot(
        &observed_proxies(),
        &preferences
            .preferences()
            .expect("shared preference state available"),
    );
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::new(
        CommandApplication::new().with_proxy_preferences(preferences.clone()),
        CaptureCapability::Search,
    ))));
    app.insert_resource(SearchCapture {
        preferences,
        groups,
        stage: 0,
    });
}

#[derive(SystemParam)]
pub struct SearchControls<'w, 's> {
    capture: ResMut<'w, SearchCapture>,
    latest: Res<'w, LatestSurfaceSnapshot>,
    route: Res<'w, ActiveRoute>,
    search: Res<'w, ProxySearchState>,
    wrappers: Query<'w, 's, (Entity, &'static Children), With<ProxySearchInput>>,
    fields: Query<'w, 's, &'static mut TextField>,
    nodes: Query<'w, 's, Entity, With<ProxyNodeButton>>,
    scroll: Query<'w, 's, Entity, With<ProxiesScrollArea>>,
}
pub fn activate(
    mut controls: SearchControls,
    feature: Res<InteractionCapture>,
    geometry: CaptureGeometry,
    mut commands: Commands,
) {
    if !selected(
        &feature,
        &controls.route,
        FeatureId::ProxiesSearchHighlight,
        Route::Proxies,
    ) {
        return;
    }
    match controls.capture.stage {
        0 => {
            let mut snapshot = controls.latest.0.clone();
            snapshot.revision += 1;
            let page = snapshot
                .pages
                .proxies
                .data
                .as_mut()
                .expect("search fixture page");
            page.groups = controls.capture.groups.clone();
            page.search_query.clear();
            page.name_runs = project_name_runs(&page.groups, "");
            commands.trigger(SurfaceSnapshotUpdated(snapshot));
            controls.capture.stage = 1;
        }
        1 => {
            let Some((_, children)) = controls.wrappers.iter().next() else {
                return;
            };
            let Some(field) = children
                .iter()
                .copied()
                .find(|child| controls.fields.contains(*child))
            else {
                return;
            };
            controls
                .fields
                .get_mut(field)
                .expect("native search input")
                .0
                .apply(TextFieldInput::SetText(SEARCH_QUERY.into()));
            controls.capture.stage = 2;
        }
        2 => {
            let preferences = controls
                .capture
                .preferences
                .preferences()
                .expect("shared preferences");
            if preferences.search_query != SEARCH_QUERY || controls.search.pending.is_some() {
                return;
            }
            let groups = project_proxy_groups(controls.capture.groups.clone(), &preferences);
            let mut snapshot = controls.latest.0.clone();
            snapshot.revision += 1;
            let page = snapshot
                .pages
                .proxies
                .data
                .as_mut()
                .expect("search fixture page");
            page.search_query = SEARCH_QUERY.into();
            page.name_runs = project_name_runs(&groups, SEARCH_QUERY);
            page.groups = groups;
            commands.trigger(SurfaceSnapshotUpdated(snapshot));
            controls.capture.stage = 3;
        }
        _ => {
            if let (Some(node), Some(scroll)) =
                (controls.nodes.iter().next(), controls.scroll.iter().next())
                && let (Some(card), Some(viewport)) = (geometry.rect(node), geometry.rect(scroll))
            {
                request_scroll(&mut commands, scroll, card, viewport);
            }
        }
    }
}
#[derive(SystemParam)]
pub struct SearchObservation<'w, 's> {
    capture: Res<'w, SearchCapture>,
    palette: Res<'w, UiPalette>,
    wrappers: Query<'w, 's, (Entity, &'static Children), With<ProxySearchInput>>,
    fields: Query<'w, 's, &'static TextField>,
    labels: Query<'w, 's, (&'static Text, &'static TextColor), With<NodeNameText>>,
    nodes: Query<'w, 's, Entity, With<ProxyNodeButton>>,
}
impl SearchObservation<'_, '_> {
    fn bounds(&self, geometry: &mut CaptureGeometry) -> Option<[f32; 4]> {
        if self.capture.stage != 3 {
            return None;
        }
        let (wrapper, children) = self.wrappers.iter().next()?;
        let field = children
            .iter()
            .copied()
            .find_map(|entity| self.fields.get(entity).ok())?;
        if field.0.text() != SEARCH_QUERY
            || !self
                .labels
                .iter()
                .any(|(text, color)| text.0 == SEARCH_QUERY && color.0 == self.palette.accent)
        {
            return None;
        }
        let input = geometry.bounds(wrapper, "proxy search")?;
        let card = geometry.bounds(self.nodes.iter().next()?, "filtered proxy node")?;
        let x = input[0].min(card[0]);
        let y = input[1].min(card[1]);
        Some([
            x,
            y,
            (input[0] + input[2]).max(card[0] + card[2]) - x,
            (input[1] + input[3]).max(card[1] + card[3]) - y,
        ])
    }
}
pub fn observe(
    search: SearchObservation,
    mut geometry: CaptureGeometry,
    mut state: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
) {
    observation.publish(&mut state, search.bounds(&mut geometry));
}
