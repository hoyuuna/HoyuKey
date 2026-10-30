// Bảng mã và xử lý dấu tiếng Việt chuẩn xác cho HoyuKey

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    None,     // Thanh ngang (không dấu)
    Acute,    // Sắc (')
    Grave,    // Huyền (`)
    Hook,     // Hỏi (?)
    Tilde,    // Ngã (~)
    Dot,      // Nặng (.)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VowelMod {
    None,
    Hat,   // â, ê, ô
    Horn,  // ơ, ư
    Breve, // ă
}

/// Bảng ánh xạ các nguyên âm tiếng Việt sang Unicode
/// Cột: Vowel base ('a', 'e', 'i', 'o', 'u', 'y') + VowelMod + Tone
pub fn get_vietnamese_vowel(base: char, vmod: VowelMod, tone: Tone) -> Option<char> {
    let lower = base.to_ascii_lowercase();
    let is_upper = base.is_ascii_uppercase();

    let res = match (lower, vmod, tone) {
        // --- A ---
        ('a', VowelMod::None, Tone::None) => 'a',
        ('a', VowelMod::None, Tone::Acute) => 'á',
        ('a', VowelMod::None, Tone::Grave) => 'à',
        ('a', VowelMod::None, Tone::Hook) => 'ả',
        ('a', VowelMod::None, Tone::Tilde) => 'ã',
        ('a', VowelMod::None, Tone::Dot) => 'ạ',

        ('a', VowelMod::Hat, Tone::None) => 'â',
        ('a', VowelMod::Hat, Tone::Acute) => 'ấ',
        ('a', VowelMod::Hat, Tone::Grave) => 'ầ',
        ('a', VowelMod::Hat, Tone::Hook) => 'ẩ',
        ('a', VowelMod::Hat, Tone::Tilde) => 'ẫ',
        ('a', VowelMod::Hat, Tone::Dot) => 'ậ',

        ('a', VowelMod::Breve, Tone::None) => 'ă',
        ('a', VowelMod::Breve, Tone::Acute) => 'ắ',
        ('a', VowelMod::Breve, Tone::Grave) => 'ằ',
        ('a', VowelMod::Breve, Tone::Hook) => 'ẳ',
        ('a', VowelMod::Breve, Tone::Tilde) => 'ẵ',
        ('a', VowelMod::Breve, Tone::Dot) => 'ặ',

        // --- E ---
        ('e', VowelMod::None, Tone::None) => 'e',
        ('e', VowelMod::None, Tone::Acute) => 'é',
        ('e', VowelMod::None, Tone::Grave) => 'è',
        ('e', VowelMod::None, Tone::Hook) => 'ẻ',
        ('e', VowelMod::None, Tone::Tilde) => 'ẽ',
        ('e', VowelMod::None, Tone::Dot) => 'ẹ',

        ('e', VowelMod::Hat, Tone::None) => 'ê',
        ('e', VowelMod::Hat, Tone::Acute) => 'ế',
        ('e', VowelMod::Hat, Tone::Grave) => 'ề',
        ('e', VowelMod::Hat, Tone::Hook) => 'ể',
        ('e', VowelMod::Hat, Tone::Tilde) => 'ễ',
        ('e', VowelMod::Hat, Tone::Dot) => 'ệ',

        // --- I ---
        ('i', VowelMod::None, Tone::None) => 'i',
        ('i', VowelMod::None, Tone::Acute) => 'í',
        ('i', VowelMod::None, Tone::Grave) => 'ì',
        ('i', VowelMod::None, Tone::Hook) => 'ỉ',
        ('i', VowelMod::None, Tone::Tilde) => 'ĩ',
        ('i', VowelMod::None, Tone::Dot) => 'ị',

        // --- O ---
        ('o', VowelMod::None, Tone::None) => 'o',
        ('o', VowelMod::None, Tone::Acute) => 'ó',
        ('o', VowelMod::None, Tone::Grave) => 'ò',
        ('o', VowelMod::None, Tone::Hook) => 'ỏ',
        ('o', VowelMod::None, Tone::Tilde) => 'õ',
        ('o', VowelMod::None, Tone::Dot) => 'ọ',

        ('o', VowelMod::Hat, Tone::None) => 'ô',
        ('o', VowelMod::Hat, Tone::Acute) => 'ố',
        ('o', VowelMod::Hat, Tone::Grave) => 'ồ',
        ('o', VowelMod::Hat, Tone::Hook) => 'ổ',
        ('o', VowelMod::Hat, Tone::Tilde) => 'ỗ',
        ('o', VowelMod::Hat, Tone::Dot) => 'ộ',

        ('o', VowelMod::Horn, Tone::None) => 'ơ',
        ('o', VowelMod::Horn, Tone::Acute) => 'ớ',
        ('o', VowelMod::Horn, Tone::Grave) => 'ờ',
        ('o', VowelMod::Horn, Tone::Hook) => 'ở',
        ('o', VowelMod::Horn, Tone::Tilde) => 'ỡ',
        ('o', VowelMod::Horn, Tone::Dot) => 'ợ',

        // --- U ---
        ('u', VowelMod::None, Tone::None) => 'u',
        ('u', VowelMod::None, Tone::Acute) => 'ú',
        ('u', VowelMod::None, Tone::Grave) => 'ù',
        ('u', VowelMod::None, Tone::Hook) => 'ủ',
        ('u', VowelMod::None, Tone::Tilde) => 'ũ',
        ('u', VowelMod::None, Tone::Dot) => 'ụ',

        ('u', VowelMod::Horn, Tone::None) => 'ư',
        ('u', VowelMod::Horn, Tone::Acute) => 'ứ',
        ('u', VowelMod::Horn, Tone::Grave) => 'ừ',
        ('u', VowelMod::Horn, Tone::Hook) => 'ử',
        ('u', VowelMod::Horn, Tone::Tilde) => 'ữ',
        ('u', VowelMod::Horn, Tone::Dot) => 'ự',

        // --- Y ---
        ('y', VowelMod::None, Tone::None) => 'y',
        ('y', VowelMod::None, Tone::Acute) => 'ý',
        ('y', VowelMod::None, Tone::Grave) => 'ỳ',
        ('y', VowelMod::None, Tone::Hook) => 'ỷ',
        ('y', VowelMod::None, Tone::Tilde) => 'ỹ',
        ('y', VowelMod::None, Tone::Dot) => 'ỵ',

        _ => return None,
    };

    if is_upper {
        // Chuyển ký tự thành hoa
        Some(res.to_uppercase().next().unwrap_or(res))
    } else {
        Some(res)
    }
}

