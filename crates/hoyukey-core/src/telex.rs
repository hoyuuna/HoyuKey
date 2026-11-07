// hoyukey-core: Dynamic Telex Transformation Engine
// Thuật toán biến đổi phím Telex dựa trên hình vị học động (Zero-dependency, không dùng từ điển).

use crate::phonotactics::{decompose_word, is_english_foreign_word, VowelPart};
use crate::vietnamese::{Tone, VowelMod};

pub const CHAR_D_STROKE_LOWER: char = '\u{0111}'; // 'đ'
pub const CHAR_D_STROKE_UPPER: char = '\u{0110}'; // 'Đ'
pub const CHAR_U_HORN_LOWER: char = '\u{01B0}';   // 'ư'
pub const CHAR_U_HORN_UPPER: char = '\u{01AF}';   // 'Ư'
pub const CHAR_O_HORN_LOWER: char = '\u{01A1}';   // 'ơ'
pub const CHAR_O_HORN_UPPER: char = '\u{01A0}';   // 'Ơ'

pub fn transform_telex(
    word: &str,
    key: char,
    modern_tone: bool,
    free_marking: bool,
    smart_english: bool,
    simple_telex: bool,
    raw_keys: Option<&str>,
) -> Option<String> {
    if word.is_empty() {
        if key == 'w' || key == 'W' {
            return Some(if key.is_uppercase() {
                CHAR_U_HORN_UPPER.to_string()
            } else {
                CHAR_U_HORN_LOWER.to_string()
            });
        }
        if key == '[' {
            return Some(CHAR_U_HORN_LOWER.to_string());
        }
        if key == '{' {
            return Some(CHAR_U_HORN_UPPER.to_string());
        }
        if key == ']' {
            return Some(CHAR_O_HORN_LOWER.to_string());
        }
        if key == '}' {
            return Some(CHAR_O_HORN_UPPER.to_string());
        }
        return None;
    }

    // Nếu bật nhận diện tiếng Anh thông minh và từ hiện tại mang đặc trưng tiếng Anh -> không can thiệp
    if smart_english && is_english_foreign_word(word) {
        return None;
    }

    let key_lower = key.to_ascii_lowercase();
    let is_key_upper = key.is_ascii_uppercase();

    // 1. Phím '[' và ']' / '{' và '}' tạo chữ 'ư', 'ơ', 'ươ' (chuẩn UniKey / EVKey)
    if matches!(key, '[' | ']' | '{' | '}') {
        if let Some(res) = handle_bracket_keys(word, key) {
            return Some(res);
        }
    }

    // 2. Phím 'd' / 'D' -> 'đ' / 'Đ'
    if key_lower == 'd' {
        if let Some(res) = handle_d_key(word, is_key_upper, free_marking) {
            return Some(res);
        }
    }

    // 3. Phím mũ: 'a', 'e', 'o'
    if matches!(key_lower, 'a' | 'e' | 'o') {
        if let Some(res) = handle_hat_key(word, key_lower, is_key_upper, free_marking) {
            return Some(res);
        }
    }

    // 4. Phím móc / trăng: 'w' / 'W'
    if key_lower == 'w' {
        if let Some(res) = handle_w_key(word, is_key_upper, free_marking, simple_telex, raw_keys) {
            return Some(res);
        }
    }

    // 5. Phím dấu thanh: 's', 'f', 'r', 'x', 'j', 'z'
    if matches!(key_lower, 's' | 'f' | 'r' | 'x' | 'j' | 'z') {
        if let Some(res) = handle_tone_key(word, key_lower, modern_tone, free_marking) {
            return Some(res);
        }
    }

    None
}

