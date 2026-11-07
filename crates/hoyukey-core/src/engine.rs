// hoyukey-core: Clean Dynamic Engine
// Quáº£n lÃ½ tráº¡ng thÃ¡i gÃµ vÃ  Ä‘iá» u phá»‘i biáº¿n Ä‘á»•i dá»±a trÃªn ngá»¯ Ã¢m há» c Ä‘á»™ng vÃ  minimal delta.

use crate::config::{InputMethod, TypingConfig};
use crate::delta::compute_minimal_delta;
use crate::phonotactics::is_valid_vietnamese_phonotactics;
use crate::telex;
use crate::vni;

#[derive(Debug, PartialEq, Eq)]
pub enum KeyAction {
    /// PhÃ­m Ä‘Æ°á»£c truyá» n Ä‘i tá»± nhiÃªn khÃ´ng cáº§n can thiá»‡p
    PassThrough,
    /// Cháº·n phÃ­m gá»‘c, gá»­i N phÃ­m Backspace vÃ  gÃµ chuá»—i thay tháº¿
    Transform {
        backspaces: usize,
        text: String,
    },
    /// Cháº·n phÃ­m gá»‘c, gá»­i Delete xÃ³a Ä‘uÃ´i, Backspace xÃ³a Ä‘áº§u vÃ  gÃµ chuá»—i thay tháº¿
    TransformCaret {
        deletes: usize,
        backspaces: usize,
        text: String,
    },
    /// Ä áº·t láº¡i buffer tá»«
    Reset,
    /// Xá»­ lÃ½ phÃ­m Backspace
    Backspace,
}

#[derive(Clone, Debug)]
pub struct TypedWord {
    pub word: String,
    pub raw: String,
    pub transformed: bool,
    pub was_line_start: bool,
    pub was_sentence_start: bool,
}

#[derive(Clone, Debug)]
pub struct EngineStep {
    pub display_word: String,
    pub raw_keys: String,
    pub has_transformed: bool,
}

