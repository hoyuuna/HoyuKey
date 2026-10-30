pub mod telex;
pub mod vietnamese;
pub mod vni;

use crate::config::{Config, InputMethod, SwitchKey};
use crate::engine::vietnamese::{decompose_vietnamese_char, Tone, VowelMod};

#[derive(Debug, PartialEq, Eq)]
pub enum KeyAction {
    /// Phím được truyền đi tự nhiên không cần can thiệp
    PassThrough,
    /// Chặn phím gốc, gửi N phím Backspace và gõ chuỗi thay thế
    Transform {
        backspaces: usize,
        text: String,
    },
    /// Đặt lại buffer từ
    Reset,
    /// Xử lý phím Backspace
    Backspace,
}

pub struct Engine {
    pub current_word: String,
    pub raw_keys: String,
    pub has_transformed: bool,
    last_word_before_space: Option<(String, String, bool)>,
    pub at_start_of_line: bool,
    pub after_sentence_end: bool,
    pub saw_sentence_end_punct: bool,
    pub has_typed_in_line: bool,
    pub last_char_typed: Option<char>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            current_word: String::new(),
            raw_keys: String::new(),
            has_transformed: false,
            last_word_before_space: None,
            at_start_of_line: true,
            after_sentence_end: false,
            saw_sentence_end_punct: false,
            has_typed_in_line: false,
            last_char_typed: None,
        }
    }

    pub fn reset(&mut self) {
        self.current_word.clear();
        self.raw_keys.clear();
        self.has_transformed = false;
        self.last_word_before_space = None;
    }

    pub fn on_enter(&mut self) {
        self.reset();
        self.at_start_of_line = true;
        self.after_sentence_end = false;
        self.saw_sentence_end_punct = false;
        self.has_typed_in_line = false;
        self.last_char_typed = Some('\n');
    }

    pub fn on_window_change(&mut self) {
        self.reset();
        self.at_start_of_line = true;
        self.after_sentence_end = false;
        self.saw_sentence_end_punct = false;
        self.has_typed_in_line = false;
        self.last_char_typed = None;
    }

    pub fn should_auto_period_before_enter(&self) -> bool {
        if !self.has_typed_in_line {
            return false;
        }
        match self.last_char_typed {
            Some(c) => !matches!(c, '.' | '?' | '!' | ':' | ';' | ',' | '-' | '\n' | '\r'),
            None => false,
        }
    }

    /// Xử lý phím Backspace:
    /// Đồng bộ xóa ký tự trong bộ đệm và cho phím Backspace vật lý đi qua tự nhiên.
    /// Nếu vừa gõ xong từ rồi gõ space (ví dụ "dụ "), ấn Backspace xóa space sẽ khôi phục từ để có thể gõ tiếp.
    pub fn handle_backspace(&mut self) -> Option<KeyAction> {
        if self.current_word.is_empty() {
            if let Some((word, raw, transformed)) = self.last_word_before_space.take() {
                self.current_word = word;
                self.raw_keys = raw;
                self.has_transformed = transformed;
            }
            return None;
        }

        self.current_word.pop();
        self.raw_keys.pop();
        if self.current_word.is_empty() {
            self.raw_keys.clear();
            self.has_transformed = false;
        }
        None
    }

    /// Xử lý một ký tự gõ vào
    pub fn process_char(&mut self, mut c: char, config: &Config) -> KeyAction {
        if !config.enabled {
            self.reset();
            return KeyAction::PassThrough;
        }

        // 1. Phím khoảng trắng (Space / Tab): ngắt từ và tạm lưu từ vừa gõ phòng khi cần sửa ngay
        if c.is_whitespace() {
            self.last_char_typed = Some(c);
            if self.saw_sentence_end_punct {
                self.after_sentence_end = true;
            }
            if !self.current_word.is_empty() {
                self.last_word_before_space = Some((
                    self.current_word.clone(),
                    self.raw_keys.clone(),
                    self.has_transformed,
                ));
                self.current_word.clear();
                self.raw_keys.clear();
                self.has_transformed = false;
            }
            return KeyAction::PassThrough;
        }

        // 2. Dấu câu / phân cách câu (phẩy, chấm, hai chấm, ngoặc...):
        // Kết thúc cụm từ hoàn toàn, reset sạch buffer để không bao giờ bị nhảy nhầm chữ ("luôn, " không bị thành "luoon")
        if is_word_boundary(c) {
            self.last_char_typed = Some(c);
            if matches!(c, '.' | '?' | '!') {
                self.saw_sentence_end_punct = true;
                self.after_sentence_end = false;
            } else {
                self.saw_sentence_end_punct = false;
                self.after_sentence_end = false;
            }
            self.reset();
            return KeyAction::PassThrough;
        }

        // Bắt đầu một từ mới: xóa lịch sử từ cũ
        if self.current_word.is_empty() {
            self.last_word_before_space = None;
            self.raw_keys.clear();
            self.has_transformed = false;
        }

        if self.current_word.len() > 30 {
            self.reset();
        }

        // 3. Xử lý tính năng Tự động viết hoa (Auto Capitalization)
        let should_capitalize = c.is_alphabetic()
            && ((config.auto_caps_line && self.at_start_of_line)
                || (config.auto_caps_sentence && self.after_sentence_end));

        let was_lowercase = c.is_ascii_lowercase();
        if should_capitalize {
            self.at_start_of_line = false;
            self.after_sentence_end = false;
            self.saw_sentence_end_punct = false;
            c = c.to_uppercase().next().unwrap_or(c);
        } else if c.is_alphabetic() {
            self.at_start_of_line = false;
            self.after_sentence_end = false;
            self.saw_sentence_end_punct = false;
        }

        self.has_typed_in_line = true;
        self.last_char_typed = Some(c);
        self.raw_keys.push(c);

        // 4. Thử biến đổi theo quy tắc Telex / VNI
        let transformed = match config.input_method {
            InputMethod::Telex => telex::transform_telex(
                &self.current_word,
                c,
                config.modern_tone,
                config.free_marking,
                config.smart_english,
            ),
            InputMethod::Vni => vni::transform_vni(
                &self.current_word,
                c,
                config.modern_tone,
                config.free_marking,
                config.smart_english,
            ),
        };

        if let Some(new_word) = transformed {
            let backspaces = self.current_word.chars().count();
            let old_accents = vietnamese::count_vietnamese_accents(&self.current_word);
            let new_accents = vietnamese::count_vietnamese_accents(&new_word);
            let is_undo = new_accents < old_accents;

            self.current_word = new_word.clone();

            if is_undo {
                // Người dùng chủ động gõ phím để hoàn tác (ví dụ ww -> w, ddd -> dd, aaa -> aa)
                self.raw_keys = new_word.clone();
                self.has_transformed = new_accents > 0;
            } else {
                self.has_transformed = true;

                // Nếu auto_restore bật và từ mới tạo ra là âm tiết không hợp lệ trong tiếng Việt:
                if config.auto_restore && !is_valid_vietnamese_syllable(&self.current_word) {
                    let bs = self.current_word.chars().count();
                    let text = self.raw_keys.clone();
                    self.reset();
                    return KeyAction::Transform {
                        backspaces: bs,
                        text,
                    };
                }
            }

            return KeyAction::Transform {
                backspaces,
                text: new_word,
            };
        }

        // 5. Tự động tái cân bằng vị trí dấu thanh khi sửa hoặc gõ thêm nguyên âm
        if config.modify_tone_in_word
            && c.is_ascii_alphabetic()
            && (!config.smart_english || !telex::is_foreign_word(&self.current_word))
        {
            let existing_tone = find_existing_tone(&self.current_word);
            if existing_tone != Tone::None {
                // Tách dấu thanh cũ ra
                let unaccented_word = remove_tone_from_word(&self.current_word);
                let mut temp_word = unaccented_word;
                temp_word.push(c);

                let tone_key = match existing_tone {
                    Tone::Acute => 's',
                    Tone::Grave => 'f',
                    Tone::Hook => 'r',
                    Tone::Tilde => 'x',
                    Tone::Dot => 'j',
                    _ => ' ',
                };

                if tone_key != ' ' {
                    if let Some(rebalanced_word) = telex::handle_tone_modifier(&temp_word, tone_key, config.modern_tone) {
                        let mut expected_no_move = self.current_word.clone();
                        expected_no_move.push(c);

                        // Chỉ can thiệp nếu vị trí dấu thanh thật sự bị dịch chuyển giữa các nguyên âm
                        if rebalanced_word != expected_no_move {
                            let backspaces = self.current_word.chars().count();
                            self.current_word = rebalanced_word.clone();
                            self.has_transformed = true;

                            if config.auto_restore && !is_valid_vietnamese_syllable(&self.current_word) {
                                let bs = backspaces;
                                let text = self.raw_keys.clone();
                                self.reset();
                                return KeyAction::Transform {
                                    backspaces: bs,
                                    text,
                                };
                            }

                            return KeyAction::Transform {
                                backspaces,
                                text: rebalanced_word,
                            };
                        }
                    }
                }
            }
        }

        // 6. Nếu không có biến đổi:
        // Nếu đã từng biến đổi (ví dụ "tá") và phím c này ("k") làm từ thành không hợp lệ ("ták"):
        if config.auto_restore && self.has_transformed {
            let mut test_word = self.current_word.clone();
            test_word.push(c);
            if !is_valid_vietnamese_syllable(&test_word) {
                let backspaces = self.current_word.chars().count();
                let text = self.raw_keys.clone();
                self.reset();
                return KeyAction::Transform {
                    backspaces,
                    text,
                };
            }
        }

        // Nếu người dùng gõ chữ thường nhưng được viết hoa tự động:
        if should_capitalize && was_lowercase {
            self.current_word.push(c);
            return KeyAction::Transform {
                backspaces: 0,
                text: c.to_string(),
            };
        }

        // Không có biến đổi và từ vẫn hợp lệ: thêm ký tự vào buffer và cho phím đi qua tự nhiên
        self.current_word.push(c);
        KeyAction::PassThrough
    }
}

