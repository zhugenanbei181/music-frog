//! Audio-Haptic multimodal feedback engine with procedural UI tone synthesizer and vibration patterns.

use bevy::ecs::event::Event;
use bevy::ecs::resource::Resource;
use std::f32::consts::PI;

/// Default PCM sample rate for procedural UI tones.
pub const DEFAULT_SAMPLE_RATE_HZ: u32 = 48_000;

/// Hard ceiling on frames synthesized for one tone (one second at the default
/// rate), so a bad duration can never allocate an unbounded buffer.
pub const MAX_TONE_FRAMES: usize = 48_000;

/// A bounded mono PCM buffer ready for platform playback.
///
/// `frames` are linear `f32` samples in `[-1.0, 1.0]`. The buffer is owned and
/// bounded on purpose: a port can hand it to a realtime thread without the
/// widget layer holding any platform state of its own.
#[derive(Clone, Debug, PartialEq)]
pub struct PcmBuffer {
    pub sample_rate_hz: u32,
    pub frames: Vec<f32>,
}

impl PcmBuffer {
    /// Number of mono frames.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Whether the buffer holds no frames.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Playback duration in seconds derived from frame count and sample rate.
    pub fn duration_secs(&self) -> f32 {
        if self.sample_rate_hz == 0 {
            return 0.0;
        }
        self.frames.len() as f32 / self.sample_rate_hz as f32
    }
}

/// Typed output port for platform audio playback and vibration.
///
/// Every method has a no-op default, so a host that cannot service a channel
/// (headless tests, a desktop without a vibrator, an unsupported target)
/// simply omits the implementation. The widget layer never probes platform
/// state; it routes intent through this port and stays backend-agnostic.
pub trait AudioHapticOutput: Send + Sync {
    /// Play a bounded procedural tone. Default: unsupported, no-op.
    fn play_tone(&self, _pcm: &PcmBuffer) {}

    /// Emit a vibration for `duration_ms`. Default: unsupported, no-op.
    fn vibrate(&self, _duration_ms: u32) {}
}

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

    /// Construct a low warning buzz tone.
    pub fn warning_buzz() -> Self {
        Self {
            frequency_hz: 220.0,
            duration_secs: 0.12,
            envelope: AdsrEnvelope::new(0.005, 0.03, 0.5, 0.05),
        }
    }

    /// Construct a short selection tick, distinct from a click.
    pub fn selection_tick() -> Self {
        Self {
            frequency_hz: 1320.0,
            duration_secs: 0.02,
            envelope: AdsrEnvelope::new(0.001, 0.008, 0.1, 0.008),
        }
    }

    /// Total audible length: note duration plus the release tail.
    pub fn total_duration_secs(&self) -> f32 {
        self.duration_secs.max(0.0) + self.envelope.release_secs.max(0.0)
    }

    /// Number of mono PCM frames this tone occupies at `sample_rate_hz`,
    /// clamped to [`MAX_TONE_FRAMES`] so synthesis stays bounded.
    pub fn frame_count(&self, sample_rate_hz: u32) -> usize {
        if sample_rate_hz == 0 {
            return 0;
        }
        let frames = (self.total_duration_secs() * sample_rate_hz as f32).round();
        frames.clamp(0.0, MAX_TONE_FRAMES as f32) as usize
    }

    /// Synthesize the bounded PCM buffer for this tone.
    pub fn render_pcm(&self, sample_rate_hz: u32) -> PcmBuffer {
        if sample_rate_hz == 0 {
            return PcmBuffer {
                sample_rate_hz: 0,
                frames: Vec::new(),
            };
        }
        let frames = (0..self.frame_count(sample_rate_hz))
            .map(|index| {
                let t = index as f32 / sample_rate_hz as f32;
                self.sample_at(t).clamp(-1.0, 1.0)
            })
            .collect();
        PcmBuffer {
            sample_rate_hz,
            frames,
        }
    }

    /// Generate PCM sample at time `t` seconds with sample rate `sample_rate_hz`.
    pub fn sample_at(&self, t: f32) -> f32 {
        let amp = self.envelope.evaluate(t, self.duration_secs);
        let phase = 2.0 * PI * self.frequency_hz * t;
        amp * phase.sin()
    }
}

