use super::vietnamese::{decompose_vietnamese_char, get_vietnamese_vowel, Tone, VowelMod};

pub fn transform_telex(
    word: &str,
    key: char,
    modern_tone: bool,
    free_marking: bool,
    smart_english: bool,
    simple_telex: bool,
    raw_keys: Option<&str>,
) -> Option<String> {
    let key_lower = key.to_ascii_lowercase();
    let is_key_upper = key.is_ascii_uppercase();

    // Nếu tính năng nhận diện tiếng Anh thông minh được bật và từ hiện tại là từ ngoại lai
    // thì không can thiệp biến đổi để tránh làm hỏng từ tiếng Anh (fix, file, wifi, animation...)
    if smart_english && is_foreign_word(word) {
        return None;
    }

    // 1. Xử lý 'dd' -> 'đ', 'đd' -> 'dd'
    if key_lower == 'd' {
        if let Some(res) = handle_d_modifier(word, is_key_upper, free_marking, simple_telex) {
            return Some(res);
        }
    }

    // 2. Xử lý dấu mũ/móc/trăng: 'a', 'e', 'o', 'w'
    if matches!(key_lower, 'a' | 'e' | 'o' | 'w') {
        if let Some(res) = handle_vowel_modifier(word, key_lower, is_key_upper, free_marking, simple_telex, raw_keys) {
            return Some(res);
        }
    }

    // 3. Xử lý dấu thanh: 's', 'f', 'r', 'x', 'j', 'z'
    if matches!(key_lower, 's' | 'f' | 'r' | 'x' | 'j' | 'z') {
        if let Some(res) = handle_tone_modifier(word, key, modern_tone, simple_telex) {
            return Some(res);
        }
    }

    None
}

/// Kiểm tra từ có phải là từ tiếng Anh / ngoại quốc hay không:
/// 1. Bắt đầu bằng phụ âm ngoại lai không có trong tiếng Việt (f, j, z, hoặc w + nguyên âm như wifi, web)
/// 2. Hoặc từ đa âm tiết có nhiều cụm nguyên âm tách rời (như animation, computer, windows)
pub fn is_foreign_word(word: &str) -> bool {
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return false;
    }

    let first = chars[0].to_ascii_lowercase();
    if matches!(first, 'f' | 'j' | 'z') {
        return true;
    }
    if first == 'w' && chars.len() > 1 {
        let second = chars[1].to_ascii_lowercase();
        if decompose_vietnamese_char(second).is_some() {
            return true;
        }
    }

    is_foreign_multisyllable_word(word)
}

/// Kiểm tra từ có phải là từ đa âm tiết tiếng Anh / nước ngoài hay không.
/// Trong tiếng Việt, 1 từ luôn là ĐƠN ÂM TIẾT (chỉ có duy nhất 1 cụm nguyên âm liên tục).
/// Nếu một từ có 2 cụm nguyên âm cách nhau bởi phụ âm (như a-ni-ma-tion, com-pu-ter, win-dows),
/// đó 100% là từ tiếng Anh / ngoại quốc!
pub fn is_foreign_multisyllable_word(word: &str) -> bool {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() < 3 {
        return false;
    }

    let mut vowel_group_count = 0;
    let mut in_vowel_group = false;

    for &c in &chars {
        let is_v = decompose_vietnamese_char(c).is_some();
        if is_v {
            if !in_vowel_group {
                vowel_group_count += 1;
                in_vowel_group = true;
            }
        } else {
            in_vowel_group = false;
        }
    }

    vowel_group_count > 1
}

