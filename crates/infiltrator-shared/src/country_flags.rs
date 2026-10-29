//! Country flag and geographical region parser for proxy node names.
//!
//! Maps standard airport / VPN provider naming patterns (such as "香港 IEPL",
//! "US-01", "[JP] Tokyo BGP", "Singapore Premium") to their regional ISO
//! code and emoji flag icon for clear visual identification in UI lists.

/// Known geographical regions and special routing destinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    HongKong,
    Taiwan,
    Japan,
    UnitedStates,
    Singapore,
    SouthKorea,
    UnitedKingdom,
    Germany,
    France,
    Canada,
    Australia,
    Russia,
    India,
    Netherlands,
    Brazil,
    Turkey,
    Argentina,
    Indonesia,
    Philippines,
    Thailand,
    Malaysia,
    Vietnam,
    UnitedArabEmirates,
    Macau,
    Switzerland,
    Sweden,
    Norway,
    Finland,
    Italy,
    Spain,
    Ireland,
    Poland,
    Ukraine,
    SouthAfrica,
    Israel,
    China,
    Direct,
    Reject,
    Global,
}

impl Region {
    /// Return the standard country flag emoji (or special symbol) for this region.
    pub const fn emoji(self) -> &'static str {
        match self {
            Region::HongKong => "🇭🇰",
            Region::Taiwan => "🇹🇼",
            Region::Japan => "🇯🇵",
            Region::UnitedStates => "🇺🇸",
            Region::Singapore => "🇸🇬",
            Region::SouthKorea => "🇰🇷",
            Region::UnitedKingdom => "🇬🇧",
            Region::Germany => "🇩🇪",
            Region::France => "🇫🇷",
            Region::Canada => "🇨🇦",
            Region::Australia => "🇦🇺",
            Region::Russia => "🇷🇺",
            Region::India => "🇮🇳",
            Region::Netherlands => "🇳🇱",
            Region::Brazil => "🇧🇷",
            Region::Turkey => "🇹🇷",
            Region::Argentina => "🇦🇷",
            Region::Indonesia => "🇮🇩",
            Region::Philippines => "🇵🇭",
            Region::Thailand => "🇹🇭",
            Region::Malaysia => "🇲🇾",
            Region::Vietnam => "🇻🇳",
            Region::UnitedArabEmirates => "🇦🇪",
            Region::Macau => "🇲🇴",
            Region::Switzerland => "🇨🇭",
            Region::Sweden => "🇸🇪",
            Region::Norway => "🇳🇴",
            Region::Finland => "🇫🇮",
            Region::Italy => "🇮🇹",
            Region::Spain => "🇪🇸",
            Region::Ireland => "🇮🇪",
            Region::Poland => "🇵🇱",
            Region::Ukraine => "🇺🇦",
            Region::SouthAfrica => "🇿🇦",
            Region::Israel => "🇮🇱",
            Region::China => "🇨🇳",
            Region::Direct => "⚡",
            Region::Reject => "🚫",
            Region::Global => "🌐",
        }
    }

    /// Two-letter ISO country code or short identifier.
    pub const fn code(self) -> &'static str {
        match self {
            Region::HongKong => "HK",
            Region::Taiwan => "TW",
            Region::Japan => "JP",
            Region::UnitedStates => "US",
            Region::Singapore => "SG",
            Region::SouthKorea => "KR",
            Region::UnitedKingdom => "GB",
            Region::Germany => "DE",
            Region::France => "FR",
            Region::Canada => "CA",
            Region::Australia => "AU",
            Region::Russia => "RU",
            Region::India => "IN",
            Region::Netherlands => "NL",
            Region::Brazil => "BR",
            Region::Turkey => "TR",
            Region::Argentina => "AR",
            Region::Indonesia => "ID",
            Region::Philippines => "PH",
            Region::Thailand => "TH",
            Region::Malaysia => "MY",
            Region::Vietnam => "VN",
            Region::UnitedArabEmirates => "AE",
            Region::Macau => "MO",
            Region::Switzerland => "CH",
            Region::Sweden => "SE",
            Region::Norway => "NO",
            Region::Finland => "FI",
            Region::Italy => "IT",
            Region::Spain => "ES",
            Region::Ireland => "IE",
            Region::Poland => "PL",
            Region::Ukraine => "UA",
            Region::SouthAfrica => "ZA",
            Region::Israel => "IL",
            Region::China => "CN",
            Region::Direct => "DIRECT",
            Region::Reject => "REJECT",
            Region::Global => "GLOBAL",
        }
    }
}

