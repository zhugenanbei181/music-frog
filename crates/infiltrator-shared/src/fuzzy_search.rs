//! Smart fuzzy and pinyin-initial search utility for node names and rules.

use crate::country_flags::match_region;

/// Extract common Chinese character pinyin initials for regional keywords.
fn to_pinyin_initials(text: &str) -> String {
    let mut initials = String::with_capacity(text.len());
    for ch in text.chars() {
        let initial = match ch {
            '香' => 'x',
            '港' => 'g',
            '台' | '臺' => 't',
            '湾' | '灣' => 'w',
            '日' => 'r',
            '本' => 'b',
            '美' => 'm',
            '国' | '國' => 'g',
            '新' => 'x',
            '加' => 'j',
            '坡' => 'p',
            '韩' | '韓' => 'h',
            '英' => 'y',
            '德' => 'd',
            '法' => 'f',
            '拿' => 'n',
            '大' => 'd',
            '澳' => 'a',
            '洲' => 'z',
            '亚' | '亞' => 'y',
            '俄' => 'e',
            '罗' | '羅' => 'l',
            '斯' => 's',
            '荷' => 'h',
            '兰' | '蘭' => 'l',
            '印' => 'y',
            '度' => 'd',
            '巴' => 'b',
            '西' => 'x',
            '土' => 't',
            '耳' => 'e',
            '其' => 'q',
            '阿' => 'a',
            '根' => 'g',
            '廷' => 't',
            '菲' => 'f',
            '律' => 'l',
            '宾' | '賓' => 'b',
            '泰' => 't',
            '马' | '馬' => 'm',
            '来' | '來' => 'l',
            '越' => 'y',
            '南' => 'n',
            '中' => 'z',
            '华' | '華' => 'h',
            '直' => 'z',
            '连' | '連' => 'l',
            '专' | '專' => 'z',
            '线' | '線' => 'x',
            '节' | '節' => 'j',
            '点' | '點' => 'd',
            '选' | '選' => 'x',
            '择' | '擇' => 'z',
            '漏' => 'l',
            '网' | '網' => 'w',
            '之' => 'z',
            '鱼' | '魚' => 'y',
            _ => ch.to_ascii_lowercase(),
        };
        initials.push(initial);
    }
    initials
}

/// Perform smart matching against a proxy node name:
/// 1. Case-insensitive substring match
/// 2. Pinyin initial abbreviation match (e.g. "xg" matches "香港", "rb" matches "日本")
/// 3. Regional ISO country code match (e.g. "hk" matches "香港 IEPL", "us" matches "美国 GIA")
pub fn pinyin_fuzzy_match(haystack: &str, query: &str) -> bool {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return true;
    }

    let h_lower = haystack.to_ascii_lowercase();
    // 1. Direct substring match
    if h_lower.contains(&q) {
        return true;
    }

    // 2. Pinyin initials match
    let initials = to_pinyin_initials(haystack);
    if initials.contains(&q) {
        return true;
    }

    // 3. Region ISO code match
    if let Some(region) = match_region(haystack) {
        let code = region.code().to_ascii_lowercase();
        if code == q || code.starts_with(&q) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pinyin_fuzzy_match() {
        assert!(pinyin_fuzzy_match("香港 IEPL-01", "xg"));
        assert!(pinyin_fuzzy_match("香港 IEPL-01", "hk"));
        assert!(pinyin_fuzzy_match("香港 IEPL-01", "iepl"));
        assert!(pinyin_fuzzy_match("日本 Tokyo BGP", "rb"));
        assert!(pinyin_fuzzy_match("日本 Tokyo BGP", "jp"));
        assert!(pinyin_fuzzy_match("日本 Tokyo BGP", "tokyo"));
        assert!(pinyin_fuzzy_match("美国 洛杉矶 CN2", "mg"));
        assert!(pinyin_fuzzy_match("美国 洛杉矶 CN2", "us"));
        assert!(pinyin_fuzzy_match("新加坡 狮城", "xjp"));
        assert!(pinyin_fuzzy_match("新加坡 狮城", "sg"));
        assert!(!pinyin_fuzzy_match("香港 01", "tokyo"));
        assert!(!pinyin_fuzzy_match("日本 01", "us"));
    }
}
