#[cfg(feature = "capture-tools")]
use std::env;

fn main() {
    #[cfg(feature = "capture-tools")]
    {
        if env::args().any(|arg| arg == "--demo")
            || env::var("INFILTRATOR_DEMO").is_ok_and(|v| v.trim() == "1")
        {
            if let Err(error) = infiltrator_slint::run_demo() {
                eprintln!("Slint demo could not start: {error}");
            }
            return;
        }
    }

    if let Err(error) = infiltrator_slint::run_desktop() {
        eprintln!("Slint product could not start: {error}");
    }
}
