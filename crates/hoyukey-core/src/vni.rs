// hoyukey-core: Dynamic VNI Transformation Engine
// Thuật toán biến đổi phím VNI dựa trên hình vị học động (Zero-dependency, không dùng từ điển).

use crate::phonotactics::{decompose_word, is_english_foreign_word};
use crate::telex::{CHAR_D_STROKE_LOWER, CHAR_D_STROKE_UPPER};
use crate::vietnamese::{Tone, VowelMod};

pub fn transform_vni(
    word: &str,
    key: char,
    modern_tone: bool,
    free_marking: bool,
    smart_english: bool,
    modify_tone_in_word: bool,
) -> Option<String> {
    if word.is_empty() {
        return None;
    }

    if smart_english && is_english_foreign_word(word) {
        return None;
    }

    match key {
        // 1..=5: Dấu thanh, 0: Xóa dấu
        '1' | '2' | '3' | '4' | '5' | '0' => {
            handle_vni_tone(word, key, modern_tone, free_marking, modify_tone_in_word)
        }
        // 6: Dấu mũ (â, ê, ô)
        '6' => handle_vni_hat(word, free_marking, modern_tone),
        // 7: Dấu móc (ơ, ư)
        '7' => handle_vni_horn(word, free_marking, modern_tone),
        // 8: Dấu trăng (ă)
        '8' => handle_vni_breve(word, free_marking, modern_tone),
        // 9: Dấu gạch đ/Đ
        '9' => handle_vni_stroke(word, free_marking),
        _ => None,
    }
}

fn handle_vni_tone(
    word: &str,
    key: char,
    modern_tone: bool,
    free_marking: bool,
    modify_tone_in_word: bool,
) -> Option<String> {
    let mut decomp = decompose_word(word)?;
    if decomp.vowels.is_empty() {
        return None;
    }

    if !free_marking && !decomp.trail.is_empty() {
        return None;
    }

    let target_tone = match key {
        '1' => Tone::Acute,
        '2' => Tone::Grave,
        '3' => Tone::Hook,
        '4' => Tone::Tilde,
        '5' => Tone::Dot,
        '0' => Tone::None,
        _ => return None,
    };

    let current = decomp.current_tone();

    // Double-tap phím số: hủy dấu và gõ số ra
    if current != Tone::None && current == target_tone {
        decomp.clear_tone();
        let base = decomp.recompose();
        return Some(format!("{}{}", base, key));
    }

    // Nếu tắt "Cho phép sửa dấu trong từ" (modify_tone_in_word == false):
    if !modify_tone_in_word && current != Tone::None && target_tone != Tone::None {
        return None;
    }

    decomp.apply_tone(target_tone, modern_tone);
    let new_word = decomp.recompose();

    if matches!(decomp.trail.as_str(), "c" | "ch" | "p" | "t") {
        if matches!(target_tone, Tone::Grave | Tone::Hook | Tone::Tilde) {
            return None;
        }
    }

    Some(new_word)
}

fn handle_vni_hat(word: &str, free_marking: bool, modern_tone: bool) -> Option<String> {
    let mut decomp = decompose_word(word)?;
    if decomp.vowels.is_empty() {
        return None;
    }

    if !free_marking && !decomp.trail.is_empty() {
        return None;
    }

    let existing_tone = decomp.current_tone();

    // Tìm a, e, o
    for v in decomp.vowels.iter_mut() {
        if matches!(v.base, 'a' | 'e' | 'o') {
            if v.vmod == VowelMod::Hat {
                v.vmod = VowelMod::None;
                let base = decomp.recompose();
                return Some(format!("{}6", base));
            } else if v.vmod == VowelMod::None {
                v.vmod = VowelMod::Hat;
                if existing_tone != Tone::None {
                    decomp.apply_tone(existing_tone, modern_tone);
                }
                return Some(decomp.recompose());
            }
        }
    }

    None
}

fn handle_vni_horn(word: &str, free_marking: bool, modern_tone: bool) -> Option<String> {
    let mut decomp = decompose_word(word)?;
    if decomp.vowels.is_empty() {
        return None;
    }

    if !free_marking && !decomp.trail.is_empty() {
        return None;
    }

    let existing_tone = decomp.current_tone();

    // Kiểm tra cặp uo -> ươ
    if decomp.vowels.len() >= 2 {
        for i in 0..(decomp.vowels.len() - 1) {
            if decomp.vowels[i].base == 'u' && decomp.vowels[i + 1].base == 'o' {
                if decomp.vowels[i].vmod == VowelMod::Horn && decomp.vowels[i + 1].vmod == VowelMod::Horn {
                    decomp.vowels[i].vmod = VowelMod::None;
                    decomp.vowels[i + 1].vmod = VowelMod::None;
                    let base = decomp.recompose();
                    return Some(format!("{}7", base));
                } else {
                    decomp.vowels[i].vmod = VowelMod::Horn;
                    decomp.vowels[i + 1].vmod = VowelMod::Horn;
                    if existing_tone != Tone::None {
                        decomp.apply_tone(existing_tone, modern_tone);
                    }
                    return Some(decomp.recompose());
                }
            }
        }
    }

    // Kiểm tra u -> ư hoặc o -> ơ
    for v in decomp.vowels.iter_mut() {
        if matches!(v.base, 'u' | 'o') {
            if v.vmod == VowelMod::Horn {
                v.vmod = VowelMod::None;
                let base = decomp.recompose();
                return Some(format!("{}7", base));
            } else if v.vmod == VowelMod::None {
                v.vmod = VowelMod::Horn;
                if existing_tone != Tone::None {
                    decomp.apply_tone(existing_tone, modern_tone);
                }
                return Some(decomp.recompose());
            }
        }
    }

    None
}

fn handle_vni_breve(word: &str, free_marking: bool, modern_tone: bool) -> Option<String> {
    let mut decomp = decompose_word(word)?;
    if decomp.vowels.is_empty() {
        return None;
    }

    if !free_marking && !decomp.trail.is_empty() {
        return None;
    }

    let existing_tone = decomp.current_tone();

    for v in decomp.vowels.iter_mut() {
        if v.base == 'a' {
            if v.vmod == VowelMod::Breve {
                v.vmod = VowelMod::None;
                let base = decomp.recompose();
                return Some(format!("{}8", base));
            } else if v.vmod == VowelMod::None {
                v.vmod = VowelMod::Breve;
                if existing_tone != Tone::None {
                    decomp.apply_tone(existing_tone, modern_tone);
                }
                return Some(decomp.recompose());
            }
        }
    }

    None
}

fn handle_vni_stroke(word: &str, free_marking: bool) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return None;
    }

    let first = chars[0];
    let first_lower = first.to_ascii_lowercase();

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

    if first == CHAR_D_STROKE_LOWER || first == CHAR_D_STROKE_UPPER {
        let unstroke = if first == CHAR_D_STROKE_UPPER { 'D' } else { 'd' };
        let mut res = String::with_capacity(word.len() + 1);
        res.push(unstroke);
        for &c in &chars[1..] {
            res.push(c);
        }
        res.push('9');
        return Some(res);
    }

    None
}
