use crate::service_init::init_android_process;
use crate::{FfiErrorCode, FfiStatus};
use jni::errors::LogErrorAndDefault;
use jni::objects::{Global, JObject, JString, JValue, Reference as _};
use jni::signature::RuntimeMethodSignature;
use jni::strings::JNIString;
use jni::sys::{jint, jstring};
use jni::{Env, EnvUnowned, JavaVM, errors};
use mihomo_api::error::{MihomoError, Result};
use mihomo_platform::android_bridge::{AndroidBridge, set_android_bridge};
use std::path::PathBuf;
use std::result;
use std::sync::Arc;

const SIG_NOARGS_BOOL: &str = "()Z";
const SIG_NOARGS_STRING: &str = "()Ljava/lang/String;";
const SIG_STR_STR_BOOL: &str = "(Ljava/lang/String;Ljava/lang/String;)Z";
const SIG_STR_STR_STR_BOOL: &str = "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Z";
const SIG_STR_STR_STRING: &str = "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;";
const SIG_BOOL_BOOL: &str = "(Z)Z";
const SIG_STRING_BOOL: &str = "(Ljava/lang/String;)Z";

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

    fn call_bool_result(&self, method: &str, sig: &str, args: &[JValue]) -> Result<()> {
        let ok = self.call_bool(method, sig, args)?;
        if ok {
            Ok(())
        } else {
            Err(MihomoError::Service(format!(
                "android bridge method {method} returned false"
            )))
        }
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

    fn call_bool_with_string(&self, method: &str, value: &str) -> Result<bool> {
        let name = JNIString::new(method);
        let signature = parse_method_signature(method, SIG_STRING_BOOL)?;
        let signature = signature.method_signature();
        self.vm
            .attach_current_thread(|env| -> errors::Result<bool> {
                let value = env.new_string(value)?;
                let args = [JValue::Object(value.as_ref())];
                env.call_method(self.host.as_obj(), &name, &signature, &args)?
                    .z()
            })
            .map_err(|err| map_jni_error(method, err))
    }
}

#[async_trait::async_trait]
impl AndroidBridge for JniBridge {
    async fn core_start(&self) -> Result<()> {
        self.call_bool_result("coreStart", SIG_NOARGS_BOOL, &[])
    }

    async fn core_stop(&self) -> Result<()> {
        self.call_bool_result("coreStop", SIG_NOARGS_BOOL, &[])
    }

    async fn core_is_running(&self) -> Result<bool> {
        self.call_bool("coreIsRunning", SIG_NOARGS_BOOL, &[])
    }

    fn core_controller_url(&self) -> Option<String> {
        self.call_string("coreControllerUrl", SIG_NOARGS_STRING, &[])
            .unwrap_or_else(|err| {
                log::warn!("android bridge coreControllerUrl failed: {err}");
                None
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

    async fn vpn_start(&self) -> Result<bool> {
        self.call_bool("vpnStart", SIG_NOARGS_BOOL, &[])
    }

    async fn vpn_apply_configuration(&self, config_json: &str) -> Result<bool> {
        self.call_bool_with_string("vpnApplyConfiguration", config_json)
    }

    async fn vpn_stop(&self) -> Result<bool> {
        self.call_bool("vpnStop", SIG_NOARGS_BOOL, &[])
    }

    async fn vpn_is_running(&self) -> Result<bool> {
        self.call_bool("vpnIsRunning", SIG_NOARGS_BOOL, &[])
    }

    async fn vpn_is_foreground(&self) -> Result<bool> {
        self.call_bool("vpnIsForeground", SIG_NOARGS_BOOL, &[])
    }

    async fn tun_set_enabled(&self, enabled: bool) -> Result<bool> {
        let name = JNIString::new("tunSetEnabled");
        let signature = parse_method_signature("tunSetEnabled", SIG_BOOL_BOOL)?;
        let signature = signature.method_signature();
        let args = [JValue::Bool(enabled)];
        self.vm
            .attach_current_thread(|env| -> errors::Result<bool> {
                env.call_method(self.host.as_obj(), &name, &signature, &args)?
                    .z()
            })
            .map_err(|err| map_jni_error("tunSetEnabled", err))
    }

    async fn tun_is_enabled(&self) -> Result<bool> {
        self.call_bool("tunIsEnabled", SIG_NOARGS_BOOL, &[])
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

fn parse_method_signature(method: &str, sig: &str) -> Result<RuntimeMethodSignature> {
    RuntimeMethodSignature::from_str(sig)
        .map_err(|err| MihomoError::Service(format!("jni signature for {method} failed: {err}")))
}

fn map_jni_error(context: &str, err: errors::Error) -> MihomoError {
    MihomoError::Service(format!("jni {context} failed: {err}"))
}
