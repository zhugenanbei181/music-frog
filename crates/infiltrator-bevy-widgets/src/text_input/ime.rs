//! CJK IME preedit segmentation, character classification, and cursor area geometry.

use bevy::ecs::component::Component;
use bevy::math::Vec2;

/// Classification of a preedit segment in CJK IME composition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PreeditClauseState {
    /// Raw uncommitted input (e.g. latin pinyin syllables).
    #[default]
    Raw,
    /// Currently focused / active candidate clause being selected.
    Selected,
    /// Converted clause that is not currently selected.
    Converted,
}

/// A segmented clause within an IME preedit composition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreeditClause {
    /// Text content of this clause.
    pub text: String,
    /// State / classification of this clause.
    pub state: PreeditClauseState,
    /// Character range `(start, end)` within the full preedit string.
    pub range: (usize, usize),
}

impl PreeditClause {
    /// Construct a new preedit clause.
    pub fn new(text: impl Into<String>, state: PreeditClauseState, range: (usize, usize)) -> Self {
        Self {
            text: text.into(),
            state,
            range,
        }
    }

    /// Whether this clause is the active / selected one.
    pub fn is_selected(&self) -> bool {
        self.state == PreeditClauseState::Selected
    }
}

/// Status of the IME Preedit composition state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PreeditStatus {
    /// No active composition.
    #[default]
    Idle,
    /// Active composition in progress.
    Composing,
    /// Composition just committed.
    Committed,
}

/// State machine for IME preedit composition and CJK syllable/clause segmentation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreeditStateMachine {
    raw_text: String,
    clauses: Vec<PreeditClause>,
    cursor: usize,
    active_clause: Option<usize>,
    status: PreeditStatus,
}

/// Snapshot of text field state prior to an IME composition session,
/// enabling transaction rollback on cancellation (e.g. Escape / Cancel).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImeTransaction {
    /// Controlled text when the transaction began.
    pub original_text: String,
    /// Caret character position when the transaction began.
    pub original_cursor: usize,
    /// Selection anchor when the transaction began.
    pub original_anchor: Option<usize>,
}

/// Absolute screen rectangle for the IME cursor / composition area.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ImeCursorArea {
    /// Top-left position in window/screen pixel coordinates.
    pub position: Vec2,
    /// Dimensions (width, height) of the cursor / composition area in pixels.
    pub size: Vec2,
}

impl ImeCursorArea {
    /// Construct a new cursor area from position and size.
    pub fn new(position: Vec2, size: Vec2) -> Self {
        Self { position, size }
    }

    /// Construct a new cursor area from individual scalar bounds.
    pub fn from_rect(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            position: Vec2::new(x, y),
            size: Vec2::new(width, height),
        }
    }

    pub fn x(&self) -> f32 {
        self.position.x
    }

    pub fn y(&self) -> f32 {
        self.position.y
    }

    pub fn width(&self) -> f32 {
        self.size.x
    }

    pub fn height(&self) -> f32 {
        self.size.y
    }

    pub fn min(&self) -> Vec2 {
        self.position
    }

    pub fn right(&self) -> f32 {
        self.position.x + self.size.x
    }

    pub fn bottom(&self) -> f32 {
        self.position.y + self.size.y
    }
}

/// Parameters for calculating the absolute IME cursor area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImeCursorAreaParams {
    /// Absolute top-left position of the text field bounding box on screen.
    pub field_origin: Vec2,
    /// Total bounding size of the text field.
    pub field_size: Vec2,
    /// Left / top padding of the text container inside the field.
    pub padding: Vec2,
    /// Horizontal offset in pixels from the start of the text to the caret.
    pub caret_offset_x: f32,
    /// Caret width in pixels (typically 2px).
    pub caret_width: f32,
    /// Caret height in pixels (typically line height / control square size).
    pub caret_height: f32,
    /// Width of any active preedit string in pixels (0 if no preedit).
    pub preedit_width: f32,
}

/// Pure function: compute the absolute screen coordinates for the IME cursor area.
pub fn compute_ime_cursor_area(params: ImeCursorAreaParams) -> ImeCursorArea {
    let x = params.field_origin.x + params.padding.x + params.caret_offset_x;
    let y = params.field_origin.y
        + params.padding.y
        + ((params.field_size.y - params.padding.y * 2.0 - params.caret_height).max(0.0) * 0.5);
    let width = if params.preedit_width > 0.0 {
        params.preedit_width.max(params.caret_width)
    } else {
        params.caret_width
    };
    ImeCursorArea {
        position: Vec2::new(x, y),
        size: Vec2::new(width, params.caret_height),
    }
}

/// Estimate text advance width in pixels based on character classes (CJK/wide vs ASCII).
pub fn estimate_text_width(text: &str, font_size: f32) -> f32 {
    let mut width = 0.0;
    for c in text.chars() {
        if is_cjk_or_wide(c) {
            width += font_size;
        } else {
            width += font_size * 0.55;
        }
    }
    width
}

/// Whether a character is CJK ideograph, Hangul, Kana, fullwidth, or emoji.
pub fn is_cjk_or_wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F | // Hangul Jamo
        0x2E80..=0xA4CF | // CJK Radicals, Kangxi, Ideographic, Hiragana, Katakana, Bopomofo, CJK Unified Ideographs, Yi
        0xAC00..=0xD7A3 | // Hangul Syllables
        0xF900..=0xFAFF | // CJK Compatibility Ideographs
        0xFE30..=0xFE4F | // CJK Compatibility Forms
        0xFF00..=0xFF60 | // Fullwidth Forms
        0xFFE0..=0xFFE6 | // Fullwidth Signs
        0x1F300..=0x1F9FF // Emojis & Pictographs
    )
}

/// Find previous word boundary character index starting backwards from `cursor_chars`.
pub fn find_prev_word_boundary(text: &str, cursor_chars: usize) -> usize {
    if cursor_chars == 0 || text.is_empty() {
        return 0;
    }
    let chars: Vec<char> = text.chars().collect();
    let mut idx = cursor_chars.min(chars.len());

    // Skip trailing whitespace if immediately before cursor
    while idx > 0 && chars[idx - 1].is_whitespace() {
        idx -= 1;
    }

    // Skip word characters
    while idx > 0 && !chars[idx - 1].is_whitespace() {
        idx -= 1;
    }

    idx
}

/// Find next word boundary character index starting forwards from `cursor_chars`.
pub fn find_next_word_boundary(text: &str, cursor_chars: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    if cursor_chars >= len {
        return len;
    }
    let mut idx = cursor_chars;

    // Skip current word characters
    while idx < len && !chars[idx].is_whitespace() {
        idx += 1;
    }

    // Skip whitespace following the word
    while idx < len && chars[idx].is_whitespace() {
        idx += 1;
    }

    idx
}

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

    /// Move active clause selection to the previous clause.
    pub fn prev_clause(&mut self) -> bool {
        if self.clauses.is_empty() {
            return false;
        }
        let current = self.active_clause.unwrap_or(0);
        if current > 0 {
            self.select_clause(current - 1)
        } else {
            false
        }
    }

    /// Move active clause selection to the next clause.
    pub fn next_clause(&mut self) -> bool {
        if self.clauses.is_empty() {
            return false;
        }
        let current = self.active_clause.unwrap_or(0);
        if current + 1 < self.clauses.len() {
            self.select_clause(current + 1)
        } else {
            false
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