pub struct Engine {
    pub current_word: String,
    pub raw_keys: String,
    pub has_transformed: bool,
    pub history: Vec<EngineStep>,
    pub last_word_before_space: Option<(String, String, bool)>,
    pub line_words: Vec<TypedWord>,
    pub spaces_after_word: usize,
    pub caret_offset: Option<usize>,
    pub at_start_of_line: bool,
    pub after_sentence_end: bool,
    pub saw_sentence_end_punct: bool,
    pub was_at_line_start: bool,
    pub was_at_sentence_start: bool,
    pub has_typed_in_line: bool,
    pub last_char_typed: Option<char>,
    pub has_space_before: bool,
    pub caret_needs_check: bool,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            current_word: String::new(),
            raw_keys: String::new(),
            has_transformed: false,
            history: Vec::new(),
            last_word_before_space: None,
            line_words: Vec::new(),
            spaces_after_word: 0,
            caret_offset: None,
            at_start_of_line: true,
            after_sentence_end: false,
            saw_sentence_end_punct: false,
            was_at_line_start: false,
            was_at_sentence_start: false,
            has_typed_in_line: false,
            last_char_typed: None,
            has_space_before: false,
            caret_needs_check: false,
        }
    }

    pub fn reset(&mut self) {
        self.current_word.clear();
        self.raw_keys.clear();
        self.has_transformed = false;
        self.history.clear();
        self.last_word_before_space = None;
        self.caret_offset = None;
        self.has_space_before = false;
        self.caret_needs_check = false;
        self.spaces_after_word = 0;
    }

    pub fn on_enter(&mut self) {
        self.reset();
        self.at_start_of_line = true;
        self.has_typed_in_line = false;
        self.line_words.clear();
        self.last_char_typed = Some('\n');
    }

    pub fn on_arrow_left(&mut self) {
        if let Some(offset) = self.caret_offset {
            if offset > 0 {
                self.caret_offset = Some(offset - 1);
            }
        } else if !self.current_word.is_empty() {
            let count = self.current_word.chars().count();
            if count > 0 {
                self.caret_offset = Some(count - 1);
            }
        }
    }

    pub fn on_arrow_right(&mut self) {
        if let Some(offset) = self.caret_offset {
            let count = self.current_word.chars().count();
            if offset < count {
                self.caret_offset = Some(offset + 1);
            }
        }
    }

    pub fn on_mouse_click(&mut self) {
        self.reset();
        self.at_start_of_line = false;
        self.after_sentence_end = false;
        self.saw_sentence_end_punct = false;
        self.caret_needs_check = true;
    }

    pub fn on_window_change(&mut self) {
        self.reset();
        self.at_start_of_line = false;
        self.after_sentence_end = false;
        self.saw_sentence_end_punct = false;
        self.caret_needs_check = false;
    }

    pub fn should_auto_period_before_enter(&self) -> bool {
        if self.current_word.is_empty() {
            return false;
        }
        let last = self.current_word.chars().last();
        if let Some(c) = last {
            !matches!(c, '.' | '?' | '!' | ':' | ';' | ',' | '\n' | '\r')
        } else {
            false
        }
    }

    pub fn handle_backspace(&mut self, _simple_telex: bool) {
        if !self.current_word.is_empty() {
            self.current_word.pop();
            self.raw_keys.pop();
            if !self.history.is_empty() {
                self.history.pop();
            }
            if self.current_word.is_empty() {
                self.has_transformed = false;
                self.raw_keys.clear();
                self.history.clear();
            } else {
                // Tự động kiểm tra lại trạng thái biến đổi sau khi xóa ký tự
                self.has_transformed = has_vietnamese_diacritic(&self.current_word);
                if !self.has_transformed {
                    self.raw_keys = self.current_word.clone();
                }
            }
        } else if self.spaces_after_word > 0 {
            self.spaces_after_word -= 1;
            if self.spaces_after_word == 0 {
                if let Some((word, raw, trans)) = self.last_word_before_space.take() {
                    self.current_word = word;
                    self.raw_keys = raw;
                    self.has_transformed = trans;
                }
            }
        }
    }

    /// Xá»­ lÃ½ 1 kÃ½ tá»± gÃµ vÃ o theo kiáº¿n trÃºc ngá»¯ Ã¢m há» c Ä‘á»™ng vÃ  delta tá»‘i thiá»ƒu
    pub fn process_char(&mut self, mut ch: char, config: &TypingConfig) -> KeyAction {
        if !config.enabled {
            self.current_word.push(ch);
            self.raw_keys.push(ch);
            self.last_char_typed = Some(ch);
            return KeyAction::PassThrough;
        }

        // 1. PhÃ¢n tÃ¡ch ranh giá»›i tá»« (khoáº£ng tráº¯ng, dáº¥u cÃ¢u)
        if is_word_boundary(ch) {
            let was_space = ch == ' ';
            if was_space {
                self.spaces_after_word += 1;
                self.has_space_before = true;
            } else {
                self.has_space_before = false;
                self.spaces_after_word = 0;
            }

            if !self.current_word.is_empty() {
                self.line_words.push(TypedWord {
                    word: self.current_word.clone(),
                    raw: self.raw_keys.clone(),
                    transformed: self.has_transformed,
                    was_line_start: self.was_at_line_start,
                    was_sentence_start: self.was_at_sentence_start,
                });
                self.last_word_before_space = Some((
                    self.current_word.clone(),
                    self.raw_keys.clone(),
                    self.has_transformed,
                ));
            }

            // Ghi nháº­n káº¿t thÃºc cÃ¢u
            if matches!(ch, '.' | '?' | '!') {
                self.saw_sentence_end_punct = true;
            } else if was_space && self.saw_sentence_end_punct {
                self.after_sentence_end = true;
                self.saw_sentence_end_punct = false;
            }

            self.current_word.clear();
            self.raw_keys.clear();
            self.has_transformed = false;
            self.history.clear();
            self.caret_offset = None;
            self.last_char_typed = Some(ch);
            return KeyAction::PassThrough;
        }

        // 2. Tá»± Ä‘á»™ng viáº¿t hoa Ä‘áº§u dÃ²ng / sau dáº¥u cháº¥m
        // QUY Táº®C Sá» T: CHá»ˆ viáº¿t hoa khi tá»« Ä‘ang trá»‘ng (current_word.is_empty()), khÃ´ng bao giá»  viáº¿t hoa giá»¯a tá»«!
        let mut did_auto_caps = false;
        if self.current_word.is_empty() {
            let should_caps = (self.at_start_of_line && config.auto_caps_line)
                || (self.after_sentence_end && config.auto_caps_sentence);

            if should_caps && ch.is_ascii_lowercase() {
                ch = ch.to_ascii_uppercase();
                did_auto_caps = true;
            }

            self.was_at_line_start = self.at_start_of_line;
            self.was_at_sentence_start = self.after_sentence_end;
            self.at_start_of_line = false;
            self.after_sentence_end = false;
            self.saw_sentence_end_punct = false;
            self.has_typed_in_line = true;
        }

        self.last_char_typed = Some(ch);
        self.spaces_after_word = 0;
        self.has_space_before = false;

        // 3. Thá»±c hiá»‡n biáº¿n Ä‘á»•i dá»±a trÃªn phÃ­m gÃµ
        let transformed = match config.input_method {
            InputMethod::Telex => telex::transform_telex(
                &self.current_word,
                ch,
                config.modern_tone,
                config.free_marking,
                config.smart_english,
                config.simple_telex,
                Some(&self.raw_keys),
            ),
            InputMethod::Vni => vni::transform_vni(
                &self.current_word,
                ch,
                config.modern_tone,
                config.free_marking,
                config.smart_english,
            ),
        };

        if let Some(new_word) = transformed {
            if new_word != self.current_word {
                let delta = compute_minimal_delta(&self.current_word, &new_word);
                self.current_word = new_word.clone();

                let is_still_transformed = has_vietnamese_diacritic(&self.current_word);
                if is_still_transformed {
                    self.raw_keys.push(ch);
                    self.has_transformed = true;
                } else {
                    // Người dùng gõ phím lặp lại để thoát dấu (ví dụ: 'té' + 's' -> 'tes', 'đ' + 'd' -> 'dd', 'â' + 'a' -> 'aa')
                    // Đồng bộ lại raw_keys và tắt trạng thái has_transformed để không bị auto-restore sai lệch!
                    self.raw_keys = self.current_word.clone();
                    self.has_transformed = false;
                }

                self.history.push(EngineStep {
                    display_word: self.current_word.clone(),
                    raw_keys: self.raw_keys.clone(),
                    has_transformed: self.has_transformed,
                });

                return KeyAction::Transform {
                    backspaces: delta.backspaces,
                    text: delta.text,
                };
            }
        }

        // 4. Không biến đổi: Kiểm tra tự động khôi phục tiếng Anh (Auto-Restore)
        // Chỉ kích hoạt khi từ hiển thị mang đặc trưng tiếng Anh hoặc chứa cụm phụ âm tiếng Anh (fast, test, react, system...)
        // TUYỆT ĐỐI không khôi phục nhầm các từ tiếng Việt như 'ưu' (u-w-u) hay 'đay' (d-a-d-y)
        if config.auto_restore && self.has_transformed {
            let candidate_restored = format!("{}{}", self.raw_keys, ch);
            let candidate_with_ch = format!("{}{}", self.current_word, ch);

            if should_auto_restore_english(&candidate_with_ch, &candidate_restored) {
                let delta = compute_minimal_delta(&self.current_word, &candidate_restored);
                self.current_word = candidate_restored.clone();
                self.raw_keys = candidate_restored.clone();
                self.has_transformed = false;
                self.history.clear();

                return KeyAction::Transform {
                    backspaces: delta.backspaces,
                    text: delta.text,
                };
            }
        }

        // 5. GÃµ kÃ½ tá»± thÃ´ng thÆ°á» ng
        // Nếu thêm một phụ âm cuối vào âm tiết đã có dấu thanh, đặt lại vị trí dấu thanh
        // theo quy tắc thanh điệu tiếng Việt (ví dụ: "ngóa" + "t" -> "ngoát").
        if !did_auto_caps && ch.is_ascii_alphabetic() {
            let lower = ch.to_ascii_lowercase();
            let is_vowel = matches!(lower, 'a' | 'e' | 'i' | 'o' | 'u' | 'y' | 'w');
            if !is_vowel && has_vietnamese_diacritic(&self.current_word) {
                let decomp = crate::phonotactics::decompose_word(&self.current_word);
                if let Some(d) = decomp {
                    let tone = d.current_tone();
                    if tone != crate::vietnamese::Tone::None {
                        let candidate = format!("{}{}", self.current_word, ch);
                        let mut d2 = crate::phonotactics::decompose_word(&candidate).unwrap_or(d.clone());
                        d2.apply_tone(tone, config.modern_tone);
                        let new_word = d2.recompose();
                        if new_word != format!("{}{}", self.current_word, ch) {
                            // tone moved; transform
                            let delta = compute_minimal_delta(&self.current_word, &new_word);
                            // append ch to raw_keys normally
                            self.raw_keys.push(ch);
                            self.current_word = new_word.clone();
                            self.has_transformed = has_vietnamese_diacritic(&self.current_word);
                            self.history.push(EngineStep {
                                display_word: self.current_word.clone(),
                                raw_keys: self.raw_keys.clone(),
                                has_transformed: self.has_transformed,
                            });
                            return KeyAction::Transform {
                                backspaces: delta.backspaces,
                                text: delta.text,
                            };
                        }
                    }
                }
            }
        }

        if did_auto_caps {
            // Chuyá»ƒn thÃ nh chá»¯ hoa Ä‘áº§u dÃ²ng
            self.current_word.push(ch);
            self.raw_keys.push(ch);
            self.history.push(EngineStep {
                display_word: self.current_word.clone(),
                raw_keys: self.raw_keys.clone(),
                has_transformed: false,
            });
            return KeyAction::Transform {
                backspaces: 0,
                text: ch.to_string(),
            };
        }

        self.current_word.push(ch);
        self.raw_keys.push(ch);
        self.history.push(EngineStep {
            display_word: self.current_word.clone(),
            raw_keys: self.raw_keys.clone(),
            has_transformed: false,
        });

        KeyAction::PassThrough
    }
}