/// ASCII region abbreviations are complete letter tokens, optionally followed by node digits.
/// Substrings of ordinary names such as "Inspection", "Default" or "Australia" are not codes.
fn has_region_code(lower: &str, code: &str) -> bool {
    lower
        .split(|ch: char| !ch.is_ascii_alphabetic())
        .any(|word| word == code)
}

/// Identify the geographical region from a proxy node name.
pub fn match_region(name: &str) -> Option<Region> {
    let lower = name.trim().to_ascii_lowercase();

    // Check special routing words first
    if lower == "direct" || lower.starts_with("direct-") || lower.contains("直连") {
        return Some(Region::Direct);
    }
    if lower == "reject" || lower.starts_with("reject-") || lower.contains("拒绝") {
        return Some(Region::Reject);
    }
    if lower == "global" || lower.starts_with("global-") || lower.contains("全局") {
        return Some(Region::Global);
    }

    // Match Hong Kong
    if name.contains("香港")
        || has_region_code(&lower, "hk")
        || has_region_code(&lower, "hkg")
        || lower.contains("hong kong")
        || lower.contains("hongkong")
        || lower.contains("hong-kong")
    {
        return Some(Region::HongKong);
    }

    // Match Taiwan
    if name.contains("台湾")
        || name.contains("台灣")
        || name.contains("台北")
        || name.contains("台中")
        || name.contains("高雄")
        || has_region_code(&lower, "tw")
        || lower.contains("taiwan")
        || lower.contains("taipei")
    {
        return Some(Region::Taiwan);
    }

    // Match Japan
    if name.contains("日本")
        || name.contains("东京")
        || name.contains("大阪")
        || name.contains("福冈")
        || has_region_code(&lower, "jp")
        || lower.contains("japan")
        || lower.contains("tokyo")
        || lower.contains("osaka")
    {
        return Some(Region::Japan);
    }

    // Match United States
    if name.contains("美国")
        || name.contains("美國")
        || name.contains("洛杉矶")
        || name.contains("圣何塞")
        || name.contains("西雅图")
        || name.contains("硅谷")
        || name.contains("芝加哥")
        || name.contains("纽约")
        || has_region_code(&lower, "us")
        || has_region_code(&lower, "usa")
        || lower.contains("united states")
        || lower.contains("america")
        || lower.contains("los angeles")
        || lower.contains("san jose")
        || lower.contains("seattle")
    {
        return Some(Region::UnitedStates);
    }

    // Match Singapore
    if name.contains("新加坡")
        || name.contains("狮城")
        || has_region_code(&lower, "sg")
        || lower.contains("singapore")
    {
        return Some(Region::Singapore);
    }

    // Match South Korea
    if name.contains("韩国")
        || name.contains("韓國")
        || name.contains("首尔")
        || has_region_code(&lower, "kr")
        || lower.contains("korea")
        || lower.contains("seoul")
    {
        return Some(Region::SouthKorea);
    }

    // Match United Kingdom
    if name.contains("英国")
        || name.contains("英國")
        || name.contains("伦敦")
        || has_region_code(&lower, "uk")
        || has_region_code(&lower, "gb")
        || lower.contains("united kingdom")
        || lower.contains("london")
    {
        return Some(Region::UnitedKingdom);
    }

    // Match Germany
    if name.contains("德国")
        || name.contains("德國")
        || name.contains("法兰克福")
        || name.contains("柏林")
        || has_region_code(&lower, "de")
        || lower.contains("germany")
        || lower.contains("deutschland")
        || lower.contains("frankfurt")
    {
        return Some(Region::Germany);
    }

    // Match France
    if name.contains("法国")
        || name.contains("法國")
        || name.contains("巴黎")
        || has_region_code(&lower, "fr")
        || lower.contains("france")
        || lower.contains("paris")
    {
        return Some(Region::France);
    }

    // Match Canada
    if name.contains("加拿大")
        || name.contains("多伦多")
        || name.contains("温哥华")
        || has_region_code(&lower, "ca")
        || lower.contains("canada")
        || lower.contains("toronto")
        || lower.contains("vancouver")
    {
        return Some(Region::Canada);
    }

    // Match Australia
    if name.contains("澳大利亚")
        || name.contains("澳洲")
        || name.contains("悉尼")
        || name.contains("墨尔本")
        || has_region_code(&lower, "au")
        || lower.contains("australia")
        || lower.contains("sydney")
        || lower.contains("melbourne")
    {
        return Some(Region::Australia);
    }

    // Match Russia
    if name.contains("俄罗斯")
        || name.contains("俄羅斯")
        || name.contains("莫斯科")
        || has_region_code(&lower, "ru")
        || lower.contains("russia")
        || lower.contains("moscow")
    {
        return Some(Region::Russia);
    }

    // Match India
    if name.contains("印度")
        || name.contains("孟买")
        || has_region_code(&lower, "in")
        || lower.contains("india")
        || lower.contains("mumbai")
    {
        return Some(Region::India);
    }

    // Match Netherlands
    if name.contains("荷兰")
        || name.contains("荷蘭")
        || name.contains("阿姆斯特丹")
        || has_region_code(&lower, "nl")
        || lower.contains("netherlands")
        || lower.contains("amsterdam")
    {
        return Some(Region::Netherlands);
    }

    // Match Brazil
    if name.contains("巴西")
        || name.contains("圣保罗")
        || has_region_code(&lower, "br")
        || lower.contains("brazil")
        || lower.contains("sao paulo")
    {
        return Some(Region::Brazil);
    }

    // Match Turkey
    if name.contains("土耳其")
        || name.contains("伊斯坦布尔")
        || has_region_code(&lower, "tr")
        || lower.contains("turkey")
        || lower.contains("istanbul")
    {
        return Some(Region::Turkey);
    }

    // Match Argentina
    if name.contains("阿根廷") || has_region_code(&lower, "ar") || lower.contains("argentina") {
        return Some(Region::Argentina);
    }

    // Match Indonesia
    if name.contains("印度尼西亚")
        || name.contains("印尼")
        || name.contains("雅加达")
        || has_region_code(&lower, "id")
        || lower.contains("indonesia")
        || lower.contains("jakarta")
    {
        return Some(Region::Indonesia);
    }

    // Match Philippines
    if name.contains("菲律宾")
        || name.contains("菲律賓")
        || name.contains("马尼拉")
        || has_region_code(&lower, "ph")
        || lower.contains("philippines")
        || lower.contains("manila")
    {
        return Some(Region::Philippines);
    }

    // Match Thailand
    if name.contains("泰国")
        || name.contains("泰國")
        || name.contains("曼谷")
        || has_region_code(&lower, "th")
        || lower.contains("thailand")
        || lower.contains("bangkok")
    {
        return Some(Region::Thailand);
    }

    // Match Malaysia
    if name.contains("马来西亚")
        || name.contains("馬來西亞")
        || name.contains("吉隆坡")
        || has_region_code(&lower, "my")
        || lower.contains("malaysia")
    {
        return Some(Region::Malaysia);
    }

    // Match Vietnam
    if name.contains("越南")
        || name.contains("胡志明")
        || name.contains("河内")
        || has_region_code(&lower, "vn")
        || lower.contains("vietnam")
    {
        return Some(Region::Vietnam);
    }

    // Match UAE / Dubai
    if name.contains("阿联酋")
        || name.contains("迪拜")
        || has_region_code(&lower, "ae")
        || lower.contains("dubai")
        || lower.contains("uae")
    {
        return Some(Region::UnitedArabEmirates);
    }

    // Match Macau
    if name.contains("澳门")
        || name.contains("澳門")
        || has_region_code(&lower, "mo")
        || lower.contains("macau")
        || lower.contains("macao")
    {
        return Some(Region::Macau);
    }

    // Match Switzerland
    if name.contains("瑞士")
        || has_region_code(&lower, "ch")
        || lower.contains("switzerland")
        || lower.contains("zurich")
    {
        return Some(Region::Switzerland);
    }

    // Match Sweden
    if name.contains("瑞典")
        || has_region_code(&lower, "se")
        || lower.contains("sweden")
        || lower.contains("stockholm")
    {
        return Some(Region::Sweden);
    }

    // Match Norway
    if name.contains("挪威")
        || has_region_code(&lower, "no")
        || lower.contains("norway")
        || lower.contains("oslo")
    {
        return Some(Region::Norway);
    }

    // Match Finland
    if name.contains("芬兰")
        || has_region_code(&lower, "fi")
        || lower.contains("finland")
        || lower.contains("helsinki")
    {
        return Some(Region::Finland);
    }

    // Match Italy
    if name.contains("意大利")
        || has_region_code(&lower, "it")
        || lower.contains("italy")
        || lower.contains("milan")
        || lower.contains("rome")
    {
        return Some(Region::Italy);
    }

    // Match Spain
    if name.contains("西班牙")
        || has_region_code(&lower, "es")
        || lower.contains("spain")
        || lower.contains("madrid")
    {
        return Some(Region::Spain);
    }

    // Match Ireland
    if name.contains("爱尔兰")
        || has_region_code(&lower, "ie")
        || lower.contains("ireland")
        || lower.contains("dublin")
    {
        return Some(Region::Ireland);
    }

    // Match Poland
    if name.contains("波兰")
        || has_region_code(&lower, "pl")
        || lower.contains("poland")
        || lower.contains("warsaw")
    {
        return Some(Region::Poland);
    }

    // Match Ukraine
    if name.contains("乌克兰")
        || has_region_code(&lower, "ua")
        || lower.contains("ukraine")
        || lower.contains("kyiv")
    {
        return Some(Region::Ukraine);
    }

    // Match South Africa
    if name.contains("南非")
        || has_region_code(&lower, "za")
        || lower.contains("south africa")
        || lower.contains("johannesburg")
    {
        return Some(Region::SouthAfrica);
    }

    // Match Israel
    if name.contains("以色列")
        || has_region_code(&lower, "il")
        || lower.contains("israel")
        || lower.contains("tel aviv")
    {
        return Some(Region::Israel);
    }

    // Match China / Direct
    if name.contains("中国")
        || name.contains("中國")
        || name.contains("回国")
        || name.contains("国内")
        || has_region_code(&lower, "cn")
        || lower.contains("china")
    {
        return Some(Region::China);
    }

    None
}

