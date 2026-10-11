fn main() {
    if let Err(error) = infiltrator_slint::run_desktop() {
        eprintln!("Slint product could not start: {error}");
    }
}
