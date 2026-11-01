use super::telex::extract_vowel_indices;
use super::vietnamese::{decompose_vietnamese_char, get_vietnamese_vowel, Tone, VowelMod};

pub fn transform_vni(
    word: &str,
    key: char,
    modern_tone: bool,
    free_marking: bool,
    smart_english: bool,
) -> Option<String> {
    if !key.is_ascii_digit() {
        return None;
    }

    // Không biến đổi lặp lại nếu từ đã kết thúc bằng chính phím số này (tránh dao động khi giữ phím số)
    if word.ends_with(key) {
        return None;
    }

    if smart_english && super::telex::is_foreign_word(word) {
        return None;
    }

    match key {
        '1'..='5' | '0' => handle_vni_tone(word, key, modern_tone),
        '6' => handle_vni_hat(word, free_marking),
        '7' => handle_vni_horn(word, free_marking),
        '8' => handle_vni_breve(word, free_marking),
        '9' => handle_vni_d(word, free_marking),
        _ => None,
    }
}

/// VNI 9: 'd' <-> 'đ'
fn handle_vni_d(word: &str, free_marking: bool) -> Option<String> {
    let mut chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return None;
    }

    // 1. Kiểm tra ký tự cuối trước
    let last_idx = chars.len() - 1;
    let last_char = chars[last_idx];

    // 'đ' + '9' -> 'd9' (undo)
    if last_char == 'đ' || last_char == 'Đ' {
        chars[last_idx] = if last_char == 'Đ' { 'D' } else { 'd' };
        chars.push('9');
        return Some(chars.into_iter().collect());
    }

    // 'd' + '9' -> 'đ'
    if last_char == 'd' || last_char == 'D' {
        chars[last_idx] = if last_char == 'D' { 'Đ' } else { 'đ' };
        return Some(chars.into_iter().collect());
    }

    // 2. Nếu bật free_marking: tìm 'd' / 'đ' ở bất kỳ vị trí nào trong từ
    if free_marking {
        // Tìm undo trước: nếu từ đã có 'đ' / 'Đ'
        if let Some(pos) = chars.iter().position(|&c| c == 'đ' || c == 'Đ') {
            chars[pos] = if chars[pos] == 'Đ' { 'D' } else { 'd' };
            chars.push('9');
            return Some(chars.into_iter().collect());
        }

        // Tìm 'd' / 'D' để biến đổi thành 'đ' / 'Đ'
        if let Some(pos) = chars.iter().position(|&c| c == 'd' || c == 'D') {
            chars[pos] = if chars[pos] == 'D' { 'Đ' } else { 'đ' };
            return Some(chars.into_iter().collect());
        }
    }

    None
}

/// VNI 6: Dấu mũ cho 'a'->'â', 'e'->'ê', 'o'->'ô'
fn handle_vni_hat(word: &str, free_marking: bool) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return None;
    }

    let last_idx = chars.len() - 1;
    let last_c = chars[last_idx];

    // 1. Kiểm tra ký tự cuối trước (undo hoặc áp dụng)
    if let Some((base, vmod, tone, is_upper)) = decompose_vietnamese_char(last_c) {
        if vmod == VowelMod::Hat && matches!(base, 'a' | 'e' | 'o') {
            let mut new_chars = chars.clone();
            let restored = get_vietnamese_vowel(base, VowelMod::None, tone).unwrap_or(base);
            new_chars[last_idx] = if is_upper { restored.to_uppercase().next().unwrap_or(restored) } else { restored };
            new_chars.push('6');
            return Some(new_chars.into_iter().collect());
        }
        if vmod == VowelMod::None && matches!(base, 'a' | 'e' | 'o') {
            let mut new_chars = chars.clone();
            let new_vowel = get_vietnamese_vowel(base, VowelMod::Hat, tone).unwrap_or(last_c);
            new_chars[last_idx] = if is_upper { new_vowel.to_uppercase().next().unwrap_or(new_vowel) } else { new_vowel };
            return Some(new_chars.into_iter().collect());
        }
    }

    // 2. Nếu bật free_marking: tìm nguyên âm 'a', 'e', 'o' trong toàn bộ từ
    if free_marking {
        let vowel_indices = extract_vowel_indices(&chars);
        if vowel_indices.is_empty() {
            return None;
        }

        // Kiểm tra undo: nếu đã có nguyên âm mang Hat
        for &idx in &vowel_indices {
            if let Some((base, vmod, tone, is_upper)) = decompose_vietnamese_char(chars[idx]) {
                if vmod == VowelMod::Hat && matches!(base, 'a' | 'e' | 'o') {
                    let mut new_chars = chars.clone();
                    let restored = get_vietnamese_vowel(base, VowelMod::None, tone).unwrap_or(base);
                    new_chars[idx] = if is_upper { restored.to_uppercase().next().unwrap_or(restored) } else { restored };
                    new_chars.push('6');
                    return Some(new_chars.into_iter().collect());
                }
            }
        }

        // Chọn nguyên âm nhận dấu mũ: ưu tiên nguyên âm từ phải qua trái (âm chính)
        let mut target_candidate = None;
        for &idx in vowel_indices.iter().rev() {
            if let Some((base, vmod, _, _)) = decompose_vietnamese_char(chars[idx]) {
                if vmod == VowelMod::None && matches!(base, 'a' | 'e' | 'o') {
                    target_candidate = Some(idx);
                    break;
                }
            }
        }

        if let Some(idx) = target_candidate {
            let (base, _, tone, is_upper) = decompose_vietnamese_char(chars[idx])?;
            let mut new_chars = chars.clone();
            let new_vowel = get_vietnamese_vowel(base, VowelMod::Hat, tone).unwrap_or(chars[idx]);
            new_chars[idx] = if is_upper { new_vowel.to_uppercase().next().unwrap_or(new_vowel) } else { new_vowel };
            return Some(new_chars.into_iter().collect());
        }
    }

    None
}