/// Xử lý phím '[' và ']' / '{' và '}' (chuẩn UniKey: '[' -> 'ư', ']' -> 'ơ', 'u]' -> 'ươ')
fn handle_bracket_keys(word: &str, key: char) -> Option<String> {
    let is_upper = matches!(key, '{' | '}');
    let is_open_bracket = matches!(key, '[' | '{');

    // 1. Double tap: nếu ký tự cuối là 'ư'/'Ư' và gõ tiếp '['/'{' -> khôi phục '[' / '{'
    if is_open_bracket {
        if word.ends_with(CHAR_U_HORN_LOWER) || word.ends_with(CHAR_U_HORN_UPPER) {
            let mut res = word.to_string();
            res.pop();
            res.push(if is_upper { '{' } else { '[' });
            return Some(res);
        }
    } else {
        // ']' hoặc '}'
        if word.ends_with(CHAR_O_HORN_LOWER) || word.ends_with(CHAR_O_HORN_UPPER) {
            let mut res = word.to_string();
            res.pop();
            res.push(if is_upper { '}' } else { ']' });
            return Some(res);
        }
    }

    let mut decomp = match decompose_word(word) {
        Some(d) => d,
        None => {
            let ch = if is_open_bracket {
                if is_upper { CHAR_U_HORN_UPPER } else { CHAR_U_HORN_LOWER }
            } else {
                if is_upper { CHAR_O_HORN_UPPER } else { CHAR_O_HORN_LOWER }
            };
            return Some(format!("{}{}", word, ch));
        }
    };

    if is_open_bracket {
        // '[' hoặc '{': biến 'u' thành 'ư', hoặc thêm 'ư'
        if !decomp.vowels.is_empty() {
            for v in decomp.vowels.iter_mut() {
                if v.base == 'u' {
                    v.vmod = VowelMod::Horn;
                    return Some(decomp.recompose());
                }
            }
        }
        let ch = if is_upper { CHAR_U_HORN_UPPER } else { CHAR_U_HORN_LOWER };
        Some(format!("{}{}", word, ch))
    } else {
        // ']' hoặc '}':
        // uo -> ươ
        if decomp.vowels.len() >= 2 {
            for i in 0..(decomp.vowels.len() - 1) {
                if decomp.vowels[i].base == 'u' && decomp.vowels[i + 1].base == 'o' {
                    decomp.vowels[i].vmod = VowelMod::Horn;
                    decomp.vowels[i + 1].vmod = VowelMod::Horn;
                    return Some(decomp.recompose());
                }
            }
        }
        // u -> ươ (ví dụ: 'du' + ']' -> 'dươ')
        if decomp.vowels.len() == 1 && decomp.vowels[0].base == 'u' {
            decomp.vowels[0].vmod = VowelMod::Horn;
            let o_horn = VowelPart {
                base: 'o',
                vmod: VowelMod::Horn,
                tone: Tone::None,
                is_upper: decomp.vowels[0].is_upper,
            };
            decomp.vowels.push(o_horn);
            return Some(decomp.recompose());
        }
        // o -> ơ
        for v in decomp.vowels.iter_mut() {
            if v.base == 'o' {
                v.vmod = VowelMod::Horn;
                return Some(decomp.recompose());
            }
        }
        let ch = if is_upper { CHAR_O_HORN_UPPER } else { CHAR_O_HORN_LOWER };
        Some(format!("{}{}", word, ch))
    }
}

/// Xử lý phím 'd' / 'D'
fn handle_d_key(word: &str, is_key_upper: bool, free_marking: bool) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return None;
    }

    let first = chars[0];
    let first_lower = first.to_ascii_lowercase();

    // Nếu từ bắt đầu bằng 'd' hoặc 'D'
    if first_lower == 'd' {
        if chars.len() == 1 || free_marking {
            let stroke = if first.is_uppercase() {
                CHAR_D_STROKE_UPPER
            } else {
                CHAR_D_STROKE_LOWER
            };
            let mut res = String::with_capacity(word.len());
            res.push(stroke);
            for &c in &chars[1..] {
                res.push(c);
            }
            return Some(res);
        }
    }

    // Nếu từ đã bắt đầu bằng 'đ' hoặc 'Đ': người dùng gõ tiếp 'd' -> khôi phục 'dd' (thoát dấu gạch)
    if first == CHAR_D_STROKE_LOWER || first == CHAR_D_STROKE_UPPER {
        let unstroke = if first == CHAR_D_STROKE_UPPER { 'D' } else { 'd' };
        let second = if is_key_upper { 'D' } else { 'd' };
        let mut res = String::with_capacity(word.len() + 1);
        res.push(unstroke);
        for &c in &chars[1..] {
            res.push(c);
        }
        res.push(second);
        return Some(res);
    }

    None
}

