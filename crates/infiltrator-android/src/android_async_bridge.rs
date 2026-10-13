//! Request-ID correlation for asynchronous Kotlin host calls.
//!
//! Android's JNI host methods must return after dispatch. The application
//! future waits on this bounded, typed reply registry while Kotlin performs
//! the Messenger exchange on its bounded host executor.

use infiltrator_contract::vpn::VpnStopTarget;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(target_os = "android")]
use std::sync::OnceLock;
#[cfg(target_os = "android")]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use tokio::sync::oneshot;

#[cfg(target_os = "android")]
const PENDING_ANDROID_BRIDGE_CALLS: usize = 20;
#[cfg(target_os = "android")]
const FIRST_ANDROID_BRIDGE_REQUEST_ID: u64 = 1_u64 << 60;

#[cfg(target_os = "android")]
static NEXT_ANDROID_BRIDGE_REQUEST_ID: AtomicU64 = AtomicU64::new(FIRST_ANDROID_BRIDGE_REQUEST_ID);

#[cfg(target_os = "android")]
pub(crate) fn next_android_bridge_request_id() -> Option<AndroidBridgeRequestId> {
    NEXT_ANDROID_BRIDGE_REQUEST_ID
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            (current < i64::MAX as u64).then(|| current + 1)
        })
        .ok()
        .map(AndroidBridgeRequestId::new)
}

#[cfg(target_os = "android")]
pub(crate) fn pending_android_bridge_calls() -> &'static AndroidBridgePendingCalls {
    static PENDING: OnceLock<AndroidBridgePendingCalls> = OnceLock::new();
    PENDING.get_or_init(|| AndroidBridgePendingCalls::new(PENDING_ANDROID_BRIDGE_CALLS))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct AndroidBridgeRequestId(u64);

impl AndroidBridgeRequestId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    #[cfg(target_os = "android")]
    pub(crate) const fn value(self) -> u64 {
        self.0
    }

    pub(crate) const fn is_valid(self) -> bool {
        self.0 > 0
    }
}

/// Exhaustive typed host call sent from a Rust async operation to Kotlin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum AndroidBridgeCall {
    CoreStart {
        start_request_json: String,
    },
    CoreStop,
    CoreProcessObservation,
    CoreProcessRecoveryCompleted {
        context_json: String,
    },
    CoreValidateConfiguration {
        request_json: String,
    },
    CoreLifecycleSnapshot,
    ExecuteProductCommand {
        command_json: String,
    },
    SurfaceSnapshot,
    ExecuteOwnerOperation {
        operation_json: String,
    },
    CredentialGet {
        service: String,
        key: String,
    },
    CredentialSet {
        service: String,
        key: String,
        value: String,
    },
    CredentialDelete {
        service: String,
        key: String,
    },
    VpnStart,
    VpnAuthorizationState,
    VpnApplyConfiguration {
        prepared_json: String,
    },
    VpnStop {
        target: VpnStopTarget,
    },
    VpnOwnerSnapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AndroidCredentialMutationOutcome {
    Applied,
    Rejected,
}

