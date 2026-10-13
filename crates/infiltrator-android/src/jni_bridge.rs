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
use crate::{FfiErrorCode, FfiStatus};
use infiltrator_contract::android_control::{
    AndroidCoreLifecycleSnapshot, AndroidCoreProcessObservation, AndroidCoreProcessRecoveryReceipt,
    AndroidHostProcessRole, VpnAuthorizationState, VpnOwnerSnapshot,
};
use infiltrator_contract::vpn::{
    PreparedVpnConfiguration, VpnConfigurationApplyOutcome, VpnOwnerStopReceipt, VpnStartOutcome,
    VpnStopTarget,
};
use infiltrator_platform::android_bridge::{
    AndroidBridge, AndroidBridgeResult as Result, AndroidCoreConfigurationValidationOutcome,
    AndroidCoreConfigurationValidationReply, AndroidCoreConfigurationValidationRequest,
    CoreProcessStartReceipt, CoreProcessStopReceipt, set_android_bridge,
};
use infiltrator_ports::error::PortError;
use jni::errors::LogErrorAndDefault;
use jni::objects::{Global, JObject, JString, JValue, Reference as _};
use jni::signature::RuntimeMethodSignature;
use jni::strings::JNIString;
use jni::sys::{jboolean, jfloat, jint, jlong, jstring};
use jni::{Env, EnvUnowned, JavaVM, errors};
use std::path::PathBuf;
use std::result;
use std::sync::Arc;
use std::time::Duration;

const SIG_NOARGS_INT: &str = "()I";
const SIG_NOARGS_STRING: &str = "()Ljava/lang/String;";
const SIG_BRIDGE_DISPATCH: &str = "(JLjava/lang/String;J)I";
const SIG_BRIDGE_CANCEL: &str = "(J)I";
const BRIDGE_CALL_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(serde::Serialize)]
struct PreparedVpnConfigurationWire<'a> {
    preparation_id: u64,
    vpn_generation: u64,
    owner_data_revision: u64,
    core_scope: PreparedVpnCoreScopeWire<'a>,
    configuration: &'a infiltrator_contract::vpn::VpnConfiguration,
}

#[derive(serde::Serialize)]
struct PreparedVpnCoreScopeWire<'a> {
    engine_id: &'a str,
    instance_id: &'a str,
    version: Option<&'a str>,
    generation: u64,
    session_token: String,
}

struct JniBridge {
    vm: JavaVM,
    host: Global<JObject<'static>>,
}

impl JniBridge {
    fn new(vm: JavaVM, host: Global<JObject<'static>>) -> Self {
        Self { vm, host }
    }

    fn call_int(&self, method: &str, sig: &str, args: &[JValue]) -> Result<i32> {
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

    fn call_string(&self, method: &str, sig: &str, args: &[JValue]) -> Result<Option<String>> {
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

    fn dispatch_android_bridge_call(
        &self,
        request_id: AndroidBridgeRequestId,
        request_json: &str,
        timeout: Duration,
    ) -> Result<i32> {
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

    fn cancel_android_bridge_call(&self, request_id: AndroidBridgeRequestId) {
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
                &signature.method_signature(),
                &arguments,
            )?
            .i()?;
            Ok(())
        });
    }

