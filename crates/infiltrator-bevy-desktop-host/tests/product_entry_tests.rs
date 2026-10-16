//! test-intent: behavior
//! The product entry starts the host; capture requires explicit opt-in.

use infiltrator_bevy_desktop_host::entry::{DesktopEntry, parse_entry};

#[test]
fn desktop_entry_selects_product_or_explicit_capture() {
    assert_eq!(parse_entry(&[]), Ok(DesktopEntry::Product));
    assert_eq!(
        parse_entry(&[String::from("--capture")]),
        Ok(DesktopEntry::Capture)
    );
    assert!(parse_entry(&[String::from("--unknown")]).is_err());
    assert!(parse_entry(&[String::from("--capture"), String::from("extra")]).is_err());
}