/// VNI 7: Dấu móc cho 'o'->'ơ', 'u'->'ư', 'uo'->'ươ'
fn handle_vni_horn(word: &str, free_marking: bool) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return None;
    }

    // 1. Kiểm tra cặp 'ua' -> 'ưa' trong toàn từ (ngoại trừ sau 'q')
    let mut found_ua = None;
    let mut found_ua_already_horned = None;
    for i in 0..chars.len().saturating_sub(1) {
        if i > 0 && chars[i - 1].to_ascii_lowercase() == 'q' {
            continue;
        }
        let c1 = chars[i].to_ascii_lowercase();
        let c2 = chars[i + 1].to_ascii_lowercase();
        if c1 == 'ư' && c2 == 'a' {
            found_ua_already_horned = Some(i);
            break;
        }
        if c1 == 'u' && c2 == 'a' {
            found_ua = Some(i);
            break;
        }
    }

    if let Some(i) = found_ua_already_horned {
        let mut new_chars = chars.clone();
        let (_, _, t1, u1) = decompose_vietnamese_char(new_chars[i]).unwrap_or(('u', VowelMod::Horn, Tone::None, false));
        let (_, _, t2, u2) = decompose_vietnamese_char(new_chars[i + 1]).unwrap_or(('a', VowelMod::None, Tone::None, false));

        let restored_u = get_vietnamese_vowel('u', VowelMod::None, t1).unwrap_or('u');
        let restored_u = if u1 { restored_u.to_uppercase().next().unwrap_or(restored_u) } else { restored_u };
        let restored_a = get_vietnamese_vowel('a', VowelMod::None, t2).unwrap_or('a');
        let restored_a = if u2 { restored_a.to_uppercase().next().unwrap_or(restored_a) } else { restored_a };

        new_chars[i] = restored_u;
        new_chars[i + 1] = restored_a;
        new_chars.push('7');
        return Some(new_chars.into_iter().collect());
    }

    if let Some(i) = found_ua {
        let mut new_chars = chars.clone();
        let (_, _, t1, u1) = decompose_vietnamese_char(new_chars[i]).unwrap_or(('u', VowelMod::None, Tone::None, false));
        let (_, _, t2, u2) = decompose_vietnamese_char(new_chars[i + 1]).unwrap_or(('a', VowelMod::None, Tone::None, false));

        let new_u = get_vietnamese_vowel('u', VowelMod::Horn, t1).unwrap_or('ư');
        let new_u = if u1 { new_u.to_uppercase().next().unwrap_or(new_u) } else { new_u };
        let new_a = get_vietnamese_vowel('a', VowelMod::None, t2).unwrap_or('a');
        let new_a = if u2 { new_a.to_uppercase().next().unwrap_or(new_a) } else { new_a };

        new_chars[i] = new_u;
        new_chars[i + 1] = new_a;
        return Some(new_chars.into_iter().collect());
    }

    // 2. Kiểm tra cặp 'uo' -> 'ươ' trong toàn từ
    let mut found_uo = None;
    let mut found_uo_already_horned = None;
    for i in 0..chars.len().saturating_sub(1) {
        if i > 0 && chars[i - 1].to_ascii_lowercase() == 'q' {
            continue;
        }
        let c1 = chars[i].to_ascii_lowercase();
        let c2 = chars[i + 1].to_ascii_lowercase();
        if c1 == 'ư' && c2 == 'ơ' {
            found_uo_already_horned = Some(i);
            break;
        }
        if (c1 == 'u' || c1 == 'ư') && (c2 == 'o' || c2 == 'ơ') {
            found_uo = Some(i);
            break;
        }
    }

    // Undo cho 'ươ' -> 'uo7'
    if let Some(i) = found_uo_already_horned {
        let mut new_chars = chars.clone();
        let (_, _, t1, u1) = decompose_vietnamese_char(new_chars[i]).unwrap_or(('u', VowelMod::Horn, Tone::None, false));
        let (_, _, t2, u2) = decompose_vietnamese_char(new_chars[i + 1]).unwrap_or(('o', VowelMod::Horn, Tone::None, false));

        let restored_u = get_vietnamese_vowel('u', VowelMod::None, t1).unwrap_or('u');
        let restored_u = if u1 { restored_u.to_uppercase().next().unwrap_or(restored_u) } else { restored_u };

        let restored_o = get_vietnamese_vowel('o', VowelMod::None, t2).unwrap_or('o');
        let restored_o = if u2 { restored_o.to_uppercase().next().unwrap_or(restored_o) } else { restored_o };

        new_chars[i] = restored_u;
        new_chars[i + 1] = restored_o;
        new_chars.push('7');
        return Some(new_chars.into_iter().collect());
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

    let last_idx = chars.len() - 1;
    let last_c = chars[last_idx];

    // Undo cho 'ơ' hoặc 'ư' ở cuối
    if let Some((base, vmod, tone, is_upper)) = decompose_vietnamese_char(last_c) {
        if vmod == VowelMod::Horn && matches!(base, 'o' | 'u') {
            let mut new_chars = chars.clone();
            let restored = get_vietnamese_vowel(base, VowelMod::None, tone).unwrap_or(base);
            new_chars[last_idx] = if is_upper { restored.to_uppercase().next().unwrap_or(restored) } else { restored };
            new_chars.push('7');
            return Some(new_chars.into_iter().collect());
        }
    }

    // Áp dụng dấu móc cho 'o' hoặc 'u' ở cuối
    if last_idx > 0 && chars[last_idx - 1].to_ascii_lowercase() == 'q' && last_c.to_ascii_lowercase() == 'u' {
        // bỏ qua 'qu'
    } else if let Some((base, vmod, tone, is_upper)) = decompose_vietnamese_char(last_c) {
        if vmod == VowelMod::None && matches!(base, 'o' | 'u') {
            let mut new_chars = chars.clone();
            let new_vowel = get_vietnamese_vowel(base, VowelMod::Horn, tone).unwrap_or(last_c);
            new_chars[last_idx] = if is_upper { new_vowel.to_uppercase().next().unwrap_or(new_vowel) } else { new_vowel };
            return Some(new_chars.into_iter().collect());
        }
    }

    // 2. Nếu bật free_marking: tìm 'u' hoặc 'o' trong toàn từ
    if free_marking {
        let vowel_indices = extract_vowel_indices(&chars);
        if vowel_indices.is_empty() {
            return None;
        }

        // Kiểm tra undo
        for &idx in &vowel_indices {
            if idx > 0 && chars[idx - 1].to_ascii_lowercase() == 'q' && chars[idx].to_ascii_lowercase() == 'u' {
                continue;
            }
            if let Some((base, vmod, tone, is_upper)) = decompose_vietnamese_char(chars[idx]) {
                if vmod == VowelMod::Horn && matches!(base, 'o' | 'u') {
                    let mut new_chars = chars.clone();
                    let restored = get_vietnamese_vowel(base, VowelMod::None, tone).unwrap_or(base);
                    new_chars[idx] = if is_upper { restored.to_uppercase().next().unwrap_or(restored) } else { restored };
                    new_chars.push('7');
                    return Some(new_chars.into_iter().collect());
                }
            }
        }

        // Áp dụng: tìm 'u' hoặc 'o'
        for &idx in vowel_indices.iter().rev() {
            if idx > 0 && chars[idx - 1].to_ascii_lowercase() == 'q' && chars[idx].to_ascii_lowercase() == 'u' {
                continue;
            }
            if let Some((base, vmod, tone, is_upper)) = decompose_vietnamese_char(chars[idx]) {
                if vmod == VowelMod::None && matches!(base, 'o' | 'u') {
                    let mut new_chars = chars.clone();
                    let new_vowel = get_vietnamese_vowel(base, VowelMod::Horn, tone).unwrap_or(chars[idx]);
                    new_chars[idx] = if is_upper { new_vowel.to_uppercase().next().unwrap_or(new_vowel) } else { new_vowel };
                    return Some(new_chars.into_iter().collect());
                }
            }
        }
    }

    None
}