/// Xử lý phím mũ 'a', 'e', 'o'
fn handle_hat_key(word: &str, key: char, is_key_upper: bool, free_marking: bool) -> Option<String> {
    let mut decomp = decompose_word(word)?;
    if decomp.vowels.is_empty() {
        return None;
    }

    // Trong âm tiết mở (không có phụ âm cuối), phím mũ chỉ biến đổi khi gõ lặp lại
    // ngay sau nguyên âm cùng loại (ví dụ: "oo" -> "ô", "aa" -> "ă", "ee" -> "ê").
    // Nếu không, ký tự được gõ thêm bình thường (ví dụ: "ngoe" + "o" -> "ngoeo").
    if decomp.trail.is_empty() {
        let last_char = word.chars().last().unwrap_or(' ').to_ascii_lowercase();
        if last_char != key {
            return None;
        }
    }

    let mut target_idx = None;
    for (i, v) in decomp.vowels.iter().enumerate() {
        if v.base == key {
            target_idx = Some(i);
        }
    }

    let idx = target_idx?;

    if !free_marking && !decomp.trail.is_empty() {
        return None;
    }

    let current_mod = decomp.vowels[idx].vmod;

    if current_mod == VowelMod::None {
        decomp.vowels[idx].vmod = VowelMod::Hat;
        let new_word = decomp.recompose();
        return Some(new_word);
    } else if current_mod == VowelMod::Hat {
        // Đã có mũ, người dùng gõ lặp lại -> thoát mũ (ví dụ: 'â' + 'a' -> 'aa')
        decomp.vowels[idx].vmod = VowelMod::None;
        let base_word = decomp.recompose();
        let appended_char = if is_key_upper {
            key.to_ascii_uppercase()
        } else {
            key
        };
        return Some(format!("{}{}", base_word, appended_char));
    }

    None
}