    async fn invoke_android_bridge_call(
        &self,
        call: AndroidBridgeCall,
    ) -> Result<AndroidBridgeCallReply> {
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

        let completion = match tokio::time::timeout(BRIDGE_CALL_TIMEOUT, receiver).await {
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

#[async_trait::async_trait]
impl AndroidBridge for JniBridge {
    fn process_role(&self) -> AndroidHostProcessRole {
        self.call_int("processRole", SIG_NOARGS_INT, &[])
            .ok()
            .and_then(AndroidHostProcessRole::from_wire_code)
            .unwrap_or(AndroidHostProcessRole::Unsupported)
    }

    async fn core_start(
        &self,
        launch_spec: &infiltrator_contract::android_control::CoreLaunchSpec,
        context: &infiltrator_ports::core_process::CoreProcessStartContext,
    ) -> Result<CoreProcessStartReceipt> {
        #[derive(serde::Serialize)]
        struct StartRequest<'a> {
            launch_spec: &'a infiltrator_contract::android_control::CoreLaunchSpec,
            start_context: &'a infiltrator_ports::core_process::CoreProcessStartContext,
        }
        let start_request_json = serde_json::to_string(&StartRequest {
            launch_spec,
            start_context: context,
        })
        .map_err(|error| {
            PortError::Failed(format!(
                "Android Core start request could not be serialized: {error}"
            ))
        })?;
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::CoreStart { start_request_json })
            .await?
        {
            AndroidBridgeCallReply::CoreStart { wire_code } => {
                CoreProcessStartReceipt::from_wire_code(wire_code).ok_or_else(|| {
                    PortError::Failed(format!(
                        "Android host returned unknown Core start receipt code {wire_code}"
                    ))
                })
            }
            _ => Err(unexpected_bridge_reply("CoreStart")),
        }
    }

    async fn core_stop(&self) -> Result<CoreProcessStopReceipt> {
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::CoreStop)
            .await?
        {
            AndroidBridgeCallReply::CoreStop { wire_code } => {
                CoreProcessStopReceipt::from_wire_code(wire_code).ok_or_else(|| {
                    PortError::Failed(format!(
                        "Android host returned unknown Core stop receipt code {wire_code}"
                    ))
                })
            }
            _ => Err(unexpected_bridge_reply("CoreStop")),
        }
    }

    async fn core_process_observation(&self) -> Result<AndroidCoreProcessObservation> {
        let encoded = match self
            .invoke_android_bridge_call(AndroidBridgeCall::CoreProcessObservation)
            .await?
        {
            AndroidBridgeCallReply::CoreProcessObservation(encoded) => encoded,
            _ => return Err(unexpected_bridge_reply("CoreProcessObservation")),
        };
        serde_json::from_str(&encoded).map_err(|error| {
            PortError::Failed(format!(
                "Android bridge returned an invalid Core process observation: {error}"
            ))
        })
    }

