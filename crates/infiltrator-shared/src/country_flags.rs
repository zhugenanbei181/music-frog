//! Country flag identity for proxy node names.
//!
//! Region classification is owned by `infiltrator_contract::geo_region`; this
//! module exposes only the one question a surface asks: "which rectangular
//! flag asset identifies this node?". A flag is square artwork keyed by an
//! uppercase ISO code, never a Unicode emoji glyph whose shape depends on the
//! host's font.

use infiltrator_contract::geo_region::NodeRegion;

/// Uppercase flag-asset code for a node name, or `None` for a routing target
/// or unknown region. Consumers draw a square code plate for `None`.
pub fn node_flag_code(name: &str) -> Option<&'static str> {
    NodeRegion::classify(name).flag_code()
}

/// Uppercase text drawn on the square plate when no flag asset applies.
pub fn node_plate_label(name: &str) -> &'static str {
    NodeRegion::classify(name).plate_label()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_codes_are_rectangular_asset_keys_not_emoji() {
        assert_eq!(node_flag_code("香港 IEPL"), Some("HK"));
        assert_eq!(node_flag_code("US-01"), Some("US"));
        assert_eq!(node_flag_code("瑞士 Zurich"), Some("CH"));
        assert_eq!(node_flag_code("Unknown Node"), None);
        assert_eq!(node_flag_code("DIRECT"), None);
        assert_eq!(node_flag_code("REJECT"), None);
        assert_eq!(node_flag_code("GLOBAL"), None);
    }

    #[test]
    fn abbreviations_cannot_invent_a_flag_from_an_ordinary_word() {
        for name in [
            "Inspection node",
            "Default",
            "Fallback",
            "Auto",
            "USAGE",
            "CACHE",
            "TROJAN",
            "BRIDGE",
            "MyProxy",
            "PHASE",
            "THREAD",
            "ARCADE",
            "RUNTIME",
        ] {
            assert_eq!(node_flag_code(name), None, "unexpected flag for {name}");
        }
        for (name, code) in [
            ("[IN] 01", "IN"),
            ("in-02", "IN"),
            ("us01", "US"),
            ("[de] Premium", "DE"),
            ("HKG-03", "HK"),
            ("[UK] Premium", "GB"),
        ] {
            assert_eq!(node_flag_code(name), Some(code), "{name}");
        }
    }

    #[test]
    fn plate_labels_are_text_so_no_surface_needs_an_emoji_font() {
        assert_eq!(node_plate_label("HK-01"), "HK");
        assert_eq!(node_plate_label("DIRECT"), "DIRECT");
        assert_eq!(node_plate_label("Unknown Node"), "\u{2014}");
        for name in ["HK-01", "DIRECT", "REJECT", "GLOBAL", "Unknown Node"] {
            let label = node_plate_label(name);
            for ch in label.chars() {
                assert!(
                    !('\u{1F1E6}'..='\u{1F1FF}').contains(&ch),
                    "plate label {label:?} for {name} is a flag glyph"
                );
            }
        }
    }

    #[test]
    fn every_geographic_code_resolves_to_a_catalogued_flag() {
        use infiltrator_contract::geo_region::GeoRegion;

        for region in [
            GeoRegion::HongKong,
            GeoRegion::Japan,
            GeoRegion::Singapore,
            GeoRegion::UnitedStates,
            GeoRegion::Switzerland,
            GeoRegion::Ukraine,
            GeoRegion::China,
        ] {
            let name = format!("{}-01", region.iso_code());
            assert_eq!(node_flag_code(&name), Some(region.iso_code()), "{name}");
        }
    }
}
