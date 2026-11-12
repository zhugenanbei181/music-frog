//! Geographic region and country code representation for proxy nodes and active exits.
//!
//! Provides typed mapping between proxy node naming patterns, standard ISO 3166-1
//! alpha-2 country codes, emoji flags, localized display labels, and high-fidelity
//! flag asset tokens for cross-surface UI rendering.

use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};

/// Standard geographic regions and countries commonly encountered in proxy nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeoRegion {
    HongKong,
    Taiwan,
    Japan,
    Singapore,
    UnitedStates,
    SouthKorea,
    UnitedKingdom,
    Germany,
    France,
    Canada,
    Australia,
    Netherlands,
    Russia,
    India,
    Macau,
    Thailand,
    Vietnam,
    Malaysia,
    Philippines,
    Indonesia,
    Turkey,
    Brazil,
    Argentina,
    SouthAfrica,
    Israel,
    UnitedArabEmirates,
    Switzerland,
    Sweden,
    Norway,
    Finland,
    Italy,
    Spain,
    Ireland,
    Poland,
    Ukraine,
    China,
    Other,
}

impl GeoRegion {
    /// Two-letter uppercase ISO 3166-1 alpha-2 code, or "ZZ" for unknown/other.
    pub const fn iso_code(self) -> &'static str {
        match self {
            Self::HongKong => "HK",
            Self::Taiwan => "TW",
            Self::Japan => "JP",
            Self::Singapore => "SG",
            Self::UnitedStates => "US",
            Self::SouthKorea => "KR",
            Self::UnitedKingdom => "GB",
            Self::Germany => "DE",
            Self::France => "FR",
            Self::Canada => "CA",
            Self::Australia => "AU",
            Self::Netherlands => "NL",
            Self::Russia => "RU",
            Self::India => "IN",
            Self::Macau => "MO",
            Self::Thailand => "TH",
            Self::Vietnam => "VN",
            Self::Malaysia => "MY",
            Self::Philippines => "PH",
            Self::Indonesia => "ID",
            Self::Turkey => "TR",
            Self::Brazil => "BR",
            Self::Argentina => "AR",
            Self::SouthAfrica => "ZA",
            Self::Israel => "IL",
            Self::UnitedArabEmirates => "AE",
            Self::Switzerland => "CH",
            Self::Sweden => "SE",
            Self::Norway => "NO",
            Self::Finland => "FI",
            Self::Italy => "IT",
            Self::Spain => "ES",
            Self::Ireland => "IE",
            Self::Poland => "PL",
            Self::Ukraine => "UA",
            Self::China => "CN",
            Self::Other => "ZZ",
        }
    }

    /// Localized Chinese display label for GUI badges and filters.
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::HongKong => "香港",
            Self::Taiwan => "台湾",
            Self::Japan => "日本",
            Self::Singapore => "新加坡",
            Self::UnitedStates => "美国",
            Self::SouthKorea => "韩国",
            Self::UnitedKingdom => "英国",
            Self::Germany => "德国",
            Self::France => "法国",
            Self::Canada => "加拿大",
            Self::Australia => "澳大利亚",
            Self::Netherlands => "荷兰",
            Self::Russia => "俄罗斯",
            Self::India => "印度",
            Self::Macau => "澳门",
            Self::Thailand => "泰国",
            Self::Vietnam => "越南",
            Self::Malaysia => "马来西亚",
            Self::Philippines => "菲律宾",
            Self::Indonesia => "印度尼西亚",
            Self::Turkey => "土耳其",
            Self::Brazil => "巴西",
            Self::Argentina => "阿根廷",
            Self::SouthAfrica => "南非",
            Self::Israel => "以色列",
            Self::UnitedArabEmirates => "阿联酋",
            Self::Switzerland => "瑞士",
            Self::Sweden => "瑞典",
            Self::Norway => "挪威",
            Self::Finland => "芬兰",
            Self::Italy => "意大利",
            Self::Spain => "西班牙",
            Self::Ireland => "爱尔兰",
            Self::Poland => "波兰",
            Self::Ukraine => "乌克兰",
            Self::China => "中国",
            Self::Other => "其他地区",
        }
    }

    /// Localized English display label for GUI badges and filters.
    pub const fn label_en(self) -> &'static str {
        match self {
            Self::HongKong => "Hong Kong",
            Self::Taiwan => "Taiwan",
            Self::Japan => "Japan",
            Self::Singapore => "Singapore",
            Self::UnitedStates => "United States",
            Self::SouthKorea => "South Korea",
            Self::UnitedKingdom => "United Kingdom",
            Self::Germany => "Germany",
            Self::France => "France",
            Self::Canada => "Canada",
            Self::Australia => "Australia",
            Self::Netherlands => "Netherlands",
            Self::Russia => "Russia",
            Self::India => "India",
            Self::Macau => "Macau",
            Self::Thailand => "Thailand",
            Self::Vietnam => "Vietnam",
            Self::Malaysia => "Malaysia",
            Self::Philippines => "Philippines",
            Self::Indonesia => "Indonesia",
            Self::Turkey => "Turkey",
            Self::Brazil => "Brazil",
            Self::Argentina => "Argentina",
            Self::SouthAfrica => "South Africa",
            Self::Israel => "Israel",
            Self::UnitedArabEmirates => "United Arab Emirates",
            Self::Switzerland => "Switzerland",
            Self::Sweden => "Sweden",
            Self::Norway => "Norway",
            Self::Finland => "Finland",
            Self::Italy => "Italy",
            Self::Spain => "Spain",
            Self::Ireland => "Ireland",
            Self::Poland => "Poland",
            Self::Ukraine => "Ukraine",
            Self::China => "China",
            Self::Other => "Other Region",
        }
    }

    /// Map from an ISO 3166-1 alpha-2 code (case-insensitive).
    pub fn from_iso(code: &str) -> Self {
        match code.trim().to_ascii_uppercase().as_str() {
            "HK" => Self::HongKong,
            "TW" => Self::Taiwan,
            "JP" => Self::Japan,
            "SG" => Self::Singapore,
            "US" => Self::UnitedStates,
            "KR" => Self::SouthKorea,
            "GB" | "UK" => Self::UnitedKingdom,
            "DE" => Self::Germany,
            "FR" => Self::France,
            "CA" => Self::Canada,
            "AU" => Self::Australia,
            "NL" => Self::Netherlands,
            "RU" => Self::Russia,
            "IN" => Self::India,
            "MO" => Self::Macau,
            "TH" => Self::Thailand,
            "VN" => Self::Vietnam,
            "MY" => Self::Malaysia,
            "PH" => Self::Philippines,
            "ID" => Self::Indonesia,
            "TR" => Self::Turkey,
            "BR" => Self::Brazil,
            "AR" => Self::Argentina,
            "ZA" => Self::SouthAfrica,
            "IL" => Self::Israel,
            "AE" => Self::UnitedArabEmirates,
            "CH" => Self::Switzerland,
            "SE" => Self::Sweden,
            "NO" => Self::Norway,
            "FI" => Self::Finland,
            "IT" => Self::Italy,
            "ES" => Self::Spain,
            "IE" => Self::Ireland,
            "PL" => Self::Poland,
            "UA" => Self::Ukraine,
            "CN" => Self::China,
            _ => Self::Other,
        }
    }

    /// Heuristic region extraction from proxy node name patterns.
    pub fn from_name_heuristic(name: &str) -> Self {
        let lower = name.trim().to_ascii_lowercase();
        if lower.is_empty() {
            return Self::Other;
        }
        if name.contains("🇭🇰")
            || name.contains("香港")
            || has_region_token(&lower, "hk")
            || lower.contains("hong kong")
            || lower.contains("hongkong")
            || lower.contains("hong-kong")
            || has_region_token(&lower, "hkg")
        {
            return Self::HongKong;
        }
        if name.contains("🇹🇼")
            || name.contains("台湾")
            || name.contains("台灣")
            || name.contains("台北")
            || name.contains("台中")
            || name.contains("高雄")
            || has_region_token(&lower, "tw")
            || lower.contains("taiwan")
            || lower.contains("taipei")
        {
            return Self::Taiwan;
        }
        if name.contains("🇯🇵")
            || name.contains("日本")
            || name.contains("东京")
            || name.contains("大阪")
            || has_region_token(&lower, "jp")
            || lower.contains("japan")
            || lower.contains("tokyo")
            || lower.contains("osaka")
            || name.contains("福冈")
        {
            return Self::Japan;
        }
        if name.contains("🇸🇬")
            || name.contains("新加坡")
            || has_region_token(&lower, "sg")
            || lower.contains("singapore")
        {
            return Self::Singapore;
        }
        if name.contains("🇺🇸")
            || name.contains("美国")
            || name.contains("美國")
            || has_region_token(&lower, "us")
            || has_region_token(&lower, "usa")
            || lower.contains("united states")
            || lower.contains("america")
            || lower.contains("los angeles")
        {
            return Self::UnitedStates;
        }
        if name.contains("🇰🇷")
            || name.contains("韩国")
            || name.contains("韓國")
            || name.contains("首尔")
            || has_region_token(&lower, "kr")
            || lower.contains("korea")
            || lower.contains("seoul")
        {
            return Self::SouthKorea;
        }
        if name.contains("🇬🇧")
            || name.contains("英国")
            || name.contains("英國")
            || has_region_token(&lower, "uk")
            || has_region_token(&lower, "gb")
            || lower.contains("united kingdom")
            || lower.contains("london")
        {
            return Self::UnitedKingdom;
        }
        if name.contains("🇩🇪")
            || name.contains("德国")
            || name.contains("德國")
            || name.contains("法兰克福")
            || has_region_token(&lower, "de")
            || lower.contains("germany")
            || lower.contains("deutschland")
            || lower.contains("frankfurt")
        {
            return Self::Germany;
        }
        if name.contains("🇫🇷")
            || name.contains("法国")
            || name.contains("法國")
            || name.contains("巴黎")
            || has_region_token(&lower, "fr")
            || lower.contains("france")
            || lower.contains("paris")
        {
            return Self::France;
        }
        if name.contains("🇨🇦")
            || name.contains("加拿大")
            || has_region_token(&lower, "ca")
            || lower.contains("canada")
            || lower.contains("toronto")
            || lower.contains("vancouver")
        {
            return Self::Canada;
        }
        if name.contains("🇦🇺")
            || name.contains("澳大利亚")
            || name.contains("澳洲")
            || has_region_token(&lower, "au")
            || lower.contains("australia")
            || lower.contains("sydney")
        {
            return Self::Australia;
        }
        if name.contains("🇳🇱")
            || name.contains("荷兰")
            || has_region_token(&lower, "nl")
            || lower.contains("netherlands")
            || lower.contains("amsterdam")
        {
            return Self::Netherlands;
        }
        if name.contains("🇷🇺")
            || name.contains("俄罗斯")
            || has_region_token(&lower, "ru")
            || lower.contains("russia")
            || lower.contains("moscow")
        {
            return Self::Russia;
        }
        if name.contains("🇮🇳")
            || name.contains("印度")
            || has_region_token(&lower, "in")
            || lower.contains("india")
            || lower.contains("mumbai")
        {
            return Self::India;
        }
        if name.contains("🇲🇴")
            || name.contains("澳门")
            || name.contains("澳門")
            || has_region_token(&lower, "mo")
            || lower.contains("macau")
            || lower.contains("macao")
        {
            return Self::Macau;
        }
        if name.contains("🇹🇭")
            || name.contains("泰国")
            || has_region_token(&lower, "th")
            || lower.contains("thailand")
            || lower.contains("bangkok")
        {
            return Self::Thailand;
        }
        if name.contains("🇻🇳")
            || name.contains("越南")
            || has_region_token(&lower, "vn")
            || lower.contains("vietnam")
        {
            return Self::Vietnam;
        }
        if name.contains("🇲🇾")
            || name.contains("马来西亚")
            || has_region_token(&lower, "my")
            || lower.contains("malaysia")
        {
            return Self::Malaysia;
        }
        if name.contains("🇵🇭")
            || name.contains("菲律宾")
            || name.contains("菲律賓")
            || has_region_token(&lower, "ph")
            || lower.contains("philippines")
            || lower.contains("manila")
        {
            return Self::Philippines;
        }
        if name.contains("🇮🇩")
            || name.contains("印度尼西亚")
            || name.contains("印尼")
            || has_region_token(&lower, "id")
            || lower.contains("indonesia")
            || lower.contains("jakarta")
        {
            return Self::Indonesia;
        }
        if name.contains("🇹🇷")
            || name.contains("土耳其")
            || has_region_token(&lower, "tr")
            || lower.contains("turkey")
            || lower.contains("istanbul")
        {
            return Self::Turkey;
        }
        if name.contains("🇧🇷")
            || name.contains("巴西")
            || has_region_token(&lower, "br")
            || lower.contains("brazil")
            || lower.contains("sao paulo")
        {
            return Self::Brazil;
        }
        if name.contains("🇦🇷")
            || name.contains("阿根廷")
            || has_region_token(&lower, "ar")
            || lower.contains("argentina")
        {
            return Self::Argentina;
        }
        if name.contains("🇿🇦")
            || name.contains("南非")
            || has_region_token(&lower, "za")
            || lower.contains("south africa")
        {
            return Self::SouthAfrica;
        }
        if name.contains("🇮🇱")
            || name.contains("以色列")
            || has_region_token(&lower, "il")
            || lower.contains("israel")
            || lower.contains("tel aviv")
        {
            return Self::Israel;
        }
        if name.contains("🇦🇪")
            || name.contains("阿联酋")
            || name.contains("迪拜")
            || has_region_token(&lower, "ae")
            || lower.contains("uae")
            || lower.contains("dubai")
        {
            return Self::UnitedArabEmirates;
        }
        if name.contains("🇨🇭")
            || name.contains("瑞士")
            || has_region_token(&lower, "ch")
            || lower.contains("switzerland")
            || lower.contains("zurich")
        {
            return Self::Switzerland;
        }
        if name.contains("🇸🇪")
            || name.contains("瑞典")
            || has_region_token(&lower, "se")
            || lower.contains("sweden")
            || lower.contains("stockholm")
        {
            return Self::Sweden;
        }
        if name.contains("🇳🇴")
            || name.contains("挪威")
            || has_region_token(&lower, "no")
            || lower.contains("norway")
            || lower.contains("oslo")
        {
            return Self::Norway;
        }
        if name.contains("🇫🇮")
            || name.contains("芬兰")
            || has_region_token(&lower, "fi")
            || lower.contains("finland")
            || lower.contains("helsinki")
        {
            return Self::Finland;
        }
        if name.contains("🇮🇹")
            || name.contains("意大利")
            || has_region_token(&lower, "it")
            || lower.contains("italy")
            || lower.contains("rome")
            || lower.contains("milan")
        {
            return Self::Italy;
        }
        if name.contains("🇪🇸")
            || name.contains("西班牙")
            || has_region_token(&lower, "es")
            || lower.contains("spain")
            || lower.contains("madrid")
        {
            return Self::Spain;
        }
        if name.contains("🇮🇪")
            || name.contains("爱尔兰")
            || has_region_token(&lower, "ie")
            || lower.contains("ireland")
            || lower.contains("dublin")
        {
            return Self::Ireland;
        }
        if name.contains("🇵🇱")
            || name.contains("波兰")
            || has_region_token(&lower, "pl")
            || lower.contains("poland")
            || lower.contains("warsaw")
        {
            return Self::Poland;
        }
        if name.contains("🇺🇦")
            || name.contains("乌克兰")
            || has_region_token(&lower, "ua")
            || lower.contains("ukraine")
            || lower.contains("kyiv")
        {
            return Self::Ukraine;
        }
        if name.contains("🇨🇳")
            || name.contains("中国")
            || name.contains("中國")
            || name.contains("回国")
            || has_region_token(&lower, "cn")
            || lower.contains("china")
        {
            return Self::China;
        }
        Self::Other
    }
}

