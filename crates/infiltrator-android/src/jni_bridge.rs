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
    AndroidHostProcessRole, CoreLaunchSpec, VpnAuthorizationState, VpnOwnerSnapshot,
};
use infiltrator_contract::vpn::{
    PreparedVpnConfiguration, VpnConfiguration, VpnConfigurationApplyOutcome, VpnOwnerStopReceipt,
    VpnStartOutcome, VpnStopTarget,
};
use infiltrator_ports::core_process::CoreProcessStartContext;
use jni::errors::LogErrorAndDefault;
use jni::objects::{Global, JObject, JString, JValue, Reference as _};
use jni::signature::RuntimeMethodSignature;
use jni::strings::JNIString;
use jni::sys::{jboolean, jfloat, jint, jlong, jstring};
use jni::{Env, EnvUnowned, JavaVM, errors};
use mihomo_api::error::{MihomoError, Result};
use mihomo_platform::android_bridge::{
    AndroidBridge, AndroidCoreConfigurationValidationOutcome,
    AndroidCoreConfigurationValidationReply, AndroidCoreConfigurationValidationRequest,
    CoreProcessStartReceipt, CoreProcessStopReceipt, set_android_bridge,
};
use std::path::PathBuf;
use std::result;
use std::sync::Arc;

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

#[derive(serde::Serialize)]
struct CoreStartRequestWire<'a> {
    launch_spec: &'a CoreLaunchSpec,
    start_context: &'a CoreProcessStartContext,
}

const SIG_NOARGS_INT: &str = "()I";
const SIG_INT_LONG_INT: &str = "(IJ)I";
const SIG_INT_LONG_STRING: &str = "(IJ)Ljava/lang/String;";
const SIG_NOARGS_STRING: &str = "()Ljava/lang/String;";
const SIG_STRING_INT: &str = "(Ljava/lang/String;)I";
const SIG_STR_STR_BOOL: &str = "(Ljava/lang/String;Ljava/lang/String;)Z";
const SIG_STR_STR_STR_BOOL: &str = "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Z";
const SIG_STR_STR_STRING: &str = "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;";
const SIG_STRING_STRING: &str = "(Ljava/lang/String;)Ljava/lang/String;";

struct JniBridge {
    vm: JavaVM,
    host: Global<JObject<'static>>,
}

impl JniBridge {
    fn new(vm: JavaVM, host: Global<JObject<'static>>) -> Self {
        Self { vm, host }
    }