/// Xử lý phím móc / trăng 'w' / 'W'
fn handle_w_key(
    word: &str,
    is_key_upper: bool,
    free_marking: bool,
    simple_telex: bool,
    raw_keys: Option<&str>,
) -> Option<String> {
    // 0. Double-tap 'w' thoát dấu móc chuẩn OpenKey / UniKey:
    if let Some(raw) = raw_keys {
        let raw_lower = raw.to_ascii_lowercase();

        // - Trường hợp 1: Chữ 'ư' đứng một mình:
        //   + Nếu tạo từ standalone 'w' (raw không kết thúc bằng 'uw'): Thoát thành 'w' (hoặc 'W').
        //   + Nếu tạo từ 'u' + 'w' (raw kết thúc bằng 'uw'): Thoát về 'uw' (hoặc 'uW').
        if word == CHAR_U_HORN_LOWER.to_string() || word == CHAR_U_HORN_UPPER.to_string() {
            if raw_lower.ends_with("uw") {
                return Some(if is_key_upper { "uW".to_string() } else { "uw".to_string() });
            } else {
                return Some(if is_key_upper { "W".to_string() } else { "w".to_string() });
            }
        }

        // - Trường hợp 2: Chữ 'ư' đứng sau phụ âm (ví dụ: 'rw' -> 'rư', 'ruw' -> 'rư'):
        if word.ends_with(CHAR_U_HORN_LOWER) || word.ends_with(CHAR_U_HORN_UPPER) {
            if raw_lower.ends_with("uw") {
                // Thoát về phụ âm + 'uw' (ví dụ: 'ruw' + 'w' -> 'ruw')
                let mut res = word.to_string();
                res.pop();
                res.push('u');
                res.push(if is_key_upper { 'W' } else { 'w' });
                return Some(res);
            } else if raw_lower.ends_with('w') {
                // Thoát về phụ âm + 'w' (ví dụ: 'rw' + 'w' -> 'rw')
                let mut res = word.to_string();
                res.pop();
                res.push(if is_key_upper { 'W' } else { 'w' });
                return Some(res);
            }
        }
    }

    let mut decomp = decompose_word(word)?;

    if decomp.vowels.is_empty() {
        // Chuẩn OpenKey (_standaloneWbad và _doubleWAllowed):
        // Chỉ các phụ âm tiếng Việt hợp lệ mới được nhận 'w' thành phụ âm + 'ư' (ví dụ: 'rw' -> 'rư', 'thw' -> 'thư')
        // Tuyệt đối không cho phép 'w', 'ww' nhận 'w' thành 'wư' (tránh gõ 'www' ra 'wư')!
        let lower = word.to_ascii_lowercase();
        let is_valid_lead = matches!(
            lower.as_str(),
            "b" | "c" | "d" | "đ" | "g" | "h" | "l" | "m" | "n" | "r" | "s" | "t" | "v" | "x"
            | "th" | "tr" | "ch" | "nh" | "ng" | "kh" | "ph" | "gh"
        );
        if is_valid_lead {
            let u_horn = if is_key_upper {
                CHAR_U_HORN_UPPER
            } else {
                CHAR_U_HORN_LOWER
            };
            return Some(format!("{}{}", word, u_horn));
        }
        return None;
    }

    if !free_marking && !decomp.trail.is_empty() {
        return None;
    }

    // 1. Kiểm tra cặp "oa" -> biến 'a' thành 'ă' (oă: hoẵng, ngoẵng, hoặc, xoăn...)
    // Trong tiếng Việt, 'oa' + 'w' LUÔN LUÔN là 'oă', tuyệt đối không bao giờ là 'ơa'!
    if !simple_telex && decomp.vowels.len() >= 2 {
        for i in 0..(decomp.vowels.len() - 1) {
            if decomp.vowels[i].base == 'o' && decomp.vowels[i + 1].base == 'a' {
                if decomp.vowels[i + 1].vmod == VowelMod::Breve {
                    decomp.vowels[i + 1].vmod = VowelMod::None;
                    let base_word = decomp.recompose();
                    let append_w = if is_key_upper { 'W' } else { 'w' };
                    return Some(format!("{}{}", base_word, append_w));
                } else {
                    decomp.vowels[i + 1].vmod = VowelMod::Breve;
                    return Some(decomp.recompose());
                }
            }
        }
    }

    // 2. Kiểm tra cặp "uo" -> biến thành "ươ" (hoặc "thuở" nếu bắt đầu bằng "th")
    if !simple_telex && decomp.vowels.len() >= 2 {
        for i in 0..(decomp.vowels.len() - 1) {
            if decomp.vowels[i].base == 'u' && decomp.vowels[i + 1].base == 'o' {
                let is_thuo = decomp.lead == "th" && decomp.trail.is_empty();
                if is_thuo {
                    // Từ "thuở" duy nhất trong tiếng Việt: u giữ nguyên, chỉ o thành ơ
                    if decomp.vowels[i + 1].vmod == VowelMod::Horn {
                        decomp.vowels[i + 1].vmod = VowelMod::None;
                        let base_word = decomp.recompose();
                        let append_w = if is_key_upper { 'W' } else { 'w' };
                        return Some(format!("{}{}", base_word, append_w));
                    } else {
                        decomp.vowels[i + 1].vmod = VowelMod::Horn;
                        return Some(decomp.recompose());
                    }
                } else if decomp.vowels[i].vmod == VowelMod::Horn
                    && decomp.vowels[i + 1].vmod == VowelMod::Horn
                {
                    decomp.vowels[i].vmod = VowelMod::None;
                    decomp.vowels[i + 1].vmod = VowelMod::None;
                    let base_word = decomp.recompose();
                    let append_w = if is_key_upper { 'W' } else { 'w' };
                    return Some(format!("{}{}", base_word, append_w));
                } else {
                    decomp.vowels[i].vmod = VowelMod::Horn;
                    decomp.vowels[i + 1].vmod = VowelMod::Horn;
                    return Some(decomp.recompose());
                }
            }
        }
    }

    // 3. Kiểm tra 'u' -> 'ư'
    for v in decomp.vowels.iter_mut() {
        if v.base == 'u' {
            if v.vmod == VowelMod::Horn {
                v.vmod = VowelMod::None;
                let base = decomp.recompose();
                let append_w = if is_key_upper { 'W' } else { 'w' };
                return Some(format!("{}{}", base, append_w));
            } else {
                v.vmod = VowelMod::Horn;
                return Some(decomp.recompose());
            }
        }
    }

    // 4. Kiểm tra 'o' -> 'ơ'
    if !simple_telex {
        for v in decomp.vowels.iter_mut() {
            if v.base == 'o' {
                if v.vmod == VowelMod::Horn {
                    v.vmod = VowelMod::None;
                    let base = decomp.recompose();
                    let append_w = if is_key_upper { 'W' } else { 'w' };
                    return Some(format!("{}{}", base, append_w));
                } else if v.vmod == VowelMod::None {
                    v.vmod = VowelMod::Horn;
                    return Some(decomp.recompose());
                }
            }
        }
    }

    // 5. Kiểm tra 'a' -> 'ă'
    if !simple_telex {
        for v in decomp.vowels.iter_mut() {
            if v.base == 'a' {
                if v.vmod == VowelMod::Breve {
                    v.vmod = VowelMod::None;
                    let base = decomp.recompose();
                    let append_w = if is_key_upper { 'W' } else { 'w' };
                    return Some(format!("{}{}", base, append_w));
                } else if v.vmod == VowelMod::None {
                    v.vmod = VowelMod::Breve;
                    return Some(decomp.recompose());
                }
            }
        }
    }

    None
}