/// Kiểm tra một từ có phải là âm tiết tiếng Việt hợp lệ theo cấu trúc ngữ âm học hay không.
pub fn is_valid_vietnamese_syllable(word: &str) -> bool {
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return true;
    }

    // 1. Ký tự ngoại lai không có trong tiếng Việt:
    for &c in &chars {
        let cl = c.to_ascii_lowercase();
        if matches!(cl, 'f' | 'j' | 'w' | 'z') {
            return false;
        }
    }

    // 2. Trích xuất vị trí nguyên âm
    let vowel_indices = telex::extract_vowel_indices(&chars);
    if vowel_indices.is_empty() {
        let s: String = chars.iter().map(|c| c.to_ascii_lowercase()).collect();
        return is_valid_initial_consonant_prefix(&s);
    }

    // Các nguyên âm trong 1 âm tiết tiếng Việt bắt buộc phải nằm liền kề nhau:
    for i in 1..vowel_indices.len() {
        if vowel_indices[i] != vowel_indices[i - 1] + 1 {
            return false;
        }
    }

    // Tối đa 3 nguyên âm trong 1 từ tiếng Việt (ví dụ: oai, uôi, yêu, nghiêng)
    if vowel_indices.len() > 3 {
        return false;
    }

    // Kiểm tra cấu trúc cụm nguyên âm (vowel cluster) có hợp lệ không:
    let cluster: String = vowel_indices
        .iter()
        .map(|&idx| {
            if let Some((base, vmod, _, _)) = decompose_vietnamese_char(chars[idx]) {
                match (base, vmod) {
                    ('a', VowelMod::Hat) => 'â',
                    ('a', VowelMod::Breve) => 'ă',
                    ('e', VowelMod::Hat) => 'ê',
                    ('o', VowelMod::Hat) => 'ô',
                    ('o', VowelMod::Horn) => 'ơ',
                    ('u', VowelMod::Horn) => 'ư',
                    (b, _) => b,
                }
            } else {
                chars[idx].to_ascii_lowercase()
            }
        })
        .collect();

    if !is_valid_vowel_cluster(&cluster) {
        return false;
    }

    let first_vowel = vowel_indices[0];
    let last_vowel = *vowel_indices.last().unwrap();

    // 3. Kiểm tra phụ âm đầu
    let initial: String = chars[..first_vowel]
        .iter()
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if !is_valid_initial_consonant(&initial) {
        return false;
    }

    // 4. Kiểm tra phụ âm cuối
    let final_consonants: String = chars[last_vowel + 1..]
        .iter()
        .map(|c| c.to_ascii_lowercase())
        .collect();

    if !is_valid_final_consonant(&final_consonants) {
        return false;
    }

    // 5. Luật ngữ âm học về dấu thanh với phụ âm cuối c, ch, p, t:
    // Các vần kết thúc bằng âm tắc c, ch, p, t chỉ đi kèm thanh Sắc hoặc Nặng (không thể có huyền, hỏi, ngã)
    if matches!(final_consonants.as_str(), "c" | "ch" | "p" | "t") {
        for &idx in &vowel_indices {
            if let Some((_, _, tone, _)) = decompose_vietnamese_char(chars[idx]) {
                if matches!(tone, Tone::Grave | Tone::Hook | Tone::Tilde) {
                    return false;
                }
            }
        }
    }

    true
}

