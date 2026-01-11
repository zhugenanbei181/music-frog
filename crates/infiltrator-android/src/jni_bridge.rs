use std::path::PathBuf;
use std::ptr;
use std::sync::Arc;

use jni::errors::Error as JniError;
use jni::objects::{GlobalRef, JObject, JString, JValue};
use jni::sys::{jint, jstring};
use jni::{JNIEnv, JavaVM};
use mihomo_api::{MihomoError, Result};
use mihomo_platform::{set_android_bridge, set_home_dir_override, AndroidBridge};

use crate::{FfiErrorCode, FfiStatus};

const SIG_NOARGS_BOOL: &str = "()Z";
const SIG_NOARGS_STRING: &str = "()Ljava/lang/String;";
const SIG_STR_STR_BOOL: &str = "(Ljava/lang/String;Ljava/lang/String;)Z";
const SIG_STR_STR_STR_BOOL: &str = "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Z";
const SIG_STR_STR_STRING: &str =
    "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;";
const SIG_BOOL_BOOL: &str = "(Z)Z";

struct JniBridge {
    vm: JavaVM,
    host: GlobalRef,
}

impl JniBridge {
    fn new(vm: JavaVM, host: GlobalRef) -> Self {
        Self { vm, host }
    }

    fn env(&self) -> Result<jni::AttachGuard<'_>> {
        self.vm
            .attach_current_thread()
            .map_err(|err| MihomoError::Service(format!("attach jni thread failed: {err}")))
    }

    fn call_bool(
        &self,
        method: &str,
        sig: &str,
        args: &[JValue],
    ) -> Result<bool> {
        let mut env = self.env()?;
        let value = env
            .call_method(self.host.as_obj(), method, sig, args)
            .map_err(|err| map_jni_error(method, err))?;
        value
            .z()
            .map_err(|err| map_jni_error(method, err))
    }

    fn call_string(
        &self,
        method: &str,
        sig: &str,
        args: &[JValue],
    ) -> Result<Option<String>> {
        let mut env = self.env()?;
        let value = env
            .call_method(self.host.as_obj(), method, sig, args)
            .map_err(|err| map_jni_error(method, err))?;
        let obj = value
            .l()
            .map_err(|err| map_jni_error(method, err))?;
        if obj.is_null() {
            return Ok(None);
        }
        let jstr = JString::from(obj);
        let text = env
            .get_string(&jstr)
            .map_err(|err| map_jni_error(method, err))?
            .into();
        Ok(Some(text))
    }

    fn to_java_string<'a>(env: &mut JNIEnv<'a>, value: &str) -> Result<JString<'a>> {
        env.new_string(value).map_err(|err| {
            MihomoError::Service(format!("create java string failed: {err}"))
        })
    }

    fn call_bool_result(
        &self,
        method: &str,
        sig: &str,
        args: &[JValue],
    ) -> Result<()> {
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
        let mut env = self.env()?;
        let arg1 = Self::to_java_string(&mut env, arg1)?;
        let arg2 = Self::to_java_string(&mut env, arg2)?;
        let args = [
            JValue::Object(arg1.as_ref()),
            JValue::Object(arg2.as_ref()),
        ];
        let value = env
            .call_method(self.host.as_obj(), method, SIG_STR_STR_STRING, &args)
            .map_err(|err| map_jni_error(method, err))?;
        let obj = value
            .l()
            .map_err(|err| map_jni_error(method, err))?;
        if obj.is_null() {
            return Ok(None);
        }
        let jstr = JString::from(obj);
        let text = env
            .get_string(&jstr)
            .map_err(|err| map_jni_error(method, err))?
            .into();
        Ok(Some(text))
    }

    fn call_bool_with_args(
        &self,
        method: &str,
        sig: &str,
        arg1: &str,
        arg2: &str,
        arg3: Option<&str>,
    ) -> Result<()> {
        let mut env = self.env()?;
        let arg1 = Self::to_java_string(&mut env, arg1)?;
        let arg2 = Self::to_java_string(&mut env, arg2)?;
        let arg3_value = match arg3 {
            Some(value) => Some(Self::to_java_string(&mut env, value)?),
            None => None,
        };
        let mut values = vec![
            JValue::Object(arg1.as_ref()),
            JValue::Object(arg2.as_ref()),
        ];
        if let Some(arg3_value) = arg3_value.as_ref() {
            values.push(JValue::Object(arg3_value.as_ref()));
        }
        let ok = env
            .call_method(self.host.as_obj(), method, sig, &values)
            .map_err(|err| map_jni_error(method, err))?
            .z()
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
        self.call_bool_with_args(
            "credentialDelete",
            SIG_STR_STR_BOOL,
            service,
            key,
            None,
        )
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

    async fn vpn_stop(&self) -> Result<bool> {
        self.call_bool("vpnStop", SIG_NOARGS_BOOL, &[])
    }

    async fn vpn_is_running(&self) -> Result<bool> {
        self.call_bool("vpnIsRunning", SIG_NOARGS_BOOL, &[])
    }

    async fn tun_set_enabled(&self, enabled: bool) -> Result<bool> {
        let mut env = self.env()?;
        let args = [JValue::Bool(enabled.into())];
        let ok = env
            .call_method(self.host.as_obj(), "tunSetEnabled", SIG_BOOL_BOOL, &args)
            .map_err(|err| map_jni_error("tunSetEnabled", err))?
            .z()
            .map_err(|err| map_jni_error("tunSetEnabled", err))?;
        Ok(ok)
    }

    async fn tun_is_enabled(&self) -> Result<bool> {
        self.call_bool("tunIsEnabled", SIG_NOARGS_BOOL, &[])
    }
}