/// Xử lý các phím dấu thanh 's', 'f', 'r', 'x', 'j', 'z'
fn handle_tone_key(
    word: &str,
    key: char,
    modern_tone: bool,
    free_marking: bool,
) -> Option<String> {
    let mut decomp = decompose_word(word)?;
    if decomp.vowels.is_empty() {
        return None;
    }

    if !free_marking && !decomp.trail.is_empty() {
        return None;
    }

    let target_tone = match key {
        's' => Tone::Acute,
        'f' => Tone::Grave,
        'r' => Tone::Hook,
        'x' => Tone::Tilde,
        'j' => Tone::Dot,
        'z' => Tone::None,
        _ => return None,
    };

    let current = decomp.current_tone();

    // Double-tap tone -> hủy dấu thanh và gõ ký tự ra
    if current != Tone::None && current == target_tone {
        decomp.clear_tone();
        let base_word = decomp.recompose();
        return Some(format!("{}{}", base_word, key));
    }

    decomp.apply_tone(target_tone, modern_tone);
    let new_word = decomp.recompose();

    // Kiểm tra quy tắc âm tắc: c, ch, p, t chỉ đi với Sắc hoặc Nặng
    if matches!(decomp.trail.as_str(), "c" | "ch" | "p" | "t") {
        if matches!(target_tone, Tone::Grave | Tone::Hook | Tone::Tilde) {
            return None;
        }
    }

    Some(new_word)
}
