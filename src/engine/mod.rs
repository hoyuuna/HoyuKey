pub mod telex;
pub mod vietnamese;
pub mod vni;

use crate::config::{Config, InputMethod};

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
    current_word: String,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            current_word: String::new(),
        }
    }

    pub fn reset(&mut self) {
        self.current_word.clear();
    }

    pub fn handle_backspace(&mut self) -> KeyAction {
        self.current_word.pop();
        KeyAction::Backspace
    }

    /// Xử lý một ký tự gõ vào
    pub fn process_char(&mut self, c: char, config: &Config) -> KeyAction {
        if !config.enabled {
            self.reset();
            return KeyAction::PassThrough;
        }

        // Nếu là phím khoảng trắng, ngắt dòng hoặc ký tự phân cách từ
        if c.is_whitespace() || is_word_boundary(c) {
            self.reset();
            return KeyAction::PassThrough;
        }

        // Giới hạn độ dài từ buffer để tránh tràn (từ tiếng Việt tối đa ~10-15 ký tự)
        if self.current_word.len() > 30 {
            self.reset();
        }

        let transformed = match config.input_method {
            InputMethod::Telex => telex::transform_telex(&self.current_word, c, config.modern_tone),
            InputMethod::Vni => vni::transform_vni(&self.current_word, c, config.modern_tone),
        };

        if let Some(new_word) = transformed {
            // Số lượng ký tự cần xóa = số ký tự hiển thị của từ cũ
            let backspaces = self.current_word.chars().count();
            self.current_word = new_word.clone();

            KeyAction::Transform {
                backspaces,
                text: new_word,
            }
        } else {
            // Không có biến đổi: chỉ thêm ký tự này vào buffer và cho phím đi qua tự nhiên
            self.current_word.push(c);
            KeyAction::PassThrough
        }
    }
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
    fn test_telex_basic() {
        let mut engine = Engine::new();
        let config = Config::default();

        // Gõ "tiếng" -> t, i, e, e, n, g, s
        engine.process_char('t', &config);
        engine.process_char('i', &config);
        engine.process_char('e', &config);
        let action = engine.process_char('e', &config);
        assert_eq!(
            action,
            KeyAction::Transform {
                backspaces: 3,
                text: "tiê".to_string(),
            }
        );
        engine.process_char('n', &config);
        engine.process_char('g', &config);
        let action = engine.process_char('s', &config);
        assert_eq!(
            action,
            KeyAction::Transform {
                backspaces: 5,
                text: "tiếng".to_string(),
            }
        );
    }

    #[test]
    fn test_telex_viet() {
        let mut engine = Engine::new();
        let config = Config::default();

        // Gõ "việt" -> v, i, e, e, t, j
        engine.process_char('v', &config);
        engine.process_char('i', &config);
        engine.process_char('e', &config);
        engine.process_char('e', &config);
        engine.process_char('t', &config);
        let action = engine.process_char('j', &config);
        assert_eq!(
            action,
            KeyAction::Transform {
                backspaces: 4,
                text: "việt".to_string(),
            }
        );
    }

    #[test]
    fn test_telex_dd() {
        let mut engine = Engine::new();
        let config = Config::default();

        engine.process_char('d', &config);
        let action = engine.process_char('d', &config);
        assert_eq!(
            action,
            KeyAction::Transform {
                backspaces: 1,
                text: "đ".to_string(),
            }
        );
    }

    #[test]
    fn test_telex_qua_quoc() {
        let mut engine = Engine::new();
        let config = Config::default();

        // Gõ "quá" -> q, u, a, s
        engine.process_char('q', &config);
        engine.process_char('u', &config);
        engine.process_char('a', &config);
        let action = engine.process_char('s', &config);
        assert_eq!(
            action,
            KeyAction::Transform {
                backspaces: 3,
                text: "quá".to_string(),
            }
        );

        engine.reset();
        // Gõ "quốc" -> q, u, o, o, c, s
        // Lưu ý: sau khi gõ 'oo', từ hiển thị là "quô" (3 ký tự), thêm 'c' là "quôc" (4 ký tự)
        engine.process_char('q', &config);
        engine.process_char('u', &config);
        engine.process_char('o', &config);
        engine.process_char('o', &config);
        engine.process_char('c', &config);
        let action = engine.process_char('s', &config);
        assert_eq!(
            action,
            KeyAction::Transform {
                backspaces: 4,
                text: "quốc".to_string(),
            }
        );
    }

    #[test]
    fn test_telex_gia() {
        let mut engine = Engine::new();
        let config = Config::default();

        // Gõ "giá" -> g, i, a, s
        engine.process_char('g', &config);
        engine.process_char('i', &config);
        engine.process_char('a', &config);
        let action = engine.process_char('s', &config);
        assert_eq!(
            action,
            KeyAction::Transform {
                backspaces: 3,
                text: "giá".to_string(),
            }
        );
    }
}
