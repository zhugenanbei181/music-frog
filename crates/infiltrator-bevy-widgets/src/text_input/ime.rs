//! IME preedit segmentation and state-machine behavior for [`super`].
//!
//! The public state types stay defined by the parent `text_input` module so
//! their canonical paths and the widget API do not move. This child owns the
//! implementation boundary for Pinyin segmentation and clause navigation.

use super::{PreeditClause, PreeditClauseState, PreeditStateMachine, PreeditStatus};

/// Standard Pinyin syllables sorted by descending length for greedy clause segmentation.
const PINYIN_SYLLABLES: &[&str] = &[
    // 6 letters
    "zhuang", "shuang", "chuang", // 5 letters
    "zheng", "zhong", "zhuai", "zhang", "xiang", "xiong", "shang", "sheng", "shuai", "shuan",
    "kuang", "guang", "jiang", "jiong", "huang", "liang", "niang", "qiang", "qiong", "chang",
    "cheng", "chuan", "chong", "chuai", // 4 letters
    "zhao", "zhai", "zhan", "zhen", "zhua", "zhuo", "zhui", "zhun", "zhou", "chao", "chai", "chan",
    "chen", "chua", "chuo", "chui", "chun", "chou", "shao", "shai", "shan", "shen", "shua", "shuo",
    "shui", "shun", "shou", "biao", "piao", "miao", "diao", "tiao", "niao", "liao", "jiao", "qiao",
    "xiao", "bian", "pian", "mian", "dian", "tian", "nian", "lian", "jian", "qian", "xian", "guan",
    "kuan", "huan", "duan", "tuan", "nuan", "luan", "zuan", "cuan", "suan", "ruan", "yuan", "juan",
    "quan", "xuan", "guai", "kuai", "huai", "bing", "ping", "ming", "ding", "ting", "ning", "ling",
    "jing", "qing", "xing", "ying", "dong", "tong", "nong", "long", "gong", "kong", "hong", "zong",
    "cong", "song", "yong", "wang", "yang", "rang", "lang", "dang", "tang", "nang", "gang", "kang",
    "hang", "zang", "cang", "sang", "weng", "feng", "deng", "teng", "neng", "leng", "geng", "keng",
    "heng", "reng", "zeng", "ceng", "seng", "meng", "peng", "beng", // 3 letters
    "zha", "zhe", "zhi", "zhu", "cha", "che", "chi", "chu", "sha", "she", "shi", "shu", "bai",
    "pai", "mai", "dai", "tai", "nai", "lai", "gai", "kai", "hai", "zai", "cai", "sai", "wai",
    "bei", "pei", "mei", "fei", "dei", "tei", "nei", "lei", "gei", "kei", "hei", "zei", "cei",
    "sei", "wei", "shei", "zhei", "bao", "pao", "mao", "dao", "tao", "nao", "lao", "gao", "kao",
    "hao", "zao", "cao", "sao", "rao", "yao", "dou", "tou", "nou", "lou", "gou", "kou", "hou",
    "zou", "cou", "sou", "rou", "you", "liu", "niu", "miu", "diu", "jiu", "qiu", "xiu", "ban",
    "pan", "man", "fan", "dan", "tan", "nan", "lan", "gan", "kan", "han", "zan", "can", "san",
    "ran", "wan", "yan", "ben", "pen", "men", "fen", "den", "ten", "nen", "len", "gen", "ken",
    "hen", "zen", "cen", "sen", "ren", "wen", "yin", "pin", "bin", "min", "nin", "lin", "jin",
    "qin", "xin", "gua", "kua", "hua", "guo", "kuo", "huo", "gui", "kui", "hui", "gun", "kun",
    "hun", "duo", "tuo", "nuo", "luo", "ruo", "zuo", "cuo", "suo", "dui", "tui", "rui", "zui",
    "cui", "sui", "dun", "tun", "nun", "lun", "run", "zun", "cun", "sun", "bie", "pie", "mie",
    "die", "tie", "nie", "lie", "jie", "qie", "xie", "yue", "jue", "que", "xue", "ang", "eng",
    "dia", "lia", "jia", "qia", "xia", // 2 letters
    "ba", "pa", "ma", "fa", "da", "ta", "na", "la", "ga", "ka", "ha", "za", "ca", "sa", "ra", "wa",
    "ya", "bo", "po", "mo", "fo", "lo", "wo", "yo", "me", "de", "te", "ne", "le", "ge", "ke", "he",
    "ze", "ce", "se", "re", "ye", "bi", "pi", "mi", "di", "ti", "ni", "li", "ji", "qi", "xi", "zi",
    "ci", "si", "ri", "yi", "bu", "pu", "mu", "fu", "du", "tu", "nu", "lu", "gu", "ku", "hu", "zu",
    "cu", "su", "ru", "wu", "yu", "nv", "lv", "qu", "xu", "ju", "er", "ai", "ei", "ao", "ou", "an",
    "en", "in", "un", // 1 letter
    "a", "o", "e",
];

fn match_pinyin_syllable(s: &str) -> usize {
    for &syllable in PINYIN_SYLLABLES {
        if s.starts_with(syllable) {
            return syllable.chars().count();
        }
    }
    0
}

impl PreeditStateMachine {
    /// Create a new idle preedit state machine.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether an active composition is currently in progress.
    pub fn is_composing(&self) -> bool {
        self.status == PreeditStatus::Composing && !self.raw_text.is_empty()
    }

