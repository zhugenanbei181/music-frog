//! Typed wire decoder and lifecycle-fenced Activity inset feed.

#[cfg(any(target_os = "android", test))]
use infiltrator_bevy_ui::surface::host_insets::WindowInsetsInputSender;
use infiltrator_bevy_ui::surface::host_insets::{EdgeInsets, WindowInsets};
use infiltrator_contract::error::{ErrorCode, Failure};
use serde::Deserialize;
#[cfg(target_os = "android")]
use std::sync::OnceLock;
#[cfg(any(target_os = "android", test))]
use std::sync::RwLock;
#[cfg(any(target_os = "android", test))]
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_WINDOW_INSETS_JSON_BYTES: usize = 4 * 1024;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ActivityWindowInsetsObservation {
    pub(crate) activity_generation: u64,
    pub(crate) insets: WindowInsets,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(any(target_os = "android", test))]
enum ActivityGeneration {
    AwaitingObservation,
    Observed(u64),
}

#[derive(Clone)]
#[cfg(any(target_os = "android", test))]
enum DriverInsetsEndpoint {
    AwaitingHost,
    Registered {
        endpoint_generation: u64,
        activity_generation: ActivityGeneration,
        sender: WindowInsetsInputSender,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(any(target_os = "android", test))]
pub(crate) enum InsetsDeliveryResult {
    AwaitingHost,
    Delivered,
    StaleActivity,
    ReceiverClosed,
}

#[cfg(any(target_os = "android", test))]
struct InsetsEndpointRegistry {
    endpoint: RwLock<DriverInsetsEndpoint>,
    next_endpoint_generation: AtomicU64,
}

#[cfg(any(target_os = "android", test))]
impl InsetsEndpointRegistry {
    fn new() -> Self {
        Self {
            endpoint: RwLock::new(DriverInsetsEndpoint::AwaitingHost),
            next_endpoint_generation: AtomicU64::new(1),
        }
    }

    /// Replace the current Bevy app's callback endpoint before Activity seeding.
    /// Recreated surfaces receive a new generation; callbacks never retain a
    /// sender to the retired app's channel.
    fn register(&self, sender: WindowInsetsInputSender) -> u64 {
        let endpoint_generation = self
            .next_endpoint_generation
            .fetch_add(1, Ordering::Relaxed);
        *self
            .endpoint
            .write()
            .expect("Bevy window-insets endpoint lock") = DriverInsetsEndpoint::Registered {
            endpoint_generation,
            activity_generation: ActivityGeneration::AwaitingObservation,
            sender,
        };
        endpoint_generation
    }

    /// Deliver a typed report from either the synchronous startup seed or the
    /// Activity's instance JNI callback. The Activity generation rejects late
    /// observations from an Activity that has already been recreated.
    fn publish(&self, observation: ActivityWindowInsetsObservation) -> InsetsDeliveryResult {
        let endpoint_snapshot = self
            .endpoint
            .read()
            .expect("Bevy window-insets endpoint lock")
            .clone();
        self.publish_from_snapshot(endpoint_snapshot, observation)
    }

    fn publish_from_snapshot(
        &self,
        endpoint_snapshot: DriverInsetsEndpoint,
        observation: ActivityWindowInsetsObservation,
    ) -> InsetsDeliveryResult {
        let DriverInsetsEndpoint::Registered {
            endpoint_generation,
            activity_generation,
            sender: _,
        } = endpoint_snapshot
        else {
            return InsetsDeliveryResult::AwaitingHost;
        };

        if matches!(activity_generation, ActivityGeneration::Observed(current) if observation.activity_generation < current)
        {
            return InsetsDeliveryResult::StaleActivity;
        }

        {
            let mut endpoint = self
                .endpoint
                .write()
                .expect("Bevy window-insets endpoint lock");
            let DriverInsetsEndpoint::Registered {
                endpoint_generation: current_endpoint_generation,
                activity_generation: current_activity_generation,
                sender: current_sender,
            } = &mut *endpoint
            else {
                return InsetsDeliveryResult::AwaitingHost;
            };

            // The endpoint can be replaced while the read snapshot is being
            // processed. Never deliver an old Activity callback to its new
            // receiver, and never let a retired callback clear that receiver.
            if *current_endpoint_generation != endpoint_generation {
                return InsetsDeliveryResult::StaleActivity;
            }
            if matches!(current_activity_generation, ActivityGeneration::Observed(current) if observation.activity_generation < *current)
            {
                return InsetsDeliveryResult::StaleActivity;
            }
            *current_activity_generation =
                ActivityGeneration::Observed(observation.activity_generation);
            let current_sender = current_sender.clone();

            match current_sender.send(observation.insets) {
                Ok(()) => InsetsDeliveryResult::Delivered,
                Err(_) => {
                    *endpoint = DriverInsetsEndpoint::AwaitingHost;
                    InsetsDeliveryResult::ReceiverClosed
                }
            }
        }
    }
}

#[cfg(target_os = "android")]
static INSETS_ENDPOINT: OnceLock<InsetsEndpointRegistry> = OnceLock::new();

#[cfg(target_os = "android")]
fn insets_endpoint() -> &'static InsetsEndpointRegistry {
    INSETS_ENDPOINT.get_or_init(InsetsEndpointRegistry::new)
}