/// Convenience helper to return the flag emoji for a node name, or 🌐 if unrecognized.
pub fn node_flag_emoji(name: &str) -> &'static str {
    match_region(name).map(|r| r.emoji()).unwrap_or("🌐")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_flag_matching() {
        assert_eq!(match_region("香港 IEPL-01"), Some(Region::HongKong));
        assert_eq!(match_region("HK-02"), Some(Region::HongKong));
        assert_eq!(match_region("🇭🇰 香港 01"), Some(Region::HongKong));
        assert_eq!(match_region("日本 NTT BGP"), Some(Region::Japan));
        assert_eq!(match_region("JP-Tokyo-01"), Some(Region::Japan));
        assert_eq!(match_region("美国 CN2 GIA"), Some(Region::UnitedStates));
        assert_eq!(match_region("US Los Angeles"), Some(Region::UnitedStates));
        assert_eq!(match_region("新加坡 BGP"), Some(Region::Singapore));
        assert_eq!(match_region("SG-01"), Some(Region::Singapore));
        assert_eq!(match_region("台湾 Hinet 01"), Some(Region::Taiwan));
        assert_eq!(match_region("TW Taipei"), Some(Region::Taiwan));
        assert_eq!(match_region("英国 伦敦 01"), Some(Region::UnitedKingdom));
        assert_eq!(match_region("Germany Frankfurt"), Some(Region::Germany));
        assert_eq!(match_region("DIRECT"), Some(Region::Direct));
        assert_eq!(match_region("REJECT"), Some(Region::Reject));
        assert_eq!(match_region("GLOBAL"), Some(Region::Global));
    }

    #[test]
    fn test_node_flag_emoji_fallback() {
        assert_eq!(node_flag_emoji("香港 IEPL"), "🇭🇰");
        assert_eq!(node_flag_emoji("US-01"), "🇺🇸");
        assert_eq!(node_flag_emoji("SomeUnmatchedNodeName"), "🌐");
    }

    #[test]
    fn abbreviations_cannot_invent_a_region_from_an_ordinary_word_or_protocol() {
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
            assert_eq!(match_region(name), None, "unexpected region for {name}");
            assert_eq!(node_flag_emoji(name), "🌐");
        }
        for (name, region) in [
            ("[IN] 01", Region::India),
            ("in-02", Region::India),
            ("us01", Region::UnitedStates),
            ("[de] Premium", Region::Germany),
            ("HKG-03", Region::HongKong),
            ("[UK] Premium", Region::UnitedKingdom),
        ] {
            assert_eq!(match_region(name), Some(region));
        }
    }
}
