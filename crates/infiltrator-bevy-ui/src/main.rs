//! Thin binary wrapper: the lib hosts the module tree and the bootstrap,
//! the binary keeps the package name for `target/debug/infiltrator-bevy-ui`.
fn main() {
    infiltrator_bevy_ui::run();
}
