// Canonical UniFFI entry point for generating foreign-language bindings from
// this workspace (proc-macro mode, no UDL files). Run via:
//   cargo run -p infiltrator-android --bin uniffi-bindgen -- generate \
//       target/debug/libinfiltrator_android.so --language kotlin \
//       --out-dir android/app/src/main/java
fn main() {
    uniffi::uniffi_bindgen_main()
}