/// The identity shown beside a proxy node. Geographic resolution has exactly
/// one owner ([`GeoRegion::from_name_heuristic`]); the three routing targets
/// are explicit products facts, never geographic regions and never flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeRegion {
    Geographic(GeoRegion),
    Direct,
    Reject,
    Global,
}

impl NodeRegion {
    /// Classify a node name from its region aliases or its routing target.
    pub fn classify(name: &str) -> Self {
        let lower = name.trim().to_ascii_lowercase();
        if lower == "direct" || lower.starts_with("direct-") || lower.contains("直连") {
            return Self::Direct;
        }
        if lower == "reject" || lower.starts_with("reject-") || lower.contains("拒绝") {
            return Self::Reject;
        }
        if lower == "global" || lower.starts_with("global-") || lower.contains("全局") {
            return Self::Global;
        }
        Self::Geographic(GeoRegion::from_name_heuristic(name))
    }

    /// Uppercase search token. Pseudo-targets keep their own words instead of
    /// being silently folded into the unknown geographic region.
    pub const fn search_code(self) -> &'static str {
        match self {
            Self::Geographic(region) => region.iso_code(),
            Self::Direct => "DIRECT",
            Self::Reject => "REJECT",
            Self::Global => "GLOBAL",
        }
    }

    /// Uppercase ISO code only when a rectangular flag asset exists.
    pub const fn flag_code(self) -> Option<&'static str> {
        match self {
            Self::Geographic(GeoRegion::Other) | Self::Direct | Self::Reject | Self::Global => None,
            Self::Geographic(region) => Some(region.iso_code()),
        }
    }

    /// Text drawn on the square plate when no flag asset applies.
    pub const fn plate_label(self) -> &'static str {
        match self {
            Self::Direct => "DIRECT",
            Self::Reject => "REJECT",
            Self::Global => "GLOBAL",
            Self::Geographic(GeoRegion::Other) => "—",
            Self::Geographic(region) => region.iso_code(),
        }
    }
}

impl Display for GeoRegion {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.iso_code())
    }
}

/// Token boundary matching so ordinary words do not produce false country codes.
fn has_region_token(lower: &str, code: &str) -> bool {
    lower
        .split(|ch: char| !ch.is_ascii_alphabetic())
        .any(|word| word == code)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
