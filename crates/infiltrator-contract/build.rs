use std::env;

fn main() {
    let target = env::var("TARGET").expect("Cargo provides the exact Rust target triple");
    println!("cargo:rustc-env=INFILTRATOR_BUILD_TARGET={target}");
    println!("cargo:rerun-if-env-changed=TARGET");
}