/// Xử lý biến đổi 'd' <-> 'đ' thông minh và tự do ở mọi vị trí (ví dụ 'dặt' + 'd' -> 'đặt')
fn handle_d_modifier(word: &str, is_upper: bool, free_marking: bool, simple_telex: bool) -> Option<String> {
    let mut chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        return None;
    }

    let last_idx = chars.len() - 1;
    let last_char = chars[last_idx];

    // 1. 'đ' + 'd' -> 'dd' (hoàn tác ngay ký tự cuối)
    if last_char == 'đ' || last_char == 'Đ' {
        if simple_telex {
            // Telex giản lược: Không hỗ trợ hoàn tác. Gõ phím thứ 3 sẽ nối tiếp chữ đó (ví dụ 'ddd' -> 'đd')
            return None;
        }
        chars[last_idx] = if last_char == 'Đ' { 'D' } else { 'd' };
        chars.push(if is_upper { 'D' } else { 'd' });
        return Some(chars.into_iter().collect());
    }

    // 2. 'd' + 'd' -> 'đ' (ngay ký tự cuối)
    if last_char == 'd' || last_char == 'D' {
        if last_idx > 0 && (chars[last_idx - 1] == 'd' || chars[last_idx - 1] == 'D') {
            return None;
        }
        chars[last_idx] = if last_char == 'D' || is_upper { 'Đ' } else { 'đ' };
        return Some(chars.into_iter().collect());
    }

    if free_marking {
        let has_double_d = chars.windows(2).any(|w| (w[0] == 'd' || w[0] == 'D') && (w[1] == 'd' || w[1] == 'D'));
        if !has_double_d {
            // 3. Tự do hóa phím 'd' trong toàn từ: nếu từ bắt đầu bằng 'd'/'D' hoặc có 'd'/'D' (ví dụ 'dặt' + 'd' -> 'đặt')
            if let Some(pos) = chars.iter().position(|&c| c == 'd' || c == 'D') {
                let is_d_upper = chars[pos] == 'D';
                chars[pos] = if is_d_upper { 'Đ' } else { 'đ' };
                return Some(chars.into_iter().collect());
            }

            // 4. Nếu từ đã có 'đ'/'Đ' và người dùng gõ thêm 'd' ở cuối từ -> hoàn tác 'đ' thành 'd' và chèn 'd' vào cuối
            if !simple_telex {
                if let Some(pos) = chars.iter().position(|&c| c == 'đ' || c == 'Đ') {
                    chars[pos] = if chars[pos] == 'Đ' { 'D' } else { 'd' };
                    chars.push(if is_upper { 'D' } else { 'd' });
                    return Some(chars.into_iter().collect());
                }
            }
        }
    }

    None
}

