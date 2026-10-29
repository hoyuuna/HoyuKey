use super::telex::extract_vowel_indices;
use super::vietnamese::{decompose_vietnamese_char, get_vietnamese_vowel, Tone, VowelMod};

pub fn transform_vni(word: &str, key: char, modern_tone: bool) -> Option<String> {
    if !key.is_ascii_digit() {
        return None;
    }

    match key {
        '1'..='5' | '0' => handle_vni_tone(word, key, modern_tone),
        '6' => handle_vni_hat(word),
        '7' => handle_vni_horn(word),
        '8' => handle_vni_breve(word),
        '9' => handle_vni_d(word),
        _ => None,
    }
}

/// VNI 9: 'd' <-> 'đ'
fn handle_vni_d(word: &str) -> Option<String> {
    let mut chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return None;
    }

    // Đảo ngược 'đ' + '9' -> 'd9'
    for (i, &c) in chars.iter().enumerate() {
        if c == 'đ' || c == 'Đ' {
            chars[i] = if c == 'Đ' { 'D' } else { 'd' };
            chars.insert(i + 1, '9');
            return Some(chars.into_iter().collect());
        }
    }

    // 'd' + '9' -> 'đ'
    for (i, &c) in chars.iter().enumerate() {
        if c == 'd' || c == 'D' {
            chars[i] = if c == 'D' { 'Đ' } else { 'đ' };
            return Some(chars.into_iter().collect());
        }
    }

    None
}

/// VNI 6: Dấu mũ cho 'a'->'â', 'e'->'ê', 'o'->'ô'
fn handle_vni_hat(word: &str) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return None;
    }

    // Kiểm tra Undo nếu đã có dấu mũ
    for (i, &c) in chars.iter().enumerate() {
        if let Some((base, vmod, tone, is_upper)) = decompose_vietnamese_char(c) {
            if vmod == VowelMod::Hat && matches!(base, 'a' | 'e' | 'o') {
                let mut new_chars = chars.clone();
                let restored = get_vietnamese_vowel(base, VowelMod::None, tone).unwrap_or(base);
                new_chars[i] = if is_upper { restored.to_uppercase().next().unwrap_or(restored) } else { restored };
                new_chars.push('6');
                return Some(new_chars.into_iter().collect());
            }
        }
    }

    // Áp dụng dấu mũ vào nguyên âm thích hợp
    for i in (0..chars.len()).rev() {
        let c = chars[i];
        if let Some((base, vmod, tone, is_upper)) = decompose_vietnamese_char(c) {
            if vmod == VowelMod::None && matches!(base, 'a' | 'e' | 'o') {
                let mut new_chars = chars.clone();
                let new_vowel = get_vietnamese_vowel(base, VowelMod::Hat, tone).unwrap_or(c);
                new_chars[i] = if is_upper { new_vowel.to_uppercase().next().unwrap_or(new_vowel) } else { new_vowel };
                return Some(new_chars.into_iter().collect());
            }
        }
    }

    None
}

/// VNI 7: Dấu móc cho 'o'->'ơ', 'u'->'ư', 'uo'->'ươ'
fn handle_vni_horn(word: &str) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return None;
    }

    // 1. Kiểm tra cặp 'uo' -> 'ươ' (trừ 'qu')
    let mut found_uo = None;
    for i in 0..chars.len().saturating_sub(1) {
        if i > 0 && chars[i - 1].to_ascii_lowercase() == 'q' {
            continue;
        }
        let c1 = chars[i].to_ascii_lowercase();
        let c2 = chars[i + 1].to_ascii_lowercase();
        if (c1 == 'u' || c1 == 'ư') && (c2 == 'o' || c2 == 'ơ') {
            found_uo = Some(i);
            break;
        }
    }

    if let Some(i) = found_uo {
        let mut new_chars = chars.clone();
        let (_, _, t1, u1) = decompose_vietnamese_char(new_chars[i]).unwrap_or(('u', VowelMod::None, Tone::None, false));
        let (_, _, t2, u2) = decompose_vietnamese_char(new_chars[i + 1]).unwrap_or(('o', VowelMod::None, Tone::None, false));

        let new_u = get_vietnamese_vowel('u', VowelMod::Horn, t1).unwrap_or('ư');
        let new_u = if u1 { new_u.to_uppercase().next().unwrap_or(new_u) } else { new_u };

        let new_o = get_vietnamese_vowel('o', VowelMod::Horn, t2).unwrap_or('ơ');
        let new_o = if u2 { new_o.to_uppercase().next().unwrap_or(new_o) } else { new_o };

        new_chars[i] = new_u;
        new_chars[i + 1] = new_o;
        return Some(new_chars.into_iter().collect());
    }

    // 2. Kiểm tra Undo cho 'ơ' hoặc 'ư'
    for (i, &c) in chars.iter().enumerate() {
        if let Some((base, vmod, tone, is_upper)) = decompose_vietnamese_char(c) {
            if vmod == VowelMod::Horn && matches!(base, 'o' | 'u') {
                let mut new_chars = chars.clone();
                let restored = get_vietnamese_vowel(base, VowelMod::None, tone).unwrap_or(base);
                new_chars[i] = if is_upper { restored.to_uppercase().next().unwrap_or(restored) } else { restored };
                new_chars.push('7');
                return Some(new_chars.into_iter().collect());
            }
        }
    }

    // 3. Áp dụng dấu móc cho 'o' hoặc 'u' (trừ 'qu')
    for i in (0..chars.len()).rev() {
        if chars[i].to_ascii_lowercase() == 'u' && i > 0 && chars[i - 1].to_ascii_lowercase() == 'q' {
            continue;
        }

        let c = chars[i];
        if let Some((base, vmod, tone, is_upper)) = decompose_vietnamese_char(c) {
            if vmod == VowelMod::None && matches!(base, 'o' | 'u') {
                let mut new_chars = chars.clone();
                let new_vowel = get_vietnamese_vowel(base, VowelMod::Horn, tone).unwrap_or(c);
                new_chars[i] = if is_upper { new_vowel.to_uppercase().next().unwrap_or(new_vowel) } else { new_vowel };
                return Some(new_chars.into_iter().collect());
            }
        }
    }

    None
}

