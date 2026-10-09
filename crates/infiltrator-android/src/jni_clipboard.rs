//! BANDROID-008: the JNI implementation of the typed clipboard channel.
//!
//! It calls `readClipboard`/`writeClipboard` on the native host object the
//! Activity registered through `nativeRegisterNativeHost`. No `Context`,
//! `ClipboardManager` or `ClipData` type crosses into the shared ports: the
//! only values that leave this module are `Option<String>`/`PortError`.
//!
//! The module deliberately does not import the mihomo `Result` alias so the
//! trait method signatures use the std prelude `Result` directly.

use crate::jni_bridge::map_jni_error;
use crate::jni_bridge::parse_method_signature;
use crate::native_host::clipboard::NativeClipboardChannel;
use infiltrator_ports::error::PortError;
use jni::JavaVM;
use jni::errors;
use jni::objects::Global;
use jni::objects::JObject;
use jni::objects::JString;
use jni::objects::JValue;
use jni::objects::Reference as _;
use jni::strings::JNIString;
use mihomo_api::error::MihomoError;

const SIG_NOARGS_STRING: &str = "()Ljava/lang/String;";
const SIG_STRING_BOOL: &str = "(Ljava/lang/String;)Z";

/// The JNI-backed clipboard channel. It owns the global ref of the Activity
/// adapter; dropping it (on re-registration or clear) releases that ref.
pub(crate) struct JniClipboardChannel {
    vm: JavaVM,
    host: Global<JObject<'static>>,
}

impl JniClipboardChannel {
    pub(crate) fn new(vm: JavaVM, host: Global<JObject<'static>>) -> Self {
        Self { vm, host }
    }

    fn read(&self) -> Result<Option<String>, MihomoError> {
        let name = JNIString::new("readClipboard");
        let signature = parse_method_signature("readClipboard", SIG_NOARGS_STRING)?;
        let signature = signature.method_signature();
        self.vm
            .attach_current_thread(|env| -> errors::Result<Option<String>> {
                let value = env.call_method(self.host.as_obj(), &name, &signature, &[])?;
                let obj = value.l()?;
                if obj.is_null() {
                    return Ok(None);
                }
                let text = JString::cast_local(env, obj)?.try_to_string(env)?;
                Ok(Some(text))
            })
            .map_err(|err| map_jni_error("readClipboard", err))
    }

    fn write(&self, text: &str) -> Result<bool, MihomoError> {
        let name = JNIString::new("writeClipboard");
        let signature = parse_method_signature("writeClipboard", SIG_STRING_BOOL)?;
        let signature = signature.method_signature();
        self.vm
            .attach_current_thread(|env| -> errors::Result<bool> {
                let value = env.new_string(text)?;
                let args = [JValue::Object(value.as_ref())];
                env.call_method(self.host.as_obj(), &name, &signature, &args)?
                    .z()
            })
            .map_err(|err| map_jni_error("writeClipboard", err))
    }
}

impl NativeClipboardChannel for JniClipboardChannel {
    fn read_clipboard(&self) -> Result<Option<String>, PortError> {
        self.read()
            .map_err(|err| PortError::Failed(err.to_string()))
    }

    fn write_clipboard(&self, text: &str) -> Result<(), PortError> {
        match self.write(text) {
            Ok(true) => Ok(()),
            Ok(false) => Err(PortError::PermissionDenied(
                "the Android host rejected the clipboard write".to_string(),
            )),
            Err(err) => Err(PortError::Failed(err.to_string())),
        }
    }
}