fn is_valid_vowel_cluster(cluster: &str) -> bool {
    matches!(
        cluster,
        // Đơn âm
        "a" | "ă" | "â" | "e" | "ê" | "i" | "y" | "o" | "ô" | "ơ" | "u" | "ư"
        // Nhị trùng âm (diphthong)
        | "ia" | "iê" | "ya" | "yê" | "ua" | "uô" | "ưa" | "ươ"
        | "ai" | "ao" | "au" | "ay" | "âu" | "ây"
        | "eo" | "êu"
        | "iu"
        | "oi" | "ôi" | "ơi"
        | "ui" | "uy" | "uơ" | "uâ" | "ưi" | "ưu"
        | "oa" | "oă" | "oe" | "oo"
        // Tam trùng âm (triphthong)
        | "iêu" | "yêu" | "oai" | "oay" | "oao" | "oeo" | "uôi" | "ươi" | "uya" | "uyê" | "uyu" | "uye"
    )
}

fn is_valid_initial_consonant(s: &str) -> bool {
    matches!(
        s,
        "" | "b"
            | "c"
            | "d"
            | "đ"
            | "g"
            | "h"
            | "k"
            | "l"
            | "m"
            | "n"
            | "p"
            | "r"
            | "s"
            | "t"
            | "v"
            | "x"
            | "ch"
            | "gh"
            | "gi"
            | "kh"
            | "nh"
            | "ng"
            | "ph"
            | "qu"
            | "th"
            | "tr"
            | "ngh"
    )
}

