use infiltrator_bevy_desktop_host::{
    DesktopEntry, DesktopEntryParseError, parse_entry, run_desktop,
};

fn main() {
    match parse_entry(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(DesktopEntry::Product) => run_desktop(),
        Ok(DesktopEntry::Capture) => run_capture(),
        Err(
            DesktopEntryParseError::UnknownArgument(_)
            | DesktopEntryParseError::UnexpectedExtraArguments,
        ) => {
            eprintln!("usage: infiltrator-bevy-ui [--capture]");
            std::process::exit(2);
        }
    }
}

fn run_capture() {
    #[cfg(feature = "capture-tools")]
    infiltrator_bevy_ui::run_demo();

    #[cfg(not(feature = "capture-tools"))]
    {
        eprintln!("capture entry requires the `capture-tools` feature");
        std::process::exit(2);
    }
}
