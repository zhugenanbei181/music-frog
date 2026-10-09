//! BANDROID-014: typed native-host seams in the Bevy UI layer.
//!
//! The Android host (Activity / Compose / `Vibrator` / system settings) owns
//! the real platform wiring. This module defines only the typed ports and
//! intents the host feeds into the shared shell; there is no JNI here. A host
//! supplies a [`HapticsPort`] and sets [`HostPreferences`], and the UI keeps a
//! silent, no-op default until it does.
//!
//! - Haptics: the shared core lives in
//!   [`infiltrator_bevy_widgets::haptics`]; [`HapticsHost`] is the capability
//!   gate in front of it. It is disableable through [`AudioHapticsSettings`]
//!   and no-ops when the host does not advertise a vibrator.
//! - Reduce-motion / energy: [`HostPreferences`] is the typed preference input,
//!   consumed by the animation and cadence paths so the shell can drop
//!   non-essential motion without touching business state.

use bevy::app::{App, Plugin};
use bevy::ecs::event::Event;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Res, ResMut};
use infiltrator_bevy_widgets::abi::{HostCapabilities, WidgetCapability};
use infiltrator_bevy_widgets::haptics::{
    AudioHapticsSettings, FeedbackTriggerEvent, HapticPattern,
};
use std::fmt;
use std::sync::{Arc, OnceLock};

/// Platform haptic sink the host implements (Android `Vibrator`, iOS haptics,
/// desktop no-op). The UI never talks to the OS directly.
pub trait HapticsPort: Send + Sync {
    /// Emit one semantic haptic pattern. A host without a vibrator returns
    /// without side effects.
    fn vibrate(&self, pattern: HapticPattern);
}

/// Default no-op sink used by headless and desktop hosts.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopHaptics;

impl HapticsPort for NoopHaptics {
    fn vibrate(&self, _pattern: HapticPattern) {}
}

/// The host's haptic sink plus whether it advertises a working vibrator.
#[derive(Resource, Clone)]
pub struct HapticsHost {
    port: Arc<dyn HapticsPort>,
    supported: bool,
}

impl Default for HapticsHost {
    fn default() -> Self {
        Self {
            port: Arc::new(NoopHaptics),
            supported: false,
        }
    }
}

impl fmt::Debug for HapticsHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HapticsHost")
            .field("supported", &self.supported)
            .finish_non_exhaustive()
    }
}

impl HapticsHost {
    /// Install a host sink. `supported` is the host's capability answer; an
    /// unsupported host stays a no-op even when a port is supplied.
    pub fn new(port: Arc<dyn HapticsPort>, supported: bool) -> Self {
        Self { port, supported }
    }

    /// Install a host sink gated by the host's advertised capabilities.
    pub fn from_capabilities(port: Arc<dyn HapticsPort>, capabilities: HostCapabilities) -> Self {
        Self::new(port, capabilities.has(WidgetCapability::HapticFeedback))
    }

    /// Whether the host advertises a working vibrator.
    pub const fn is_supported(&self) -> bool {
        self.supported
    }

    /// Emit one pattern when the host supports it and the user keeps haptics
    /// enabled. Returns whether a vibration was actually sent.
    pub fn emit(&self, settings: &AudioHapticsSettings, pattern: HapticPattern) -> bool {
        if !self.supported || !settings.haptics_enabled {
            return false;
        }
        self.port.vibrate(pattern);
        true
    }
}

/// Coarse power mode the host can request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EnergyPreference {
    /// No host restriction; full cadence and motion.
    #[default]
    Balanced,
    /// Host asks to conserve power: drop non-essential motion and cadence.
    Saver,
}

/// Typed accessibility / power preference the host can set (Android system
/// "remove animations", battery saver). Consumed by animation/cadence paths.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostPreferences {
    /// Reduce or remove non-essential motion.
    pub reduce_motion: bool,
    /// Coarse energy preference.
    pub energy: EnergyPreference,
}

impl HostPreferences {
    /// Whether animation-driven cadence and motion should be suppressed.
    pub const fn animations_suppressed(self) -> bool {
        self.reduce_motion || matches!(self.energy, EnergyPreference::Saver)
    }
}

/// Typed host intent carrying the current preference set. The host triggers
/// this instead of writing the resource directly, so the shell owns the apply
/// point (and can later fan it out to more consumers).
#[derive(Event, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostPreferenceIntent(pub HostPreferences);

fn apply_host_preferences(
    intent: On<HostPreferenceIntent>,
    mut preferences: ResMut<HostPreferences>,
) {
    if *preferences != intent.0 {
        *preferences = intent.0;
    }
}