/// Kiểm tra xem có cần khôi phục lại chuỗi tiếng Anh hay không (Auto-Restore)
pub fn should_auto_restore_english(candidate_with_ch: &str, candidate_restored: &str) -> bool {
    let lower_disp = candidate_with_ch.to_lowercase();
    let lower_raw = candidate_restored.to_lowercase();

    // Nếu từ gốc bắt đầu bằng cụm phụ âm đặc trưng của tiếng Việt (qu, ng, nh, ch, th, tr, kh, ph, gh, gi),
    // đây gần như chắc chắn là từ tiếng Việt (quát, ngoát, khuỵu, ngoẻo...), không khôi phục tiếng Anh.
    let vietnamese_leads = ["qu", "ng", "nh", "ch", "th", "tr", "kh", "ph", "gh", "gi"];
    if vietnamese_leads.iter().any(|lead| lower_raw.starts_with(lead)) {
        return false;
    }

    // 1. Nếu từ hiển thị bắt đầu bằng phụ âm ngoại lai không có trong tiếng Việt:
    // f, j, z, hoặc w + nguyên âm (web, win, wifi)
    if let Some(first) = lower_disp.chars().next() {
        if matches!(first, 'f' | 'j' | 'z') {
            return true;
        }
        if first == 'w' && lower_disp.chars().count() > 1 {
            if let Some(second) = lower_disp.chars().nth(1) {
                if crate::phonotactics::is_vietnamese_vowel_char(second) {
                    return true;
                }
            }
        }
    }

    // 2. Nếu từ hiển thị có nhiều hơn 1 cụm nguyên âm tách biệt (từ đa âm tiết tiếng Anh):
    // re-lease, sys-tem, sta-tus, com-pu-ter, in-ter-net...
    if crate::phonotactics::has_multiple_vowel_clusters(&lower_disp) {
        return true;
    }

    // 3. Nếu chuỗi phím gốc (raw) hoặc từ hiển thị kết thúc bằng cụm phụ âm/hậu tố tiếng Anh đặc trưng:
    let english_ending_clusters = [
        "st", "sk", "sp", "ft", "ct", "pt", "lt", "mp", "nd", "nk", "nt",
        "rk", "rd", "rm", "rn", "rp", "rs", "tch", "ts", "ff", "ss", "ll", "ck", "sh",
        "er", "re", "ce", "se", "ty", "ly", "ry", "al", "ment",
    ];
    for &cluster in &english_ending_clusters {
        if lower_raw.ends_with(cluster) || lower_disp.ends_with(cluster) {
            return true;
        }
    }

    // 4. Nếu chuỗi phím gốc hoặc từ hiển thị bắt đầu bằng cụm phụ âm tiếng Anh đặc trưng:
    let english_starting_clusters = [
        "str", "spr", "spl", "scr", "br", "cr", "dr", "fr", "gr", "pr",
        "cl", "fl", "gl", "pl", "sl", "sc", "sk", "sm", "sn", "sp", "st", "sw", "tw", "wr",
    ];
    for &cluster in &english_starting_clusters {
        if lower_raw.starts_with(cluster) || lower_disp.starts_with(cluster) {
            return true;
        }
    }

    false
}