/// VNI 8: Dấu trăng cho 'a'->'ă'
fn handle_vni_breve(word: &str, free_marking: bool) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return None;
    }

    let last_idx = chars.len() - 1;
    let last_c = chars[last_idx];

    // 1. Kiểm tra ký tự cuối trước
    if let Some((base, vmod, tone, is_upper)) = decompose_vietnamese_char(last_c) {
        if vmod == VowelMod::Breve && base == 'a' {
            let mut new_chars = chars.clone();
            let restored = get_vietnamese_vowel(base, VowelMod::None, tone).unwrap_or(base);
            new_chars[last_idx] = if is_upper { restored.to_uppercase().next().unwrap_or(restored) } else { restored };
            new_chars.push('8');
            return Some(new_chars.into_iter().collect());
        }
        if vmod == VowelMod::None && base == 'a' {
            let mut new_chars = chars.clone();
            let new_vowel = get_vietnamese_vowel(base, VowelMod::Breve, tone).unwrap_or(last_c);
            new_chars[last_idx] = if is_upper { new_vowel.to_uppercase().next().unwrap_or(new_vowel) } else { new_vowel };
            return Some(new_chars.into_iter().collect());
        }
    }

    // 2. Nếu bật free_marking: tìm 'a' trong toàn từ
    if free_marking {
        let vowel_indices = extract_vowel_indices(&chars);
        if vowel_indices.is_empty() {
            return None;
        }

        // Undo
        for &idx in &vowel_indices {
            if let Some((base, vmod, tone, is_upper)) = decompose_vietnamese_char(chars[idx]) {
                if vmod == VowelMod::Breve && base == 'a' {
                    let mut new_chars = chars.clone();
                    let restored = get_vietnamese_vowel(base, VowelMod::None, tone).unwrap_or(base);
                    new_chars[idx] = if is_upper { restored.to_uppercase().next().unwrap_or(restored) } else { restored };
                    new_chars.push('8');
                    return Some(new_chars.into_iter().collect());
                }
            }
        }

        // Áp dụng
        for &idx in vowel_indices.iter().rev() {
            if let Some((base, vmod, tone, is_upper)) = decompose_vietnamese_char(chars[idx]) {
                if vmod == VowelMod::None && base == 'a' {
                    let mut new_chars = chars.clone();
                    let new_vowel = get_vietnamese_vowel(base, VowelMod::Breve, tone).unwrap_or(chars[idx]);
                    new_chars[idx] = if is_upper { new_vowel.to_uppercase().next().unwrap_or(new_vowel) } else { new_vowel };
                    return Some(new_chars.into_iter().collect());
                }
            }
        }
    }

    None
}

/// VNI 1-5, 0: Dấu thanh
fn handle_vni_tone(word: &str, key: char, modern_tone: bool) -> Option<String> {
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

    // Nếu gõ '0' (bỏ dấu) nhưng từ hiện tại không có dấu thanh nào để bỏ -> không biến đổi
    if target_tone == Tone::None && current_tone == Tone::None {
        return None;
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

    let target_idx = super::telex::select_tone_vowel_index(&chars, &vowel_indices, modern_tone);

    let mut new_chars = chars.clone();
    for &idx in &vowel_indices {
        let (base, vmod, _, is_upper) = decompose_vietnamese_char(new_chars[idx])?;
        let applied_tone = if idx == target_idx { target_tone } else { Tone::None };
        let new_vowel = get_vietnamese_vowel(base, vmod, applied_tone)?;
        new_chars[idx] = if is_upper { new_vowel.to_uppercase().next().unwrap_or(new_vowel) } else { new_vowel };
    }

    let res: String = new_chars.into_iter().collect();
    if res == word {
        return None;
    }

    Some(res)
}