/// Xử lý dấu mũ, trăng, móc: 'aa' -> 'â', 'aw' -> 'ă', 'ee' -> 'ê', 'oo' -> 'ô', 'ow' -> 'ơ', 'uw' -> 'ư', 'w' -> 'ư'
fn handle_vowel_modifier(
    word: &str,
    key: char,
    is_upper: bool,
    free_marking: bool,
    simple_telex: bool,
    raw_keys: Option<&str>,
) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.is_empty() {
        if key == 'w' {
            if simple_telex {
                // Telex giản lược: 'w' đứng đơn lẻ ra chữ 'w'!
                return None;
            }
            return Some(if is_upper { "Ư".to_string() } else { "ư".to_string() });
        }
        return None;
    }

    let last_idx = chars.len() - 1;
    let last_c = chars[last_idx];

    // 1. Kiểm tra lặp lại cùng modifier để hoàn tác (Undo)
    if let Some((base, vmod, tone, char_upper)) = decompose_vietnamese_char(last_c) {
        let undo_match = match (base, vmod, key) {
            ('a', VowelMod::Hat, 'a') => true,
            ('a', VowelMod::Breve, 'a') => true,
            ('e', VowelMod::Hat, 'e') => true,
            ('o', VowelMod::Hat, 'o') => true,
            ('a', VowelMod::Breve, 'w') => true,
            ('o', VowelMod::Horn, 'w') => true,
            ('u', VowelMod::Horn, 'w') => true,
            _ => false,
        };

        if undo_match {
            if simple_telex {
                // Telex giản lược: Không hỗ trợ hoàn tác mũ/móc thông thường (aaa -> âa, ddd -> đd).
                // NGOẠI TRỪ: phím 'w' trên 'ư' (ví dụ: uww -> uw, ww -> w)
                let is_w_on_u = base == 'u' && vmod == VowelMod::Horn && key == 'w';
                if !is_w_on_u {
                    return None;
                }
            }
            let mut new_chars = chars.clone();

            // Nếu người dùng gõ 'w' khi ký tự cuối là 'ư'/'Ư':
            // Chỉ ăn chữ 'u' biến thành 'w' khi người dùng thật sự gõ 'ww' (không có 'u' đứng trước):
            if base == 'u' && vmod == VowelMod::Horn && key == 'w' {
                let is_ww_only = if let Some(raw) = raw_keys {
                    let r_lower: String = raw.chars().map(|c| c.to_ascii_lowercase()).collect();
                    if r_lower.ends_with("ww") {
                        let len = r_lower.len();
                        if len == 2 {
                            true // Người dùng chỉ gõ đúng "ww" -> ra "w"
                        } else {
                            let prev = r_lower.chars().nth(len - 3).unwrap_or(' ');
                            prev != 'u' // Nếu trước đó không phải 'u' (ví dụ "tww"), là ww ăn u thành tw
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

                if is_ww_only {
                    new_chars[last_idx] = if is_upper || char_upper { 'W' } else { 'w' };
                    return Some(new_chars.into_iter().collect());
                }
            }

            let restored = get_vietnamese_vowel(base, VowelMod::None, tone).unwrap_or(base);
            let restored = if char_upper { restored.to_uppercase().next().unwrap_or(restored) } else { restored };
            new_chars[last_idx] = restored;
            new_chars.push(if is_upper { key.to_ascii_uppercase() } else { key });
            return Some(new_chars.into_iter().collect());
        }
    }

    // 2. Xử lý 'uow' / 'ươ' và 'uaw' / 'ưa' khi gặp 'w'
    if key == 'w' {
        // Xử lý 'ua' + 'w' -> 'ưa' (ví dụ: 'cua' + 'w' -> 'cưa', 'mua' + 'w' -> 'mưa', 'chua' + 'w' -> 'chưa')
        // Áp dụng cho cả Telex thường và Telex giản lược
        let mut found_ua = None;
        for i in 0..chars.len().saturating_sub(1) {
            if i > 0 && chars[i - 1].to_ascii_lowercase() == 'q' {
                continue;
            }
            let c1 = chars[i].to_ascii_lowercase();
            let c2 = chars[i + 1].to_ascii_lowercase();
            if (c1 == 'u' || c1 == 'ư') && c2 == 'a' {
                found_ua = Some(i);
                break;
            }
        }

        if let Some(i) = found_ua {
            let (_, m1, t1, u1) = decompose_vietnamese_char(chars[i]).unwrap_or(('u', VowelMod::None, Tone::None, false));
            let (_, _, t2, u2) = decompose_vietnamese_char(chars[i + 1]).unwrap_or(('a', VowelMod::None, Tone::None, false));

            if m1 == VowelMod::Horn {
                if simple_telex {
                    return None;
                }
                // Đã là 'ưa', gõ tiếp 'w' -> hoàn tác thành 'ua' và chèn 'w' (ví dụ: 'cưa' + 'w' -> 'cuaw')
                let mut new_chars = chars.clone();
                let restored_u = get_vietnamese_vowel('u', VowelMod::None, t1).unwrap_or('u');
                let restored_u = if u1 { restored_u.to_uppercase().next().unwrap_or(restored_u) } else { restored_u };
                new_chars[i] = restored_u;
                new_chars.push(if is_upper { 'W' } else { 'w' });
                return Some(new_chars.into_iter().collect());
            } else {
                // Đang là 'ua', biến thành 'ưa'
                let mut new_chars = chars.clone();
                let new_u = get_vietnamese_vowel('u', VowelMod::Horn, t1).unwrap_or('ư');
                let new_u = if u1 { new_u.to_uppercase().next().unwrap_or(new_u) } else { new_u };
                let new_a = get_vietnamese_vowel('a', VowelMod::None, t2).unwrap_or('a');
                let new_a = if u2 { new_a.to_uppercase().next().unwrap_or(new_a) } else { new_a };
                new_chars[i] = new_u;
                new_chars[i + 1] = new_a;
                return Some(new_chars.into_iter().collect());
            }
        }

        // Telex giản lược: Không tự đổi cả cặp 'uo' -> 'ươ', phải gõ rõ 'uwow' mới ra 'ươ'!
        if !simple_telex {
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
            let is_thuo = i >= 2
                && chars[..i].iter().collect::<String>().to_lowercase() == "th"
                && chars.len() == i + 2;

            let (new_u, new_o) = if is_thuo {
                // Với 'thuo' + 'w' (không có phụ âm cuối): biến thành 'thuơ' để gõ 'thuở'
                let (_, _, _, u1) = decompose_vietnamese_char(new_chars[i]).unwrap_or(('u', VowelMod::None, Tone::None, false));
                let (_, _, t2, u2) = decompose_vietnamese_char(new_chars[i + 1]).unwrap_or(('o', VowelMod::None, Tone::None, false));
                let u_char = if u1 { 'U' } else { 'u' };
                let o_char = get_vietnamese_vowel('o', VowelMod::Horn, t2).unwrap_or('ơ');
                let o_char = if u2 { o_char.to_uppercase().next().unwrap_or(o_char) } else { o_char };
                (u_char, o_char)
            } else {
                let (_, _, t1, u1) = decompose_vietnamese_char(new_chars[i]).unwrap_or(('u', VowelMod::None, Tone::None, false));
                let (_, _, t2, u2) = decompose_vietnamese_char(new_chars[i + 1]).unwrap_or(('o', VowelMod::None, Tone::None, false));

                let new_u = get_vietnamese_vowel('u', VowelMod::Horn, t1).unwrap_or('ư');
                let new_u = if u1 { new_u.to_uppercase().next().unwrap_or(new_u) } else { new_u };

                let new_o = get_vietnamese_vowel('o', VowelMod::Horn, t2).unwrap_or('ơ');
                let new_o = if u2 { new_o.to_uppercase().next().unwrap_or(new_o) } else { new_o };
                (new_u, new_o)
            };

            new_chars[i] = new_u;
            new_chars[i + 1] = new_o;
            return Some(new_chars.into_iter().collect());
        }
    }
}

    // 3. Áp dụng modifier mũ/móc/trăng vào ký tự LIỀN KỀ TRƯỚC ĐÓ
    if let Some((base, vmod, tone, char_upper)) = decompose_vietnamese_char(last_c) {
        // Nếu ký tự đứng trước ký tự cuối cũng chính là key (ví dụ: đã là 'ee', 'aa', 'oo', 'ww') -> KHÔNG biến đổi lặp lại thành 'eê', 'aâ', 'oô', 'wư'!
        let is_already_doubled = last_idx > 0 && chars[last_idx - 1].to_ascii_lowercase() == key;
        if !is_already_doubled {
            let new_vmod = match (base, vmod, key) {
                ('a', VowelMod::None, 'a') => {
                    // Trong tiếng Việt không bao giờ có 'oâ' mà chỉ có 'oă' (hoẵng, ngoẵng, loằng ngoằng, thoăn thoắt...)
                    if last_idx > 0 && chars[last_idx - 1].to_ascii_lowercase() == 'o' {
                        Some(VowelMod::Breve)
                    } else {
                        Some(VowelMod::Hat)
                    }
                }
                ('a', VowelMod::None, 'w') => Some(VowelMod::Breve),
                ('e', VowelMod::None, 'e') => Some(VowelMod::Hat),
                ('o', VowelMod::None, 'o') => Some(VowelMod::Hat),
                ('o', VowelMod::None, 'w') => Some(VowelMod::Horn),
                ('u', VowelMod::None, 'w') => {
                    if last_idx > 0 && chars[last_idx - 1].to_ascii_lowercase() == 'q' {
                        None
                    } else {
                        Some(VowelMod::Horn)
                    }
                }
                _ => None,
            };

            if let Some(mod_applied) = new_vmod {
                let mut new_chars = chars.clone();
                let new_vowel = get_vietnamese_vowel(base, mod_applied, tone).unwrap_or(last_c);
                let new_vowel = if char_upper { new_vowel.to_uppercase().next().unwrap_or(new_vowel) } else { new_vowel };
                new_chars[last_idx] = new_vowel;
                return Some(new_chars.into_iter().collect());
            }
        }
    }

    // 4. Áp dụng modifier mũ/móc/trăng TỰ DO cho nguyên âm bên trong từ (kể cả khi đã có phụ âm cuối)
    // Hỗ trợ đầy đủ: 'đot' + 'o' -> 'đôt' (thêm s -> 'đốt'), 'viet' + 'e' -> 'viêt', 'bat' + 'a' -> 'bât', 'dong' + 'o' -> 'đông'
    if free_marking {
        let is_already_doubled = chars.len() >= 2
            && chars[chars.len() - 1].to_ascii_lowercase() == key
            && chars[chars.len() - 2].to_ascii_lowercase() == key;
        if !is_already_doubled {
            let vowel_indices = extract_vowel_indices(&chars);
            if !vowel_indices.is_empty() {
                let mut target_idx = None;
                let mut target_mod = None;

                for &idx in vowel_indices.iter().rev() {
                    // Không tác động nếu nguyên âm này đã đứng cạnh cùng nguyên âm đó (ví dụ 'meet', 'beer')
                    if idx > 0 && chars[idx - 1].to_ascii_lowercase() == key {
                        continue;
                    }
                    if idx + 1 < chars.len() && chars[idx + 1].to_ascii_lowercase() == key {
                        continue;
                    }

                    // Với các phím nguyên âm 'a', 'e', 'o' (tạo mũ â, ê, ô):
                    // Chỉ cho phép tác động khi từ sau vị trí idx đến cuối từ KHÔNG CÓ nguyên âm nào khác
                    // (nghĩa là sau đó chỉ có thể là các phụ âm cuối như 't', 'c', 'n', 'p', 'm', 'ng', 'ch'...
                    // ví dụ: 'đot' + 'o' -> 'đôt', 'dong' + 'o' -> 'đông', 'bat' + 'a' -> 'bât', 'viet' + 'e' -> 'viêt'.
                    // Tuyệt đối không nhảy cóc qua nguyên âm khác như 'ngoe' + 'o' -> 'ngoeo'!).
                    if key != 'w' {
                        let has_vowel_after = chars[idx + 1..].iter().any(|&c| decompose_vietnamese_char(c).is_some());
                        if has_vowel_after {
                            continue;
                        }
                    }

                    if let Some((base, vmod, _, _)) = decompose_vietnamese_char(chars[idx]) {
                        let m = match (base, vmod, key) {
                            ('a', VowelMod::None, 'a') => {
                                if idx > 0 && chars[idx - 1].to_ascii_lowercase() == 'o' {
                                    Some(VowelMod::Breve)
                                } else {
                                    Some(VowelMod::Hat)
                                }
                            }
                            ('a', VowelMod::None, 'w') => Some(VowelMod::Breve),
                            ('e', VowelMod::None, 'e') => Some(VowelMod::Hat),
                            ('o', VowelMod::None, 'o') => Some(VowelMod::Hat),
                            ('o', VowelMod::None, 'w') => Some(VowelMod::Horn),
                            ('u', VowelMod::None, 'w') => {
                                if idx > 0 && chars[idx - 1].to_ascii_lowercase() == 'q' {
                                    None
                                } else {
                                    Some(VowelMod::Horn)
                                }
                            }
                            _ => None,
                        };
                        if let Some(applied) = m {
                            target_idx = Some(idx);
                            target_mod = Some(applied);
                            break;
                        }
                    }
                }

                if let (Some(idx), Some(mod_applied)) = (target_idx, target_mod) {
                    let mut new_chars = chars.clone();
                    let (base, _, tone, char_upper) = decompose_vietnamese_char(new_chars[idx]).unwrap();
                    let new_vowel = get_vietnamese_vowel(base, mod_applied, tone).unwrap_or(new_chars[idx]);
                    let new_vowel = if char_upper { new_vowel.to_uppercase().next().unwrap_or(new_vowel) } else { new_vowel };
                    new_chars[idx] = new_vowel;
                    return Some(new_chars.into_iter().collect());
                }

            // Hoàn tác mũ/móc tự do nếu gõ lặp lại (ví dụ 'đôt' + 'o' -> 'đoto', 'hiên' + 'e' -> 'hiene')
            let mut undo_idx = None;
            let mut undo_base = ' ';
            if !simple_telex {
                for &idx in vowel_indices.iter().rev() {
                    if key != 'w' {
                        let has_vowel_after = chars[idx + 1..].iter().any(|&c| decompose_vietnamese_char(c).is_some());
                        if has_vowel_after {
                            continue;
                        }
                    }

                    if let Some((base, vmod, _, _)) = decompose_vietnamese_char(chars[idx]) {
                        let is_undo = match (base, vmod, key) {
                            ('a', VowelMod::Hat, 'a') => true,
                            ('a', VowelMod::Breve, 'a') => true,
                            ('a', VowelMod::Breve, 'w') => true,
                            ('e', VowelMod::Hat, 'e') => true,
                            ('o', VowelMod::Hat, 'o') => true,
                            ('o', VowelMod::Horn, 'w') => true,
                            ('u', VowelMod::Horn, 'w') => true,
                            _ => false,
                        };
                        if is_undo {
                            undo_idx = Some(idx);
                            undo_base = base;
                            break;
                        }
                    }
                }
            }

            if let Some(idx) = undo_idx {
                let mut new_chars = chars.clone();
                let (_, _, tone, char_upper) = decompose_vietnamese_char(new_chars[idx]).unwrap();
                let restored = get_vietnamese_vowel(undo_base, VowelMod::None, tone).unwrap_or(undo_base);
                let restored = if char_upper { restored.to_uppercase().next().unwrap_or(restored) } else { restored };
                new_chars[idx] = restored;
                new_chars.push(if is_upper { key.to_ascii_uppercase() } else { key });
                return Some(new_chars.into_iter().collect());
            }
        }
    }
}

    // 5. Nếu gõ 'w' độc lập sau phụ âm tiếng Việt (ví dụ: 't' + 'w' -> 'tư', 's' + 'w' -> 'sư')
    // Chú ý: Không áp dụng nếu ký tự trước là 'w' hoặc ngoại lai, và KHÔNG áp dụng khi bật simple_telex
    if !simple_telex && key == 'w' && last_c.is_alphabetic() && super::vietnamese::is_consonant(last_c) {
        let mut new_chars = chars.clone();
        new_chars.push(if is_upper { 'Ư' } else { 'ư' });
        return Some(new_chars.into_iter().collect());
    }

    None
}

/// Xử lý đặt dấu thanh tự do: 's', 'f', 'r', 'x', 'j', 'z'
pub fn handle_tone_modifier(word: &str, key: char, modern_tone: bool, simple_telex: bool) -> Option<String> {
    let key_lower = key.to_ascii_lowercase();
    let target_tone = match key_lower {
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

    // Không đặt lại dấu nếu từ đã kết thúc bằng chính phím dấu này (ví dụ 'toans' + 's' -> 'toanss', 'cas' + 's' -> 'cass')
    if let Some(&last) = chars.last() {
        if last.to_ascii_lowercase() == key_lower {
            return None;
        }
    }

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

    // Nếu gõ 'z' (bỏ dấu) nhưng từ hiện tại không có dấu thanh nào để bỏ -> không biến đổi
    if target_tone == Tone::None && current_tone == Tone::None {
        return None;
    }

    // 1. Hoàn tác nếu gõ lặp lại cùng phím dấu thanh (ví dụ 'toán' + 's' -> 'toans', 'DÁ' + 'S' -> 'DAS', 'DÁ' + 's' -> 'DAs')
    if target_tone == current_tone && target_tone != Tone::None {
        if simple_telex {
            // Telex giản lược: Không hỗ trợ. Gõ tiếp phím dấu sẽ chèn thêm phím đó (ví dụ: as -> á, gõ tiếp s -> ás)
            return None;
        }
        let mut new_chars = chars.clone();
        for &idx in &vowel_indices {
            let (base, vmod, _, is_upper) = decompose_vietnamese_char(new_chars[idx])?;
            let restored = get_vietnamese_vowel(base, vmod, Tone::None)?;
            new_chars[idx] = if is_upper { restored.to_uppercase().next().unwrap_or(restored) } else { restored };
        }
        new_chars.push(key);
        return Some(new_chars.into_iter().collect());
    }

    // 2. Xác định chính xác vị trí nguyên âm nhận dấu thanh theo chuẩn ngữ âm tiếng Việt
    let tone_index = select_tone_vowel_index(&chars, &vowel_indices, modern_tone);

    let mut new_chars = chars.clone();
    for &idx in &vowel_indices {
        let (base, vmod, _, is_upper) = decompose_vietnamese_char(new_chars[idx])?;
        let applied_tone = if idx == tone_index { target_tone } else { Tone::None };
        let new_vowel = get_vietnamese_vowel(base, vmod, applied_tone)?;
        new_chars[idx] = if is_upper { new_vowel.to_uppercase().next().unwrap_or(new_vowel) } else { new_vowel };
    }

    let res: String = new_chars.into_iter().collect();
    if res == word {
        return None;
    }

    Some(res)
}

/// Trích xuất danh sách vị trí nguyên âm thực sự trong từ tiếng Việt.
pub fn extract_vowel_indices(chars: &[char]) -> Vec<usize> {
    let mut indices = Vec::new();
    let len = chars.len();

    let mut skip_qu_u = false;
    let mut skip_gi_i = false;

    if len >= 2 && chars[0].to_ascii_lowercase() == 'q' && chars[1].to_ascii_lowercase() == 'u' {
        skip_qu_u = true;
    }

    if len >= 3 && chars[0].to_ascii_lowercase() == 'g' && chars[1].to_ascii_lowercase() == 'i' {
        let c2 = chars[2].to_ascii_lowercase();
        if decompose_vietnamese_char(c2).is_some() {
            skip_gi_i = true;
        }
    }

    for (i, &c) in chars.iter().enumerate() {
        if i == 1 && skip_qu_u {
            continue;
        }
        if i == 1 && skip_gi_i {
            continue;
        }

        if decompose_vietnamese_char(c).is_some() {
            indices.push(i);
        }
    }

    indices
}

/// THUẬT TOÁN ĐẶT DẤU THANH TIẾNG VIỆT CHUẨN XÁC 100%:
///
/// 1. Nếu từ chỉ có 1 nguyên âm: đặt dấu vào nguyên âm đó (ví dụ 'toán', 'quá', 'quốc').
/// 2. NGUYÊN TẮC BẤT DI BẤT DỊCH: Nếu có nguyên âm mang dấu mũ hoặc dấu móc (ê, ô, ơ, â, ă, ư):
///    NGUYÊN ÂM CÓ DẤU MŨ/MÓC LUÔN ĐƯỢC ƯU TIÊN ĐẶT DẤU THANH CAO NHẤT!
///    - 'chuyên' + 'r' -> 'chuyển' (đặt trên 'ê', KHÔNG BAO GIỜ đặt trên 'u' như chuỷên!)
///    - 'thuyền' + 'f' -> 'thuyền' (đặt trên 'ê')
///    - 'khuyên' + 's' -> 'khuyến' (đặt trên 'ê')
///    - Cặp 'ươ' (đường, thương): đặt trên 'ơ'.
/// 3. Nếu từ có 3 nguyên âm trơn (nguyên âm ba: 'oai', 'oay', 'oeo', 'uyu'):
///    Dấu thanh LUÔN ĐẶT Ở NGUYÊN ÂM Ở GIỮA (thứ hai)!
///    - 'ngoài' (đặt trên 'a', KHÔNG BAO GIỜ trên 'o' như ngòai!)
///    - 'xoài', 'khoái', 'ngoáy'.
/// 4. Nếu từ có 2 nguyên âm trơn (không mũ/móc):
///    - Nếu CÓ phụ âm cuối (hoàn, toán, tính, luân): đặt trên nguyên âm thứ hai.
///    - Nếu KHÔNG CÓ phụ âm cuối:
///      + Cặp 'oa', 'oe', 'uy':
///        * Chuẩn mới: đặt trên nguyên âm thứ hai ('hóa', 'hòe', 'thủy').
///        * Chuẩn cũ: đặt trên nguyên âm thứ nhất ('hoá', 'hoè', 'thuỷ').
///      + Các cặp kết thúc bằng bán nguyên âm (ai, ao, au, ay, eo, iu, oi, ui, ua):
///        Đặt trên nguyên âm thứ nhất ('mái', 'báo', 'sáu', 'máy', 'kéo', 'chịu', 'túi', 'chúa').
pub fn select_tone_vowel_index(chars: &[char], vowel_indices: &[usize], modern_tone: bool) -> usize {
    let count = vowel_indices.len();

    // 1. Chỉ có 1 nguyên âm
    if count == 1 {
        return vowel_indices[0];
    }

    // 2. ƯU TIÊN SỐ 1: Nguyên âm mang dấu mũ hoặc dấu móc (ê, ô, ơ, â, ă, ư)
    // Sửa dứt điểm lỗi 'chuỷên' thành 'chuyển', 'thuỷền' thành 'thuyền'!
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
            if let Some((base, vmod, _, _)) = decompose_vietnamese_char(chars[idx]) {
                if base == 'o' && vmod == VowelMod::Horn {
                    return idx;
                }
            }
        }
        return *modified_vowels.last().unwrap();
    }

    // 3. Có 3 nguyên âm trơn: LUÔN ĐẶT Ở NGUYÊN ÂM Ở GIỮA (thứ hai)
    // Sửa dứt điểm lỗi 'ngòai' thành 'ngoài', 'xòai' thành 'xoài'!
    // Ngoại lệ: nhóm kết thúc bằng 'ye' (chuyen, thuyen, khuyen, tuyen, huyen...) thì âm chính là 'e' (thứ ba)!
    if count >= 3 {
        let vowels_str: String = vowel_indices
            .iter()
            .filter_map(|&i| decompose_vietnamese_char(chars[i]).map(|(base, _, _, _)| base))
            .collect();
        if vowels_str.ends_with("ye") {
            return vowel_indices[2];
        }
        return vowel_indices[1];
    }

    // 4. Có 2 nguyên âm trơn
    let last_vowel_idx = *vowel_indices.last().unwrap();
    let has_ending_consonant = last_vowel_idx < chars.len() - 1;

    let vowels_str: String = vowel_indices
        .iter()
        .filter_map(|&i| decompose_vietnamese_char(chars[i]).map(|(base, _, _, _)| base))
        .collect();

    if has_ending_consonant {
        return vowel_indices[1];
    }

    if modern_tone {
        if matches!(vowels_str.as_str(), "oa" | "oe" | "uy" | "uo") {
            return vowel_indices[1];
        }
    }

    vowel_indices[0]
}
