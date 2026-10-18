use crate::android_async_bridge::AndroidBridgeCallCompletionResult::CallerCanceled;
use crate::android_async_bridge::AndroidBridgeCallCompletionResult::Delivered;
use crate::android_async_bridge::AndroidBridgeCallCompletionResult::UnknownRequest;
use crate::android_async_bridge::{
    AndroidBridgeCall, AndroidBridgeCallCompletion, AndroidBridgeCallFailure,
    AndroidBridgeCallReply, AndroidBridgeDispatchAcceptance, AndroidBridgeRequestId,
    AndroidCredentialMutationOutcome, next_android_bridge_request_id, pending_android_bridge_calls,
};
use crate::jni_clipboard::JniClipboardChannel;
use crate::native_host::clipboard::clear_native_clipboard_channel;
use crate::native_host::clipboard::set_native_clipboard_channel;
use crate::native_host::declare_touch_host;
use crate::native_host::insets::RawInsetsPx;
use crate::native_host::lifecycle::ActivityPhase;
use crate::native_host::lifecycle::HostLifecycleFact;
use crate::native_host::lifecycle::LifecycleUpdate;
use crate::native_host::lifecycle::WindowFocus;
use crate::native_host::push_lifecycle_fact;
use crate::native_host::push_raw_insets;
use crate::service_init::init_android_process;
use crate::uniffi_api::profile_age_credential_owner::{
    PROFILE_AGE_CREDENTIAL_OWNER_PROTOCOL_V2, ProfileAgeCredentialOwnerWireOutcome,
    ProfileAgeCredentialOwnerWireReply, ProfileAgeCredentialOwnerWireRequest,
};
use crate::{FfiErrorCode, FfiStatus};
use infiltrator_contract::android_control::core_launch::CoreLaunchSpec;
use infiltrator_contract::android_control::{
    AndroidCoreLifecycleSnapshot, AndroidCoreProcessObservation, AndroidCoreProcessRecoveryReceipt,
    AndroidHostProcessRole, VpnAuthorizationState, VpnOwnerSnapshot,
};
use infiltrator_contract::profile_age_credential::{
    ProfileAgeCredentialOwnerReadback, ProfileAgeCredentialOwnerSubmission,
};
use infiltrator_contract::vpn::VpnConfiguration;
use infiltrator_contract::vpn::{
    PreparedVpnConfiguration, VpnConfigurationApplyOutcome, VpnOwnerStopReceipt, VpnStartOutcome,
    VpnStopTarget,
};
use infiltrator_platform::android_bridge::{
    AndroidBridge, AndroidBridgeResult, AndroidCoreConfigurationValidationOutcome,
    AndroidCoreConfigurationValidationReply, AndroidCoreConfigurationValidationRequest,
    CoreProcessStartReceipt, CoreProcessStopReceipt, set_android_bridge,
};
use jni::errors::LogErrorAndDefault;
use jni::objects::{Global, JObject, JString, JValue, Reference as _};
use jni::signature::RuntimeMethodSignature;
use jni::strings::JNIString;
use jni::sys::{jfloat, jint, jlong, jstring};
use jni::{Env, EnvUnowned, JavaVM, errors};
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_ports::error::PortError;
use std::path::PathBuf;
use std::result;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

const SIG_NOARGS_INT: &str = "()I";
const SIG_NOARGS_STRING: &str = "()Ljava/lang/String;";
const SIG_BRIDGE_DISPATCH: &str = "(JLjava/lang/String;J)I";
const SIG_BRIDGE_AGE_CREDENTIAL_DISPATCH: &str = "(JLjava/lang/String;[BJ)I";
const SIG_BRIDGE_CANCEL: &str = "(J)I";
const BRIDGE_CALL_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(serde::Serialize)]
struct PreparedVpnConfigurationWire<'a> {
    preparation_id: u64,
    vpn_generation: u64,
    owner_data_revision: u64,
    core_scope: PreparedVpnCoreScopeWire<'a>,
    configuration: &'a VpnConfiguration,
}

#[derive(serde::Serialize)]
struct PreparedVpnCoreScopeWire<'a> {
    engine_id: &'a str,
    instance_id: &'a str,
    version: Option<&'a str>,
    generation: u64,
    session_token: String,
}

pub(super) struct JniBridge {
    pub(super) vm: JavaVM,
    pub(super) host: Global<JObject<'static>>,
}

