pub mod telex;
pub mod vietnamese;
pub mod vni;

use crate::config::{Config, InputMethod};
use crate::engine::vietnamese::{decompose_vietnamese_char, Tone};

#[derive(Debug, PartialEq, Eq)]
pub enum KeyAction {
    /// Phím được truyền đi tự nhiên không cần can thiệp
    PassThrough,
    /// Chặn phím gốc, gửi N phím Backspace và gõ chuỗi thay thế
    Transform {
        backspaces: usize,
        text: String,
    },
    /// Chặn phím gốc, gửi Delete xóa đuôi, Backspace xóa đầu và gõ chuỗi thay thế
    TransformCaret {
        deletes: usize,
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
    pub last_word_before_space: Option<(String, String, bool)>,
    pub caret_offset: Option<usize>,
    pub at_start_of_line: bool,
    pub after_sentence_end: bool,
    pub saw_sentence_end_punct: bool,
    pub has_typed_in_line: bool,
    pub last_char_typed: Option<char>,
    pub has_space_before: bool,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            current_word: String::new(),
            raw_keys: String::new(),
            has_transformed: false,
            last_word_before_space: None,
            caret_offset: None,
            at_start_of_line: true,
            after_sentence_end: false,
            saw_sentence_end_punct: false,
            has_typed_in_line: false,
            last_char_typed: None,
            has_space_before: false,
        }
    }

    pub fn reset(&mut self) {
        self.current_word.clear();
        self.raw_keys.clear();
        self.has_transformed = false;
        self.last_word_before_space = None;
        self.caret_offset = None;
        self.has_space_before = false;
    }

    pub fn on_enter(&mut self) {
        self.reset();
        self.at_start_of_line = true;
        self.after_sentence_end = false;
        self.saw_sentence_end_punct = false;
        self.has_typed_in_line = false;
        self.last_char_typed = Some('\n');
        self.has_space_before = false;
    }

    pub fn on_window_change(&mut self) {
        self.reset();
        self.at_start_of_line = true;
        self.after_sentence_end = false;
        self.saw_sentence_end_punct = false;
        self.has_typed_in_line = false;
        self.last_char_typed = None;
        self.has_space_before = false;
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
    pub fn handle_backspace(&mut self, simple_telex: bool) -> Option<KeyAction> {
        if simple_telex {
            // Telex giản lược: Không tự động can thiệp hay khôi phục tổ hợp phím trước đó
            if !self.current_word.is_empty() {
                self.current_word.pop();
                self.raw_keys.pop();
                if self.current_word.is_empty() {
                    self.raw_keys.clear();
                    self.has_transformed = false;
                    self.caret_offset = None;
                } else {
                    self.caret_offset = Some(self.current_word.chars().count());
                }
            }
            return None;
        }

        if self.current_word.is_empty() {
            if let Some((word, raw, transformed)) = self.last_word_before_space.take() {
                let len = word.chars().count();
                self.current_word = word;
                self.raw_keys = raw;
                self.has_transformed = transformed;
                self.caret_offset = Some(len);
            }
            return None;
        }

        if let Some(offset) = self.caret_offset {
            let len = self.current_word.chars().count();
            if offset < len {
                if offset > 0 {
                    let mut chars: Vec<char> = self.current_word.chars().collect();
                    chars.remove(offset - 1);
                    self.current_word = chars.into_iter().collect();
                    self.caret_offset = Some(offset - 1);
                }
                return None;
            }
        }

        self.current_word.pop();
        self.raw_keys.pop();
        if self.current_word.is_empty() {
            self.raw_keys.clear();
            self.has_transformed = false;
            self.caret_offset = None;
        } else {
            self.caret_offset = Some(self.current_word.chars().count());
        }
        None
    }

    /// Khôi phục từ vừa gõ khi bấm mũi tên trái (cho phép sửa dấu / từ mà không cần Backspace)
    pub fn on_arrow_left(&mut self) {
        if self.current_word.is_empty() {
            if let Some((word, raw, transformed)) = self.last_word_before_space.take() {
                let len = word.chars().count();
                self.current_word = word;
                self.raw_keys = raw;
                self.has_transformed = transformed;
                self.caret_offset = Some(len);
            }
        } else if let Some(offset) = self.caret_offset {
            if offset > 0 {
                self.caret_offset = Some(offset - 1);
            } else {
                // Đã lùi vượt quá đầu từ -> xóa sạch buffer để dynamic caret nhận diện từ trước đó
                self.reset();
            }
        } else {
            let len = self.current_word.chars().count();
            if len > 0 {
                self.caret_offset = Some(len - 1);
            }
        }
    }

    /// Di chuyển con trỏ sang phải trong từ vừa gõ
    pub fn on_arrow_right(&mut self) {
        if let Some(offset) = self.caret_offset {
            let len = self.current_word.chars().count();
            if offset < len {
                self.caret_offset = Some(offset + 1);
            } else {
                // Đã tiến vượt quá cuối từ -> xóa sạch buffer để dynamic caret nhận diện từ tiếp theo
                self.reset();
            }
        }
    }

    /// Reset sạch buffer khi người dùng click chuột bất kỳ đâu (nhường quyền cho dynamic caret đọc từ thực tế tại vị trí click)
    pub fn on_mouse_click(&mut self) {
        self.reset();
        self.last_word_before_space = None;
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
            self.has_space_before = true;
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
            self.has_space_before = true;
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

        // 4. Nếu con trỏ đang ở giữa từ (di chuyển bằng phím mũi tên)
        if let Some(offset) = self.caret_offset {
            let word_len = self.current_word.chars().count();
            if offset < word_len {
                let chars: Vec<char> = self.current_word.chars().collect();
                let pre_word: String = chars[..offset].iter().collect();
                let post_word: String = chars[offset..].iter().collect();

                let transformed = match config.input_method {
                    InputMethod::Telex => telex::transform_telex(
                        &self.current_word,
                        c,
                        config.modern_tone,
                        config.free_marking,
                        config.smart_english,
                        config.simple_telex,
                        Some(&self.raw_keys),
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
                    // Nguyên tắc: Chỉ sửa nếu ký tự biến đổi nằm ở ĐẰNG SAU con trỏ (phần post_word không đổi)
                    if new_word != self.current_word && crate::automation::is_modification_behind_caret(&post_word, &new_word) {
                        let deletes = post_word.chars().count();
                        let backspaces = pre_word.chars().count();
                        self.current_word = new_word.clone();
                        self.raw_keys = new_word.clone();
                        self.has_transformed = true;
                        self.caret_offset = Some(self.current_word.chars().count() - deletes);
                        return KeyAction::TransformCaret {
                            deletes,
                            backspaces,
                            text: new_word,
                        };
                    }
                }

                // Nếu không phải phím sửa dấu hoặc dấu nằm trước con trỏ: chèn ký tự c vào giữa từ
                let mut new_chars = chars;
                new_chars.insert(offset, c);
                let new_word: String = new_chars.into_iter().collect();
                self.current_word = new_word;
                self.raw_keys.push(c);
                self.caret_offset = Some(offset + 1);
                return KeyAction::PassThrough;
            }
        }

        // 5. Thử biến đổi theo quy tắc Telex / VNI
        let transformed = match config.input_method {
            InputMethod::Telex => telex::transform_telex(
                &self.current_word,
                c,
                config.modern_tone,
                config.free_marking,
                config.smart_english,
                config.simple_telex,
                Some(&self.raw_keys),
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
            self.caret_offset = Some(self.current_word.chars().count());

            if is_undo {
                // Người dùng chủ động gõ phím để hoàn tác (ví dụ ww -> w, ddd -> dd, aaa -> aa)
                self.raw_keys = new_word.clone();
                self.has_transformed = new_accents > 0;
            } else {
                self.has_transformed = true;

                // Nếu auto_restore bật và từ mới tạo ra là âm tiết không hợp lệ trong tiếng Việt:
                let chars: Vec<char> = self.current_word.chars().collect();
                let vowel_indices = telex::extract_vowel_indices(&chars);
                let starts_with_d_stroke = self.current_word.starts_with('đ') || self.current_word.starts_with('Đ');
                if config.auto_restore && !config.simple_telex && !starts_with_d_stroke && !vowel_indices.is_empty() && !is_valid_vietnamese_syllable(&self.current_word) {
                    let bs = self.current_word.chars().count();
                    let text = self.raw_keys.clone();
                    self.current_word = text.clone();
                    self.has_transformed = false;
                    self.caret_offset = Some(self.current_word.chars().count());
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
                    if let Some(rebalanced_word) = telex::handle_tone_modifier(&temp_word, tone_key, config.modern_tone, config.simple_telex) {
                        let mut expected_no_move = self.current_word.clone();
                        expected_no_move.push(c);

                        // Chỉ can thiệp nếu vị trí dấu thanh thật sự bị dịch chuyển giữa các nguyên âm
                        if rebalanced_word != expected_no_move {
                            let backspaces = self.current_word.chars().count();
                            self.current_word = rebalanced_word.clone();
                            self.has_transformed = true;
                            self.caret_offset = Some(self.current_word.chars().count());

                            if config.auto_restore && !config.simple_telex && !is_valid_vietnamese_syllable(&self.current_word) {
                                let bs = backspaces;
                                let text = self.raw_keys.clone();
                                self.current_word = text.clone();
                                self.has_transformed = false;
                                self.caret_offset = Some(self.current_word.chars().count());
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
        // QUY TẮC CỐT LÕI: Chỉ kích hoạt Auto-Restore khi từ ĐÃ CÓ NGUYÊN ÂM (đã hình thành vần âm tiết).
        // Nếu từ chưa có nguyên âm nào (ví dụ: đt, đc, đk, đz, đm, đcl... và mọi kiểu viết tắt / teencode sáng tạo tự do),
        // bộ gõ tuyệt đối KHÔNG rollback, tôn trọng 100% mọi phong cách viết tắt không giới hạn của người dùng!
        if config.auto_restore && !config.simple_telex && self.has_transformed {
            let starts_with_d_stroke = self.current_word.starts_with('đ') || self.current_word.starts_with('Đ');
            if !starts_with_d_stroke {
                let mut test_word = self.current_word.clone();
                test_word.push(c);
                let chars: Vec<char> = test_word.chars().collect();
                let vowel_indices = telex::extract_vowel_indices(&chars);

                if !vowel_indices.is_empty() && !is_valid_vietnamese_syllable(&test_word) {
                    let backspaces = self.current_word.chars().count();
                    let text = self.raw_keys.clone();
                    self.current_word = text.clone();
                    self.has_transformed = false;
                    self.caret_offset = Some(self.current_word.chars().count());
                    return KeyAction::Transform {
                        backspaces,
                        text,
                    };
                }
            }
        }

        // Nếu người dùng gõ chữ thường nhưng được viết hoa tự động:
        if should_capitalize && was_lowercase {
            self.current_word.push(c);
            self.caret_offset = Some(self.current_word.chars().count());
            return KeyAction::Transform {
                backspaces: 0,
                text: c.to_string(),
            };
        }

        // Không có biến đổi và từ vẫn hợp lệ: thêm ký tự vào buffer và cho phím đi qua tự nhiên
        self.current_word.push(c);
        self.caret_offset = Some(self.current_word.chars().count());
        KeyAction::PassThrough
    }
}

/// Kiểm tra một từ có phải là âm tiết tiếng Việt hợp lệ theo cấu trúc ngữ âm học hay không.
pub fn is_valid_vietnamese_syllable(word: &str) -> bool {
    let lower_word = word.to_lowercase();
    let chars: Vec<char> = lower_word.chars().collect();
    if chars.is_empty() {
        return true;
    }

    // 1. Ký tự ngoại lai không có trong tiếng Việt:
    for &c in &chars {
        if matches!(c, 'f' | 'j' | 'w' | 'z') {
            return false;
        }
    }

    // 2. Trích xuất vị trí nguyên âm
    let vowel_indices = telex::extract_vowel_indices(&chars);
    if vowel_indices.is_empty() {
        // Từ không chứa nguyên âm (từ viết tắt, ký hiệu thuần phụ âm):
        // Không đoán từ, luôn chấp nhận để người dùng tự do viết tắt không giới hạn.
        return true;
    }

    // Các nguyên âm trong 1 âm tiết tiếng Việt bắt buộc phải nằm liền kề nhau
    // (nếu bị phụ âm chêm vào giữa như vi-de-o, re-set thì là từ đa âm tiết ngoại lai):
    for i in 1..vowel_indices.len() {
        if vowel_indices[i] != vowel_indices[i - 1] + 1 {
            return false;
        }
    }

    // Tối đa 3 nguyên âm trong 1 âm tiết tiếng Việt:
    if vowel_indices.len() > 3 {
        return false;
    }

    let first_vowel = vowel_indices[0];
    let last_vowel = *vowel_indices.last().unwrap();

    // 3. Kiểm tra phụ âm đầu theo bảng chữ cái ngữ âm tiếng Việt
    let initial: String = chars[..first_vowel].iter().collect();
    if !is_valid_initial_consonant(&initial) {
        return false;
    }

    // 4. Kiểm tra phụ âm cuối theo bảng chữ cái ngữ âm tiếng Việt
    let final_consonants: String = chars[last_vowel + 1..].iter().collect();

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
            | '`' | '_'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SwitchKey;

    #[test]
    fn test_repeated_e_no_oscillation() {
        let mut engine = Engine::new();
        let config = Config::default();

        // 1st 'e' -> "e" (PassThrough)
        let a1 = engine.process_char('e', &config);
        assert_eq!(a1, KeyAction::PassThrough);
        assert_eq!(engine.current_word, "e");

        // 2nd 'e' -> "ê" (Transform)
        let a2 = engine.process_char('e', &config);
        assert_eq!(a2, KeyAction::Transform { backspaces: 1, text: "ê".to_string() });
        assert_eq!(engine.current_word, "ê");

        // 3rd 'e' -> "ee" (Undo transform)
        let a3 = engine.process_char('e', &config);
        assert_eq!(a3, KeyAction::Transform { backspaces: 1, text: "ee".to_string() });
        assert_eq!(engine.current_word, "ee");

        // 4th 'e' -> "eee" (Must be PassThrough! NEVER turn into "eê"!)
        let a4 = engine.process_char('e', &config);
        assert_eq!(a4, KeyAction::PassThrough);
        assert_eq!(engine.current_word, "eee");

        // 5th 'e' -> "eeee" (Must be PassThrough!)
        let a5 = engine.process_char('e', &config);
        assert_eq!(a5, KeyAction::PassThrough);
        assert_eq!(engine.current_word, "eeee");
    }

    #[test]
    fn test_repeated_d_no_oscillation() {
        let mut engine = Engine::new();
        let config = Config::default();

        engine.process_char('d', &config);
        assert_eq!(engine.current_word, "d");

        engine.process_char('d', &config);
        assert_eq!(engine.current_word, "đ");

        engine.process_char('d', &config);
        assert_eq!(engine.current_word, "dd");

        // 4th 'd' -> "ddd" (Must be PassThrough! NEVER turn into "dđ"!)
        let a4 = engine.process_char('d', &config);
        assert_eq!(a4, KeyAction::PassThrough);
        assert_eq!(engine.current_word, "ddd");
    }

    #[test]
    fn test_simple_telex_rules() {
        let mut engine = Engine::new();
        let mut config = Config::default();
        config.simple_telex = true;

        // Tiêu chí 1: w đứng đơn lẻ ra 'w'
        engine.reset();
        engine.process_char('w', &config);
        assert_eq!(engine.current_word, "w");

        // Tiêu chí 2: uo + w -> uơ (không tự đổi cả cặp), phải uwow mới ra ươ
        engine.reset();
        for ch in "uow".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "uơ");

        engine.reset();
        for ch in "uwow".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "ươ");

        // Tiêu chí 3: as -> á, gõ tiếp s -> ás (không xóa dấu)
        engine.reset();
        for ch in "as".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "á");
        engine.process_char('s', &config);
        assert_eq!(engine.current_word, "ás");

        // Tiêu chí 4: Gõ lặp lại phím mũ: aaa -> âa, ddd -> đd
        engine.reset();
        for ch in "aaa".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "âa");

        engine.reset();
        for ch in "ddd".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "đd");

        // Tiêu chí 5: Backspace không khôi phục trạng thái từ trước khoảng trắng
        engine.reset();
        for ch in "hocj ".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "");
        engine.handle_backspace(true);
        // Trong simple_telex, không khôi phục từ trước khoảng trắng:
        assert_eq!(engine.current_word, "");
    }

    #[test]
    fn test_arrow_left_and_mouse_click_restore() {
        let mut engine = Engine::new();
        let config = Config::default();

        // Gõ "hocj " -> "học "
        for ch in "hocj ".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "");

        // Bấm mũi tên trái quay về từ -> khôi phục "học"
        engine.on_arrow_left();
        assert_eq!(engine.current_word, "học");

        // Gõ 's' sửa dấu -> "hóc"
        engine.process_char('s', &config);
        assert_eq!(engine.current_word, "hóc");

        // Thử lại với bấm chuột:
        engine.process_char(' ', &config);
        assert_eq!(engine.current_word, "");

        // Bấm chuột bất kỳ đâu -> reset sạch buffer để dynamic caret đọc từ thực tế
        engine.on_mouse_click();
        assert_eq!(engine.current_word, "");
    }

    #[test]
    fn test_caret_editing_rules() {
        let mut engine = Engine::new();
        let config = Config::default();

        // 1. Gõ "tayf" -> "tày"
        for ch in "tayf".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "tày");

        // Bấm mũi tên trái 1 lần -> con trỏ ở giữa "tà|y"
        engine.on_arrow_left();
        assert_eq!(engine.caret_offset, Some(2));

        // Gõ 's' -> vì trỏ ở sau 'à', được phép sửa dấu 'à' thành 'á' -> "táy"
        let act = engine.process_char('s', &config);
        assert_eq!(
            act,
            KeyAction::TransformCaret {
                deletes: 1,
                backspaces: 2,
                text: "táy".to_string(),
            }
        );
        assert_eq!(engine.current_word, "táy");

        // 2. Thử trường hợp con trỏ ở trước nguyên âm:
        engine.reset();
        for ch in "tayf".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "tày");

        // Bấm mũi tên trái 2 lần -> con trỏ ở "t|ày"
        engine.on_arrow_left();
        engine.on_arrow_left();
        assert_eq!(engine.caret_offset, Some(1));

        // Gõ 's' -> trỏ đứng TRƯỚC ký tự cần sửa -> KHÔNG ĐƯỢC PHÉP SỬA DẤU!
        // Phải chèn 's' vào vị trí con trỏ thành "tsày"
        let act2 = engine.process_char('s', &config);
        assert_eq!(act2, KeyAction::PassThrough);
        assert_eq!(engine.current_word, "tsày");
    }

    #[test]
    fn test_telex_z_tone_canceling() {
        let mut engine = Engine::new();
        let config = Config::default();

        // 1. Gõ "maz" khi chưa có dấu -> không nuốt 'z', ra "maz"
        for ch in "maz".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "maz");

        // 2. Gõ "mas" -> "má", rồi gõ 'z' -> xóa dấu ra "ma"
        engine.reset();
        for ch in "mas".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "má");

        engine.process_char('z', &config);
        assert_eq!(engine.current_word, "ma");
    }

    #[test]
    fn test_user_stress_test_suite() {
        let mut engine = Engine::new();
        let mut config = Config::default();
        config.auto_caps_line = false;

        let mut type_word = |keys: &str| -> String {
            engine.reset();
            for ch in keys.chars() {
                engine.process_char(ch, &config);
            }
            engine.current_word.clone()
        };

        // 1. hoẵng:
        assert_eq!(type_word("hoaangx"), "hoẵng");

        // 2. ngoẵng:
        assert_eq!(type_word("ngoaangx"), "ngoẵng");

        // 3. khuỵu:
        assert_eq!(type_word("khuyuj"), "khuỵu");
        assert_eq!(type_word("khuyju"), "khuỵu");

        // 4. ngoẻo:
        assert_eq!(type_word("ngoero"), "ngoẻo");

        // 5. thuở:
        assert_eq!(type_word("thuowr"), "thuở");
        assert_eq!(type_word("thuorw"), "thuở");

        // 6. xoong (gõ xooong thoát ô thành oo):
        assert_eq!(type_word("xooong"), "xoong");

        // 7. Từ tiếng Anh không bị dính dấu:
        assert_eq!(type_word("process"), "process");
        assert_eq!(type_word("commit"), "commit");
        assert_eq!(type_word("install"), "install");

        // 8. Chữ hoa:
        assert_eq!(type_word("Quangr"), "Quảng");
        assert_eq!(type_word("Bachj"), "Bạch");
        assert_eq!(type_word("Dduowngf"), "Đường");
    }

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
    fn test_telex_dot_o_s() {
        let mut engine = Engine::new();
        let config = Config::default();

        // Gõ "dd" -> "đ", "o" -> "đo", "t" -> "đot"
        for ch in "ddot".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "đot");

        // Gõ 'o' -> phải ra "đôt" (không bị ddoto)
        engine.process_char('o', &config);
        assert_eq!(engine.current_word, "đôt");

        // Gõ 's' -> phải ra "đốt"
        engine.process_char('s', &config);
        assert_eq!(engine.current_word, "đốt");
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
        assert_eq!(engine.current_word, "task");
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
        config.browser_compatibility = false;
        config.browser_apps = "custom.exe, test.exe".to_string();

        let json = config.to_json_string();
        let mut loaded = Config::default();
        loaded.merge_from_json(&json);

        assert_eq!(loaded.input_method, InputMethod::Vni);
        assert_eq!(loaded.modern_tone, false);
        assert_eq!(loaded.switch_key, SwitchKey::AltZ);
        assert_eq!(loaded.auto_restore, false);
        assert_eq!(loaded.browser_compatibility, false);
        assert_eq!(loaded.browser_apps, "custom.exe, test.exe");
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

    #[test]
    fn test_telex_abbreviation() {
        let mut engine = Engine::new();
        let config = Config::default();

        // 1. Viết tắt "đt" (điện thoại) không bị đá về "ddt"
        engine.process_char('d', &config);
        let a_d2 = engine.process_char('d', &config);
        assert_eq!(
            a_d2,
            KeyAction::Transform {
                backspaces: 1,
                text: "đ".to_string(),
            }
        );
        let a_t = engine.process_char('t', &config);
        assert_eq!(a_t, KeyAction::PassThrough);
        assert_eq!(engine.current_word, "đt");

        // 2. Viết tắt "đc" (được) không bị đá về "ddc"
        engine.reset();
        engine.process_char('d', &config);
        engine.process_char('d', &config);
        let a_c = engine.process_char('c', &config);
        assert_eq!(a_c, KeyAction::PassThrough);
        assert_eq!(engine.current_word, "đc");

        // 3. Viết tắt "đk" (đăng ký) không bị đá về "ddk"
        engine.reset();
        engine.process_char('d', &config);
        engine.process_char('d', &config);
        let a_k = engine.process_char('k', &config);
        assert_eq!(a_k, KeyAction::PassThrough);
        assert_eq!(engine.current_word, "đk");

        // 4. Viết tắt sáng tạo tự do "đz" (đẹp zai / trai) không bị đá về "ddz"
        engine.reset();
        engine.process_char('d', &config);
        engine.process_char('d', &config);
        let a_z = engine.process_char('z', &config);
        assert_eq!(a_z, KeyAction::PassThrough);
        assert_eq!(engine.current_word, "đz");

        // 5. Viết tắt sáng tạo tự do "đm", "đcl" không bị đá về "dd..."
        engine.reset();
        engine.process_char('d', &config);
        engine.process_char('d', &config);
        let a_m = engine.process_char('m', &config);
        assert_eq!(a_m, KeyAction::PassThrough);
        assert_eq!(engine.current_word, "đm");
    }

    #[test]
    fn test_telex_rich_vowels() {
        let mut engine = Engine::new();
        let config = Config::default();

        // "khuấy" (vần uây)
        for ch in "khuaays".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "khuấy");

        // "rượu" (vần ươu)
        engine.reset();
        for ch in "ruouwj".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "rượu");

        // "chuối" (vần uôi)
        engine.reset();
        for ch in "chuoois".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "chuối");
    }

    #[test]
    fn test_no_guesswork_free_vowels() {
        let mut engine = Engine::new();
        let config = Config::default();

        // Gõ từ mượn / tự do không bị bộ gõ tự ý phán đoán hoặc rollback
        for ch in "cacao".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "cacao");
    }

    #[test]
    fn test_capslock_undo_tone() {
        let mut engine = Engine::new();
        let config = Config::default();

        // 1. CapsLock bật: gõ 'D', 'A', 'S' -> 'DÁ'
        engine.process_char('D', &config);
        engine.process_char('A', &config);
        let a = engine.process_char('S', &config);
        assert_eq!(a, KeyAction::Transform { backspaces: 2, text: "DÁ".to_string() });
        assert_eq!(engine.current_word, "DÁ");

        // Vẫn giữ CapsLock gõ tiếp 'S' -> ra "DAS"
        let a2 = engine.process_char('S', &config);
        assert_eq!(a2, KeyAction::Transform { backspaces: 2, text: "DAS".to_string() });
        assert_eq!(engine.current_word, "DAS");

        // 2. Nếu từ đang là "DÁ", nhưng người dùng tắt CapsLock rồi gõ 's' thường -> ra "DAs"
        engine.reset();
        engine.process_char('D', &config);
        engine.process_char('A', &config);
        engine.process_char('S', &config);
        assert_eq!(engine.current_word, "DÁ");

        let a3 = engine.process_char('s', &config);
        assert_eq!(a3, KeyAction::Transform { backspaces: 2, text: "DAs".to_string() });
        assert_eq!(engine.current_word, "DAs");
    }

    #[test]
    fn test_space_tracking_browser() {
        let mut engine = Engine::new();
        let config = Config::default();

        // Gõ "arial"
        for ch in "arial".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.has_space_before, false);

        // Gõ dấu cách
        engine.process_char(' ', &config);
        assert_eq!(engine.has_space_before, true);

        // Gõ từ tiếp theo "là" (l + a + f)
        engine.process_char('l', &config);
        engine.process_char('a', &config);
        let a = engine.process_char('f', &config);
        assert_eq!(a, KeyAction::Transform { backspaces: 2, text: "là".to_string() });
        assert_eq!(engine.has_space_before, true);
    }

    fn simulate_type(engine: &mut Engine, config: &Config, input: &str) -> String {
        let mut output = String::new();
        for ch in input.chars() {
            let action = engine.process_char(ch, config);
            match action {
                KeyAction::PassThrough => {
                    output.push(ch);
                }
                KeyAction::Transform { backspaces, text } => {
                    for _ in 0..backspaces {
                        output.pop();
                    }
                    output.push_str(&text);
                }
                KeyAction::TransformCaret { deletes: _, backspaces, text } => {
                    for _ in 0..backspaces {
                        output.pop();
                    }
                    output.push_str(&text);
                }
                KeyAction::Reset => {
                    output.push(ch);
                }
                KeyAction::Backspace => {
                    output.pop();
                }
            }
        }
        output
    }

    #[test]
    fn test_user_requested_dduf_capslock() {
        let mut engine = Engine::new();
        let config = Config::default();

        let res = simulate_type(&mut engine, &config, "DDUF");
        assert_eq!(res, "ĐÙ");
    }

    #[test]
    fn test_user_requested_pangram() {
        let mut engine = Engine::new();
        let mut config = Config::default();
        config.auto_caps_line = false;
        config.modern_tone = true;

        // "Trần Vũ Hải Đăng, một chuyên gia xử lý phím ngoằn ngoèo, đang cố gắng hoàn thiện bộ gõ tuyệt vời này!"
        let input = "Traanf Vux Hair Ddawng, mootj chuyeen gia xuwr lys phims ngoawfn ngoeof, ddang coos gawngs hoanf thieenj booj gox tuyeetj vowif nayf!";
        let res = simulate_type(&mut engine, &config, input);
        assert_eq!(
            res,
            "Trần Vũ Hải Đăng, một chuyên gia xử lý phím ngoằn ngoèo, đang cố gắng hoàn thiện bộ gõ tuyệt vời này!"
        );

        // Test với full CapsLock:
        engine.reset();
        let input_caps = "TRAANF VUX HAIR DDAWNG, MOOTJ CHUYEEN GIA XUWR LYS PHIMS NGOAWFN NGOEOF, DDANG COOS GAWNGS HOANF THIEENJ BOOJ GOX TUYEETJ VOWIF NAYF!";
        let res_caps = simulate_type(&mut engine, &config, input_caps);
        assert_eq!(
            res_caps,
            "TRẦN VŨ HẢI ĐĂNG, MỘT CHUYÊN GIA XỬ LÝ PHÍM NGOẰN NGOÈO, ĐANG CỐ GẮNG HOÀN THIỆN BỘ GÕ TUYỆT VỜI NÀY!"
        );
    }

    #[test]
    fn test_user_requested_complex_vowels() {
        let mut engine = Engine::new();
        let mut config = Config::default();
        config.auto_caps_line = false;
        config.modern_tone = true;

        // "Con chim hoẵng quắt queo nhảy qua khoảnh sân ngoằn ngoèo, xoắn quẩy trong buổi chiều muộn."
        let sentence = "Con chim hoawxng quawst queo nhayr qua khoarnh saan ngoawfn ngoeof, xoawns quaayr trong buooir chieeuf muoonj.";
        let res = simulate_type(&mut engine, &config, sentence);
        assert_eq!(
            res,
            "Con chim hoẵng quắt queo nhảy qua khoảnh sân ngoằn ngoèo, xoắn quẩy trong buổi chiều muộn."
        );

        // CapsLock toàn bộ câu:
        engine.reset();
        let sentence_caps = "CON CHIM HOAWXNG QUAWST QUEO NHAYR QUA KHOARNH SAAN NGOAWFN NGOEOF, XOAWNS QUAAYR TRONG BUOOIR CHIEEUF MUOONJ.";
        let res_caps = simulate_type(&mut engine, &config, sentence_caps);
        assert_eq!(
            res_caps,
            "CON CHIM HOẴNG QUẮT QUEO NHẢY QUA KHOẢNH SÂN NGOẰN NGOÈO, XOẮN QUẨY TRONG BUỔI CHIỀU MUỘN."
        );
    }

    #[test]
    fn test_user_requested_repeated_keys_undo() {
        let mut engine = Engine::new();
        let config = Config::default();

        // 1. Phím lặp hoàn tác dấu thanh: toans + s -> toans (phục hồi)
        let r1 = simulate_type(&mut engine, &config, "toanss");
        assert_eq!(r1, "toans");

        // 2. Phím lặp hoàn tác dấu mũ: dd -> đ, ddd -> dd
        engine.reset();
        let r2 = simulate_type(&mut engine, &config, "ddd");
        assert_eq!(r2, "dd");

        // 3. Phím lặp hoàn tác dấu móc: w -> ư, ww -> w
        engine.reset();
        let r3 = simulate_type(&mut engine, &config, "ww");
        assert_eq!(r3, "w");

        // 4. Phím lặp hoàn tác CapsLock: D -> D, DD -> Đ, DDD -> DD
        engine.reset();
        let r4 = simulate_type(&mut engine, &config, "DDD");
        assert_eq!(r4, "DD");
    }

    #[test]
    fn test_user_requested_tone_placement() {
        let mut engine = Engine::new();
        let mut config_modern = Config::default();
        config_modern.auto_caps_line = false;
        config_modern.modern_tone = true;

        // Chuẩn mới (hoà, thuỷ - dấu đặt trên nguyên âm thứ 2: a, y):
        let input = "Hoaf binhf duowis cown muwa thuyr tinh lamf buwngf sangs car gocs trowif thuowr aays.";
        let res_modern = simulate_type(&mut engine, &config_modern, input);
        assert_eq!(
            res_modern,
            "Hoà bình dưới cơn mưa thuỷ tinh làm bừng sáng cả góc trời thuở ấy."
        );

        // Chuẩn cũ (hòa, thủy - dấu đặt trên âm đệm o, u):
        let mut config_old = Config::default();
        config_old.auto_caps_line = false;
        config_old.modern_tone = false;
        engine.reset();
        let res_old = simulate_type(&mut engine, &config_old, input);
        assert_eq!(
            res_old,
            "Hòa bình dưới cơn mưa thủy tinh làm bừng sáng cả góc trời thuở ấy."
        );

        // CapsLock chuẩn mới:
        engine.reset();
        let input_caps = "HOAF BINHF DUOWIS COWN MUWA THUYR TINH LAMF BUWNGF SANGS CAR GOCS TROWIF THUOWR AAYS.";
        let res_caps = simulate_type(&mut engine, &config_modern, input_caps);
        assert_eq!(
            res_caps,
            "HOÀ BÌNH DƯỚI CƠN MƯA THUỶ TINH LÀM BỪNG SÁNG CẢ GÓC TRỜI THUỞ ẤY."
        );
    }

    #[test]
    fn test_user_requested_english_terms() {
        let mut engine = Engine::new();
        let mut config = Config::default();
        config.auto_caps_line = false;
        config.smart_english = true;

        // "Developer đang debug ứng dụng trên Visual Studio Code, cài đặt PostgREST, Tailwind CSS và Supabase."
        let input = "Developer ddang debug uwngs dujng treen Visual Studio Code, caif ddawtj PostgREST, Tailwind CSS vaf Supabase.";
        let res = simulate_type(&mut engine, &config, input);
        assert_eq!(
            res,
            "Developer đang debug ứng dụng trên Visual Studio Code, cài đặt PostgREST, Tailwind CSS và Supabase."
        );

        // CapsLock:
        engine.reset();
        let input_caps = "DEVELOPER DEBUG POSTGREST TAILWIND CSS SUPABASE";
        let res_caps = simulate_type(&mut engine, &config, input_caps);
        assert_eq!(res_caps, "DEVELOPER DEBUG POSTGREST TAILWIND CSS SUPABASE");
    }

    #[test]
    fn test_user_requested_cuawr_to_cua() {
        let mut engine = Engine::new();
        let config = Config::default();

        // cuawr -> cửa
        let res = simulate_type(&mut engine, &config, "cuawr");
        assert_eq!(res, "cửa");

        // muaw -> mưa
        engine.reset();
        assert_eq!(simulate_type(&mut engine, &config, "muaw"), "mưa");

        // chuaw -> chưa
        engine.reset();
        assert_eq!(simulate_type(&mut engine, &config, "chuaw"), "chưa");

        // quawst -> quắt (không bị biến thành quắt/quưa)
        engine.reset();
        assert_eq!(simulate_type(&mut engine, &config, "quawst"), "quắt");

        // cuaww -> cuaw (undo)
        engine.reset();
        assert_eq!(simulate_type(&mut engine, &config, "cuaww"), "cuaw");
    }

    #[test]
    fn test_dynamic_caret_word_transformation() {
        let config = Config::default();
        assert_eq!(config.dynamic_caret, false);

        // Telex: "tày" + 's' -> "táy"
        let transformed = telex::transform_telex("tày", 's', config.modern_tone, config.free_marking, config.smart_english, config.simple_telex, None);
        assert_eq!(transformed, Some("táy".to_string()));

        // Telex: "chuyen" + 'e' -> "chuyên"
        let transformed_e = telex::transform_telex("chuyen", 'e', config.modern_tone, config.free_marking, config.smart_english, config.simple_telex, None);
        assert_eq!(transformed_e, Some("chuyên".to_string()));

        // VNI: "tày" + '1' -> "táy"
        let transformed_vni = vni::transform_vni("tày", '1', config.modern_tone, config.free_marking, config.smart_english);
        assert_eq!(transformed_vni, Some("táy".to_string()));

        // Foreign word with smart_english: "file" + 's' -> None
        let transformed_en = telex::transform_telex("file", 's', config.modern_tone, config.free_marking, config.smart_english, config.simple_telex, None);
        assert_eq!(transformed_en, None);
    }

    #[test]
    fn test_uww_and_simple_telex_tone() {
        let mut engine = Engine::new();
        let config = Config::default();

        // Standard Telex:
        // uww -> uw
        engine.reset();
        assert_eq!(simulate_type(&mut engine, &config, "uww"), "uw");
        // ww -> w
        engine.reset();
        assert_eq!(simulate_type(&mut engine, &config, "ww"), "w");
        // tuww -> tuw
        engine.reset();
        assert_eq!(simulate_type(&mut engine, &config, "tuww"), "tuw");
        // tww -> tw
        engine.reset();
        assert_eq!(simulate_type(&mut engine, &config, "tww"), "tw");
        // chuyenr -> chuyển
        engine.reset();
        assert_eq!(simulate_type(&mut engine, &config, "chuyenr"), "chuyển");

        // Simple Telex:
        let mut simple_config = Config::default();
        simple_config.simple_telex = true;

        // uww -> uw in simple telex
        engine.reset();
        assert_eq!(simulate_type(&mut engine, &simple_config, "uww"), "uw");
        // cuawr -> cửa in simple telex
        engine.reset();
        assert_eq!(simulate_type(&mut engine, &simple_config, "cuawr"), "cửa");
        // chuyenr -> chuyển in simple telex
        engine.reset();
        assert_eq!(simulate_type(&mut engine, &simple_config, "chuyenr"), "chuyển");
    }
}
