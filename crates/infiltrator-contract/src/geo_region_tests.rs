use super::*;
use crate::flag_codes_generated::FLAG_CODES;

#[test]
fn iso_codes_round_trip_and_unknown_is_explicit() {
    for region in [
        GeoRegion::HongKong,
        GeoRegion::Taiwan,
        GeoRegion::Japan,
        GeoRegion::Singapore,
        GeoRegion::UnitedStates,
        GeoRegion::SouthKorea,
        GeoRegion::UnitedKingdom,
        GeoRegion::Germany,
        GeoRegion::France,
        GeoRegion::Canada,
        GeoRegion::Australia,
        GeoRegion::Netherlands,
        GeoRegion::Russia,
        GeoRegion::India,
        GeoRegion::Macau,
        GeoRegion::Thailand,
        GeoRegion::Vietnam,
        GeoRegion::Malaysia,
        GeoRegion::Philippines,
        GeoRegion::Indonesia,
        GeoRegion::Turkey,
        GeoRegion::Brazil,
        GeoRegion::Argentina,
        GeoRegion::SouthAfrica,
        GeoRegion::Israel,
        GeoRegion::UnitedArabEmirates,
        GeoRegion::Switzerland,
        GeoRegion::Sweden,
        GeoRegion::Norway,
        GeoRegion::Finland,
        GeoRegion::Italy,
        GeoRegion::Spain,
        GeoRegion::Ireland,
        GeoRegion::Poland,
        GeoRegion::Ukraine,
        GeoRegion::China,
    ] {
        assert_eq!(GeoRegion::from_iso(region.iso_code()), region);
        assert_eq!(
            GeoRegion::from_iso(&region.iso_code().to_lowercase()),
            region
        );
    }
    assert_eq!(GeoRegion::from_iso("??"), GeoRegion::Other);
}

#[test]
fn name_detection_uses_region_tokens_without_matching_substrings() {
    for (name, region) in [
        ("🇭🇰 Hong Kong IEPL", GeoRegion::HongKong),
        ("JP-Tokyo-01", GeoRegion::Japan),
        ("SG-01", GeoRegion::Singapore),
        ("US-Silicon Valley", GeoRegion::UnitedStates),
        ("DE Frankfurt", GeoRegion::Germany),
        ("TW Taipei", GeoRegion::Taiwan),
        ("UK London", GeoRegion::UnitedKingdom),
        ("Unknown Planet", GeoRegion::Other),
    ] {
        assert_eq!(GeoRegion::from_name_heuristic(name), region, "{name}");
    }
    for name in [
        "Inspection node",
        "Default",
        "Fallback",
        "TROJAN",
        "USAGE",
        "CACHE",
    ] {
        assert_eq!(
            GeoRegion::from_name_heuristic(name),
            GeoRegion::Other,
            "{name}"
        );
    }
}

#[test]
fn node_region_classifies_flags_and_routing_targets_without_glyphs() {
    assert_eq!(
        NodeRegion::classify("🇭🇰 Hong Kong IEPL").flag_code(),
        Some("HK")
    );
    assert_eq!(
        NodeRegion::classify("瑞士 Zurich 01").flag_code(),
        Some("CH")
    );
    assert_eq!(NodeRegion::classify("DIRECT").flag_code(), None);
    assert_eq!(NodeRegion::classify("DIRECT").plate_label(), "DIRECT");
    assert_eq!(NodeRegion::classify("Unknown Node").flag_code(), None);
    assert_eq!(NodeRegion::classify("Unknown Node").plate_label(), "—");
    assert_eq!(NodeRegion::classify("DIRECT").search_code(), "DIRECT");
}

#[test]
fn every_catalogued_flag_is_resolvable_from_a_node_name() {
    assert_eq!(FLAG_CODES.len(), 258);
    for code in FLAG_CODES {
        let name = format!("{code}-01");
        assert_eq!(
            NodeRegion::classify(&name).flag_code(),
            Some(*code),
            "{name}"
        );
    }
}

#[test]
fn catalogued_codes_do_not_turn_ordinary_words_into_countries() {
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
        assert_eq!(NodeRegion::classify(name).flag_code(), None, "{name}");
    }
    for (name, code) in [
        ("PT-Lisbon-01", "PT"),
        ("MX-01", "MX"),
        ("CL-Santiago", "CL"),
        ("NZ-Auckland", "NZ"),
        ("AC-01", "AC"),
        ("XK-01", "XK"),
        ("[PT] Lisbon", "PT"),
    ] {
        assert_eq!(NodeRegion::classify(name).flag_code(), Some(code), "{name}");
    }
}

#[test]
fn every_geographic_variant_is_resolvable_from_a_node_name() {
    for region in [
        GeoRegion::HongKong,
        GeoRegion::Taiwan,
        GeoRegion::Japan,
        GeoRegion::Singapore,
        GeoRegion::UnitedStates,
        GeoRegion::SouthKorea,
        GeoRegion::UnitedKingdom,
        GeoRegion::Germany,
        GeoRegion::France,
        GeoRegion::Canada,
        GeoRegion::Australia,
        GeoRegion::Netherlands,
        GeoRegion::Russia,
        GeoRegion::India,
        GeoRegion::Macau,
        GeoRegion::Thailand,
        GeoRegion::Vietnam,
        GeoRegion::Malaysia,
        GeoRegion::Philippines,
        GeoRegion::Indonesia,
        GeoRegion::Turkey,
        GeoRegion::Brazil,
        GeoRegion::Argentina,
        GeoRegion::SouthAfrica,
        GeoRegion::Israel,
        GeoRegion::UnitedArabEmirates,
        GeoRegion::Switzerland,
        GeoRegion::Sweden,
        GeoRegion::Norway,
        GeoRegion::Finland,
        GeoRegion::Italy,
        GeoRegion::Spain,
        GeoRegion::Ireland,
        GeoRegion::Poland,
        GeoRegion::Ukraine,
        GeoRegion::China,
    ] {
        let name = format!("{} 01", region.iso_code());
        assert_eq!(
            GeoRegion::from_name_heuristic(&name),
            region,
            "{name} must resolve to its own flag asset"
        );
    }
}