/// VNI 8: Dấu trăng cho 'a'->'ă'
fn handle_vni_breve(word: &str) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return None;
    }

    // Undo nếu đã có 'ă'
    for (i, &c) in chars.iter().enumerate() {
        if let Some((base, vmod, tone, is_upper)) = decompose_vietnamese_char(c) {
            if vmod == VowelMod::Breve && base == 'a' {
                let mut new_chars = chars.clone();
                let restored = get_vietnamese_vowel(base, VowelMod::None, tone).unwrap_or(base);
                new_chars[i] = if is_upper { restored.to_uppercase().next().unwrap_or(restored) } else { restored };
                new_chars.push('8');
                return Some(new_chars.into_iter().collect());
            }
        }
    }

    // Áp dụng 'a' -> 'ă'
    for i in (0..chars.len()).rev() {
        let c = chars[i];
        if let Some((base, vmod, tone, is_upper)) = decompose_vietnamese_char(c) {
            if vmod == VowelMod::None && base == 'a' {
                let mut new_chars = chars.clone();
                let new_vowel = get_vietnamese_vowel(base, VowelMod::Breve, tone).unwrap_or(c);
                new_chars[i] = if is_upper { new_vowel.to_uppercase().next().unwrap_or(new_vowel) } else { new_vowel };
                return Some(new_chars.into_iter().collect());
            }
        }
    }

    None
}

/// VNI 1-5, 0: Dấu thanh
fn handle_vni_tone(word: &str, key: char, _modern_tone: bool) -> Option<String> {
    let target_tone = match key {
        '1' => Tone::Acute,
        '2' => Tone::Grave,
        '3' => Tone::Hook,
        '4' => Tone::Tilde,
        '5' => Tone::Dot,
        '0' => Tone::None,
        _ => return None,
    };

    let chars: Vec<char> = word.chars().collect();
    let vowel_indices = extract_vowel_indices(&chars);
    if vowel_indices.is_empty() {
        return None;
    }

    let mut current_tone = Tone::None;
    for &idx in &vowel_indices {
        if let Some((_, _, tone, _)) = decompose_vietnamese_char(chars[idx]) {
            if tone != Tone::None {
                current_tone = tone;
            }
        }
    }

    // Undo nếu gõ lại cùng phím số dấu
    if target_tone == current_tone && target_tone != Tone::None {
        let mut new_chars = chars.clone();
        for &idx in &vowel_indices {
            let (base, vmod, _, is_upper) = decompose_vietnamese_char(new_chars[idx])?;
            let restored = get_vietnamese_vowel(base, vmod, Tone::None)?;
            new_chars[idx] = if is_upper { restored.to_uppercase().next().unwrap_or(restored) } else { restored };
        }
        new_chars.push(key);
        return Some(new_chars.into_iter().collect());
    }

    // Chọn vị trí nguyên âm nhận dấu
    let target_idx = if vowel_indices.len() == 1 {
        vowel_indices[0]
    } else {
        let mut modified = None;
        for &idx in &vowel_indices {
            if let Some((_, vmod, _, _)) = decompose_vietnamese_char(chars[idx]) {
                if vmod != VowelMod::None {
                    modified = Some(idx);
                }
            }
        }
        if let Some(m) = modified {
            m
        } else {
            vowel_indices[vowel_indices.len().saturating_sub(1)]
        }
    };

    let mut new_chars = chars.clone();
    for &idx in &vowel_indices {
        let (base, vmod, _, is_upper) = decompose_vietnamese_char(new_chars[idx])?;
        let applied_tone = if idx == target_idx { target_tone } else { Tone::None };
        let new_vowel = get_vietnamese_vowel(base, vmod, applied_tone)?;
        new_chars[idx] = if is_upper { new_vowel.to_uppercase().next().unwrap_or(new_vowel) } else { new_vowel };
    }

    Some(new_chars.into_iter().collect())
}