    fn call_bool(&self, method: &str, sig: &str, args: &[JValue]) -> Result<bool> {
        let name = JNIString::new(method);
        let signature = parse_method_signature(method, sig)?;
        let signature = signature.method_signature();
        self.vm
            .attach_current_thread(|env| -> errors::Result<bool> {
                env.call_method(self.host.as_obj(), &name, &signature, args)?
                    .z()
            })
            .map_err(|err| map_jni_error(method, err))
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

    fn call_int_with_string(&self, method: &str, value: &str) -> Result<i32> {
        let name = JNIString::new(method);
        let signature = parse_method_signature(method, SIG_STRING_INT)?;
        let signature = signature.method_signature();
        self.vm
            .attach_current_thread(|env| -> errors::Result<i32> {
                let value = env.new_string(value)?;
                let args = [JValue::Object(value.as_ref())];
                env.call_method(self.host.as_obj(), &name, &signature, &args)?
                    .i()
            })
            .map_err(|err| map_jni_error(method, err))
    }

    fn call_string_with_args(
        &self,
        method: &str,
        arg1: &str,
        arg2: &str,
    ) -> Result<Option<String>> {
        let name = JNIString::new(method);
        let signature = parse_method_signature(method, SIG_STR_STR_STRING)?;
        let signature = signature.method_signature();
        self.vm
            .attach_current_thread(|env| -> errors::Result<Option<String>> {
                let arg1 = env.new_string(arg1)?;
                let arg2 = env.new_string(arg2)?;
                let args = [JValue::Object(arg1.as_ref()), JValue::Object(arg2.as_ref())];
                let value = env.call_method(self.host.as_obj(), &name, &signature, &args)?;
                let obj = value.l()?;
                if obj.is_null() {
                    return Ok(None);
                }
                let text = JString::cast_local(env, obj)?.try_to_string(env)?;
                Ok(Some(text))
            })
            .map_err(|err| map_jni_error(method, err))
    }

    fn call_string_with_single_arg(&self, method: &str, value: &str) -> Result<Option<String>> {
        let name = JNIString::new(method);
        let signature = parse_method_signature(method, SIG_STRING_STRING)?;
        let signature = signature.method_signature();
        self.vm
            .attach_current_thread(|env| -> errors::Result<Option<String>> {
                let value = env.new_string(value)?;
                let args = [JValue::Object(value.as_ref())];
                let result = env.call_method(self.host.as_obj(), &name, &signature, &args)?;
                let object = result.l()?;
                if object.is_null() {
                    return Ok(None);
                }
                let text = JString::cast_local(env, object)?.try_to_string(env)?;
                Ok(Some(text))
            })
            .map_err(|err| map_jni_error(method, err))
    }

    fn call_bool_with_args(
        &self,
        method: &str,
        sig: &str,
        arg1: &str,
        arg2: &str,
        arg3: Option<&str>,
    ) -> Result<()> {
        let name = JNIString::new(method);
        let signature = parse_method_signature(method, sig)?;
        let signature = signature.method_signature();
        let ok = self
            .vm
            .attach_current_thread(|env| -> errors::Result<bool> {
                let arg1 = env.new_string(arg1)?;
                let arg2 = env.new_string(arg2)?;
                let arg3_value = match arg3 {
                    Some(value) => Some(env.new_string(value)?),
                    None => None,
                };
                let mut values = vec![JValue::Object(arg1.as_ref()), JValue::Object(arg2.as_ref())];
                if let Some(arg3_value) = arg3_value.as_ref() {
                    values.push(JValue::Object(arg3_value.as_ref()));
                }
                env.call_method(self.host.as_obj(), &name, &signature, &values)?
                    .z()
            })
            .map_err(|err| map_jni_error(method, err))?;
        if ok {
            Ok(())
        } else {
            Err(MihomoError::Service(format!(
                "android bridge method {method} returned false"
            )))
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
        launch_spec: &CoreLaunchSpec,
        context: &CoreProcessStartContext,
    ) -> Result<CoreProcessStartReceipt> {
        let request = CoreStartRequestWire {
            launch_spec,
            start_context: context,
        };
        let start_request_json = serde_json::to_string(&request).map_err(|error| {
            MihomoError::Service(format!(
                "Android Core start request could not be serialized: {error}"
            ))
        })?;
        let code = self.call_int_with_string("coreStart", &start_request_json)?;
        CoreProcessStartReceipt::from_wire_code(code).ok_or_else(|| {
            MihomoError::Service(format!(
                "android bridge returned unknown core start receipt code {code}"
            ))
        })
    }

    async fn core_stop(&self) -> Result<CoreProcessStopReceipt> {
        let code = self.call_int("coreStop", SIG_NOARGS_INT, &[])?;
        CoreProcessStopReceipt::from_wire_code(code).ok_or_else(|| {
            MihomoError::Service(format!(
                "android bridge returned unknown core stop receipt code {code}"
            ))
        })
    }

    async fn core_process_observation(&self) -> Result<AndroidCoreProcessObservation> {
        let json = self
            .call_string("coreProcessObservationJson", SIG_NOARGS_STRING, &[])?
            .ok_or_else(|| {
                MihomoError::Service(
                    "android bridge returned no Core process observation".to_owned(),
                )
            })?;
        serde_json::from_str(&json).map_err(|error| {
            MihomoError::Service(format!(
                "android bridge returned an invalid Core process observation: {error}"
            ))
        })
    }

    async fn core_process_recovery_completed(
        &self,
        context: &CoreProcessStartContext,
    ) -> Result<AndroidCoreProcessRecoveryReceipt> {
        let context_json = serde_json::to_string(context).map_err(|error| {
            MihomoError::Service(format!(
                "Android Core recovery context could not be serialized: {error}"
            ))
        })?;
        let code = self.call_int_with_string("coreProcessExitRecovered", &context_json)?;
        AndroidCoreProcessRecoveryReceipt::from_wire_code(code).ok_or_else(|| {
            MihomoError::Service(format!(
                "android bridge returned unknown Core recovery receipt code {code}"
            ))
        })
    }

    async fn core_validate_configuration(
        &self,
        request: &AndroidCoreConfigurationValidationRequest,
    ) -> Result<AndroidCoreConfigurationValidationOutcome> {
        let request_json = serde_json::to_string(request).map_err(|error| {
            MihomoError::Service(format!(
                "Android Core configuration validation request could not be serialized: {error}"
            ))
        })?;
        let encoded = self
            .call_string_with_single_arg("coreValidateConfiguration", &request_json)?
            .ok_or_else(|| {
                MihomoError::Service(
                    "Android Core configuration validation returned no receipt".to_owned(),
                )
            })?;
        match serde_json::from_str::<AndroidCoreConfigurationValidationReply>(&encoded).map_err(
            |error| {
                MihomoError::Service(format!(
                    "Android Core configuration validation returned an invalid receipt: {error}"
                ))
            },
        )? {
            AndroidCoreConfigurationValidationReply::Receipt {
                request: receipt_request,
                outcome,
            } if &receipt_request == request => Ok(outcome),
            AndroidCoreConfigurationValidationReply::Receipt { .. } => Err(MihomoError::Service(
                "Android Core configuration validation receipt did not match its request"
                    .to_owned(),
            )),
            AndroidCoreConfigurationValidationReply::InvalidRequest { reason } => {
                Err(MihomoError::Service(format!(
                    "Android Core configuration validation rejected its request: {reason}"
                )))
            }
        }
    }

    async fn core_lifecycle_snapshot(&self) -> Result<AndroidCoreLifecycleSnapshot> {
        let encoded = self
            .call_string("coreLifecycleSnapshot", SIG_NOARGS_STRING, &[])?
            .ok_or_else(|| {
                MihomoError::Service("Android core lifecycle IPC returned no snapshot".to_owned())
            })?;
        serde_json::from_str(&encoded).map_err(|error| {
            MihomoError::Service(format!(
                "Android core lifecycle IPC returned an invalid snapshot: {error}"
            ))
        })
    }

    async fn execute_product_command_json(&self, command_json: &str) -> Result<String> {
        self.call_string_with_single_arg("executeProductCommandJson", command_json)?
            .ok_or_else(|| {
                MihomoError::Service(
                    "Android product command IPC returned no terminal result".to_owned(),
                )
            })
    }

    async fn surface_snapshot_json(&self) -> Result<String> {
        self.call_string("surfaceSnapshotJson", SIG_NOARGS_STRING, &[])?
            .ok_or_else(|| {
                MihomoError::Service("Android surface IPC returned no snapshot".to_owned())
            })
    }

    async fn execute_owner_operation_json(&self, operation_json: &str) -> Result<String> {
        self.call_string_with_single_arg("executeOwnerOperationJson", operation_json)?
            .ok_or_else(|| {
                MihomoError::Service("Android owner operation IPC returned no result".to_owned())
            })
    }

    async fn credential_get(&self, service: &str, key: &str) -> Result<Option<String>> {
        self.call_string_with_args("credentialGet", service, key)
    }

    async fn credential_set(&self, service: &str, key: &str, value: &str) -> Result<()> {
        self.call_bool_with_args(
            "credentialSet",
            SIG_STR_STR_STR_BOOL,
            service,
            key,
            Some(value),
        )
    }

    async fn credential_delete(&self, service: &str, key: &str) -> Result<()> {
        self.call_bool_with_args("credentialDelete", SIG_STR_STR_BOOL, service, key, None)
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
        let code = self.call_int("vpnStartOutcome", SIG_NOARGS_INT, &[])?;
        VpnStartOutcome::from_wire_code(code).ok_or_else(|| {
            MihomoError::Service(format!(
                "android bridge returned unknown VPN start outcome code {code}"
            ))
        })
    }

    async fn vpn_authorization_state(&self) -> Result<VpnAuthorizationState> {
        match self.call_int("vpnAuthorizationState", SIG_NOARGS_INT, &[])? {
            0 => Ok(VpnAuthorizationState::Granted),
            1 => Ok(VpnAuthorizationState::Required),
            code => Err(MihomoError::Service(format!(
                "android bridge returned unknown VPN authorization code {code}"
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
                MihomoError::Service("prepared VPN Core session is unavailable".to_owned())
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
        let config_json = serde_json::to_string(&wire).map_err(|error| {
            MihomoError::Service(format!("serialize prepared VPN configuration: {error}"))
        })?;
        let code = self.call_int_with_string("vpnApplyConfiguration", &config_json)?;
        VpnConfigurationApplyOutcome::from_wire_code(code).ok_or_else(|| {
            MihomoError::Service(format!(
                "android bridge returned unknown VPN configuration outcome code {code}"
            ))
        })
    }

    async fn vpn_stop(&self, target: VpnStopTarget) -> Result<VpnOwnerStopReceipt> {
        let generation = target.generation_wire_value();
        if matches!(target, VpnStopTarget::Generation(_)) && generation == 0 {
            return Err(MihomoError::Service(
                "VPN stop generation must be non-zero".to_owned(),
            ));
        }
        let generation = i64::try_from(generation).map_err(|_| {
            MihomoError::Service("VPN stop generation exceeds Android Long range".to_owned())
        })?;
        let args = [JValue::Int(target.wire_code()), JValue::Long(generation)];
        let encoded = self
            .call_string("vpnStopResultJson", SIG_INT_LONG_STRING, &args)?
            .ok_or_else(|| {
                MihomoError::Service("Android VPN stop IPC returned no owner readback".to_owned())
            })?;
        serde_json::from_str(&encoded).map_err(|error| {
            MihomoError::Service(format!(
                "Android VPN stop IPC returned an invalid owner readback: {error}"
            ))
        })
    }

    async fn vpn_owner_snapshot(&self) -> Result<VpnOwnerSnapshot> {
        let encoded = self
            .call_string("vpnOwnerSnapshotJson", SIG_NOARGS_STRING, &[])?
            .ok_or_else(|| {
                MihomoError::Service("Android VPN owner IPC returned no snapshot".to_owned())
            })?;
        serde_json::from_str(&encoded).map_err(|error| {
            MihomoError::Service(format!(
                "Android VPN owner IPC returned an invalid snapshot: {error}"
            ))
        })
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
        .map_err(|err| MihomoError::Service(format!("jni signature for {method} failed: {err}")))
}

pub(crate) fn map_jni_error(context: &str, err: errors::Error) -> MihomoError {
    MihomoError::Service(format!("jni {context} failed: {err}"))
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
