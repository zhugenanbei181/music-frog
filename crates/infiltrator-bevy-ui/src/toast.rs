//! Shell toast overlay: redaction, shared dedup policy, and the mounted stack.
//!
//! The widget layer owns the toast cards, timers and the `ToastQueue`
//! resource. This module adds the two shell-level responsibilities the
//! surfaces must agree on: text redaction before anything enters the queue
//! (via the shared `infiltrator_domain::redact` engine) and the shared
//! dedup/capacity policy from `infiltrator_contract::toast`.

use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin, Update};
use bevy::ecs::entity::Entity;
use bevy::ecs::message::{Message, MessageReader, MessageWriter};
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::CommandsSceneExt;
use bevy::time::{Time, Virtual};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::toast::{
    ToastContainer, ToastKind, ToastQueue, ToastSpawnEvent, toast_stack_scene,
};
use infiltrator_contract::a11y::ShellA11yNode;
use infiltrator_contract::toast::{ToastAdmission, ToastGate};

/// The shared dedup gate plus the shell's toast clock.
#[derive(Resource, Clone, Debug, Default)]
pub struct ToastPolicyGate {
    gate: ToastGate,
}

impl ToastPolicyGate {
    pub fn gate(&self) -> &ToastGate {
        &self.gate
    }

    /// Redact then admit one toast. Returns whether it was enqueued (a
    /// coalesced duplicate returns `false` and spawns nothing).
    pub fn push(
        &mut self,
        spawns: &mut MessageWriter<ToastSpawnEvent>,
        kind: ToastKind,
        text: &str,
        duration_secs: f32,
        now_ms: u64,
    ) -> bool {
        let redacted = infiltrator_domain::redact::redact_line(text, &[]);
        let severity = match kind {
            ToastKind::Info => infiltrator_contract::toast::ToastSeverity::Info,
            ToastKind::Success => infiltrator_contract::toast::ToastSeverity::Success,
            ToastKind::Warning => infiltrator_contract::toast::ToastSeverity::Warning,
            ToastKind::Danger => infiltrator_contract::toast::ToastSeverity::Error,
        };
        if self.gate.admit(severity, &redacted, now_ms) == ToastAdmission::Coalesced {
            return false;
        }
        spawns.write(ToastSpawnEvent {
            content: redacted,
            kind,
            duration_secs,
        });
        true
    }
}

/// Ids currently mounted, so the overlay only re-mounts when the stack's
/// membership changes (never on a timer tick).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct ToastStackVersion(pub Vec<u64>);

/// One shell-level toast request (the ingestion point every producer uses).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct ShellToast {
    pub kind: ToastKind,
    pub text: String,
    pub duration_secs: f32,
}

impl ShellToast {
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            kind: ToastKind::Info,
            text: text.into(),
            duration_secs: 5.0,
        }
    }

    pub fn warning(text: impl Into<String>) -> Self {
        Self {
            kind: ToastKind::Warning,
            text: text.into(),
            duration_secs: 5.0,
        }
    }

    pub fn danger(text: impl Into<String>) -> Self {
        Self {
            kind: ToastKind::Danger,
            text: text.into(),
            duration_secs: 5.0,
        }
    }
}

/// Redact + dedup every shell toast request into the widget queue.
pub fn on_shell_toast(
    mut requests: MessageReader<ShellToast>,
    time: Res<Time<Virtual>>,
    mut gate: ResMut<ToastPolicyGate>,
    mut spawns: MessageWriter<ToastSpawnEvent>,
) {
    for request in requests.read() {
        gate.push(
            &mut spawns,
            request.kind,
            &request.text,
            request.duration_secs,
            now_ms(&time),
        );
    }
}

/// Millisecond clock shared by the gate (monotonic, shell-process local).
pub fn now_ms(time: &Time<Virtual>) -> u64 {
    (time.elapsed_secs_f64() * 1000.0) as u64
}

/// Mount the overlay whenever the widget queue's membership changes. The
/// stack's own [`ToastContainer`] root is the mount marker.
pub fn sync_toast_stack(
    mut commands: Commands,
    queue: Option<Res<ToastQueue>>,
    palette: Res<UiPalette>,
    mut version: ResMut<ToastStackVersion>,
    mounted: Query<Entity, With<ToastContainer>>,
) {
    let Some(queue) = queue else {
        return;
    };
    let ids: Vec<u64> = queue.items().iter().map(|toast| toast.id).collect();
    if ids == version.0 {
        return;
    }
    version.0 = ids;
    for entity in &mounted {
        commands.entity(entity).despawn();
    }
    if queue.items().is_empty() {
        return;
    }
    commands.spawn_scene(toast_stack_scene(queue.items(), &palette));
}

/// Register the policy gate, the version latch and the overlay sync system.
pub struct ShellToastPlugin;

impl Plugin for ShellToastPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ToastPolicyGate>();
        app.init_resource::<ToastStackVersion>();
        app.add_message::<ShellToast>();
        app.add_systems(
            Update,
            on_shell_toast.before(infiltrator_bevy_widgets::toast::advance_toasts),
        );
        app.add_systems(
            Update,
            sync_toast_stack.after(infiltrator_bevy_widgets::toast::advance_toasts),
        );
        // DUAL-15-10: the mounted stack root is a live region in the shared
        // grammar; the widget scene stays business-agnostic, so the semantic
        // node is attached here, where the product vocabulary lives.
        app.add_systems(Update, sync_toast_semantics.after(sync_toast_stack));
    }
}

/// Attach the shared live-region semantic node to the mounted toast stack.
fn sync_toast_semantics(
    mut commands: Commands,
    roots: Query<Entity, (With<ToastContainer>, Without<AccessibilityNode>)>,
) {
    for entity in &roots {
        commands
            .entity(entity)
            .insert(crate::a11y::semantic_node(ShellA11yNode::ToastRegion));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gate_starts_empty_with_the_product_policy() {
        let gate = ToastPolicyGate::default();
        assert_eq!(gate.gate().policy().max_visible, 3);
        assert_eq!(gate.gate().policy().dedup_window_ms, 2_000);
        assert_eq!(gate.gate().live_offers(0), 0);
    }
}
