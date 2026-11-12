//! The Iced flag plate draws the shared rectangular inventory, never a glyph.
//!
//! The complaint this file pins ("一般国旗都是方形 最好不要做这种圆形旗帜图案容器")
//! was live in the published Iced receipts until b219: the flag slot drew a
//! Unicode regional-indicator emoji, so its shape came from the host font, and
//! an unmatched node drew a literal circular globe. Both are now the same
//! 4:3 vector artwork Slint consumes from `asset-catalog.json`.

use super::*;
use infiltrator_shared::country_flags::node_flag_code;

#[test]
fn every_catalogued_flag_resolves_to_full_bleed_artwork() {
    for code in FLAG_CODES {
        assert!(flag_svg(code).is_some(), "missing artwork for {code}");
        assert!(handle(code).is_some(), "missing handle for {code}");
    }
}

#[test]
fn the_plate_is_keyed_by_the_iso_code_not_a_glyph() {
    assert_eq!(node_flag_code("香港 IEPL"), Some("HK"));
    assert_eq!(node_flag_code("DIRECT"), None);
    assert_eq!(node_flag_code("Unknown Node"), None);
}