fn on_feedback_trigger(
    trigger: On<FeedbackTriggerEvent>,
    host: Res<HapticsHost>,
    settings: Res<AudioHapticsSettings>,
) {
    host.emit(&settings, trigger.pattern);
}

static ATTACHED_HAPTICS: OnceLock<HapticsHost> = OnceLock::new();

/// Attach the host's haptic sink before launch (mirrors `attach_application`).
/// The default [`HostCapabilitiesPlugin`] picks it up; an explicit
/// [`HostCapabilitiesPlugin::new`] overrides it.
pub fn attach_haptics(host: HapticsHost) {
    let _ = ATTACHED_HAPTICS.set(host);
}

/// Retrieve the attached haptic host, if any.
pub fn attached_haptics() -> Option<HapticsHost> {
    ATTACHED_HAPTICS.get().cloned()
}

/// Installs the haptics gate and the preference input. The default is a silent
/// no-op host; a native host passes its real [`HapticsHost`] here (or calls
/// [`attach_haptics`] before the shell is built).
pub struct HostCapabilitiesPlugin {
    haptics: Option<HapticsHost>,
}

impl Default for HostCapabilitiesPlugin {
    fn default() -> Self {
        Self {
            haptics: attached_haptics(),
        }
    }
}

impl HostCapabilitiesPlugin {
    /// Install with an explicit host haptic sink.
    pub fn new(haptics: HapticsHost) -> Self {
        Self {
            haptics: Some(haptics),
        }
    }
}

impl Plugin for HostCapabilitiesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AudioHapticsSettings>()
            .init_resource::<HostPreferences>()
            .init_resource::<HapticsHost>()
            .add_observer(apply_host_preferences)
            .add_observer(on_feedback_trigger);
        if let Some(haptics) = &self.haptics {
            app.insert_resource(haptics.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingHaptics(Arc<Mutex<Vec<HapticPattern>>>);

    impl HapticsPort for RecordingHaptics {
        fn vibrate(&self, pattern: HapticPattern) {
            self.0
                .lock()
                .expect("haptics recorder poisoned")
                .push(pattern);
        }
    }

    #[test]
    fn haptics_no_op_when_unsupported_or_disabled() {
        let recorder = Arc::new(RecordingHaptics::default());
        let port: Arc<dyn HapticsPort> = recorder.clone();

        let unsupported = HapticsHost::new(port.clone(), false);
        assert!(!unsupported.emit(&AudioHapticsSettings::default(), HapticPattern::MediumClick));
        assert!(recorder.0.lock().expect("recorder").is_empty());

        let supported = HapticsHost::new(port, true);
        let disabled = AudioHapticsSettings {
            haptics_enabled: false,
            ..AudioHapticsSettings::default()
        };
        assert!(!supported.emit(&disabled, HapticPattern::MediumClick));
        assert!(recorder.0.lock().expect("recorder").is_empty());

        assert!(supported.emit(&AudioHapticsSettings::default(), HapticPattern::MediumClick));
        assert_eq!(
            *recorder.0.lock().expect("recorder"),
            vec![HapticPattern::MediumClick]
        );
    }

    #[test]
    fn haptics_gate_follows_host_capabilities() {
        let recorder = Arc::new(RecordingHaptics::default());
        let port: Arc<dyn HapticsPort> = recorder.clone();
        let headless =
            HapticsHost::from_capabilities(port.clone(), HostCapabilities::headless_minimal());
        assert!(!headless.is_supported());
        let mobile = HapticsHost::from_capabilities(port, HostCapabilities::mobile_default());
        assert!(mobile.is_supported());
    }

    #[test]
    fn preference_intent_updates_the_resource() {
        let mut app = App::new();
        app.add_plugins(HostCapabilitiesPlugin::new(HapticsHost::default()));
        assert_eq!(
            *app.world().resource::<HostPreferences>(),
            HostPreferences::default()
        );
        app.world_mut()
            .trigger(HostPreferenceIntent(HostPreferences {
                reduce_motion: true,
                energy: EnergyPreference::Saver,
            }));
        app.update();
        let preferences = *app.world().resource::<HostPreferences>();
        assert!(preferences.reduce_motion);
        assert_eq!(preferences.energy, EnergyPreference::Saver);
        assert!(preferences.animations_suppressed());
    }

    #[test]
    fn animations_suppressed_covers_motion_and_energy() {
        assert!(!HostPreferences::default().animations_suppressed());
        assert!(
            HostPreferences {
                reduce_motion: true,
                energy: EnergyPreference::Balanced,
            }
            .animations_suppressed()
        );
        assert!(
            HostPreferences {
                reduce_motion: false,
                energy: EnergyPreference::Saver,
            }
            .animations_suppressed()
        );
    }
}
