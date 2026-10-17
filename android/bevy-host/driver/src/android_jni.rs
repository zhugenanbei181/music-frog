//! JNI adapters for typed :vpn owner methods and Bevy Activity window insets.

use crate::insets::{
    ActivityWindowInsetsObservation, InsetsDeliveryResult, decode_window_insets,
    publish_window_insets,
};
use crate::owner_client::OwnerTransport;
use android_activity::AndroidApp;
use infiltrator_contract::android_control::VpnOwnerSnapshot;
use lurker_core_contract::error::{ErrorCode, Failure};
use infiltrator_contract::vpn::VpnOwnerStopReceipt;
use jni::JavaVM;
use jni::NativeMethod;
use jni::errors;
use jni::errors::LogErrorAndDefault;
use jni::objects::{Global, JObject, JString, JValue, Reference as _};
use jni::signature::RuntimeMethodSignature;
use jni::strings::JNIString;
use jni::sys::jobject;
use std::ffi::c_void;

pub(crate) struct AndroidActivityOwnerTransport {
    app: AndroidApp,
    vm: JavaVM,
}

impl AndroidActivityOwnerTransport {
    pub(crate) fn new(app: AndroidApp) -> Self {
        let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
        Self { app, vm }
    }

    fn call_string_method(
        &self,
        method: &str,
        signature: &str,
        argument: Option<&str>,
    ) -> Result<Option<String>, Failure> {
        let method_name = JNIString::new(method);
        let signature = RuntimeMethodSignature::from_str(signature).map_err(|error| {
            Failure::new(
                ErrorCode::Internal,
                format!("invalid JNI signature for {method}: {error}"),
                false,
            )
        })?;
        self.vm
            .attach_current_thread_for_scope(|env| -> errors::Result<Option<String>> {
                let activity_ptr = self.app.activity_as_ptr() as jobject;
                let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&activity_ptr)? };
                let signature = signature.method_signature();
                let result = if let Some(argument) = argument {
                    let argument = env.new_string(argument)?;
                    let arguments = [JValue::Object(argument.as_ref())];
                    env.call_method(&*activity, &method_name, &signature, &arguments)?
                } else {
                    env.call_method(&*activity, &method_name, &signature, &[])?
                };
                let value = result.l()?;
                if value.is_null() {
                    return Ok(None);
                }
                JString::cast_local(env, value)?
                    .try_to_string(env)
                    .map(Some)
            })
            .map_err(|error| {
                Failure::new(
                    ErrorCode::NotReady,
                    format!("Android Activity {method} call failed: {error}"),
                    true,
                )
            })
    }

    fn read_window_insets(&self) -> Result<Option<ActivityWindowInsetsObservation>, Failure> {
        self.call_string_method("readAndroidWindowInsetsJson", "()Ljava/lang/String;", None)?
            .map(|raw| decode_window_insets(&raw))
            .transpose()
    }

    pub(crate) fn publish_current_window_insets(&self) -> Result<(), Failure> {
        match self.read_window_insets()? {
            Some(observation) => match publish_window_insets(observation) {
                InsetsDeliveryResult::Delivered
                | InsetsDeliveryResult::StaleActivity
                | InsetsDeliveryResult::AwaitingHost => Ok(()),
                InsetsDeliveryResult::ReceiverClosed => Err(Failure::new(
                    ErrorCode::NotReady,
                    "Bevy closed its Android window-insets input before startup",
                    true,
                )),
            },
            None => Ok(()),
        }
    }
}

pub(crate) fn register_window_insets_callback(app: &AndroidApp) -> Result<(), Failure> {
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    vm.attach_current_thread_for_scope(|env| -> errors::Result<()> {
        let activity_ptr = app.activity_as_ptr() as jobject;
        let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&activity_ptr)? };
        let activity_class = env.get_object_class(&*activity)?;
        let name = JNIString::new("nativeOnWindowInsetsJson");
        let signature = JNIString::new("(Ljava/lang/String;)V");
        let callback = Java_com_musicfrog_infiltrator_BevyHostActivity_nativeOnWindowInsetsJson
            as *const () as *mut c_void;
        // SAFETY: the method is an instance `String -> Unit` callback and the
        // Rust function has the matching EnvUnowned/JObject/JString ABI.
        let method = unsafe {
            NativeMethod::from_raw_parts(name.borrowed(), signature.borrowed(), callback)
        };
        // SAFETY: the descriptor and function pointer above match the Kotlin
        // private external method on this Activity class.
        unsafe { env.register_native_methods(activity_class, &[method])? };
        Ok(())
    })
    .map_err(|error| {
        Failure::new(
            ErrorCode::NotReady,
            format!("could not register the Bevy Activity inset callback: {error}"),
            true,
        )
    })
}

impl OwnerTransport for AndroidActivityOwnerTransport {
    fn read_surface_snapshot_json(&self) -> Result<Option<String>, Failure> {
        self.call_string_method(
            "readAndroidSurfaceSnapshotJson",
            "()Ljava/lang/String;",
            None,
        )
    }

    fn read_vpn_owner_snapshot(&self) -> Result<VpnOwnerSnapshot, Failure> {
        let encoded = self
            .call_string_method(
                "readAndroidVpnOwnerSnapshotJson",
                "()Ljava/lang/String;",
                None,
            )?
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::NotReady,
                    "Android Activity returned no VPN owner snapshot",
                    true,
                )
            })?;
        serde_json::from_str(&encoded).map_err(|error| {
            Failure::new(
                ErrorCode::NotReady,
                format!("Android Activity returned an invalid VPN owner snapshot: {error}"),
                true,
            )
        })
    }

    fn execute_product_command_json(&self, request_json: &str) -> Result<String, Failure> {
        self.call_string_method(
            "executeAndroidProductCommandJson",
            "(Ljava/lang/String;)Ljava/lang/String;",
            Some(request_json),
        )?
        .ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "Android Activity returned no terminal command JSON",
                true,
            )
        })
    }

    fn stop_vpn_current(&self) -> Result<VpnOwnerStopReceipt, Failure> {
        let encoded = self
            .call_string_method("stopAndroidVpnServiceCurrent", "()Ljava/lang/String;", None)?
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::NotReady,
                    "Android Activity returned no VPN stop receipt",
                    true,
                )
            })?;
        serde_json::from_str(&encoded).map_err(|error| {
            Failure::new(
                ErrorCode::NotReady,
                format!("Android Activity returned an invalid VPN stop receipt: {error}"),
                true,
            )
        })
    }
}

/// Instance callback from `BevyHostActivity`: a native inset change enters the
/// currently registered Bevy app channel as one typed observation.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_musicfrog_infiltrator_BevyHostActivity_nativeOnWindowInsetsJson<
    'local,
>(
    mut env: jni::EnvUnowned<'local>,
    _activity: JObject<'local>,
    report_json: JString<'local>,
) {
    env.with_env(|env| -> errors::Result<()> {
        let raw = report_json.try_to_string(env)?;
        let report = match decode_window_insets(&raw) {
            Ok(report) => report,
            Err(failure) => {
                eprintln!("Bevy Android inset callback rejected: {failure}");
                return Ok(());
            }
        };
        match publish_window_insets(report) {
            InsetsDeliveryResult::AwaitingHost
            | InsetsDeliveryResult::Delivered
            | InsetsDeliveryResult::StaleActivity => {}
            InsetsDeliveryResult::ReceiverClosed => {
                eprintln!("Bevy Android inset callback found a retired UI receiver");
            }
        }
        Ok(())
    })
    .resolve::<LogErrorAndDefault>();
}