    /// The current status of the state machine.
    pub fn status(&self) -> PreeditStatus {
        self.status
    }

    /// The raw unsegmented preedit string.
    pub fn raw_text(&self) -> &str {
        &self.raw_text
    }

    /// The segmented preedit clauses.
    pub fn clauses(&self) -> &[PreeditClause] {
        &self.clauses
    }

    /// Cursor position in character index within the preedit string.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Index of the active / selected clause, if any.
    pub fn active_clause_index(&self) -> Option<usize> {
        self.active_clause
    }

    /// The active / selected clause, if any.
    pub fn active_clause(&self) -> Option<&PreeditClause> {
        self.active_clause.and_then(|idx| self.clauses.get(idx))
    }

    /// Total character length of the preedit composition.
    pub fn total_chars(&self) -> usize {
        self.raw_text.chars().count()
    }

    /// Update preedit text with optional cursor position (in char index).
    /// Performs rule-based syllable and delimiter segmentation into clauses.
    pub fn update(&mut self, text: impl Into<String>, cursor: Option<usize>) -> bool {
        let text = text.into();
        if text.is_empty() {
            return self.cancel();
        }

        let changed = self.raw_text != text;
        self.raw_text = text;
        self.clauses = Self::segment_pinyin(&self.raw_text);
        let char_count = self.raw_text.chars().count();
        self.cursor = cursor.unwrap_or(char_count).min(char_count);

        if !self.clauses.is_empty() {
            self.active_clause = Some(self.clause_index_at_cursor(self.cursor).unwrap_or(0));
            self.update_clause_states();
        } else {
            self.active_clause = None;
        }

        self.status = PreeditStatus::Composing;
        changed
    }

    /// Segment a raw composition string into Pinyin / CJK clauses.
    /// Handles explicit apostrophe delimiters (e.g. "xi'an") and standard Pinyin syllable boundaries.
    pub fn segment_pinyin(raw: &str) -> Vec<PreeditClause> {
        if raw.is_empty() {
            return Vec::new();
        }

        let mut clauses = Vec::new();
        let mut char_offset = 0;

        for part in raw.split(['\'', ' ']) {
            if part.is_empty() {
                char_offset += 1;
                continue;
            }

            let sub_clauses = Self::segment_syllables(part, char_offset);
            char_offset += part.chars().count() + 1;
            clauses.extend(sub_clauses);
        }

        if clauses.is_empty() {
            let count = raw.chars().count();
            clauses.push(PreeditClause::new(
                raw,
                PreeditClauseState::Selected,
                (0, count),
            ));
        }

        clauses
    }

    fn segment_syllables(chunk: &str, base_offset: usize) -> Vec<PreeditClause> {
        let mut result = Vec::new();
        let lower = chunk.to_lowercase();
        let chars: Vec<char> = lower.chars().collect();
        let original_chars: Vec<char> = chunk.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let remaining: String = chars[i..].iter().collect();
            let matched_len = match_pinyin_syllable(&remaining);
            let len = if matched_len > 0 { matched_len } else { 1 };

            let seg_text: String = original_chars[i..i + len].iter().collect();
            let start = base_offset + i;
            let end = start + len;
            result.push(PreeditClause::new(
                seg_text,
                PreeditClauseState::Raw,
                (start, end),
            ));
            i += len;
        }

        result
    }

    fn clause_index_at_cursor(&self, cursor: usize) -> Option<usize> {
        for (i, clause) in self.clauses.iter().enumerate() {
            if cursor >= clause.range.0 && cursor <= clause.range.1 {
                return Some(i);
            }
        }
        if !self.clauses.is_empty() {
            Some(self.clauses.len() - 1)
        } else {
            None
        }
    }

    fn update_clause_states(&mut self) {
        let active = self.active_clause;
        for (i, clause) in self.clauses.iter_mut().enumerate() {
            clause.state = if Some(i) == active {
                PreeditClauseState::Selected
            } else {
                PreeditClauseState::Converted
            };
        }
    }

    /// Select a specific clause by index.
    pub fn select_clause(&mut self, index: usize) -> bool {
        if index < self.clauses.len() {
            self.active_clause = Some(index);
            self.update_clause_states();
            true
        } else {
            false
        }
    }

    /// Navigate to next clause.
    pub fn next_clause(&mut self) -> bool {
        if let Some(curr) = self.active_clause
            && curr + 1 < self.clauses.len()
        {
            return self.select_clause(curr + 1);
        }
        false
    }

    /// Navigate to previous clause.
    pub fn prev_clause(&mut self) -> bool {
        if let Some(curr) = self.active_clause
            && curr > 0
        {
            return self.select_clause(curr - 1);
        }
        false
    }

    /// Commit the preedit composition, returning the text to insert.
    pub fn commit(&mut self) -> String {
        let text = std::mem::take(&mut self.raw_text);
        self.clauses.clear();
        self.cursor = 0;
        self.active_clause = None;
        self.status = PreeditStatus::Committed;
        text
    }

    /// Cancel the composition and reset to Idle.
    pub fn cancel(&mut self) -> bool {
        let was_composing = self.is_composing();
        self.raw_text.clear();
        self.clauses.clear();
        self.cursor = 0;
        self.active_clause = None;
        self.status = PreeditStatus::Idle;
        was_composing
    }
}
