use super::vietnamese::{decompose_vietnamese_char, get_vietnamese_vowel, Tone, VowelMod};


pub fn transform_telex(word: &str, key: char, modern_tone: bool) -> Option<String> {
    let key_lower = key.to_ascii_lowercase();
    let is_key_upper = key.is_ascii_uppercase();

    // 1. Xử lý 'dd' -> 'đ', 'đd' -> 'dd'
    if key_lower == 'd' {
        if let Some(res) = handle_d_modifier(word, is_key_upper) {
            return Some(res);
        }
    }

    // 2. Xử lý dấu mũ/móc/trăng: 'a', 'e', 'o', 'w'
    if matches!(key_lower, 'a' | 'e' | 'o' | 'w') {
        if let Some(res) = handle_vowel_modifier(word, key_lower, is_key_upper) {
            return Some(res);
        }
    }

    // 3. Xử lý dấu thanh: 's', 'f', 'r', 'x', 'j', 'z'
    if matches!(key_lower, 's' | 'f' | 'r' | 'x' | 'j' | 'z') {
        if let Some(res) = handle_tone_modifier(word, key_lower, modern_tone) {
            return Some(res);
        }
    }

    None
}

/// Xử lý biến đổi 'd' <-> 'đ'
fn handle_d_modifier(word: &str, is_upper: bool) -> Option<String> {
    let mut chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return None;
    }

    // Kiểm tra đảo ngược 'đ' + 'd' -> 'dd'
    for (i, &c) in chars.iter().enumerate() {
        if c == 'đ' || c == 'Đ' {
            let base = if c == 'Đ' { 'D' } else { 'd' };
            let rep = if is_upper { 'D' } else { 'd' };
            chars[i] = base;
            chars.insert(i + 1, rep);
            return Some(chars.into_iter().collect());
        }
    }

    // Kiểm tra 'd' + 'd' -> 'đ' (thường ở đầu từ hoặc phụ âm)
    for (i, &c) in chars.iter().enumerate() {
        if c == 'd' || c == 'D' {
            chars[i] = if c == 'D' || is_upper { 'Đ' } else { 'đ' };
            return Some(chars.into_iter().collect());
        }
    }

    None
}

/// Xử lý dấu mũ, trăng, móc: 'aa' -> 'â', 'aw' -> 'ă', 'ee' -> 'ê', 'oo' -> 'ô', 'ow' -> 'ơ', 'uw' -> 'ư', 'w' -> 'ư'
fn handle_vowel_modifier(word: &str, key: char, is_upper: bool) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        // Nếu bắt đầu từ bằng 'w' -> 'ư'
        if key == 'w' {
            return Some(if is_upper { "Ư".to_string() } else { "ư".to_string() });
        }
        return None;
    }

    // 1. Kiểm tra lặp lại cùng modifier để khôi phục (Undo)
    // Ví dụ: 'â' + 'a' -> 'aa', 'ê' + 'e' -> 'ee', 'ô' + 'o' -> 'oo', 'ă' + 'w' -> 'aw', 'ơ' + 'w' -> 'ow', 'ư' + 'w' -> 'uw'
    for (i, &c) in chars.iter().enumerate() {
        if let Some((base, vmod, tone, char_upper)) = decompose_vietnamese_char(c) {
            let undo_match = match (base, vmod, key) {
                ('a', VowelMod::Hat, 'a') => true,
                ('e', VowelMod::Hat, 'e') => true,
                ('o', VowelMod::Hat, 'o') => true,
                ('a', VowelMod::Breve, 'w') => true,
                ('o', VowelMod::Horn, 'w') => true,
                ('u', VowelMod::Horn, 'w') => true,
                _ => false,
            };

            if undo_match {
                let mut new_chars = chars.clone();
                // Bỏ mod về none, giữ lại tone
                let restored = get_vietnamese_vowel(base, VowelMod::None, tone).unwrap_or(base);
                let restored = if char_upper { restored.to_uppercase().next().unwrap_or(restored) } else { restored };
                new_chars[i] = restored;
                new_chars.push(if is_upper { key.to_ascii_uppercase() } else { key });
                return Some(new_chars.into_iter().collect());
            }
        }
    }

    // 2. Xử lý 'uow' / 'ươ' khi gặp 'w'
    // Ví dụ 'duong' + 'w' -> 'dương', 'thuong' + 'w' -> 'thương', 'uong' + 'w' -> 'ương'
    if key == 'w' {
        let mut found_uo = None;
        for i in 0..chars.len().saturating_sub(1) {
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
    }

    // 3. Áp dụng modifier mũ/móc/trăng:
    // Tìm nguyên âm phù hợp nhất trong từ (ưu tiên từ phải sang trái hoặc nguyên âm chưa có mod)
    for i in (0..chars.len()).rev() {
        let c = chars[i];
        if let Some((base, vmod, tone, char_upper)) = decompose_vietnamese_char(c) {
            let new_vmod = match (base, vmod, key) {
                ('a', VowelMod::None, 'a') => Some(VowelMod::Hat),
                ('a', VowelMod::None, 'w') => Some(VowelMod::Breve),
                ('e', VowelMod::None, 'e') => Some(VowelMod::Hat),
                ('o', VowelMod::None, 'o') => Some(VowelMod::Hat),
                ('o', VowelMod::None, 'w') => Some(VowelMod::Horn),
                ('u', VowelMod::None, 'w') => Some(VowelMod::Horn),
                _ => None,
            };

            if let Some(mod_applied) = new_vmod {
                let mut new_chars = chars.clone();
                let new_vowel = get_vietnamese_vowel(base, mod_applied, tone).unwrap_or(c);
                let new_vowel = if char_upper { new_vowel.to_uppercase().next().unwrap_or(new_vowel) } else { new_vowel };
                new_chars[i] = new_vowel;
                return Some(new_chars.into_iter().collect());
            }
        }
    }

    // 4. Nếu gõ 'w' độc lập sau phụ âm hoặc ở cuối (ví dụ 't' + 'w' -> 'tư')
    if key == 'w' {
        let mut new_chars = chars.clone();
        new_chars.push(if is_upper { 'Ư' } else { 'ư' });
        return Some(new_chars.into_iter().collect());
    }

    None
}

