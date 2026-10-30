pub mod telex;
pub mod vietnamese;
pub mod vni;

use crate::config::{Config, InputMethod};
use crate::engine::vietnamese::Tone;

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

#[derive(Debug, Clone)]
struct HistoryToken {
    word: String,
    _separator: Option<char>,
}

pub struct Engine {
    current_word: String,
    history: Vec<HistoryToken>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            current_word: String::new(),
            history: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.current_word.clear();
        self.history.clear();
    }

    /// Xử lý phím Backspace thông minh:
    /// 1. Nếu buffer từ hiện tại rỗng nhưng có từ vừa nhập trước đó (ví dụ gõ "dụ" rồi space, giờ lùi lại),
    ///    khôi phục ngay từ trước đó để người dùng sửa dấu/ký tự liền mạch.
    /// 2. Nếu từ hiện tại đang có dấu thanh (ví dụ "quảng" hoặc "dụ"), Backspace sẽ gỡ bỏ dấu thanh trước (trở về "quang", "du")
    ///    để người dùng có thể gõ ngay phím dấu khác (như 'x' thành "quãng", "dũ") như trên bàn phím điện thoại!
    pub fn handle_backspace(&mut self) -> Option<KeyAction> {
        // Nếu từ hiện tại rỗng, kiểm tra xem có từ trong lịch sử vừa phân cách bằng space không
        if self.current_word.is_empty() {
            if let Some(token) = self.history.pop() {
                self.current_word = token.word;
            }
            return None;
        }

        let mut has_tone = false;
        for c in self.current_word.chars() {
            if let Some((_, _, tone, _)) = vietnamese::decompose_vietnamese_char(c) {
                if tone != Tone::None {
                    has_tone = true;
                    break;
                }
            }
        }

        if has_tone {
            let old_len = self.current_word.chars().count();
            let mut new_chars: Vec<char> = Vec::new();
            for c in self.current_word.chars() {
                if let Some((base, vmod, _, is_upper)) = vietnamese::decompose_vietnamese_char(c) {
                    let unaccented = vietnamese::get_vietnamese_vowel(base, vmod, Tone::None).unwrap_or(c);
                    new_chars.push(if is_upper { unaccented.to_uppercase().next().unwrap_or(unaccented) } else { unaccented });
                } else {
                    new_chars.push(c);
                }
            }
            let new_word: String = new_chars.into_iter().collect();
            self.current_word = new_word.clone();

            return Some(KeyAction::Transform {
                backspaces: old_len,
                text: new_word,
            });
        }

        self.current_word.pop();
        None
    }

    /// Xử lý một ký tự gõ vào
    pub fn process_char(&mut self, c: char, config: &Config) -> KeyAction {
        if !config.enabled {
            self.reset();
            return KeyAction::PassThrough;
        }

        // Nếu là phím khoảng trắng, ngắt dòng hoặc ký tự phân cách từ
        if c.is_whitespace() || is_word_boundary(c) {
            if !self.current_word.is_empty() {
                self.history.push(HistoryToken {
                    word: self.current_word.clone(),
                    _separator: Some(c),
                });
                if self.history.len() > 30 {
                    self.history.remove(0);
                }
                self.current_word.clear();
            }
            return KeyAction::PassThrough;
        }

        if self.current_word.len() > 30 {
            self.reset();
        }

        // 1. Thử biến đổi theo quy tắc Telex / VNI
        let transformed = match config.input_method {
            InputMethod::Telex => telex::transform_telex(&self.current_word, c, config.modern_tone),
            InputMethod::Vni => vni::transform_vni(&self.current_word, c, config.modern_tone),
        };

        if let Some(new_word) = transformed {
            let backspaces = self.current_word.chars().count();
            self.current_word = new_word.clone();

            return KeyAction::Transform {
                backspaces,
                text: new_word,
            };
        }

        // 2. Tự động tái cân bằng vị trí dấu thanh (Dynamic Tone Re-balancing):
        // Nếu từ hiện tại đã có dấu thanh (ví dụ "khí" có dấu sắc), mà người dùng gõ thêm nguyên âm (như 'o', 'a'),
        // hệ thống tự động gỡ dấu cũ và tái định vị dấu thanh đúng quy tắc ngữ âm ("khoái" thay vì "khoaí")!
        if c.is_ascii_alphabetic() && !telex::is_foreign_multisyllable_word(&self.current_word) {
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
                        if rebalanced_word != temp_word {
                            let backspaces = self.current_word.chars().count();
                            self.current_word = rebalanced_word.clone();
                            return KeyAction::Transform {
                                backspaces,
                                text: rebalanced_word,
                            };
                        }
                    }
                }
            }
        }

        // Không có biến đổi: chỉ thêm ký tự này vào buffer và cho phím đi qua tự nhiên
        self.current_word.push(c);
        KeyAction::PassThrough
    }
}