    async fn core_process_recovery_completed(
        &self,
        context: &infiltrator_ports::core_process::CoreProcessStartContext,
    ) -> Result<AndroidCoreProcessRecoveryReceipt> {
        let context_json = serde_json::to_string(context).map_err(|error| {
            PortError::Failed(format!(
                "Android Core recovery context serialization failed: {error}"
            ))
        })?;
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::CoreProcessRecoveryCompleted {
                context_json,
            })
            .await?
        {
            AndroidBridgeCallReply::CoreProcessRecoveryCompleted { wire_code } => {
                AndroidCoreProcessRecoveryReceipt::from_wire_code(wire_code).ok_or_else(|| {
                    PortError::Failed(format!(
                        "Android host returned unknown Core recovery receipt code {wire_code}"
                    ))
                })
            }
            _ => Err(unexpected_bridge_reply("CoreProcessRecoveryCompleted")),
        }
    }

    async fn core_validate_configuration(
        &self,
        request: &AndroidCoreConfigurationValidationRequest,
    ) -> Result<AndroidCoreConfigurationValidationOutcome> {
        let request_json = serde_json::to_string(request).map_err(|error| {
            PortError::Failed(format!(
                "Android Core validation request serialization failed: {error}"
            ))
        })?;
        let encoded = match self
            .invoke_android_bridge_call(AndroidBridgeCall::CoreValidateConfiguration {
                request_json,
            })
            .await?
        {
            AndroidBridgeCallReply::CoreValidateConfiguration(encoded) => encoded,
            _ => return Err(unexpected_bridge_reply("CoreValidateConfiguration")),
        };
        match serde_json::from_str::<AndroidCoreConfigurationValidationReply>(&encoded).map_err(
            |error| {
                PortError::Failed(format!(
                    "Android Core configuration validation returned an invalid receipt: {error}"
                ))
            },
        )? {
            AndroidCoreConfigurationValidationReply::Receipt {
                request: receipt_request,
                outcome,
            } if &receipt_request == request => Ok(outcome),
            AndroidCoreConfigurationValidationReply::Receipt { .. } => Err(PortError::Failed(
                "Android Core configuration validation receipt did not match its request"
                    .to_owned(),
            )),
            AndroidCoreConfigurationValidationReply::InvalidRequest { reason } => {
                Err(PortError::Failed(format!(
                    "Android Core configuration validation rejected its request: {reason}"
                )))
            }
        }
    }

    async fn core_lifecycle_snapshot(&self) -> Result<AndroidCoreLifecycleSnapshot> {
        let encoded = match self
            .invoke_android_bridge_call(AndroidBridgeCall::CoreLifecycleSnapshot)
            .await?
        {
            AndroidBridgeCallReply::CoreLifecycleSnapshot(encoded) => encoded,
            _ => return Err(unexpected_bridge_reply("CoreLifecycleSnapshot")),
        };
        serde_json::from_str(&encoded).map_err(|error| {
            PortError::Failed(format!("Android lifecycle snapshot is invalid: {error}"))
        })
    }

    async fn execute_product_command_json(&self, command_json: &str) -> Result<String> {
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::ExecuteProductCommand {
                command_json: command_json.to_owned(),
            })
            .await?
        {
            AndroidBridgeCallReply::ExecuteProductCommand(result) => Ok(result),
            _ => Err(unexpected_bridge_reply("ExecuteProductCommand")),
        }
    }

    async fn surface_snapshot_json(&self) -> Result<String> {
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::SurfaceSnapshot)
            .await?
        {
            AndroidBridgeCallReply::SurfaceSnapshot(snapshot) => Ok(snapshot),
            _ => Err(unexpected_bridge_reply("SurfaceSnapshot")),
        }
    }

    async fn execute_owner_operation_json(&self, operation_json: &str) -> Result<String> {
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::ExecuteOwnerOperation {
                operation_json: operation_json.to_owned(),
            })
            .await?
        {
            AndroidBridgeCallReply::ExecuteOwnerOperation(result) => Ok(result),
            _ => Err(unexpected_bridge_reply("ExecuteOwnerOperation")),
        }
    }

    async fn credential_get(&self, service: &str, key: &str) -> Result<Option<String>> {
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::CredentialGet {
                service: service.to_owned(),
                key: key.to_owned(),
            })
            .await?
        {
            AndroidBridgeCallReply::CredentialGet(value) => Ok(value),
            _ => Err(unexpected_bridge_reply("CredentialGet")),
        }
    }

    async fn credential_set(&self, service: &str, key: &str, value: &str) -> Result<()> {
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::CredentialSet {
                service: service.to_owned(),
                key: key.to_owned(),
                value: value.to_owned(),
            })
            .await?
        {
            AndroidBridgeCallReply::CredentialSet {
                outcome: AndroidCredentialMutationOutcome::Applied,
            } => Ok(()),
            AndroidBridgeCallReply::CredentialSet {
                outcome: AndroidCredentialMutationOutcome::Rejected,
            } => Err(PortError::Failed(
                "Android credential store rejected the write".to_owned(),
            )),
            _ => Err(unexpected_bridge_reply("CredentialSet")),
        }
    }

    async fn credential_delete(&self, service: &str, key: &str) -> Result<()> {
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::CredentialDelete {
                service: service.to_owned(),
                key: key.to_owned(),
            })
            .await?
        {
            AndroidBridgeCallReply::CredentialDelete {
                outcome: AndroidCredentialMutationOutcome::Applied,
            } => Ok(()),
            AndroidBridgeCallReply::CredentialDelete {
                outcome: AndroidCredentialMutationOutcome::Rejected,
            } => Err(PortError::Failed(
                "Android credential store rejected the delete".to_owned(),
            )),
            _ => Err(unexpected_bridge_reply("CredentialDelete")),
        }
    }

    fn data_dir(&self) -> Option<PathBuf> {
        self.call_string("dataDir", SIG_NOARGS_STRING, &[])
            .ok()
            .flatten()
            .map(PathBuf::from)
    }

    fn cache_dir(&self) -> Option<PathBuf> {
        self.call_string("cacheDir", SIG_NOARGS_STRING, &[])
            .ok()
            .flatten()
            .map(PathBuf::from)
    }

    async fn vpn_start(&self) -> Result<VpnStartOutcome> {
        let wire_code = match self
            .invoke_android_bridge_call(AndroidBridgeCall::VpnStart)
            .await?
        {
            AndroidBridgeCallReply::VpnStart { wire_code } => wire_code,
            _ => return Err(unexpected_bridge_reply("VpnStart")),
        };
        VpnStartOutcome::from_wire_code(wire_code).ok_or_else(|| {
            PortError::Failed(format!(
                "Android host returned unknown VPN start outcome {wire_code}"
            ))
        })
    }

    async fn vpn_authorization_state(&self) -> Result<VpnAuthorizationState> {
        let wire_code = match self
            .invoke_android_bridge_call(AndroidBridgeCall::VpnAuthorizationState)
            .await?
        {
            AndroidBridgeCallReply::VpnAuthorizationState { wire_code } => wire_code,
            _ => return Err(unexpected_bridge_reply("VpnAuthorizationState")),
        };
        match wire_code {
            0 => Ok(VpnAuthorizationState::Granted),
            1 => Ok(VpnAuthorizationState::Required),
            code => Err(PortError::Failed(format!(
                "Android host returned unknown VPN authorization state {code}"
            ))),
        }
    }

    async fn vpn_apply_configuration(
        &self,
        prepared: &PreparedVpnConfiguration,
    ) -> Result<VpnConfigurationApplyOutcome> {
        let session_token = prepared
            .core_scope
            .session_token
            .filter(|token| token.is_valid())
            .ok_or_else(|| {
                PortError::Failed("prepared VPN Core session is unavailable".to_owned())
            })?;
        let wire = PreparedVpnConfigurationWire {
            preparation_id: prepared.preparation_id.value(),
            vpn_generation: prepared.vpn_generation.value(),
            owner_data_revision: prepared.owner_data_revision,
            core_scope: PreparedVpnCoreScopeWire {
                engine_id: prepared.core_scope.identity.engine.as_str(),
                instance_id: prepared.core_scope.identity.instance.as_str(),
                version: prepared
                    .core_scope
                    .identity
                    .version
                    .as_ref()
                    .map(|version| version.raw()),
                generation: prepared.core_scope.generation.value(),
                session_token: session_token.value().to_string(),
            },
            configuration: &prepared.configuration,
        };
        let prepared_json = serde_json::to_string(&wire).map_err(|error| {
            PortError::Failed(format!("serialize prepared VPN configuration: {error}"))
        })?;
        let wire_code = match self
            .invoke_android_bridge_call(AndroidBridgeCall::VpnApplyConfiguration { prepared_json })
            .await?
        {
            AndroidBridgeCallReply::VpnConfigurationApply { wire_code } => wire_code,
            _ => return Err(unexpected_bridge_reply("VpnApplyConfiguration")),
        };
        VpnConfigurationApplyOutcome::from_wire_code(wire_code).ok_or_else(|| {
            PortError::Failed(format!(
                "Android host returned unknown VPN configuration outcome {wire_code}"
            ))
        })
    }

    async fn vpn_stop(&self, target: VpnStopTarget) -> Result<VpnOwnerStopReceipt> {
        if matches!(target, VpnStopTarget::Generation(_)) && target.generation_wire_value() == 0 {
            return Err(PortError::Failed(
                "VPN stop generation must be non-zero".to_owned(),
            ));
        }
        let encoded = match self
            .invoke_android_bridge_call(AndroidBridgeCall::VpnStop { target })
            .await?
        {
            AndroidBridgeCallReply::VpnStop(encoded) => encoded,
            _ => return Err(unexpected_bridge_reply("VpnStop")),
        };
        serde_json::from_str(&encoded).map_err(|error| {
            PortError::Failed(format!("Android VPN stop receipt is invalid: {error}"))
        })
    }

    async fn vpn_owner_snapshot(&self) -> Result<VpnOwnerSnapshot> {
        let encoded = match self
            .invoke_android_bridge_call(AndroidBridgeCall::VpnOwnerSnapshot)
            .await?
        {
            AndroidBridgeCallReply::VpnOwnerSnapshot(encoded) => encoded,
            _ => return Err(unexpected_bridge_reply("VpnOwnerSnapshot")),
        };
        serde_json::from_str(&encoded).map_err(|error| {
            PortError::Failed(format!("Android VPN owner snapshot is invalid: {error}"))
        })
    }
}