/// Xử lý dấu thanh: 's', 'f', 'r', 'x', 'j', 'z'
fn handle_tone_modifier(word: &str, key: char, modern_tone: bool) -> Option<String> {
    let target_tone = match key {
        's' => Tone::Acute,
        'f' => Tone::Grave,
        'r' => Tone::Hook,
        'x' => Tone::Tilde,
        'j' => Tone::Dot,
        'z' => Tone::None,
        _ => return None,
    };

    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return None;
    }

    // Tìm tất cả vị trí nguyên âm trong từ
    let mut vowel_indices: Vec<usize> = Vec::new();
    let mut current_tone = Tone::None;

    for (i, &c) in chars.iter().enumerate() {
        if let Some((_, _, tone, _)) = decompose_vietnamese_char(c) {
            vowel_indices.push(i);
            if tone != Tone::None {
                current_tone = tone;
            }
        }
    }

    if vowel_indices.is_empty() {
        return None;
    }

    // 1. Nếu gõ lại cùng phím dấu thanh -> khôi phục từ không dấu + phím đó (Undo)
    // Ví dụ: 'toán' + 's' -> 'toans'
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

    // 2. Xác định vị trí nguyên âm nhận dấu thanh
    let tone_index = select_tone_vowel_index(&chars, &vowel_indices, modern_tone);

    // Xóa dấu cũ trên tất cả nguyên âm và đặt dấu mới vào tone_index
    let mut new_chars = chars.clone();
    for &idx in &vowel_indices {
        let (base, vmod, _, is_upper) = decompose_vietnamese_char(new_chars[idx])?;
        let applied_tone = if idx == tone_index { target_tone } else { Tone::None };
        let new_vowel = get_vietnamese_vowel(base, vmod, applied_tone)?;
        new_chars[idx] = if is_upper { new_vowel.to_uppercase().next().unwrap_or(new_vowel) } else { new_vowel };
    }

    Some(new_chars.into_iter().collect())
}

/// Thuật toán tìm nguyên âm đặt dấu thanh tiếng Việt (chuẩn mới / cũ)
fn select_tone_vowel_index(chars: &[char], vowel_indices: &[usize], modern_tone: bool) -> usize {
    if vowel_indices.len() == 1 {
        return vowel_indices[0];
    }

    // 1. Nếu có nguyên âm mang dấu mũ hoặc dấu móc (ê, ô, ơ, â, ă, ư) -> ưu tiên nhận dấu
    let mut modified_vowels: Vec<usize> = Vec::new();
    for &idx in vowel_indices {
        if let Some((_, vmod, _, _)) = decompose_vietnamese_char(chars[idx]) {
            if vmod != VowelMod::None {
                modified_vowels.push(idx);
            }
        }
    }

    if modified_vowels.len() == 1 {
        return modified_vowels[0];
    } else if modified_vowels.len() > 1 {
        // Cặp 'ươ' (ví dụ: 'đường', 'người', 'thương') -> dấu thanh luôn đặt trên 'ơ'
        for &idx in &modified_vowels {
            let c = chars[idx].to_ascii_lowercase();
            if c == 'ơ' || c == 'o' {
                return idx;
            }
        }
        return *modified_vowels.last().unwrap();
    }

    // 2. Kiểm tra có phụ âm cuối không (các ký tự đứng sau nguyên âm cuối cùng)
    let last_vowel_idx = *vowel_indices.last().unwrap();
    let has_ending_consonant = last_vowel_idx < chars.len() - 1;

    let vowels_str: String = vowel_indices.iter().map(|&i| chars[i].to_ascii_lowercase()).collect();

    if has_ending_consonant {
        // Có phụ âm cuối (ví dụ: 'toán', 'hoàn', 'kiếm', 'thuyền'):
        // Đặt dấu vào nguyên âm chính (nguyên âm đứng gần phụ âm cuối hơn)
        if vowel_indices.len() >= 2 {
            return vowel_indices[vowel_indices.len() - 1];
        }
        return vowel_indices[0];
    }

    // Không có phụ âm cuối (ví dụ: 'hóa', 'hòa', 'thủy', 'túi', 'mái'):
    if modern_tone {
        // Chuẩn mới: 'oa', 'oe', 'uy' đặt ở nguyên âm thứ hai ('hóa', 'hòe', 'thủy')
        if matches!(vowels_str.as_str(), "oa" | "oe" | "uy") && vowel_indices.len() >= 2 {
            return vowel_indices[1];
        }
    }

    // Trường hợp thông thường: 'ai', 'ao', 'au', 'ay', 'eo', 'iu', 'oi', 'ui' hoặc chuẩn cũ 'hoá'
    // Đặt vào nguyên âm đầu tiên
    vowel_indices[0]
}