/// Kiểm tra xem biến đổi có xảy ra hoàn toàn ở phía sau con trỏ (dùng cho dynamic caret UI Automation)
pub fn is_modification_behind_caret(post_word: &str, new_word: &str) -> bool {
    let post_len = post_word.chars().count();
    let new_chars: Vec<char> = new_word.chars().collect();
    let post_chars: Vec<char> = post_word.chars().collect();
    if new_chars.len() < post_len {
        return false;
    }
    new_chars[new_chars.len() - post_len..] == post_chars[..]
}

/// Kiểm tra ký tự ranh giới từ (không bao gồm '[' và ']' vì dùng gõ chữ 'ư' và 'ơ' trong Telex)
pub fn is_word_boundary(c: char) -> bool {
    c.is_whitespace() || matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | '(' | ')'
        | '<' | '>' | '/' | '\\' | '|' | '@' | '#' | '$' | '%' | '^' | '&'
        | '*' | '-' | '_' | '+' | '=' | '~' | '`' | '"' | '\''
    )
}

/// Kiểm tra xem một chuỗi có chứa bất kỳ ký tự tiếng Việt có dấu nào không (non-ASCII > 127)
#[inline]
pub fn has_vietnamese_diacritic(word: &str) -> bool {
    word.chars().any(|c| (c as u32) > 127)
}