fn is_valid_initial_consonant_prefix(s: &str) -> bool {
    matches!(
        s,
        "" | "b"
            | "c"
            | "d"
            | "đ"
            | "g"
            | "h"
            | "k"
            | "l"
            | "m"
            | "n"
            | "p"
            | "r"
            | "s"
            | "t"
            | "v"
            | "x"
            | "ch"
            | "gh"
            | "gi"
            | "kh"
            | "nh"
            | "ng"
            | "ngh"
            | "ph"
            | "q"
            | "qu"
            | "th"
            | "tr"
    )
}

fn is_valid_final_consonant(s: &str) -> bool {
    matches!(
        s,
        "" | "c" | "ch" | "m" | "n" | "ng" | "nh" | "p" | "t"
    )
}

fn find_existing_tone(word: &str) -> Tone {
    for c in word.chars() {
        if let Some((_, _, tone, _)) = decompose_vietnamese_char(c) {
            if tone != Tone::None {
                return tone;
            }
        }
    }
    Tone::None
}

fn remove_tone_from_word(word: &str) -> String {
    let mut chars = Vec::new();
    for c in word.chars() {
        if let Some((base, vmod, _, is_upper)) = decompose_vietnamese_char(c) {
            let unaccented = vietnamese::get_vietnamese_vowel(base, vmod, Tone::None).unwrap_or(c);
            chars.push(if is_upper { unaccented.to_uppercase().next().unwrap_or(unaccented) } else { unaccented });
        } else {
            chars.push(c);
        }
    }
    chars.into_iter().collect()
}