fn find_existing_tone(word: &str) -> Tone {
    for c in word.chars() {
        if let Some((_, _, tone, _)) = vietnamese::decompose_vietnamese_char(c) {
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
        if let Some((base, vmod, _, is_upper)) = vietnamese::decompose_vietnamese_char(c) {
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

        // c, h, u, y, e, e, n, r -> chuyển (bắt buộc đặt trên 'ê', không bao giờ chuỷên!)
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
    }

    #[test]
    fn test_telex_quang_backspace_x() {
        let mut engine = Engine::new();
        let config = Config::default();

        // Gõ "quảng" -> quangr
        for ch in "quangr".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "quảng");

        // Backspace -> tự động gỡ dấu hỏi, đưa từ về "quang"
        let bs_action = engine.handle_backspace();
        assert_eq!(
            bs_action,
            Some(KeyAction::Transform {
                backspaces: 5,
                text: "quang".to_string(),
            })
        );
        assert_eq!(engine.current_word, "quang");

        // Gõ 'x' -> ngay lập tức thành "quãng"
        let action = engine.process_char('x', &config);
        assert_eq!(
            action,
            KeyAction::Transform {
                backspaces: 5,
                text: "quãng".to_string(),
            }
        );
    }

    #[test]
    fn test_telex_ngoai() {
        let mut engine = Engine::new();
        let config = Config::default();

        for ch in "ngoai".chars() {
            engine.process_char(ch, &config);
        }
        let action = engine.process_char('f', &config);
        assert_eq!(
            action,
            KeyAction::Transform {
                backspaces: 5,
                text: "ngoài".to_string(),
            }
        );
    }

    #[test]
    fn test_telex_double_w() {
        let mut engine = Engine::new();
        let config = Config::default();

        // Gõ 'w' lần 1 -> "ư" (từ trước rỗng nên backspaces: 0)
        let a1 = engine.process_char('w', &config);
        assert_eq!(a1, KeyAction::Transform { backspaces: 0, text: "ư".to_string() });
        assert_eq!(engine.current_word, "ư");

        // Gõ 'w' lần 2 -> hoàn tác thành "w" (chứ không phải "uw")
        let a2 = engine.process_char('w', &config);
        assert_eq!(a2, KeyAction::Transform { backspaces: 1, text: "w".to_string() });
        assert_eq!(engine.current_word, "w");
    }

    #[test]
    fn test_telex_edit_previous_word_du_to_du() {
        let mut engine = Engine::new();
        let config = Config::default();

        // Gõ "duj" -> "dụ"
        for ch in "duj".chars() {
            engine.process_char(ch, &config);
        }
        assert_eq!(engine.current_word, "dụ");

        // Gõ dấu cách: "dụ " -> commit từ vào lịch sử
        engine.process_char(' ', &config);
        assert_eq!(engine.current_word, "");

        // Người dùng ấn Backspace xóa dấu cách:
        let bs1 = engine.handle_backspace();
        assert_eq!(bs1, None); // Phím space trên màn hình được xóa bình thường
        assert_eq!(engine.current_word, "dụ"); // Engine khôi phục ngay từ "dụ"!

        // Người dùng gõ ngay phím 'x' để sửa thành "dũ":
        let a = engine.process_char('x', &config);
        assert_eq!(
            a,
            KeyAction::Transform {
                backspaces: 2,
                text: "dũ".to_string(),
            }
        );
        assert_eq!(engine.current_word, "dũ");
    }

    #[test]
    fn test_telex_foreign_animation() {
        let mut engine = Engine::new();
        let config = Config::default();

        for ch in "animation".chars() {
            let action = engine.process_char(ch, &config);
            assert_eq!(action, KeyAction::PassThrough);
        }
        assert_eq!(engine.current_word, "animation");
    }
}
