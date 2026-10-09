//! Host-back seam: the layered dismissal policy that runs before the route
//! history. A dismissible layer (modal, drawer, confirmation) consumes the
//! system back affordance first; only when no layer is open does back pop the
//! route history. Pure policy ([`resolve_back`]) plus its observer
//! ([`on_host_back`]), extracted from [`super`] to keep the router under the
//! line budget.

use super::{Route, RouteChanged, RouteHistory};
use bevy::ecs::event::Event;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, ResMut};

/// A dismissible layer above the page (modal, drawer, confirmation) that must
/// consume the system back affordance before the route history does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BackLayer {
    /// A blocking modal dialog.
    Modal,
    /// A side drawer / inspection panel.
    Drawer,
    /// A destructive-action confirmation.
    Confirmation,
}

/// Ordered stack of open layers; the top of the stack owns back first. Pages
/// (or the host) push and pop as their layers open and close.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct BackLayerStack(Vec<BackLayer>);

impl BackLayerStack {
    /// Register a newly opened layer. Re-pushing the current top is a no-op.
    pub fn push(&mut self, layer: BackLayer) {
        if self.0.last() != Some(&layer) {
            self.0.push(layer);
        }
    }

    /// Remove and return the top layer, if any.
    pub fn pop(&mut self) -> Option<BackLayer> {
        self.0.pop()
    }

    /// The layer that currently owns back, if any.
    pub fn top(&self) -> Option<BackLayer> {
        self.0.last().copied()
    }

    /// Whether any layer is open.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Outcome of resolving a system back intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackResolution {
    /// A layered surface consumed the back; the route did not change.
    DismissLayer(BackLayer),
    /// The route history moved to the previous route.
    Navigated(Route),
    /// Nothing to dismiss and no history to pop (already at the root).
    Ignored,
}

/// Resolve one back step: the top open layer closes first, otherwise the route
/// history pops. Pure so both the observer and the tests share one policy.
pub fn resolve_back(layers: &mut BackLayerStack, history: &mut RouteHistory) -> BackResolution {
    if let Some(layer) = layers.pop() {
        return BackResolution::DismissLayer(layer);
    }
    match history.go_back() {
        Some(route) => BackResolution::Navigated(route),
        None => BackResolution::Ignored,
    }
}

/// Typed intent the host (Android Activity/Compose, desktop chrome) triggers
/// when the OS back affordance fires. Handled by [`on_host_back`].
#[derive(Event, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostBackIntent;

/// Fired when the back intent closed a layered surface. Layer owners observe
/// this to dismiss themselves; the route does not change.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DismissBackLayer(pub BackLayer);

/// Handle the host's system back intent (BANDROID-014). The top open layer
/// closes first; otherwise the route history pops. This observer deliberately
/// has no command sink: back is pure navigation and never writes config.
pub fn on_host_back(
    _intent: On<HostBackIntent>,
    mut layers: ResMut<BackLayerStack>,
    mut history: ResMut<RouteHistory>,
    mut commands: Commands,
) {
    match resolve_back(&mut layers, &mut history) {
        BackResolution::DismissLayer(layer) => commands.trigger(DismissBackLayer(layer)),
        BackResolution::Navigated(route) => commands.trigger(RouteChanged(route)),
        BackResolution::Ignored => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandSinkHandle, DemoCommandSink};
    use bevy::app::App;
    use std::sync::Arc;

    #[test]
    fn back_intent_navigates_the_route_history_layer() {
        let mut history = RouteHistory::new(10);
        let mut layers = BackLayerStack::default();
        history.push(Route::Proxies);
        history.push(Route::Logs);
        assert_eq!(
            resolve_back(&mut layers, &mut history),
            BackResolution::Navigated(Route::Proxies)
        );
        assert_eq!(history.current(), Route::Proxies);
    }

    #[test]
    fn back_intent_closes_the_top_layer_before_navigating() {
        let mut history = RouteHistory::new(10);
        let mut layers = BackLayerStack::default();
        history.push(Route::Proxies);
        layers.push(BackLayer::Modal);
        layers.push(BackLayer::Confirmation);

        // The newest layer owns back; the route must not change.
        assert_eq!(
            resolve_back(&mut layers, &mut history),
            BackResolution::DismissLayer(BackLayer::Confirmation)
        );
        assert_eq!(history.current(), Route::Proxies);
        assert_eq!(layers.top(), Some(BackLayer::Modal));

        assert_eq!(
            resolve_back(&mut layers, &mut history),
            BackResolution::DismissLayer(BackLayer::Modal)
        );
        assert_eq!(history.current(), Route::Proxies);
        assert!(layers.is_empty());

        // With no layer left, the same intent pops the route history.
        assert_eq!(
            resolve_back(&mut layers, &mut history),
            BackResolution::Navigated(Route::Overview)
        );
    }

    #[test]
    fn back_intent_at_the_root_is_ignored() {
        let mut history = RouteHistory::new(10);
        let mut layers = BackLayerStack::default();
        assert_eq!(
            resolve_back(&mut layers, &mut history),
            BackResolution::Ignored
        );
    }

    #[test]
    fn host_back_observer_navigates_without_writing_config() {
        let sink = Arc::new(DemoCommandSink::accepting());
        let mut app = App::new();
        app.init_resource::<RouteHistory>();
        app.init_resource::<BackLayerStack>();
        app.insert_resource(CommandSinkHandle(sink.clone()));
        app.add_observer(on_host_back);
        {
            let mut history = app.world_mut().resource_mut::<RouteHistory>();
            history.push(Route::Proxies);
            history.push(Route::Logs);
        }
        app.world_mut().trigger(HostBackIntent);
        app.update();
        assert_eq!(
            app.world().resource::<RouteHistory>().current(),
            Route::Proxies
        );
        // Back is pure navigation: no UI command (config write) may be emitted.
        assert!(sink.submitted().is_empty());
    }
}