/// Phân tách 1 ký tự tiếng Việt ra thành (base_char, VowelMod, Tone)
pub fn decompose_vietnamese_char(c: char) -> Option<(char, VowelMod, Tone, bool)> {
    let lower = c.to_lowercase().next()?;
    let is_upper = c.is_uppercase();

    let (base, vmod, tone) = match lower {
        'a' => ('a', VowelMod::None, Tone::None),
        'á' => ('a', VowelMod::None, Tone::Acute),
        'à' => ('a', VowelMod::None, Tone::Grave),
        'ả' => ('a', VowelMod::None, Tone::Hook),
        'ã' => ('a', VowelMod::None, Tone::Tilde),
        'ạ' => ('a', VowelMod::None, Tone::Dot),

        'â' => ('a', VowelMod::Hat, Tone::None),
        'ấ' => ('a', VowelMod::Hat, Tone::Acute),
        'ầ' => ('a', VowelMod::Hat, Tone::Grave),
        'ẩ' => ('a', VowelMod::Hat, Tone::Hook),
        'ẫ' => ('a', VowelMod::Hat, Tone::Tilde),
        'ậ' => ('a', VowelMod::Hat, Tone::Dot),

        'ă' => ('a', VowelMod::Breve, Tone::None),
        'ắ' => ('a', VowelMod::Breve, Tone::Acute),
        'ằ' => ('a', VowelMod::Breve, Tone::Grave),
        'ẳ' => ('a', VowelMod::Breve, Tone::Hook),
        'ẵ' => ('a', VowelMod::Breve, Tone::Tilde),
        'ặ' => ('a', VowelMod::Breve, Tone::Dot),

        'e' => ('e', VowelMod::None, Tone::None),
        'é' => ('e', VowelMod::None, Tone::Acute),
        'è' => ('e', VowelMod::None, Tone::Grave),
        'ẻ' => ('e', VowelMod::None, Tone::Hook),
        'ẽ' => ('e', VowelMod::None, Tone::Tilde),
        'ẹ' => ('e', VowelMod::None, Tone::Dot),

        'ê' => ('e', VowelMod::Hat, Tone::None),
        'ế' => ('e', VowelMod::Hat, Tone::Acute),
        'ề' => ('e', VowelMod::Hat, Tone::Grave),
        'ể' => ('e', VowelMod::Hat, Tone::Hook),
        'ễ' => ('e', VowelMod::Hat, Tone::Tilde),
        'ệ' => ('e', VowelMod::Hat, Tone::Dot),

        'i' => ('i', VowelMod::None, Tone::None),
        'í' => ('i', VowelMod::None, Tone::Acute),
        'ì' => ('i', VowelMod::None, Tone::Grave),
        'ỉ' => ('i', VowelMod::None, Tone::Hook),
        'ĩ' => ('i', VowelMod::None, Tone::Tilde),
        'ị' => ('i', VowelMod::None, Tone::Dot),

        'o' => ('o', VowelMod::None, Tone::None),
        'ó' => ('o', VowelMod::None, Tone::Acute),
        'ò' => ('o', VowelMod::None, Tone::Grave),
        'ỏ' => ('o', VowelMod::None, Tone::Hook),
        'õ' => ('o', VowelMod::None, Tone::Tilde),
        'ọ' => ('o', VowelMod::None, Tone::Dot),

        'ô' => ('o', VowelMod::Hat, Tone::None),
        'ố' => ('o', VowelMod::Hat, Tone::Acute),
        'ồ' => ('o', VowelMod::Hat, Tone::Grave),
        'ổ' => ('o', VowelMod::Hat, Tone::Hook),
        'ỗ' => ('o', VowelMod::Hat, Tone::Tilde),
        'ộ' => ('o', VowelMod::Hat, Tone::Dot),

        'ơ' => ('o', VowelMod::Horn, Tone::None),
        'ớ' => ('o', VowelMod::Horn, Tone::Acute),
        'ờ' => ('o', VowelMod::Horn, Tone::Grave),
        'ở' => ('o', VowelMod::Horn, Tone::Hook),
        'ỡ' => ('o', VowelMod::Horn, Tone::Tilde),
        'ợ' => ('o', VowelMod::Horn, Tone::Dot),

        'u' => ('u', VowelMod::None, Tone::None),
        'ú' => ('u', VowelMod::None, Tone::Acute),
        'ù' => ('u', VowelMod::None, Tone::Grave),
        'ủ' => ('u', VowelMod::None, Tone::Hook),
        'ũ' => ('u', VowelMod::None, Tone::Tilde),
        'ụ' => ('u', VowelMod::None, Tone::Dot),

        'ư' => ('u', VowelMod::Horn, Tone::None),
        'ứ' => ('u', VowelMod::Horn, Tone::Acute),
        'ừ' => ('u', VowelMod::Horn, Tone::Grave),
        'ử' => ('u', VowelMod::Horn, Tone::Hook),
        'ữ' => ('u', VowelMod::Horn, Tone::Tilde),
        'ự' => ('u', VowelMod::Horn, Tone::Dot),

        'y' => ('y', VowelMod::None, Tone::None),
        'ý' => ('y', VowelMod::None, Tone::Acute),
        'ỳ' => ('y', VowelMod::None, Tone::Grave),
        'ỷ' => ('y', VowelMod::None, Tone::Hook),
        'ỹ' => ('y', VowelMod::None, Tone::Tilde),
        'ỵ' => ('y', VowelMod::None, Tone::Dot),

        _ => return None,
    };

    Some((base, vmod, tone, is_upper))
}

#[allow(dead_code)]
pub fn is_vowel(c: char) -> bool {
    decompose_vietnamese_char(c).is_some()
}

#[allow(dead_code)]
pub fn is_consonant(c: char) -> bool {
    let lower = c.to_ascii_lowercase();
    matches!(
        lower,
        'b' | 'c' | 'd' | 'đ' | 'g' | 'h' | 'k' | 'l' | 'm' | 'n' | 'p' | 'q' | 'r' | 's' | 't' | 'v' | 'x'
    )
}

/// Đếm tổng số dấu tiếng Việt (mũ, móc, trăng, thanh, chữ đ/Đ) có trong từ
pub fn count_vietnamese_accents(word: &str) -> usize {
    let mut count = 0;
    for c in word.chars() {
        if c == 'đ' || c == 'Đ' {
            count += 1;
        } else if let Some((_, vmod, tone, _)) = decompose_vietnamese_char(c) {
            if vmod != VowelMod::None {
                count += 1;
            }
            if tone != Tone::None {
                count += 1;
            }
        }
    }
    count
}

