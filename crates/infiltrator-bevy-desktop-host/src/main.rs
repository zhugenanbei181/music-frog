use infiltrator_bevy_desktop_host::desktop::run_desktop;
use infiltrator_bevy_desktop_host::entry::{DesktopEntry, DesktopEntryParseError, parse_entry};
use std::env;
use std::process;

fn main() {
    match parse_entry(&env::args().skip(1).collect::<Vec<_>>()) {
        Ok(DesktopEntry::Product) => run_desktop(),
        Ok(DesktopEntry::Capture) => run_capture(),
        Err(
            DesktopEntryParseError::UnknownArgument(_)
            | DesktopEntryParseError::UnexpectedExtraArguments,
        ) => {
            eprintln!("usage: infiltrator-bevy-ui [--capture]");
            process::exit(2);
        }
    }
}

fn run_capture() {
    #[cfg(feature = "capture-tools")]
    infiltrator_bevy_ui::run_demo();

    #[cfg(not(feature = "capture-tools"))]
    {
        eprintln!("capture entry requires the `capture-tools` feature");
        process::exit(2);
    }
}
