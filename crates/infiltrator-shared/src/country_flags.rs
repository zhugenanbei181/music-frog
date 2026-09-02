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
    Philippines,
    Thailand,
    Malaysia,
    Vietnam,
    UnitedArabEmirates,
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
            Region::Philippines => "🇵🇭",
            Region::Thailand => "🇹🇭",
            Region::Malaysia => "🇲🇾",
            Region::Vietnam => "🇻🇳",
            Region::UnitedArabEmirates => "🇦🇪",
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
            Region::Philippines => "PH",
            Region::Thailand => "TH",
            Region::Malaysia => "MY",
            Region::Vietnam => "VN",
            Region::UnitedArabEmirates => "AE",
            Region::China => "CN",
            Region::Direct => "DIRECT",
            Region::Reject => "REJECT",
            Region::Global => "GLOBAL",
        }
    }
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
        || name.contains("HK")
        || name.contains("Hkg")
        || lower.contains("hong kong")
        || lower.contains("hongkong")
        || lower.contains("hong-kong")
        || lower.contains("hk-")
        || lower.starts_with("hk")
    {
        return Some(Region::HongKong);
    }

    // Match Taiwan
    if name.contains("台湾")
        || name.contains("台灣")
        || name.contains("台北")
        || name.contains("台中")
        || name.contains("高雄")
        || name.contains("TW")
        || lower.contains("taiwan")
        || lower.contains("taipei")
        || lower.starts_with("tw")
    {
        return Some(Region::Taiwan);
    }

    // Match Japan
    if name.contains("日本")
        || name.contains("东京")
        || name.contains("大阪")
        || name.contains("福冈")
        || name.contains("JP")
        || lower.contains("japan")
        || lower.contains("tokyo")
        || lower.contains("osaka")
        || lower.starts_with("jp")
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
        || name.contains("US")
        || name.contains("USA")
        || lower.contains("united states")
        || lower.contains("america")
        || lower.contains("los angeles")
        || lower.contains("san jose")
        || lower.contains("seattle")
        || lower.starts_with("us")
    {
        return Some(Region::UnitedStates);
    }

    // Match Singapore
    if name.contains("新加坡")
        || name.contains("狮城")
        || name.contains("SG")
        || lower.contains("singapore")
        || lower.starts_with("sg")
    {
        return Some(Region::Singapore);
    }

    // Match South Korea
    if name.contains("韩国")
        || name.contains("韓國")
        || name.contains("首尔")
        || name.contains("KR")
        || lower.contains("korea")
        || lower.contains("seoul")
        || lower.starts_with("kr")
    {
        return Some(Region::SouthKorea);
    }

    // Match United Kingdom
    if name.contains("英国")
        || name.contains("英國")
        || name.contains("伦敦")
        || name.contains("UK")
        || name.contains("GB")
        || lower.contains("united kingdom")
        || lower.contains("london")
        || lower.starts_with("uk")
        || lower.starts_with("gb")
    {
        return Some(Region::UnitedKingdom);
    }

    // Match Germany
    if name.contains("德国")
        || name.contains("德國")
        || name.contains("法兰克福")
        || name.contains("柏林")
        || name.contains("DE")
        || lower.contains("germany")
        || lower.contains("deutschland")
        || lower.contains("frankfurt")
        || lower.starts_with("de")
    {
        return Some(Region::Germany);
    }

    // Match France
    if name.contains("法国")
        || name.contains("法國")
        || name.contains("巴黎")
        || name.contains("FR")
        || lower.contains("france")
        || lower.contains("paris")
        || lower.starts_with("fr")
    {
        return Some(Region::France);
    }

    // Match Canada
    if name.contains("加拿大")
        || name.contains("多伦多")
        || name.contains("温哥华")
        || name.contains("CA")
        || lower.contains("canada")
        || lower.contains("toronto")
        || lower.contains("vancouver")
        || lower.starts_with("ca")
    {
        return Some(Region::Canada);
    }

    // Match Australia
    if name.contains("澳大利亚")
        || name.contains("澳洲")
        || name.contains("悉尼")
        || name.contains("墨尔本")
        || name.contains("AU")
        || lower.contains("australia")
        || lower.contains("sydney")
        || lower.contains("melbourne")
        || lower.starts_with("au")
    {
        return Some(Region::Australia);
    }

    // Match Russia
    if name.contains("俄罗斯")
        || name.contains("俄羅斯")
        || name.contains("莫斯科")
        || name.contains("RU")
        || lower.contains("russia")
        || lower.contains("moscow")
        || lower.starts_with("ru")
    {
        return Some(Region::Russia);
    }

    // Match India
    if name.contains("印度")
        || name.contains("孟买")
        || name.contains("IN")
        || lower.contains("india")
        || lower.contains("mumbai")
        || lower.starts_with("in")
    {
        return Some(Region::India);
    }

    // Match Netherlands
    if name.contains("荷兰")
        || name.contains("荷蘭")
        || name.contains("阿姆斯特丹")
        || name.contains("NL")
        || lower.contains("netherlands")
        || lower.contains("amsterdam")
        || lower.starts_with("nl")
    {
        return Some(Region::Netherlands);
    }

    // Match Brazil
    if name.contains("巴西")
        || name.contains("圣保罗")
        || name.contains("BR")
        || lower.contains("brazil")
        || lower.contains("sao paulo")
        || lower.starts_with("br")
    {
        return Some(Region::Brazil);
    }

    // Match Turkey
    if name.contains("土耳其")
        || name.contains("伊斯坦布尔")
        || name.contains("TR")
        || lower.contains("turkey")
        || lower.contains("istanbul")
        || lower.starts_with("tr")
    {
        return Some(Region::Turkey);
    }

    // Match Argentina
    if name.contains("阿根廷")
        || name.contains("AR")
        || lower.contains("argentina")
        || lower.starts_with("ar")
    {
        return Some(Region::Argentina);
    }

    // Match Philippines
    if name.contains("菲律宾")
        || name.contains("菲律賓")
        || name.contains("马尼拉")
        || name.contains("PH")
        || lower.contains("philippines")
        || lower.contains("manila")
        || lower.starts_with("ph")
    {
        return Some(Region::Philippines);
    }

    // Match Thailand
    if name.contains("泰国")
        || name.contains("泰國")
        || name.contains("曼谷")
        || name.contains("TH")
        || lower.contains("thailand")
        || lower.contains("bangkok")
        || lower.starts_with("th")
    {
        return Some(Region::Thailand);
    }

    // Match Malaysia
    if name.contains("马来西亚")
        || name.contains("馬來西亞")
        || name.contains("吉隆坡")
        || name.contains("MY")
        || lower.contains("malaysia")
        || lower.starts_with("my")
    {
        return Some(Region::Malaysia);
    }

    // Match Vietnam
    if name.contains("越南")
        || name.contains("胡志明")
        || name.contains("河内")
        || name.contains("VN")
        || lower.contains("vietnam")
        || lower.starts_with("vn")
    {
        return Some(Region::Vietnam);
    }

    // Match UAE / Dubai
    if name.contains("阿联酋")
        || name.contains("迪拜")
        || name.contains("AE")
        || lower.contains("dubai")
        || lower.contains("uae")
        || lower.starts_with("ae")
    {
        return Some(Region::UnitedArabEmirates);
    }

    // Match China / Direct
    if name.contains("中国")
        || name.contains("中國")
        || name.contains("回国")
        || name.contains("国内")
        || name.contains("CN")
        || lower.contains("china")
        || lower.starts_with("cn")
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
}
