fn main() {
    let target = std::env::var("TARGET").expect("Cargo provides the exact Rust target triple");
    println!("cargo:rustc-env=INFILTRATOR_BUILD_TARGET={target}");
    println!("cargo:rerun-if-env-changed=TARGET");
}