fn is_word_boundary(c: char) -> bool {
    matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '<'
            | '>' | '/' | '\\' | '|' | '@' | '#' | '$' | '%' | '^' | '&' | '*' | '+' | '=' | '~'
            | '`'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telex_chuyen() {
        let mut engine = Engine::new();
        let config = Config::default();

        for ch in "chuyeen".chars() {
            engine.process_char(ch, &config);
        }
        let action = engine.process_char('r', &config);
        assert_eq!(
            action,
            KeyAction::Transform {
                backspaces: 6,
                text: "chuyển".to_string(),
            }
        );
        assert_eq!(engine.current_word, "chuyển");
    }

    #[test]
    fn test_telex_toi() {
        let mut engine = Engine::new();
        let mut config = Config::default();
        config.auto_caps_line = false;

        for ch in "tooi".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "tôi");
    }

    #[test]
    fn test_writing_assistant() {
        let mut engine = Engine::new();
        let mut config = Config::default();
        config.auto_caps_line = true;
        config.auto_caps_sentence = true;
        config.auto_period_enter = true;

        // 1. Viết hoa đầu dòng: 't' -> "T"
        let a1 = engine.process_char('t', &config);
        assert_eq!(a1, KeyAction::Transform { backspaces: 0, text: "T".to_string() });

        engine.process_char('o', &config);
        engine.process_char('o', &config);
        engine.process_char('i', &config);
        assert_eq!(engine.current_word, "Tôi");

        // gõ dấu chấm và khoảng trắng
        engine.process_char('.', &config);
        engine.process_char(' ', &config);

        // 2. Viết hoa sau dấu chấm: 'b' -> "B"
        let a2 = engine.process_char('b', &config);
        assert_eq!(a2, KeyAction::Transform { backspaces: 0, text: "B".to_string() });
        engine.process_char('a', &config);
        engine.process_char('n', &config);
        engine.process_char('j', &config);
        assert_eq!(engine.current_word, "Bạn");

        // 3. Tự động chấm câu khi xuống dòng:
        assert!(engine.should_auto_period_before_enter());
    }

    #[test]
    fn test_auto_restore_task() {
        let mut engine = Engine::new();
        let config = Config::default();

        // Gõ "tas" -> "tá"
        for ch in "tas".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "tá");

        // Gõ tiếp 'k' -> "ták" không phải từ tiếng Việt, tự động khôi phục "task"
        let action = engine.process_char('k', &config);
        assert_eq!(
            action,
            KeyAction::Transform {
                backspaces: 2,
                text: "task".to_string(),
            }
        );
        assert_eq!(engine.current_word, "");
    }

    #[test]
    fn test_auto_restore_text() {
        let mut engine = Engine::new();
        let config = Config::default();

        // Gõ "tex" -> "tẽ"
        for ch in "tex".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "tẽ");

        // Gõ tiếp 't' -> "tẽt" không thể có dấu ngã với âm t, tự động khôi phục "text"
        let action = engine.process_char('t', &config);
        assert_eq!(
            action,
            KeyAction::Transform {
                backspaces: 2,
                text: "text".to_string(),
            }
        );
    }

    #[test]
    fn test_vni_smart_free_marking() {
        let mut engine = Engine::new();
        let mut config = Config::default();
        config.input_method = InputMethod::Vni;
        config.free_marking = true;

        // "ngan" + '6' -> "ngân"
        for ch in "ngan".chars() {
            engine.process_char(ch, &config);
        }
        let action = engine.process_char('6', &config);
        assert_eq!(
            action,
            KeyAction::Transform {
                backspaces: 4,
                text: "ngân".to_string(),
            }
        );

        // "duong" + '7' -> "dương"
        engine.reset();
        for ch in "duong".chars() {
            engine.process_char(ch, &config);
        }
        let action2 = engine.process_char('7', &config);
        assert_eq!(
            action2,
            KeyAction::Transform {
                backspaces: 5,
                text: "dương".to_string(),
            }
        );

        // "dat" + '9' -> "đat"
        engine.reset();
        for ch in "dat".chars() {
            engine.process_char(ch, &config);
        }
        let action3 = engine.process_char('9', &config);
        assert_eq!(
            action3,
            KeyAction::Transform {
                backspaces: 3,
                text: "đat".to_string(),
            }
        );
    }

    #[test]
    fn test_telex_toggle_modern_tone() {
        let mut engine = Engine::new();
        let mut config = Config::default();
        config.modern_tone = true;

        for ch in "hoaf".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "hoà");

        let mut engine_old = Engine::new();
        let mut config_old = Config::default();
        config_old.modern_tone = false;

        for ch in "hoaf".chars() {
            engine_old.process_char(ch, &config_old);
        }
        assert_eq!(engine_old.current_word, "hòa");
    }

    #[test]
    fn test_telex_toggle_free_marking() {
        let mut engine = Engine::new();
        let mut config = Config::default();
        config.free_marking = false;

        for ch in "hienej".chars() {
            engine.process_char(ch, &config);
        }
        assert_ne!(engine.current_word, "hiện");
    }

    #[test]
    fn test_telex_toggle_smart_english() {
        let mut engine = Engine::new();
        let mut config = Config::default();
        config.smart_english = false;
        config.auto_restore = false;

        for ch in "fix".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "fĩ");
    }

    #[test]
    fn test_telex_toggle_modify_tone_in_word() {
        let mut engine = Engine::new();
        let mut config_on = Config::default();
        config_on.modern_tone = false;
        config_on.modify_tone_in_word = true;

        for ch in "hoaf".chars() {
            engine.process_char(ch, &config_on);
        }
        assert_eq!(engine.current_word, "hòa");
        engine.process_char('n', &config_on);
        assert_eq!(engine.current_word, "hoàn");

        let mut engine_off = Engine::new();
        let mut config_off = Config::default();
        config_off.modern_tone = false;
        config_off.modify_tone_in_word = false;

        for ch in "hoaf".chars() {
            engine_off.process_char(ch, &config_off);
        }
        assert_eq!(engine_off.current_word, "hòa");
        engine_off.process_char('n', &config_off);
        assert_eq!(engine_off.current_word, "hòan");
    }

    #[test]
    fn test_config_persistence() {
        let mut config = Config::default();
        config.input_method = InputMethod::Vni;
        config.modern_tone = false;
        config.switch_key = SwitchKey::AltZ;
        config.auto_restore = false;

        let json = config.to_json_string();
        let mut loaded = Config::default();
        loaded.merge_from_json(&json);

        assert_eq!(loaded.input_method, InputMethod::Vni);
        assert_eq!(loaded.modern_tone, false);
        assert_eq!(loaded.switch_key, SwitchKey::AltZ);
        assert_eq!(loaded.auto_restore, false);
    }

    #[test]
    fn test_telex_ww() {
        let mut engine = Engine::new();
        let config = Config::default();

        // 1. Gõ 'w' lần 1 -> "ư"
        let a1 = engine.process_char('w', &config);
        assert_eq!(
            a1,
            KeyAction::Transform {
                backspaces: 0,
                text: "ư".to_string(),
            }
        );
        assert_eq!(engine.current_word, "ư");

        // 2. Gõ 'w' lần 2 -> hoàn tác thành "w" (chứ không phải "ww")
        let a2 = engine.process_char('w', &config);
        assert_eq!(
            a2,
            KeyAction::Transform {
                backspaces: 1,
                text: "w".to_string(),
            }
        );
        assert_eq!(engine.current_word, "w");

        // 3. Gõ 'w' lần 3 -> ký tự 'w' đi qua tự nhiên thành "ww" (để người dùng có thể gõ www, web...)
        let a3 = engine.process_char('w', &config);
        assert_eq!(a3, KeyAction::PassThrough);
        assert_eq!(engine.current_word, "ww");

        // 4. Gõ 'w' lần 4 -> ký tự 'w' đi qua tự nhiên thành "www"
        let a4 = engine.process_char('w', &config);
        assert_eq!(a4, KeyAction::PassThrough);
        assert_eq!(engine.current_word, "www");
    }

    #[test]
    fn test_telex_tw() {
        let mut engine = Engine::new();
        let config = Config::default();

        let a1 = engine.process_char('t', &config);
        assert_eq!(a1, KeyAction::PassThrough);

        // 't' + 'w' -> "tư"
        let a2 = engine.process_char('w', &config);
        assert_eq!(
            a2,
            KeyAction::Transform {
                backspaces: 1,
                text: "tư".to_string(),
            }
        );
        assert_eq!(engine.current_word, "tư");

        // "tư" + 'w' -> "tw" (hoàn tác)
        let a3 = engine.process_char('w', &config);
        assert_eq!(
            a3,
            KeyAction::Transform {
                backspaces: 2,
                text: "tw".to_string(),
            }
        );
        assert_eq!(engine.current_word, "tw");
    }

    #[test]
    fn test_telex_undo_dd() {
        let mut engine = Engine::new();
        let config = Config::default();

        let a1 = engine.process_char('d', &config);
        assert_eq!(a1, KeyAction::PassThrough);

        let a2 = engine.process_char('d', &config);
        assert_eq!(
            a2,
            KeyAction::Transform {
                backspaces: 1,
                text: "đ".to_string(),
            }
        );

        let a3 = engine.process_char('d', &config);
        assert_eq!(
            a3,
            KeyAction::Transform {
                backspaces: 1,
                text: "dd".to_string(),
            }
        );
        assert_eq!(engine.current_word, "dd");
    }

    #[test]
    fn test_telex_undo_aa() {
        let mut engine = Engine::new();
        let config = Config::default();

        let a1 = engine.process_char('a', &config);
        assert_eq!(a1, KeyAction::PassThrough);

        let a2 = engine.process_char('a', &config);
        assert_eq!(
            a2,
            KeyAction::Transform {
                backspaces: 1,
                text: "â".to_string(),
            }
        );

        let a3 = engine.process_char('a', &config);
        assert_eq!(
            a3,
            KeyAction::Transform {
                backspaces: 1,
                text: "aa".to_string(),
            }
        );
        assert_eq!(engine.current_word, "aa");
    }
}
