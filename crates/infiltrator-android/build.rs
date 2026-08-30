fn main() {
    println!("cargo:rerun-if-changed=src/uniffi_api.rs");
    println!("cargo:rerun-if-changed=src/uniffi_api");
}
