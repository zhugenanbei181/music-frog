//! Audio-Haptic multimodal feedback engine with procedural UI tone synthesizer and vibration patterns.

use bevy::ecs::event::Event;
use bevy::ecs::resource::Resource;

/// Semantic vibration and haptic feedback pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HapticPattern {
    LightTick,
    MediumClick,
    HeavyThud,
    SuccessPulse,
    WarningDoublePulse,
    SelectionTick,
}

impl HapticPattern {
    /// Nominal vibration duration in milliseconds.
    pub fn duration_ms(&self) -> u32 {
        match self {
            HapticPattern::LightTick | HapticPattern::SelectionTick => 15,
            HapticPattern::MediumClick => 30,
            HapticPattern::HeavyThud => 60,
            HapticPattern::SuccessPulse => 80,
            HapticPattern::WarningDoublePulse => 120,
        }
    }
}

/// Attack-Decay-Sustain-Release (ADSR) amplitude envelope.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdsrEnvelope {
    pub attack_secs: f32,
    pub decay_secs: f32,
    pub sustain_level: f32,
    pub release_secs: f32,
}

impl AdsrEnvelope {
    pub fn new(attack: f32, decay: f32, sustain: f32, release: f32) -> Self {
        Self {
            attack_secs: attack,
            decay_secs: decay,
            sustain_level: sustain,
            release_secs: release,
        }
    }

    /// Evaluate normalized envelope amplitude at time `t` relative to note start and note duration.
    pub fn evaluate(&self, t: f32, note_duration_secs: f32) -> f32 {
        if t < 0.0 {
            return 0.0;
        }

        if t < self.attack_secs {
            return (t / self.attack_secs.max(1e-4)).clamp(0.0, 1.0);
        }

        let decay_t = t - self.attack_secs;
        if decay_t < self.decay_secs {
            let frac = decay_t / self.decay_secs.max(1e-4);
            return 1.0 - frac * (1.0 - self.sustain_level);
        }

        if t <= note_duration_secs {
            return self.sustain_level;
        }

        let release_t = t - note_duration_secs;
        if release_t < self.release_secs {
            let frac = release_t / self.release_secs.max(1e-4);
            return self.sustain_level * (1.0 - frac);
        }

        0.0
    }
}

/// Procedural audio tone synthesizer generator without external asset dependencies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProceduralTone {
    pub frequency_hz: f32,
    pub duration_secs: f32,
    pub envelope: AdsrEnvelope,
}

impl ProceduralTone {
    /// Construct standard click feedback tone.
    pub fn click() -> Self {
        Self {
            frequency_hz: 880.0,
            duration_secs: 0.03,
            envelope: AdsrEnvelope::new(0.002, 0.015, 0.2, 0.01),
        }
    }

    /// Construct success chime tone.
    pub fn success_chime() -> Self {
        Self {
            frequency_hz: 523.25, // C5
            duration_secs: 0.15,
            envelope: AdsrEnvelope::new(0.01, 0.04, 0.6, 0.08),
        }
    }

    /// Generate PCM sample at time `t` seconds with sample rate `sample_rate_hz`.
    pub fn sample_at(&self, t: f32) -> f32 {
        let amp = self.envelope.evaluate(t, self.duration_secs);
        let phase = 2.0 * std::f32::consts::PI * self.frequency_hz * t;
        amp * phase.sin()
    }
}

/// Global audio and haptics coordinator resource.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioHapticsSettings {
    pub haptics_enabled: bool,
    pub sound_enabled: bool,
}

impl Default for AudioHapticsSettings {
    fn default() -> Self {
        Self {
            haptics_enabled: true,
            sound_enabled: true,
        }
    }
}

/// Event triggering haptic and procedural audio feedback.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeedbackTriggerEvent {
    pub pattern: HapticPattern,
    pub play_sound: bool,
}