impl JniBridge {
    pub(super) fn new(vm: JavaVM, host: Global<JObject<'static>>) -> Self {
        Self { vm, host }
    }

    pub(super) fn call_int(
        &self,
        method: &str,
        sig: &str,
        args: &[JValue],
    ) -> AndroidBridgeResult<i32> {
        let name = JNIString::new(method);
        let signature = parse_method_signature(method, sig)?;
        let signature = signature.method_signature();
        self.vm
            .attach_current_thread(|env| -> errors::Result<i32> {
                env.call_method(self.host.as_obj(), &name, &signature, args)?
                    .i()
            })
            .map_err(|err| map_jni_error(method, err))
    }

    pub(super) fn call_string(
        &self,
        method: &str,
        sig: &str,
        args: &[JValue],
    ) -> AndroidBridgeResult<Option<String>> {
        let name = JNIString::new(method);
        let signature = parse_method_signature(method, sig)?;
        let signature = signature.method_signature();
        self.vm
            .attach_current_thread(|env| -> errors::Result<Option<String>> {
                let value = env.call_method(self.host.as_obj(), &name, &signature, args)?;
                let obj = value.l()?;
                if obj.is_null() {
                    return Ok(None);
                }
                let text = JString::cast_local(env, obj)?.try_to_string(env)?;
                Ok(Some(text))
            })
            .map_err(|err| map_jni_error(method, err))
    }

    pub(super) fn dispatch_android_bridge_call(
        &self,
        request_id: AndroidBridgeRequestId,
        request_json: &str,
        timeout: Duration,
    ) -> AndroidBridgeResult<i32> {
        let method = "dispatchAndroidBridgeCall";
        let name = JNIString::new(method);
        let signature = parse_method_signature(method, SIG_BRIDGE_DISPATCH)?;
        let signature = signature.method_signature();
        let timeout_millis = i64::try_from(timeout.as_millis()).map_err(|_| {
            PortError::Failed("Android host-call timeout exceeds the JNI Long range".to_owned())
        })?;
        self.vm
            .attach_current_thread(|env| -> errors::Result<i32> {
                let request_json = env.new_string(request_json)?;
                let arguments = [
                    JValue::Long(request_id.value() as jlong),
                    JValue::Object(request_json.as_ref()),
                    JValue::Long(timeout_millis as jlong),
                ];
                env.call_method(self.host.as_obj(), &name, &signature, &arguments)?
                    .i()
            })
            .map_err(|error| map_jni_error(method, error))
    }

    pub(super) fn dispatch_profile_age_credential_operation(
        &self,
        request_id: AndroidBridgeRequestId,
        metadata_json: &str,
        private_key_bytes: &[u8],
        timeout: Duration,
    ) -> AndroidBridgeResult<i32> {
        let method = "dispatchProfileAgeCredentialOwnerOperation";
        let name = JNIString::new(method);
        let signature = parse_method_signature(method, SIG_BRIDGE_AGE_CREDENTIAL_DISPATCH)?;
        let timeout_millis = i64::try_from(timeout.as_millis()).map_err(|_| {
            PortError::Failed("Android host-call timeout exceeds the JNI Long range".to_owned())
        })?;
        self.vm
            .attach_current_thread(|env| -> errors::Result<i32> {
                let metadata = env.new_string(metadata_json)?;
                let private_key = env.byte_array_from_slice(private_key_bytes)?;
                let zeroed = vec![0_i8; private_key_bytes.len()];
                let arguments = [
                    JValue::Long(request_id.value() as jlong),
                    JValue::Object(metadata.as_ref()),
                    JValue::Object(private_key.as_ref()),
                    JValue::Long(timeout_millis as jlong),
                ];
                let admission = match env.call_method(
                    self.host.as_obj(),
                    &name,
                    signature.method_signature(),
                    &arguments,
                ) {
                    Ok(value) => value.i(),
                    Err(error) => {
                        if env.exception_check() {
                            env.exception_clear();
                        }
                        #[allow(deprecated)]
                        env.set_byte_array_region(&private_key, 0, &zeroed)?;
                        return Err(error);
                    }
                };
                match admission {
                    Ok(code) if code == 0 => Ok(code),
                    Ok(code) => {
                        #[allow(deprecated)]
                        env.set_byte_array_region(&private_key, 0, &zeroed)?;
                        Ok(code)
                    }
                    Err(error) => {
                        if env.exception_check() {
                            env.exception_clear();
                        }
                        #[allow(deprecated)]
                        env.set_byte_array_region(&private_key, 0, &zeroed)?;
                        Err(error)
                    }
                }
            })
            .map_err(|error| map_jni_error(method, error))
    }

