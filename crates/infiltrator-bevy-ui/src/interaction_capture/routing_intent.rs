//! Capture submits the typed routing command and observes its real failure surface.
use super::geometry::CaptureGeometry;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::app::RoutingIntentActionState;
use crate::command::CommandSinkHandle;
use crate::command_execution::UnavailableCommandSink;
use crate::pages::overview_cards::OverviewRoutingIntentSegmentPill;
use crate::route::{ActiveRoute, Route};
use crate::shell_routing_intent_issue::{
    DismissRoutingIntentIssue, RetryRoutingIntentChange, RoutingIntentIssueRoot,
    RoutingIntentIssueText, RoutingIntentSettingsGuide,
};
use crate::surface::LatestSurfaceSnapshot;
use bevy::app::{App, Update};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::parity::FeatureId;
use infiltrator_presentation::routing_intent_projection::routing_intent_failure_copy;
use infiltrator_shared::locales::Lang;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::routing_intent::RoutingIntent;
use std::sync::Arc;

pub fn install(app: &mut App, feature: FeatureId) {
    let failure = if feature == FeatureId::ShellRoutingIntentAuthentication {
        Failure::new(
            ErrorCode::Authentication,
            "Controller token rejected",
            false,
        )
    } else {
        Failure::new(
            ErrorCode::Network,
            "Controller temporarily refused the routing change",
            true,
        )
    };
    app.insert_resource(CommandSinkHandle(Arc::new(UnavailableCommandSink::new(
        failure,
    ))));
    app.add_systems(Update, activate);
}

pub fn activate(
    feature: Res<InteractionCapture>,
    route: Res<ActiveRoute>,
    state: Res<RoutingIntentActionState>,
    controls: Query<(Entity, &OverviewRoutingIntentSegmentPill, &ButtonDisabled)>,
    mut commands: Commands,
) {
    if !selected(&feature, &route, feature.feature, Route::Overview)
        || state.0.pending_change().is_some()
        || state.0.failure().is_some()
    {
        return;
    }
    if let Some((entity, _, _)) = controls.iter().find(|(_, intent, disabled)| {
        intent.0 == RoutingIntent::RouteAllThroughProxy && !disabled.0
    }) {
        commands.trigger(Activate { entity });
    }
}

#[derive(SystemParam)]
pub struct RoutingIntentSurface<'w, 's> {
    state: Res<'w, RoutingIntentActionState>,
    latest: Res<'w, LatestSurfaceSnapshot>,
    locale: Res<'w, UiLocale>,
    roots: Query<'w, 's, Entity, With<RoutingIntentIssueRoot>>,
    retry: Query<'w, 's, (Entity, &'static ButtonDisabled), With<RetryRoutingIntentChange>>,
    dismiss: Query<'w, 's, Entity, With<DismissRoutingIntentIssue>>,
    guides: Query<'w, 's, Entity, With<RoutingIntentSettingsGuide>>,
    texts: Query<'w, 's, (Entity, &'static Text), With<RoutingIntentIssueText>>,
}

pub fn observe(
    surface: RoutingIntentSurface,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observed: ResMut<ObservedInteraction>,
) {
    let bounds = (|| {
        let state = &surface.state.0;
        let authentication = feature.feature == FeatureId::ShellRoutingIntentAuthentication;
        let failure_code = if authentication {
            ErrorCode::Authentication
        } else {
            ErrorCode::Network
        };
        if state.pending_change().is_some()
            || state.render_snapshot().current.as_ref() != Some(&RoutingIntent::FollowRules)
            || state.failure()?.code != failure_code
            || state.requested_target() != Some(&RoutingIntent::RouteAllThroughProxy)
            || surface.latest.0.core.lifecycle != CoreLifecycle::Running
        {
            return None;
        }
        let (label, text) = surface.texts.single().ok()?;
        let expected = routing_intent_failure_copy(state, &Lang(surface.locale.code()));
        if text.0 != expected {
            return None;
        }
        geometry.bounds(label, "routing intent failure label")?;
        let (retry, disabled) = surface.retry.single().ok()?;
        if disabled.0 != authentication {
            return None;
        }
        if authentication {
            if !state.needs_controller_settings() || state.retry_target().is_some() {
                return None;
            }
            geometry.bounds(surface.guides.single().ok()?, "controller settings guide")?;
        } else {
            if state.retry_target() != Some(RoutingIntent::RouteAllThroughProxy) {
                return None;
            }
            geometry.bounds(retry, "routing intent retry")?;
        }
        geometry.bounds(surface.dismiss.single().ok()?, "routing intent dismiss")?;
        geometry.bounds(
            surface.roots.single().ok()?,
            "routing intent failure surface",
        )
    })();
    observed.publish(&mut feature, bounds);
}