/// Typed result variants mirror [`AndroidBridgeCall`] one-for-one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", content = "value", rename_all = "snake_case")]
pub(crate) enum AndroidBridgeCallReply {
    CoreStart {
        wire_code: i32,
    },
    CoreStop {
        wire_code: i32,
    },
    CoreProcessObservation(String),
    CoreProcessRecoveryCompleted {
        wire_code: i32,
    },
    CoreValidateConfiguration(String),
    CoreLifecycleSnapshot(String),
    ExecuteProductCommand(String),
    SurfaceSnapshot(String),
    ExecuteOwnerOperation(String),
    CredentialGet(Option<String>),
    CredentialSet {
        outcome: AndroidCredentialMutationOutcome,
    },
    CredentialDelete {
        outcome: AndroidCredentialMutationOutcome,
    },
    VpnStart {
        wire_code: i32,
    },
    VpnAuthorizationState {
        wire_code: i32,
    },
    VpnConfigurationApply {
        wire_code: i32,
    },
    VpnStop(String),
    VpnOwnerSnapshot(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub(crate) enum AndroidBridgeCallCompletion {
    Reply(AndroidBridgeCallReply),
    Failed(AndroidBridgeCallFailure),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", content = "message", rename_all = "snake_case")]
pub(crate) enum AndroidBridgeCallFailure {
    Host(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AndroidBridgeCallAdmission {
    DuplicateRequestId,
    CapacityExceeded,
    InvalidRequestId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AndroidBridgeCallCompletionResult {
    Delivered,
    CallerCanceled,
    UnknownRequest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AndroidBridgeCallCancellation {
    Canceled,
    UnknownRequest,
}

#[cfg(target_os = "android")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AndroidBridgeDispatchAcceptance {
    Accepted,
    QueueFull,
    DuplicateRequestId,
    HostUnavailable,
    InvalidRequest,
}

#[cfg(target_os = "android")]
impl AndroidBridgeDispatchAcceptance {
    pub(crate) const fn from_wire_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Accepted),
            1 => Some(Self::QueueFull),
            2 => Some(Self::DuplicateRequestId),
            3 => Some(Self::HostUnavailable),
            4 => Some(Self::InvalidRequest),
            _ => None,
        }
    }
}

/// Bounded correlation table; Android host calls never share a process-local
/// Core session, only their request-ID completion channel.
pub(crate) struct AndroidBridgePendingCalls {
    capacity: usize,
    pending: Mutex<HashMap<AndroidBridgeRequestId, oneshot::Sender<AndroidBridgeCallCompletion>>>,
}

impl AndroidBridgePendingCalls {
    pub(crate) fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "pending call capacity must be positive");
        Self {
            capacity,
            pending: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn admit(
        &self,
        request_id: AndroidBridgeRequestId,
    ) -> Result<oneshot::Receiver<AndroidBridgeCallCompletion>, AndroidBridgeCallAdmission> {
        if !request_id.is_valid() {
            return Err(AndroidBridgeCallAdmission::InvalidRequestId);
        }
        let (sender, receiver) = oneshot::channel();
        let mut pending = self.lock_pending();
        if pending.contains_key(&request_id) {
            return Err(AndroidBridgeCallAdmission::DuplicateRequestId);
        }
        if pending.len() >= self.capacity {
            return Err(AndroidBridgeCallAdmission::CapacityExceeded);
        }
        pending.insert(request_id, sender);
        Ok(receiver)
    }

    pub(crate) fn complete(
        &self,
        request_id: AndroidBridgeRequestId,
        completion: AndroidBridgeCallCompletion,
    ) -> AndroidBridgeCallCompletionResult {
        let Some(sender) = self.lock_pending().remove(&request_id) else {
            return AndroidBridgeCallCompletionResult::UnknownRequest;
        };
        match sender.send(completion) {
            Ok(()) => AndroidBridgeCallCompletionResult::Delivered,
            Err(_) => AndroidBridgeCallCompletionResult::CallerCanceled,
        }
    }

    pub(crate) fn cancel(
        &self,
        request_id: AndroidBridgeRequestId,
    ) -> AndroidBridgeCallCancellation {
        match self.lock_pending().remove(&request_id) {
            Some(sender) => {
                drop(sender);
                AndroidBridgeCallCancellation::Canceled
            }
            None => AndroidBridgeCallCancellation::UnknownRequest,
        }
    }

    #[cfg(test)]
    pub(crate) fn pending_count(&self) -> usize {
        self.lock_pending().len()
    }

    fn lock_pending(
        &self,
    ) -> MutexGuard<'_, HashMap<AndroidBridgeRequestId, oneshot::Sender<AndroidBridgeCallCompletion>>>
    {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_id(value: u64) -> AndroidBridgeRequestId {
        AndroidBridgeRequestId::new(value)
    }

    #[tokio::test]
    async fn request_and_reply_wire_round_trip_keep_operation_variants_typed() {
        let call = AndroidBridgeCall::CredentialSet {
            service: "core".to_owned(),
            key: "secret".to_owned(),
            value: "value".to_owned(),
        };
        let encoded = serde_json::to_string(&call).expect("request serializes");
        assert!(encoded.contains("\"operation\":\"credential_set\""));
        let decoded: AndroidBridgeCall =
            serde_json::from_str(&encoded).expect("request decodes exhaustively");
        assert_eq!(decoded, call);

        let reply =
            AndroidBridgeCallCompletion::Reply(AndroidBridgeCallReply::VpnStart { wire_code: 0 });
        let encoded = serde_json::to_string(&reply).expect("reply serializes");
        let decoded: AndroidBridgeCallCompletion =
            serde_json::from_str(&encoded).expect("reply decodes exhaustively");
        assert_eq!(decoded, reply);

        let unit_request = serde_json::to_value(AndroidBridgeCall::CoreStop)
            .expect("unit request serializes with an explicit payload");
        assert_eq!(unit_request["operation"], "core_stop");
        assert!(unit_request["payload"].is_null());

        let stop_request = serde_json::to_value(AndroidBridgeCall::VpnStop {
            target: VpnStopTarget::Current,
        })
        .expect("VPN stop target serializes as its typed variant");
        assert!(stop_request["payload"]["target"]["Current"].is_null());
    }

    #[tokio::test]
    async fn pending_calls_correlate_out_of_order_replies_by_request_id() {
        let pending = AndroidBridgePendingCalls::new(2);
        let first = pending.admit(request_id(31)).expect("first admitted");
        let second = pending.admit(request_id(32)).expect("second admitted");

        assert_eq!(
            pending.complete(
                request_id(32),
                AndroidBridgeCallCompletion::Reply(AndroidBridgeCallReply::CredentialSet {
                    outcome: AndroidCredentialMutationOutcome::Applied,
                }),
            ),
            AndroidBridgeCallCompletionResult::Delivered
        );
        assert_eq!(
            second.await.expect("matching completion"),
            AndroidBridgeCallCompletion::Reply(AndroidBridgeCallReply::CredentialSet {
                outcome: AndroidCredentialMutationOutcome::Applied,
            })
        );
        assert_eq!(
            pending.complete(
                request_id(31),
                AndroidBridgeCallCompletion::Failed(AndroidBridgeCallFailure::Host(
                    "owner unavailable".to_owned(),
                )),
            ),
            AndroidBridgeCallCompletionResult::Delivered
        );
        assert_eq!(
            first.await.expect("matching failure"),
            AndroidBridgeCallCompletion::Failed(AndroidBridgeCallFailure::Host(
                "owner unavailable".to_owned(),
            ))
        );
        assert_eq!(pending.pending_count(), 0);
    }

    #[test]
    fn admission_rejects_duplicate_zero_and_over_capacity_ids() {
        let pending = AndroidBridgePendingCalls::new(1);
        let _first = pending.admit(request_id(41)).expect("first admitted");
        assert_eq!(
            pending.admit(request_id(41)).err(),
            Some(AndroidBridgeCallAdmission::DuplicateRequestId)
        );
        assert_eq!(
            pending.admit(request_id(42)).err(),
            Some(AndroidBridgeCallAdmission::CapacityExceeded)
        );
        assert_eq!(
            pending.admit(request_id(0)).err(),
            Some(AndroidBridgeCallAdmission::InvalidRequestId)
        );
    }

    #[tokio::test]
    async fn cancellation_and_late_reply_cannot_resurrect_a_pending_call() {
        let pending = AndroidBridgePendingCalls::new(1);
        let receiver = pending.admit(request_id(51)).expect("request admitted");
        assert_eq!(
            pending.cancel(request_id(51)),
            AndroidBridgeCallCancellation::Canceled
        );
        assert!(
            receiver.await.is_err(),
            "canceled request resolves as canceled"
        );
        assert_eq!(
            pending.complete(
                request_id(51),
                AndroidBridgeCallCompletion::Reply(AndroidBridgeCallReply::CredentialDelete {
                    outcome: AndroidCredentialMutationOutcome::Applied,
                }),
            ),
            AndroidBridgeCallCompletionResult::UnknownRequest
        );
        assert_eq!(pending.pending_count(), 0);
    }

    #[test]
    fn typed_wire_rejects_unknown_operation_variants_and_fields() {
        assert!(
            serde_json::from_str::<AndroidBridgeCall>(r#"{"operation":"launch_core"}"#).is_err()
        );
        assert!(
            serde_json::from_str::<AndroidBridgeCall>(
                r#"{"operation":"core_stop","payload":null,"started":true}"#
            )
            .is_err()
        );
    }

    #[test]
    fn kotlin_completion_shapes_decode_as_typed_success_and_failure() {
        let mutation: AndroidBridgeCallCompletion = serde_json::from_str(
            r#"{"state":"reply","value":{"result":"credential_set","value":{"outcome":"applied"}}}"#,
        )
        .expect("Kotlin's credential write reply decodes as a typed result");
        assert_eq!(
            mutation,
            AndroidBridgeCallCompletion::Reply(AndroidBridgeCallReply::CredentialSet {
                outcome: AndroidCredentialMutationOutcome::Applied,
            })
        );

        let failure: AndroidBridgeCallCompletion = serde_json::from_str(
            r#"{"state":"failed","value":{"reason":"host","message":"owner unavailable"}}"#,
        )
        .expect("Kotlin's host failure decodes as a typed failure");
        assert_eq!(
            failure,
            AndroidBridgeCallCompletion::Failed(AndroidBridgeCallFailure::Host(
                "owner unavailable".to_owned()
            ))
        );

        let delete_rejected: AndroidBridgeCallCompletion = serde_json::from_str(
            r#"{"state":"reply","value":{"result":"credential_delete","value":{"outcome":"rejected"}}}"#,
        )
        .expect("Kotlin's credential delete rejection decodes as a typed result");
        assert_eq!(
            delete_rejected,
            AndroidBridgeCallCompletion::Reply(AndroidBridgeCallReply::CredentialDelete {
                outcome: AndroidCredentialMutationOutcome::Rejected,
            })
        );
    }
}