    pub(super) async fn invoke_profile_age_credential_owner_operation(
        &self,
        metadata_json: &str,
        private_key_bytes: &[u8],
    ) -> AndroidBridgeResult<String> {
        let request_id = next_android_bridge_request_id().ok_or_else(|| {
            PortError::Failed("Android host-call request ID space is exhausted".to_owned())
        })?;
        let receiver = pending_android_bridge_calls()
            .admit(request_id)
            .map_err(|admission| {
                PortError::Failed(format!(
                    "Android host-call request admission failed: {admission:?}"
                ))
            })?;
        let mut guard = AndroidBridgeCallGuard::new(self, request_id);
        let admission_code = self.dispatch_profile_age_credential_operation(
            request_id,
            metadata_json,
            private_key_bytes,
            BRIDGE_CALL_TIMEOUT,
        )?;
        match AndroidBridgeDispatchAcceptance::from_wire_code(admission_code) {
            Some(AndroidBridgeDispatchAcceptance::Accepted) => {}
            Some(AndroidBridgeDispatchAcceptance::QueueFull) => {
                guard.retire();
                pending_android_bridge_calls().cancel(request_id);
                return Err(PortError::Failed(
                    "Android host-call executor is at capacity".to_owned(),
                ));
            }
            Some(AndroidBridgeDispatchAcceptance::DuplicateRequestId) => {
                guard.retire();
                pending_android_bridge_calls().cancel(request_id);
                return Err(PortError::Failed(
                    "Android host-call request ID is already pending".to_owned(),
                ));
            }
            Some(AndroidBridgeDispatchAcceptance::HostUnavailable) => {
                guard.retire();
                pending_android_bridge_calls().cancel(request_id);
                return Err(PortError::Failed(
                    "Android host-call dispatcher is unavailable".to_owned(),
                ));
            }
            Some(AndroidBridgeDispatchAcceptance::InvalidRequest) | None => {
                guard.retire();
                pending_android_bridge_calls().cancel(request_id);
                return Err(PortError::Failed(
                    "Android host rejected the Age credential request".to_owned(),
                ));
            }
        }
        let completion = match timeout(BRIDGE_CALL_TIMEOUT, receiver).await {
            Ok(Ok(completion)) => completion,
            Ok(Err(_)) => {
                guard.retire();
                return Err(PortError::Failed(
                    "Android host-call completion channel closed".to_owned(),
                ));
            }
            Err(_) => {
                return Err(PortError::Failed(
                    "Android Age credential request exceeded its deadline".to_owned(),
                ));
            }
        };
        guard.retire();
        match completion {
            AndroidBridgeCallCompletion::Reply(
                AndroidBridgeCallReply::ProfileAgeCredentialOwnerOperation(reply),
            ) => Ok(reply),
            AndroidBridgeCallCompletion::Reply(_) => Err(unexpected_bridge_reply(
                "ProfileAgeCredentialOwnerOperation",
            )),
            AndroidBridgeCallCompletion::Failed(AndroidBridgeCallFailure::Host(message)) => {
                Err(PortError::Failed(message))
            }
        }
    }

    pub(super) fn cancel_android_bridge_call(&self, request_id: AndroidBridgeRequestId) {
        let method = "cancelAndroidBridgeCall";
        let name = JNIString::new(method);
        let signature = match parse_method_signature(method, SIG_BRIDGE_CANCEL) {
            Ok(signature) => signature,
            Err(_) => return,
        };
        let _ = self.vm.attach_current_thread(|env| -> errors::Result<()> {
            let arguments = [JValue::Long(request_id.value() as jlong)];
            env.call_method(
                self.host.as_obj(),
                &name,
                signature.method_signature(),
                &arguments,
            )?
            .i()?;
            Ok(())
        });
    }