#[no_mangle]
pub extern "system" fn Java_com_musicfrog_despicableinfiltrator_RustBridge_nativePing(
    env: JNIEnv,
    _object: JObject,
) -> jstring {
    match env.new_string("ok") {
        Ok(value) => value.into_raw(),
        Err(err) => {
            log::warn!("nativePing failed: {err}");
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_musicfrog_despicableinfiltrator_RustBridge_nativeInit(
    mut env: JNIEnv,
    _object: JObject,
    data_dir: JString,
    cache_dir: JString,
) -> jint {
    let status = init_dirs(&mut env, data_dir, cache_dir);
    status.code as jint
}

#[no_mangle]
pub extern "system" fn Java_com_musicfrog_despicableinfiltrator_RustBridge_nativeRegisterBridge(
    mut env: JNIEnv,
    _object: JObject,
    host: JObject,
) -> jint {
    let status = register_bridge(&mut env, host);
    status.code as jint
}

fn init_dirs(env: &mut JNIEnv, data_dir: JString, cache_dir: JString) -> FfiStatus {
    let data_dir = match read_java_string(env, &data_dir, "dataDir") {
        Ok(value) => value,
        Err(status) => return status,
    };
    let cache_dir = match read_java_string(env, &cache_dir, "cacheDir") {
        Ok(value) => value,
        Err(status) => return status,
    };

    if data_dir.trim().is_empty() || cache_dir.trim().is_empty() {
        return FfiStatus::err(FfiErrorCode::InvalidInput, "dir is empty");
    }

    crate::tls::ensure_rustls_provider();

    let override_ok = set_home_dir_override(PathBuf::from(data_dir));
    if !override_ok {
        log::warn!("data dir override already set");
    }

    FfiStatus::ok()
}

fn register_bridge(env: &mut JNIEnv, host: JObject) -> FfiStatus {
    if host.is_null() {
        return FfiStatus::err(FfiErrorCode::InvalidInput, "host is null");
    }

    let vm = match env.get_java_vm() {
        Ok(vm) => vm,
        Err(err) => {
            return FfiStatus::err(
                FfiErrorCode::InvalidState,
                format!("get java vm failed: {err}"),
            )
        }
    };

    let global = match env.new_global_ref(host) {
        Ok(global) => global,
        Err(err) => {
            return FfiStatus::err(
                FfiErrorCode::InvalidState,
                format!("create global ref failed: {err}"),
            )
        }
    };

    let bridge = JniBridge::new(vm, global);
    set_android_bridge(Arc::new(bridge));
    FfiStatus::ok()
}

fn read_java_string(
    env: &mut JNIEnv,
    input: &JString,
    label: &str,
) -> std::result::Result<String, FfiStatus> {
    if input.is_null() {
        return Err(FfiStatus::err(
            FfiErrorCode::InvalidInput,
            format!("{label} is null"),
        ));
    }
    env.get_string(input)
        .map(|value| value.into())
        .map_err(|err| {
            FfiStatus::err(
                FfiErrorCode::InvalidInput,
                format!("read {label} failed: {err}"),
            )
        })
}

fn map_jni_error(context: &str, err: JniError) -> MihomoError {
    MihomoError::Service(format!("jni {context} failed: {err}"))
}