fn unexpected_bridge_reply(expected: &str) -> PortError {
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
            crate::android_async_bridge::AndroidBridgeCallCompletionResult::Delivered => 0,
            crate::android_async_bridge::AndroidBridgeCallCompletionResult::CallerCanceled => 1,
            crate::android_async_bridge::AndroidBridgeCallCompletionResult::UnknownRequest => 2,
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
    focused: jboolean,
    visible: jboolean,
) -> jint {
    let mut status = FfiErrorCode::InvalidState as jint;
    env.with_env(|_env| -> errors::Result<()> {
        let Some(phase) = ActivityPhase::from_code(phase) else {
            status = FfiErrorCode::InvalidInput as jint;
            return Ok(());
        };
        let fact = HostLifecycleFact {
            generation: generation.max(0) as u64,
            phase,
            focus: if focused {
                WindowFocus::Focused
            } else {
                WindowFocus::Unfocused
            },
            visible,
        };
        status = match push_lifecycle_fact(fact) {
            LifecycleUpdate::Applied { .. } => FfiErrorCode::Ok as jint,
            LifecycleUpdate::Stale => FfiErrorCode::InvalidState as jint,
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

pub(crate) fn parse_method_signature(method: &str, sig: &str) -> Result<RuntimeMethodSignature> {
    RuntimeMethodSignature::from_str(sig)
        .map_err(|err| PortError::Failed(format!("jni signature for {method} failed: {err}")))
}

pub(crate) fn map_jni_error(context: &str, err: errors::Error) -> PortError {
    PortError::Failed(format!("jni {context} failed: {err}"))
}

#[cfg(test)]
mod start_request_tests {
    use super::CoreStartRequestWire;
    use infiltrator_contract::android_control::{
        CoreLaunchArguments, CoreLaunchBinary, CoreLaunchConfigurationIdentity, CoreLaunchPath,
        CoreLaunchSpec,
    };
    use infiltrator_contract::core_identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, EngineId,
    };
    use infiltrator_contract::profile_document_identity::{ProfileDocumentRevision, ProfileId};
    use infiltrator_contract::session::SessionToken;
    use infiltrator_ports::core_process::CoreProcessStartContext;
    use infiltrator_ports::error::PortError;
    use std::path::PathBuf;

    #[test]
    fn core_start_wire_carries_typed_binary_profile_and_managed_owner_context() {
        let launch_spec = CoreLaunchSpec::new(
            CoreLaunchConfigurationIdentity {
                engine_id: EngineId::new("mihomo").expect("engine ID"),
                profile_id: ProfileId::new("primary").expect("profile ID"),
                document_revision: ProfileDocumentRevision::new("sha256:abc")
                    .expect("document revision"),
            },
            CoreLaunchBinary::MihomoBundled,
            PathBuf::from("/data/app/files"),
            CoreLaunchArguments::Mihomo {
                configuration_path: CoreLaunchPath::new("/data/app/files/config.yaml".to_owned())
                    .expect("absolute config path"),
            },
        )
        .expect("launch spec");
        let context = CoreProcessStartContext::managed(
            CoreIdentity::new(
                EngineId::new("mihomo").expect("engine ID"),
                CoreInstanceId::new("android-active").expect("instance ID"),
                None,
            ),
            CoreGeneration::new(7),
            SessionToken::new(42),
        );
        let request = CoreStartRequestWire {
            launch_spec: &launch_spec,
            start_context: &context,
        };

        assert_eq!(
            serde_json::to_value(request).expect("serialize Android Core start request"),
            serde_json::json!({
                "launch_spec": {
                    "configuration_identity": {
                        "engine_id": "mihomo",
                        "profile_id": "primary",
                        "document_revision": "sha256:abc"
                    },
                    "binary": "MihomoBundled",
                    "working_directory": "/data/app/files",
                    "launch_arguments": {
                        "Mihomo": {
                            "configuration_path": "/data/app/files/config.yaml"
                        }
                    }
                },
                "start_context": {
                    "Managed": {
                        "identity": {
                            "engine": "mihomo",
                            "instance": "android-active",
                            "version": null
                        },
                        "generation": 7,
                        "session_token": 42
                    }
                }
            }),
        );
    }
}