/// Kiểm tra âm tiết tiếng Việt hợp lệ (dùng cho automation / spellcheck)
pub fn is_valid_vietnamese_syllable(word: &str) -> bool {
    is_valid_vietnamese_phonotactics(word)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_basic_telex() {
        let mut engine = Engine::new();
        let config = TypingConfig::default();

        assert_eq!(engine.process_char('t', &config), KeyAction::PassThrough);
        assert_eq!(engine.process_char('i', &config), KeyAction::PassThrough);
        assert_eq!(engine.process_char('e', &config), KeyAction::PassThrough);

        // 'e' -> 'ê'
        match engine.process_char('e', &config) {
            KeyAction::Transform { backspaces, text } => {
                assert_eq!(backspaces, 1);
                assert_eq!(text, "ê");
            }
            _ => panic!("Expected Transform"),
        }

        assert_eq!(engine.process_char('n', &config), KeyAction::PassThrough);
        assert_eq!(engine.process_char('g', &config), KeyAction::PassThrough);

        // 's' -> 'tiếng'
        match engine.process_char('s', &config) {
            KeyAction::Transform { backspaces, text } => {
                assert_eq!(backspaces, 3); // deletes 'g', 'n', 'ê'
                assert_eq!(text, "ếng");
            }
            _ => panic!("Expected Transform"),
        }

        assert_eq!(engine.current_word, "tiếng");
    }

    #[test]
    fn test_engine_auto_restore_english() {
        let mut engine = Engine::new();
        let config = TypingConfig::default();

        engine.process_char('f', &config);
        engine.process_char('a', &config);
        // 's' makes 'fá'
        engine.process_char('s', &config);
        assert_eq!(engine.current_word, "fá");

        // 't' makes 'fast' (auto restore English)
        match engine.process_char('t', &config) {
            KeyAction::Transform { backspaces, text } => {
                assert_eq!(backspaces, 1);
                assert_eq!(text, "ast");
            }
            _ => panic!("Expected Transform"),
        }
        assert_eq!(engine.current_word, "fast");
    }

    #[test]
    fn test_engine_auto_caps_start_of_line_only() {
        let mut engine = Engine::new();
        let mut config = TypingConfig::default();
        config.auto_caps_line = true;

        // First char at line start
        match engine.process_char('x', &config) {
            KeyAction::Transform { backspaces, text } => {
                assert_eq!(backspaces, 0);
                assert_eq!(text, "X");
            }
            _ => panic!("Expected Transform with capitalized X"),
        }

        // Subsequent char in same word must NOT be capitalized
        assert_eq!(engine.process_char('i', &config), KeyAction::PassThrough);
        assert_eq!(engine.process_char('n', &config), KeyAction::PassThrough);
        assert_eq!(engine.current_word, "Xin");
    }

    #[test]
    fn test_engine_complex_words_telex() {
        let mut engine = Engine::new();
        let config = TypingConfig::default();

        // "quát" -> q, u, a, s, t
        for c in "quast".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "quát");
        engine.reset();

        // "nghiêng" -> n, g, h, i, e, e, n, g
        for c in "nghieeng".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "nghiêng");
        engine.reset();

        // "hoàng" -> h, o, a, n, g, f
        for c in "hoangf".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "hoàng");
        engine.reset();

        // "thuyền" -> t, h, u, y, e, e, n, f
        for c in "thuyeenf".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "thuyền");
    }

    #[test]
    fn test_engine_user_reported_cases() {
        let mut engine = Engine::new();
        let config = TypingConfig::default();

        // 1. "chỉ" -> c, h, i, r
        for c in "chir".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "chỉ");
        engine.reset();

        // 2. "ưu" -> u, w, u
        for c in "uwu".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "ưu");
        engine.reset();

        // 3. "đay" -> d, a, d, y
        for c in "dady".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "đay");
        engine.reset();

        // 4. "đây" -> d, d, a, y
        for c in "dday".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "đay");
    }

    #[test]
    fn test_engine_bracket_shortcuts() {
        let mut engine = Engine::new();
        let config = TypingConfig::default();

        // '[' -> 'ư'
        engine.process_char('[', &config);
        assert_eq!(engine.current_word, "ư");
        // '[[' -> '['
        engine.process_char('[', &config);
        assert_eq!(engine.current_word, "[");
        engine.reset();

        // ']' -> 'ơ'
        engine.process_char(']', &config);
        assert_eq!(engine.current_word, "ơ");
        // ']]' -> ']'
        engine.process_char(']', &config);
        assert_eq!(engine.current_word, "]");
        engine.reset();

        // 'b]' -> 'bơ'
        engine.process_char('b', &config);
        engine.process_char(']', &config);
        assert_eq!(engine.current_word, "bơ");
        engine.reset();

        // 'du]' -> 'dươ'
        engine.process_char('d', &config);
        engine.process_char('u', &config);
        engine.process_char(']', &config);
        assert_eq!(engine.current_word, "dươ");
        engine.reset();

        // 't[' -> 'tư'
        engine.process_char('t', &config);
        engine.process_char('[', &config);
        assert_eq!(engine.current_word, "tư");
    }

    #[test]
    fn test_engine_vni() {
        let mut engine = Engine::new();
        let mut config = TypingConfig::default();
        config.input_method = InputMethod::Vni;

        for c in "toan1".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "toán");
        engine.reset();

        for c in "duong7".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "dương");
    }

    #[test]
    fn test_engine_double_tap_undo() {
        let mut engine = Engine::new();
        let config = TypingConfig::default();

        // "toanss" -> "toans"
        for c in "toanss".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "toans");
    }

    #[test]
    fn test_engine_english_auto_restore_and_escapes() {
        let mut engine = Engine::new();
        let config = TypingConfig::default();

        // 1. "test" gõ trực tiếp t-e-s-t: s tạo 'té', t kích hoạt auto-restore về "test"
        for c in "test".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "test");
        assert!(!engine.has_transformed);
        assert_eq!(engine.raw_keys, "test");
        engine.reset();

        // 2. "test" gõ theo kiểu double-tap s: t-e-s-s-t
        // t-e -> "te"
        // s -> "té"
        // s -> "tes" (thoát dấu sắc, không bị biến thành 'tess')
        // t -> "test" (không bị auto-restore thành 'tesst')
        for c in "tesst".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "test");
        assert!(!engine.has_transformed);
        assert_eq!(engine.raw_keys, "test");
        engine.reset();

        // 3. "fast" -> f-a-s-t
        for c in "fast".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "fast");
        engine.reset();

        // 4. "react" -> r-e-a-c-t
        for c in "react".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "react");
        engine.reset();

        // 5. "task" -> t-a-s-k
        for c in "task".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "task");
    }

    #[test]
    fn test_engine_backspace_workflow() {
        let mut engine = Engine::new();
        let config = TypingConfig::default();

        for c in "toan".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "toan");

        engine.handle_backspace(false);
        assert_eq!(engine.current_word, "toa");

        engine.process_char('g', &config);
        assert_eq!(engine.current_word, "toag");
    }

    #[test]
    fn test_production_stress_test_matrix() {
        let mut engine = Engine::new();
        let config = TypingConfig::default();

        let type_sentence = |engine: &mut Engine, input: &str| -> String {
            engine.reset();
            let mut out = String::new();
            for c in input.chars() {
                if is_word_boundary(c) {
                    engine.process_char(c, &config);
                    out.push(c);
                } else {
                    let action = engine.process_char(c, &config);
                    match action {
                        KeyAction::PassThrough => {
                            out.push(c);
                        }
                        KeyAction::Transform { backspaces, text } => {
                            for _ in 0..backspaces {
                                out.pop();
                            }
                            out.push_str(&text);
                        }
                        _ => {}
                    }
                }
            }
            out
        };

        let type_single_word = |engine: &mut Engine, input: &str| -> String {
            engine.reset();
            for c in input.chars() {
                engine.process_char(c, &config);
            }
            engine.current_word.clone()
        };

        // 0. Full Stress Test Sentence (Chữ thường):
        let input_lowercase = "Chafng trai hoawngx quowrn khuyuj xuoongs, ngoeor coor ngoast mieengj khuays nooid banhs chuwng, hoan hir ngawsm chuoongf hoawngx vaf xooong ngoawngx.";
        let expected_lowercase = "Chàng trai hoẵng quởn khuỵu xuống, ngoẻo cổ ngoát miệng khuáy nôid bánh chưng, hoan hỉ ngắm chuồng hoẵng và xôong ngoẵng.";
        assert_eq!(type_sentence(&mut engine, input_lowercase), expected_lowercase);

        // 0. Full Stress Test Sentence (CapsLock):
        let input_capslock = "CHAFNG TRAI HOAWNGX QUOWRN KHUYUJ XUOONGS, NGOEOR COOR NGOAST MIEENGJ KHUAYS NOOID BANHS CHUWNG, HOAN HIR NGAWSM CHUOONGF HOAWNGX VAF XOOONG NGOAWNGX.";
        let expected_capslock = "CHÀNG TRAI HOẴNG QUỞN KHUỴU XUỐNG, NGOẺO CỔ NGOÁT MIỆNG KHUÁY NÔID BÁNH CHƯNG, HOAN HỈ NGẮM CHUỒNG HOẴNG VÀ XÔONG NGOẴNG.";
        assert_eq!(type_sentence(&mut engine, input_capslock), expected_capslock);

        // 1. Các từ bẫy âm ghép dài & phức tạp:
        assert_eq!(type_single_word(&mut engine, "hoawngx"), "hoẵng");
        assert_eq!(type_single_word(&mut engine, "ngoawngx"), "ngoẵng");
        assert_eq!(type_single_word(&mut engine, "khuyuj"), "khuỵu");
        assert_eq!(type_single_word(&mut engine, "ngoeor"), "ngoẻo");
        assert_eq!(type_single_word(&mut engine, "thuowr"), "thuở");
        assert_eq!(type_single_word(&mut engine, "xooong"), "xôong");
        assert_eq!(type_single_word(&mut engine, "xoong"), "xông");

        // 2. Gõ hoa trộn lẫn (Shift + CapsLock):
        assert_eq!(type_single_word(&mut engine, "Quangr"), "Quảng");
        assert_eq!(type_single_word(&mut engine, "Bachj"), "Bạch");
        assert_eq!(type_single_word(&mut engine, "Dduowngf"), "Đường");

        // CapsLock toàn bộ:
        assert_eq!(type_single_word(&mut engine, "DDUOWNGF"), "ĐƯỜNG");
        assert_eq!(type_single_word(&mut engine, "TRUWOWNGF"), "TRƯỜNG");
        assert_eq!(type_single_word(&mut engine, "HOAWNGX"), "HOẴNG");

        // 3. Thoát dấu lặp phím:
        assert_eq!(type_single_word(&mut engine, "ddd"), "dd");
        assert_eq!(type_single_word(&mut engine, "toanss"), "toans");
        // Kiểm tra logic w chuẩn OpenKey / UniKey:
        // Standalone 'w' -> 'ư', double 'w' ('ww') -> 'w' (KHÔNG PHẢI 'uw')
        assert_eq!(type_single_word(&mut engine, "w"), "ư");
        assert_eq!(type_single_word(&mut engine, "ww"), "w");
        assert_eq!(type_single_word(&mut engine, "www"), "ww");
        assert_eq!(type_single_word(&mut engine, "wwww"), "www");
        assert_eq!(type_single_word(&mut engine, "W"), "Ư");
        assert_eq!(type_single_word(&mut engine, "WW"), "W");
        // 'rw' -> 'rư', 'rww' -> 'rw'
        assert_eq!(type_single_word(&mut engine, "rw"), "rư");
        assert_eq!(type_single_word(&mut engine, "rww"), "rw");
        // 'uw' -> 'ư', 'uww' -> 'uw' (khi đã có 'u' rõ ràng thì thoát về 'uw')
        assert_eq!(type_single_word(&mut engine, "uw"), "ư");
        assert_eq!(type_single_word(&mut engine, "uww"), "uw");
        // 'ruw' -> 'rư', 'ruww' -> 'ruw'
        assert_eq!(type_single_word(&mut engine, "ruw"), "rư");
        assert_eq!(type_single_word(&mut engine, "ruww"), "ruw");

        // Gõ câu chứa khoảng trắng và 'u ww':
        assert_eq!(type_sentence(&mut engine, "u ww"), "u w");

        // Gõ từ có nguyên âm ghép và phụ âm chuẩn:
        assert_eq!(type_single_word(&mut engine, "vaanx"), "vẫn");
        assert_eq!(type_single_word(&mut engine, "ruw"), "rư");
        assert_eq!(type_single_word(&mut engine, "uaanx"), "uẫn");

        // 4. Backspace không mất dấu:
        // "toans" -> "toán", xóa 1 ký tự -> "toá", gõ 'm' -> "toám"
        engine.reset();
        for c in "toans".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "toán");
        engine.handle_backspace(false);
        assert_eq!(engine.current_word, "toá");
        engine.process_char('m', &config);
        assert_eq!(engine.current_word, "toám");

        // "tiếng" -> backspace từng ký tự về đầu không crash
        engine.reset();
        for c in "tieengs".chars() {
            engine.process_char(c, &config);
        }
        assert_eq!(engine.current_word, "tiếng");
        for _ in 0..5 {
            engine.handle_backspace(false);
        }
        assert_eq!(engine.current_word, "");
        assert_eq!(engine.raw_keys, "");
        assert!(!engine.has_transformed);

        // 5. Tiếng Anh / Code không bị dính dấu:
        assert_eq!(type_single_word(&mut engine, "user"), "user");
        assert_eq!(type_single_word(&mut engine, "process"), "process");
        assert_eq!(type_single_word(&mut engine, "commit"), "commit");
        assert_eq!(type_single_word(&mut engine, "install"), "install");
    }
}