    pub(super) async fn invoke_android_bridge_call(
        &self,
        call: AndroidBridgeCall,
    ) -> AndroidBridgeResult<AndroidBridgeCallReply> {
        let request_id = next_android_bridge_request_id().ok_or_else(|| {
            PortError::Failed("Android host-call request ID space is exhausted".to_owned())
        })?;
        let request_json = serde_json::to_string(&call).map_err(|error| {
            PortError::Failed(format!(
                "Android host call could not be serialized: {error}"
            ))
        })?;
        let receiver = pending_android_bridge_calls()
            .admit(request_id)
            .map_err(|admission| {
                PortError::Failed(format!(
                    "Android host-call request admission failed: {admission:?}"
                ))
            })?;
        let mut guard = AndroidBridgeCallGuard::new(self, request_id);

        let admission_code =
            self.dispatch_android_bridge_call(request_id, &request_json, BRIDGE_CALL_TIMEOUT)?;
        let admission = AndroidBridgeDispatchAcceptance::from_wire_code(admission_code)
            .ok_or_else(|| {
                PortError::Failed(format!(
                    "Android host returned unknown dispatch acceptance code {admission_code}"
                ))
            })?;
        match admission {
            AndroidBridgeDispatchAcceptance::Accepted => {}
            AndroidBridgeDispatchAcceptance::QueueFull => {
                guard.retire();
                pending_android_bridge_calls().cancel(request_id);
                return Err(PortError::Failed(
                    "Android host-call executor is at capacity".to_owned(),
                ));
            }
            AndroidBridgeDispatchAcceptance::DuplicateRequestId => {
                guard.retire();
                pending_android_bridge_calls().cancel(request_id);
                return Err(PortError::Failed(
                    "Android host-call request ID is already pending".to_owned(),
                ));
            }
            AndroidBridgeDispatchAcceptance::HostUnavailable => {
                guard.retire();
                pending_android_bridge_calls().cancel(request_id);
                return Err(PortError::Failed(
                    "Android host-call dispatcher is unavailable".to_owned(),
                ));
            }
            AndroidBridgeDispatchAcceptance::InvalidRequest => {
                guard.retire();
                pending_android_bridge_calls().cancel(request_id);
                return Err(PortError::Failed(
                    "Android host rejected a malformed typed call".to_owned(),
                ));
            }
        }

        let completion = match timeout(BRIDGE_CALL_TIMEOUT, receiver).await {
            Ok(Ok(completion)) => completion,
            Ok(Err(_)) => {
                guard.retire();
                return Err(PortError::Failed(
                    "Android host-call completion channel closed".to_owned(),
                ));
            }
            Err(_) => {
                return Err(PortError::Failed(
                    "Android host call exceeded its end-to-end deadline".to_owned(),
                ));
            }
        };
        guard.retire();
        match completion {
            AndroidBridgeCallCompletion::Reply(reply) => Ok(reply),
            AndroidBridgeCallCompletion::Failed(failure) => Err(match failure {
                AndroidBridgeCallFailure::Host(message) => PortError::Failed(message),
            }),
        }
    }
}

#[path = "jni_bridge/bridge_impl.rs"]
mod bridge_impl;