/// Replace the current Bevy receiver before synchronously seeding the latest
/// Activity observation.
#[cfg(target_os = "android")]
pub(crate) fn register_window_insets_sender(sender: WindowInsetsInputSender) {
    insets_endpoint().register(sender);
}

/// Deliver one typed report from the startup seed or Activity JNI callback.
#[cfg(target_os = "android")]
pub(crate) fn publish_window_insets(
    observation: ActivityWindowInsetsObservation,
) -> InsetsDeliveryResult {
    insets_endpoint().publish(observation)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WindowInsetsWire {
    activity_generation: u64,
    density: f32,
    status_bar: EdgeInsetsWire,
    nav_bar: EdgeInsetsWire,
    cutout: EdgeInsetsWire,
    system_gestures: EdgeInsetsWire,
    ime: EdgeInsetsWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgeInsetsWire {
    top: f32,
    right: f32,
    bottom: f32,
    left: f32,
}

impl From<EdgeInsetsWire> for EdgeInsets {
    fn from(value: EdgeInsetsWire) -> Self {
        Self::new(value.top, value.bottom, value.left, value.right)
    }
}

/// Decode the current Activity observation and normalize it through the same
/// typed physical-pixel constructor used by the Bevy shell.
pub(crate) fn decode_window_insets(raw: &str) -> Result<ActivityWindowInsetsObservation, Failure> {
    if raw.len() > MAX_WINDOW_INSETS_JSON_BYTES {
        return Err(Failure::new(
            ErrorCode::InvalidInput,
            "Android Activity window insets exceed the bounded payload size",
            false,
        ));
    }
    let wire = serde_json::from_str::<WindowInsetsWire>(raw).map_err(|error| {
        Failure::new(
            ErrorCode::InvalidInput,
            format!("Android Activity returned invalid typed window insets: {error}"),
            false,
        )
    })?;
    Ok(ActivityWindowInsetsObservation {
        activity_generation: wire.activity_generation,
        insets: WindowInsets::physical(
            wire.density,
            wire.status_bar.into(),
            wire.nav_bar.into(),
            wire.cutout.into(),
            wire.system_gestures.into(),
            wire.ime.into(),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ActivityGeneration, ActivityWindowInsetsObservation, InsetsDeliveryResult,
        InsetsEndpointRegistry, decode_window_insets,
    };
    use infiltrator_bevy_ui::surface::host_insets::{
        EdgeInsets, WindowInsets, window_insets_input_channel,
    };

    #[test]
    fn activity_wire_preserves_generation_density_and_system_gesture_insets() {
        let report = decode_window_insets(
            r#"{
                "activity_generation":7,
                "density":3.0,
                "status_bar":{"top":72,"right":0,"bottom":0,"left":0},
                "nav_bar":{"top":0,"right":0,"bottom":48,"left":0},
                "cutout":{"top":0,"right":0,"bottom":0,"left":0},
                "system_gestures":{"top":0,"right":0,"bottom":96,"left":0},
                "ime":{"top":0,"right":0,"bottom":0,"left":0}
            }"#,
        )
        .expect("valid Activity insets");

        assert_eq!(report.activity_generation, 7);
        assert_eq!(
            report.insets,
            WindowInsets::physical(
                3.0,
                EdgeInsets::new(72.0, 0.0, 0.0, 0.0),
                EdgeInsets::new(0.0, 48.0, 0.0, 0.0),
                EdgeInsets::ZERO,
                EdgeInsets::new(0.0, 96.0, 0.0, 0.0),
                EdgeInsets::ZERO,
            )
        );
        assert_eq!(report.insets.permanent_safe_area().bottom_px, 32.0);
    }

    #[test]
    fn activity_wire_rejects_missing_generation_or_oversized_observation() {
        assert!(decode_window_insets(r#"{"density":3.0}"#).is_err());
        assert!(decode_window_insets(&" ".repeat(4097)).is_err());
    }

    #[test]
    fn startup_seed_and_callbacks_share_the_replaced_receiver_and_fence_old_activities() {
        let registry = InsetsEndpointRegistry::new();
        let (retired_sender, retired_receiver) = window_insets_input_channel();
        registry.register(retired_sender);
        drop(retired_receiver);

        let (current_sender, _current_receiver) = window_insets_input_channel();
        registry.register(current_sender);

        let current = ActivityWindowInsetsObservation {
            activity_generation: 12,
            insets: WindowInsets::default(),
        };
        assert_eq!(registry.publish(current), InsetsDeliveryResult::Delivered);
        // The first value is the startup seed; subsequent callbacks from this
        // same Activity generation continue on the replacement receiver.
        assert_eq!(
            registry.publish(ActivityWindowInsetsObservation {
                activity_generation: 12,
                insets: WindowInsets::physical(
                    3.0,
                    EdgeInsets::ZERO,
                    EdgeInsets::new(0.0, 48.0, 0.0, 0.0),
                    EdgeInsets::ZERO,
                    EdgeInsets::new(0.0, 96.0, 0.0, 0.0),
                    EdgeInsets::ZERO,
                ),
            }),
            InsetsDeliveryResult::Delivered
        );
        assert_eq!(
            registry.publish(ActivityWindowInsetsObservation {
                activity_generation: 11,
                insets: WindowInsets::default(),
            }),
            InsetsDeliveryResult::StaleActivity
        );
    }

    #[test]
    fn retired_receiver_snapshot_cannot_reach_or_clear_a_new_endpoint() {
        let registry = InsetsEndpointRegistry::new();
        let (retired_sender, retired_receiver) = window_insets_input_channel();
        registry.register(retired_sender);
        let retired_snapshot = registry
            .endpoint
            .read()
            .expect("Bevy window-insets endpoint lock")
            .clone();
        drop(retired_receiver);

        let (current_sender, _current_receiver) = window_insets_input_channel();
        registry.register(current_sender);

        let current = ActivityWindowInsetsObservation {
            activity_generation: 2,
            insets: WindowInsets::default(),
        };
        assert_eq!(
            registry.publish_from_snapshot(retired_snapshot, current),
            InsetsDeliveryResult::StaleActivity
        );
        assert_eq!(registry.publish(current), InsetsDeliveryResult::Delivered);
    }

    #[test]
    fn disconnected_current_receiver_is_retired_before_a_replacement_is_seeded() {
        let registry = InsetsEndpointRegistry::new();
        let (retired_sender, retired_receiver) = window_insets_input_channel();
        registry.register(retired_sender);
        drop(retired_receiver);
        let report = ActivityWindowInsetsObservation {
            activity_generation: 4,
            insets: WindowInsets::default(),
        };

        assert_eq!(
            registry.publish(report),
            InsetsDeliveryResult::ReceiverClosed
        );
        assert_eq!(registry.publish(report), InsetsDeliveryResult::AwaitingHost);

        let (replacement_sender, _replacement_receiver) = window_insets_input_channel();
        registry.register(replacement_sender);
        assert_eq!(registry.publish(report), InsetsDeliveryResult::Delivered);
    }

    #[test]
    fn registered_endpoint_starts_without_an_activity_generation() {
        let registry = InsetsEndpointRegistry::new();
        let (sender, _receiver) = window_insets_input_channel();
        registry.register(sender);
        let endpoint = registry
            .endpoint
            .read()
            .expect("Bevy window-insets endpoint lock")
            .clone();
        assert!(matches!(
            endpoint,
            super::DriverInsetsEndpoint::Registered {
                activity_generation: ActivityGeneration::AwaitingObservation,
                ..
            }
        ));
    }
}