/// Global audio and haptics coordinator resource.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioHapticsSettings {
    pub haptics_enabled: bool,
    pub sound_enabled: bool,
    /// Master gate: when set, both channels are silenced regardless of the
    /// per-channel flags, which stay intact for when mute is lifted.
    pub muted: bool,
}

impl Default for AudioHapticsSettings {
    fn default() -> Self {
        Self {
            haptics_enabled: true,
            sound_enabled: true,
            muted: false,
        }
    }
}

impl AudioHapticsSettings {
    /// Settings with both channels silenced by the master gate.
    pub fn muted() -> Self {
        Self {
            muted: true,
            ..Self::default()
        }
    }

    /// Whether the master gate is engaged.
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Engage or lift the master gate without touching per-channel flags.
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Whether haptic output may fire right now.
    pub fn allows_haptics(&self) -> bool {
        !self.muted && self.haptics_enabled
    }

    /// Whether audio output may fire right now.
    pub fn allows_sound(&self) -> bool {
        !self.muted && self.sound_enabled
    }
}

/// A single multimodal feedback intent: one haptic pattern plus an optional tone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeedbackIntent {
    pub pattern: HapticPattern,
    pub tone: Option<ProceduralTone>,
}

impl FeedbackIntent {
    /// Haptics only, no audio.
    pub fn haptic_only(pattern: HapticPattern) -> Self {
        Self {
            pattern,
            tone: None,
        }
    }

    /// An explicit haptic pattern paired with an explicit tone.
    pub fn with_tone(pattern: HapticPattern, tone: ProceduralTone) -> Self {
        Self {
            pattern,
            tone: Some(tone),
        }
    }

    /// Canonical semantic mapping from a pattern to its combined intent.
    pub fn for_pattern(pattern: HapticPattern) -> Self {
        let tone = match pattern {
            HapticPattern::LightTick | HapticPattern::SelectionTick => {
                Some(ProceduralTone::selection_tick())
            }
            HapticPattern::MediumClick => Some(ProceduralTone::click()),
            HapticPattern::HeavyThud => None,
            HapticPattern::SuccessPulse => Some(ProceduralTone::success_chime()),
            HapticPattern::WarningDoublePulse => Some(ProceduralTone::warning_buzz()),
        };
        Self { pattern, tone }
    }

    /// Haptic duration in milliseconds.
    pub fn duration_ms(&self) -> u32 {
        self.pattern.duration_ms()
    }
}

/// Which channels a dispatch actually exercised after gating.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EmittedFeedback {
    pub haptic: bool,
    pub audio: bool,
}

/// Routes feedback intents to a platform port, honoring the global gate.
pub struct FeedbackDispatcher<'a> {
    output: &'a dyn AudioHapticOutput,
    settings: AudioHapticsSettings,
    sample_rate_hz: u32,
}

impl<'a> FeedbackDispatcher<'a> {
    pub fn new(output: &'a dyn AudioHapticOutput, settings: AudioHapticsSettings) -> Self {
        Self {
            output,
            settings,
            sample_rate_hz: DEFAULT_SAMPLE_RATE_HZ,
        }
    }

    /// Override the PCM sample rate used for tone synthesis.
    pub fn with_sample_rate(mut self, sample_rate_hz: u32) -> Self {
        self.sample_rate_hz = sample_rate_hz;
        self
    }

    /// Dispatch one intent; returns the channels that actually fired.
    pub fn dispatch(&self, intent: FeedbackIntent) -> EmittedFeedback {
        let mut emitted = EmittedFeedback::default();
        if self.settings.allows_haptics() {
            self.output.vibrate(intent.pattern.duration_ms());
            emitted.haptic = true;
        }
        if self.settings.allows_sound()
            && let Some(tone) = intent.tone
        {
            self.output.play_tone(&tone.render_pcm(self.sample_rate_hz));
            emitted.audio = true;
        }
        emitted
    }
}

/// Event triggering haptic and procedural audio feedback.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeedbackTriggerEvent {
    pub pattern: HapticPattern,
    pub play_sound: bool,
}

impl FeedbackTriggerEvent {
    /// Map the event into a combined intent, dropping audio when not requested.
    pub fn to_intent(&self) -> FeedbackIntent {
        if self.play_sound {
            FeedbackIntent::for_pattern(self.pattern)
        } else {
            FeedbackIntent::haptic_only(self.pattern)
        }
    }
}