pub(super) fn unexpected_bridge_reply(expected: &str) -> PortError {
    PortError::Failed(format!(
        "Android host returned a reply variant different from {expected}"
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AndroidBridgeCallGuardState {
    AwaitingReply,
    Retired,
}

struct AndroidBridgeCallGuard<'a> {
    bridge: &'a JniBridge,
    request_id: AndroidBridgeRequestId,
    state: AndroidBridgeCallGuardState,
}

impl<'a> AndroidBridgeCallGuard<'a> {
    fn new(bridge: &'a JniBridge, request_id: AndroidBridgeRequestId) -> Self {
        Self {
            bridge,
            request_id,
            state: AndroidBridgeCallGuardState::AwaitingReply,
        }
    }

    fn retire(&mut self) {
        self.state = AndroidBridgeCallGuardState::Retired;
    }
}

impl Drop for AndroidBridgeCallGuard<'_> {
    fn drop(&mut self) {
        if self.state == AndroidBridgeCallGuardState::AwaitingReply {
            pending_android_bridge_calls().cancel(self.request_id);
            self.bridge.cancel_android_bridge_call(self.request_id);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_musicfrog_infiltrator_RustBridge_nativePing<'local>(
    mut env: EnvUnowned<'local>,
    _object: JObject<'local>,
) -> jstring {
    env.with_env(|env| -> errors::Result<jstring> { Ok(env.new_string("ok")?.into_raw()) })
        .resolve::<LogErrorAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_musicfrog_infiltrator_RustBridge_nativeInit<'local>(
    mut env: EnvUnowned<'local>,
    _object: JObject<'local>,
    data_dir: JString<'local>,
    cache_dir: JString<'local>,
) -> jint {
    let mut status = FfiStatus::ok();
    env.with_env(|env| -> errors::Result<()> {
        status = init_dirs(env, &data_dir, &cache_dir);
        Ok(())
    })
    .resolve::<LogErrorAndDefault>();
    status.code as jint
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_musicfrog_infiltrator_RustBridge_nativeRegisterBridge<'local>(
    mut env: EnvUnowned<'local>,
    _object: JObject<'local>,
    host: JObject<'local>,
) -> jint {
    let mut status = FfiStatus::ok();
    env.with_env(|env| -> errors::Result<()> {
        status = register_bridge(env, host);
        Ok(())
    })
    .resolve::<LogErrorAndDefault>();
    status.code as jint
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_musicfrog_infiltrator_RustBridge_nativeCompleteAndroidBridgeCall<
    'local,
>(
    mut env: EnvUnowned<'local>,
    _object: JObject<'local>,
    request_id: jlong,
    completion_json: JString<'local>,
) -> jint {
    let mut result = 3;
    env.with_env(|env| -> errors::Result<()> {
        if request_id <= 0 {
            result = 4;
            return Ok(());
        }
        let encoded = match read_java_string(env, &completion_json, "Android bridge completion") {
            Ok(encoded) => encoded,
            Err(_) => {
                result = 4;
                return Ok(());
            }
        };
        let completion = match serde_json::from_str::<AndroidBridgeCallCompletion>(&encoded) {
            Ok(completion) => completion,
            Err(_) => {
                result = 4;
                return Ok(());
            }
        };
        result = match pending_android_bridge_calls()
            .complete(AndroidBridgeRequestId::new(request_id as u64), completion)
        {
            Delivered => 0,
            CallerCanceled => 1,
            UnknownRequest => 2,
        };
        Ok(())
    })
    .resolve::<LogErrorAndDefault>();
    result
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_musicfrog_infiltrator_RustBridge_nativeRegisterNativeHost<
    'local,
>(
    mut env: EnvUnowned<'local>,
    _object: JObject<'local>,
    host: JObject<'local>,
) -> jint {
    let mut status = FfiStatus::ok();
    env.with_env(|env| -> errors::Result<()> {
        status = register_native_host(env, host);
        Ok(())
    })
    .resolve::<LogErrorAndDefault>();
    status.code as jint
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_musicfrog_infiltrator_RustBridge_nativeClearNativeHost<'local>(
    mut env: EnvUnowned<'local>,
    _object: JObject<'local>,
) -> jint {
    // Release the retired Activity's global ref. The pushed lifecycle/insets
    // facts are plain state overwritten by the next Activity, so they are not
    // cleared here (another live UI Activity may own them).
    env.with_env(|_env| -> errors::Result<()> {
        clear_native_clipboard_channel();
        Ok(())
    })
    .resolve::<LogErrorAndDefault>();
    FfiErrorCode::Ok as jint
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_musicfrog_infiltrator_RustBridge_nativeOnLifecycle<'local>(
    mut env: EnvUnowned<'local>,
    _object: JObject<'local>,
    generation: jlong,
    phase: jint,
    focus: jint,
) -> jint {
    let mut status = FfiErrorCode::InvalidState as jint;
    env.with_env(|_env| -> errors::Result<()> {
        let Some(phase) = ActivityPhase::from_code(phase) else {
            status = FfiErrorCode::InvalidInput as jint;
            return Ok(());
        };
        let Some(focus) = WindowFocus::from_code(focus) else {
            status = FfiErrorCode::InvalidInput as jint;
            return Ok(());
        };
        if generation < 0 {
            status = FfiErrorCode::InvalidInput as jint;
            return Ok(());
        }
        let fact = HostLifecycleFact {
            generation: generation as u64,
            phase,
            focus,
        };
        status = match push_lifecycle_fact(fact) {
            LifecycleUpdate::Applied | LifecycleUpdate::Recreated => FfiErrorCode::Ok as jint,
            LifecycleUpdate::Stale | LifecycleUpdate::Rejected { .. } => {
                FfiErrorCode::InvalidState as jint
            }
        };
        Ok(())
    })
    .resolve::<LogErrorAndDefault>();
    status
}

/// BANDROID-006: push one `WindowInsetsCompat` observation (physical pixels).
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_musicfrog_infiltrator_RustBridge_nativeOnInsets<'local>(
    mut env: EnvUnowned<'local>,
    _object: JObject<'local>,
    density: jfloat,
    system_top: jint,
    system_right: jint,
    system_bottom: jint,
    system_left: jint,
    ime_top: jint,
    ime_right: jint,
    ime_bottom: jint,
    ime_left: jint,
) -> jint {
    let mut status = FfiErrorCode::InvalidState as jint;
    env.with_env(|_env| -> errors::Result<()> {
        let raw = RawInsetsPx::from_edges(
            density,
            system_top,
            system_right,
            system_bottom,
            system_left,
            ime_top,
            ime_right,
            ime_bottom,
            ime_left,
        );
        let _ = push_raw_insets(raw);
        status = FfiErrorCode::Ok as jint;
        Ok(())
    })
    .resolve::<LogErrorAndDefault>();
    status
}

fn init_dirs(env: &mut Env, data_dir: &JString, cache_dir: &JString) -> FfiStatus {
    let data_dir = match read_java_string(env, data_dir, "dataDir") {
        Ok(value) => value,
        Err(status) => return status,
    };
    let cache_dir = match read_java_string(env, cache_dir, "cacheDir") {
        Ok(value) => value,
        Err(status) => return status,
    };

    if data_dir.trim().is_empty() || cache_dir.trim().is_empty() {
        return FfiStatus::err(FfiErrorCode::InvalidInput, "dir is empty");
    }

    // The same typed seam the `:vpn` service process calls directly: install
    // the rustls provider and the process-local home override without any
    // Activity having run.
    match init_android_process(data_dir, cache_dir) {
        Ok(_) => FfiStatus::ok(),
        Err(error) => FfiStatus::err(FfiErrorCode::InvalidInput, error.to_string()),
    }
}

fn register_bridge(env: &mut Env, host: JObject) -> FfiStatus {
    if host.is_null() {
        return FfiStatus::err(FfiErrorCode::InvalidInput, "host is null");
    }

    let vm = match env.get_java_vm() {
        Ok(vm) => vm,
        Err(err) => {
            return FfiStatus::err(
                FfiErrorCode::InvalidState,
                format!("get java vm failed: {err}"),
            );
        }
    };

    let global = match env.new_global_ref(host) {
        Ok(global) => global,
        Err(err) => {
            return FfiStatus::err(
                FfiErrorCode::InvalidState,
                format!("create global ref failed: {err}"),
            );
        }
    };

    let bridge = JniBridge::new(vm, global);
    set_android_bridge(Arc::new(bridge));
    FfiStatus::ok()
}

/// BANDROID-005..008: register the Activity's native host adapter. It becomes
/// the process clipboard channel and declares that this host delivers touch.
fn register_native_host(env: &mut Env, host: JObject) -> FfiStatus {
    if host.is_null() {
        return FfiStatus::err(FfiErrorCode::InvalidInput, "native host is null");
    }

    let vm = match env.get_java_vm() {
        Ok(vm) => vm,
        Err(err) => {
            return FfiStatus::err(
                FfiErrorCode::InvalidState,
                format!("get java vm failed: {err}"),
            );
        }
    };

    let global = match env.new_global_ref(host) {
        Ok(global) => global,
        Err(err) => {
            return FfiStatus::err(
                FfiErrorCode::InvalidState,
                format!("create native host global ref failed: {err}"),
            );
        }
    };

    set_native_clipboard_channel(Arc::new(JniClipboardChannel::new(vm, global)));
    declare_touch_host();
    FfiStatus::ok()
}

fn read_java_string(
    env: &mut Env,
    input: &JString,
    label: &str,
) -> result::Result<String, FfiStatus> {
    if input.is_null() {
        return Err(FfiStatus::err(
            FfiErrorCode::InvalidInput,
            format!("{label} is null"),
        ));
    }
    input.try_to_string(env).map_err(|err| {
        FfiStatus::err(
            FfiErrorCode::InvalidInput,
            format!("read {label} failed: {err}"),
        )
    })
}

pub(crate) fn parse_method_signature(
    method: &str,
    sig: &str,
) -> AndroidBridgeResult<RuntimeMethodSignature> {
    RuntimeMethodSignature::from_str(sig)
        .map_err(|err| PortError::Failed(format!("jni signature for {method} failed: {err}")))
}

pub(crate) fn map_jni_error(context: &str, err: errors::Error) -> PortError {
    PortError::Failed(format!("jni {context} failed: {err}"))
}

#[cfg(test)]
#[path = "jni_bridge/start_request_tests.rs"]
mod start_request_tests;
