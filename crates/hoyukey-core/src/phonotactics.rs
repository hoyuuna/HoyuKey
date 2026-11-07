// hoyukey-core: Dynamic Phonotactics Engine
// Quy luáº­t ngá»¯ Ã¢m há» c tiáº¿ng Viá»‡t Ä‘á»™ng (Zero-dependency, khÃ´ng dÃ¹ng tá»« Ä‘iá»ƒn cá»©ng).

use crate::vietnamese::{decompose_vietnamese_char, get_vietnamese_vowel, Tone, VowelMod};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VowelPart {
    pub base: char,
    pub vmod: VowelMod,
    pub tone: Tone,
    pub is_upper: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecomposedSyllable {
    pub lead: String,
    pub vowels: Vec<VowelPart>,
    pub trail: String,
    pub is_qu_lead: bool,
    pub is_gi_lead: bool,
}

impl DecomposedSyllable {
    /// TÃ¡i táº¡o chuá»—i Unicode hoÃ n chá»‰nh tá»« cÃ¡c thÃ nh pháº§n Ã¢m tiáº¿t
    pub fn recompose(&self) -> String {
        let mut result = String::with_capacity(self.lead.len() + self.vowels.len() * 4 + self.trail.len());
        result.push_str(&self.lead);
        for v in &self.vowels {
            if let Some(ch) = get_vietnamese_vowel(v.base, v.vmod, v.tone) {
                if v.is_upper {
                    result.extend(ch.to_uppercase());
                } else {
                    result.push(ch);
                }
            } else {
                result.push(v.base);
            }
        }
        result.push_str(&self.trail);
        result
    }

    /// Láº¥y thanh Ä‘iá»‡u hiá»‡n táº¡i cá»§a Ã¢m tiáº¿t
    pub fn current_tone(&self) -> Tone {
        for v in &self.vowels {
            if v.tone != Tone::None {
                return v.tone;
            }
        }
        Tone::None
    }

    /// XÃ³a toÃ n bá»™ dáº¥u thanh khá» i cÃ¡c nguyÃªn Ã¢m
    pub fn clear_tone(&mut self) {
        for v in &mut self.vowels {
            v.tone = Tone::None;
        }
    }

    /// Ä áº·t dáº¥u thanh má»›i vÃ o Ä‘Ãºng nguyÃªn Ã¢m chÃ­nh dá»±a theo quy luáº­t thanh Ä‘iá»‡u tiáº¿ng Viá»‡t
    pub fn apply_tone(&mut self, tone: Tone, modern_tone: bool) {
        self.clear_tone();
        if tone == Tone::None || self.vowels.is_empty() {
            return;
        }

        let target_idx = select_tone_vowel_index(&self.vowels, !self.trail.is_empty(), modern_tone);
        if target_idx < self.vowels.len() {
            self.vowels[target_idx].tone = tone;
        }
    }
}

/// PhÃ¢n tÃ­ch Ä‘á»™ng 1 tá»« tiáº¿ng Viá»‡t thÃ nh cÃ¡c thÃ nh pháº§n: Phá»¥ Ã¢m Ä‘áº§u, Cá»¥m nguyÃªn Ã¢m, Phá»¥ Ã¢m cuá»‘i.
/// Tráº£ vá»  None náº¿u tá»« lÃ  tá»« ngoáº¡i lai hoáº·c cÃ³ nhiá» u hÆ¡n 1 cá»¥m nguyÃªn Ã¢m (Ä‘a Ã¢m tiáº¿t kiá»ƒu tiáº¿ng Anh).
pub fn decompose_word(word: &str) -> Option<DecomposedSyllable> {
    if word.is_empty() {
        return None;
    }

    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();

    // Kiá»ƒm tra 'qu' vÃ  'gi' á»Ÿ Ä‘áº§u tá»«
    let mut is_qu = false;
    let mut is_gi = false;
    let mut start_idx = 0;

    let lower_first = chars[0].to_ascii_lowercase();
    if n >= 2 {
        let lower_second = chars[1].to_ascii_lowercase();
        if lower_first == 'q' && lower_second == 'u' {
            if n > 2 && is_vietnamese_vowel_char(chars[2]) {
                is_qu = true;
                start_idx = 2;
            }
        } else if lower_first == 'g' && lower_second == 'i' {
            if n > 2 && is_vietnamese_vowel_char(chars[2]) {
                let third_lower = chars[2].to_ascii_lowercase();
                if third_lower != 'i' {
                    is_gi = true;
                    start_idx = 2;
                }
            }
        }
    }

    // TÃ¬m pháº¡m vi cá»¥m nguyÃªn Ã¢m duy nháº¥t
    let mut v_start = None;
    let mut v_end = None;

    let mut i = start_idx;
    while i < n {
        if is_vietnamese_vowel_char(chars[i]) {
            if v_start.is_none() {
                v_start = Some(i);
            }
            v_end = Some(i + 1);
        } else if v_start.is_some() {
            // Ä Ã£ qua cá»¥m nguyÃªn Ã¢m, gáº·p phá»¥ Ã¢m
            // Kiá»ƒm tra xem phÃ­a sau cÃ³ láº¡i xuáº¥t hiá»‡n nguyÃªn Ã¢m khÃ´ng (náº¿u cÃ³ -> Ä‘a Ã¢m tiáº¿t ngoáº¡i lai)
            for j in (i + 1)..n {
                if is_vietnamese_vowel_char(chars[j]) {
                    return None; // CÃ³ > 1 cá»¥m nguyÃªn Ã¢m riÃªng biá»‡t (vÃ­ dá»¥: re-lease, com-pu-ter)
                }
            }
            break;
        }
        i += 1;
    }

    let (v_start, v_end) = match (v_start, v_end) {
        (Some(s), Some(e)) => (s, e),
        _ => {
            // KhÃ´ng cÃ³ nguyÃªn Ã¢m nÃ o (chá»‰ cÃ³ phá»¥ Ã¢m hoáº·c kÃ½ tá»± trá»‘ng)
            return Some(DecomposedSyllable {
                lead: word.to_string(),
                vowels: Vec::new(),
                trail: String::new(),
                is_qu_lead: is_qu,
                is_gi_lead: is_gi,
            });
        }
    };

    let lead: String = chars[..v_start].iter().collect();
    let trail: String = chars[v_end..].iter().collect();

    let mut vowels = Vec::with_capacity(v_end - v_start);
    for &c in &chars[v_start..v_end] {
        if let Some((base, vmod, tone, is_upper)) = decompose_vietnamese_char(c) {
            vowels.push(VowelPart {
                base,
                vmod,
                tone,
                is_upper,
            });
        } else {
            vowels.push(VowelPart {
                base: c.to_ascii_lowercase(),
                vmod: VowelMod::None,
                tone: Tone::None,
                is_upper: c.is_uppercase(),
            });
        }
    }

    Some(DecomposedSyllable {
        lead,
        vowels,
        trail,
        is_qu_lead: is_qu,
        is_gi_lead: is_gi,
    })
}

/// Thuáº­t toÃ¡n xÃ¡c Ä‘á»‹nh nguyÃªn Ã¢m chÃ­nh mang dáº¥u thanh theo quy luáº­t ngá»¯ Ã¢m há» c tiáº¿ng Viá»‡t.
/// KhÃ´ng dÃ¹ng tá»« Ä‘iá»ƒn, suy luáº­n toÃ¡n há» c tuyá»‡t Ä‘á»‘i.
pub fn select_tone_vowel_index(vowels: &[VowelPart], has_trail: bool, modern_tone: bool) -> usize {
    let len = vowels.len();
    if len <= 1 {
        return 0;
    }

    // Náº¿u cÃ³ 1 nguyÃªn Ã¢m Ä‘Ã£ mang mÅ©/mÃ³c (Ã¢, Äƒ, Ãª, Ã´, Æ¡, Æ°), nguyÃªn Ã¢m Ä‘Ã³ luÃ´n lÃ  Ã¢m chÃ­nh mang dáº¥u thanh!
    let mut marked_vowel = None;
    for (idx, v) in vowels.iter().enumerate() {
        if v.vmod != VowelMod::None {
            marked_vowel = Some(idx);
        }
    }

    if len == 2 {
        if has_trail {
            // CÃ³ phá»¥ Ã¢m cuá»‘i (toÃ¡n, tiáº¿ng, buá»“n, hoÃ ng, xuá»‘ng):
            // Dáº¥u luÃ´n Ä‘áº·t á»Ÿ nguyÃªn Ã¢m thá»© 2!
            return 1;
        }

        // KhÃ´ng cÃ³ phá»¥ Ã¢m cuá»‘i (Ã¢m tiáº¿t má»Ÿ):
        let v0 = vowels[0].base;
        let v1 = vowels[1].base;

        // CÃ¡c cáº·p nguyÃªn Ã¢m Ä‘Ã´i má»Ÿ ia, ua, Æ°a: dáº¥u luÃ´n á»Ÿ nguyÃªn Ã¢m thá»© nháº¥t (mÃ­a, mÃºa, má»©a, kÃ¬a, chÃ¹a)
        if (v0 == 'i' && v1 == 'a') || (v0 == 'u' && v1 == 'a') || (v0 == 'u' && vowels[0].vmod == VowelMod::Horn && v1 == 'a') {
            return 0;
        }

        // CÃ¡c cáº·p oa, oe, uy:
        if (v0 == 'o' && (v1 == 'a' || v1 == 'e')) || (v0 == 'u' && v1 == 'y') {
            return if modern_tone { 0 } else { 1 }; // Chuáº©n má»›i: hÃ³a, hÃ²e, thÃºy; Chuáº©n cÅ©: hoÃ¡, hoÃ¨, thuÃ½
        }

        // CÃ¡c cáº·p cÃ³ bÃ¡n Ã¢m cuá»‘i (ai, ao, au, ay, eo, eu, oi, ui): dáº¥u luÃ´n á»Ÿ nguyÃªn Ã¢m thá»© nháº¥t (mÃ¡i, bÃ¡o, sÃ¡u, chÃ¨o, ngá»“i, tÃºi)
        if let Some(idx) = marked_vowel {
            return idx;
        }
        return 0;
    }

    if len >= 3 {
        // CÃ¡c cá»¥m 3 nguyÃªn Ã¢m (oai, oay, uay, uÃ´i, Æ°Æ¡i, Æ°Æ¡u, uyÃª):
        if let Some(idx) = marked_vowel {
            return idx;
        }
        // Máº·c Ä‘á»‹nh nguyÃªn Ã¢m á»Ÿ giá»¯a lÃ  Ã¢m chÃ­nh (thoÃ¡i, xoay, khuáº¥y, khuá»·u)
        return 1;
    }

    0
}

/// Kiá»ƒm tra 1 kÃ½ tá»± cÃ³ pháº£i lÃ  nguyÃªn Ã¢m tiáº¿ng Viá»‡t hay khÃ´ng
pub fn is_vietnamese_vowel_char(c: char) -> bool {
    decompose_vietnamese_char(c).is_some()
}

/// Kiểm tra xem từ có chứa nhiều hơn 1 cụm nguyên âm tách biệt hay không (đặc trưng từ tiếng Anh đa âm tiết: com-pu-ter, re-lease, sys-tem)
pub fn has_multiple_vowel_clusters(word: &str) -> bool {
    let lower = word.to_lowercase();
    let mut vowel_groups = 0;
    let mut in_vowel = false;

    for c in lower.chars() {
        if is_vietnamese_vowel_char(c) {
            if !in_vowel {
                vowel_groups += 1;
                in_vowel = true;
            }
        } else {
            in_vowel = false;
        }
    }

    vowel_groups > 1
}

/// Kiểm tra xem từ có chứa cụm phụ âm bất hợp lệ ở cuối hay không (Dynamic Coda Invariant)
/// Trong tiếng Việt, âm cuối chỉ có thể là:
/// - Không có phụ âm (vần mở: ba, mẹ, chỉ, ưu, đay)
/// - Đúng 1 phụ âm đơn: c, m, n, p, t
/// - Đúng 3 phụ âm kép: ng, nh, ch
/// Bất kỳ từ nào có >= 2 phụ âm ở cuối mà không phải ng, nh, ch -> 100% là tiếng Anh (st, sk, sp, ft, ct, rt, rd...)
pub fn has_invalid_coda_consonants(word: &str) -> bool {
    let lower = word.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    let mut coda_chars = Vec::new();

    for &c in chars.iter().rev() {
        if is_vietnamese_vowel_char(c) {
            break;
        }
        if c.is_alphabetic() {
            coda_chars.push(c);
        }
    }
    coda_chars.reverse();

    if coda_chars.len() >= 2 {
        let s: String = coda_chars.into_iter().collect();
        if s != "ng" && s != "nh" && s != "ch" {
            return true;
        }
    }

    false
}

/// Kiểm tra xem từ có phải là từ tiếng Anh / ngoại quốc hay không:
/// Hoàn toàn DYNAMIC, KHÔNG DÙNG TỪ ĐIỂN:
/// 1. Bắt đầu bằng phụ âm ngoại lai không có trong tiếng Việt (f, j, z, hoặc w + nguyên âm như win, wifi, web)
/// 2. Chứa cụm phụ âm đuôi tiếng Anh (st, sk, sp, ft, ct...)
/// 3. Chứa nhiều hơn 1 cụm nguyên âm cách nhau bởi phụ âm (re-lease, sys-tem, sta-tus, com-pu-ter...)
pub fn is_english_foreign_word(word: &str) -> bool {
    let lower = word.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if chars.is_empty() || chars.len() < 3 {
        return false;
    }

    // 1. Ký tự đầu tiên ngoại lai
    if matches!(chars[0], 'f' | 'j' | 'z') {
        return true;
    }
    if chars[0] == 'w' && chars.len() > 1 && is_vietnamese_vowel_char(chars[1]) {
        return true;
    }

    // 2. Cụm phụ âm đuôi tiếng Anh
    if has_invalid_coda_consonants(&lower) {
        return true;
    }

    // 3. Đa âm tiết (nhiều cụm nguyên âm)
    has_multiple_vowel_clusters(&lower)
}

/// Kiểm tra tính hợp lệ của âm tiết tiếng Việt động (Dynamic Vietnamese Syllable Check).
/// Hoàn toàn động:
/// - Không kiểm tra whitelist phụ âm đầu (mọi phụ âm đều được phép ghép vần tiếng Việt).
/// - Phụ âm cuối không được là cụm phụ âm tiếng Anh.
/// - Luật thanh điệu âm tắc: Nếu kết thúc bằng c, ch, p, t thì CHỈ được mang thanh Sắc hoặc Nặng.
pub fn is_valid_vietnamese_phonotactics(word: &str) -> bool {
    let lower = word.to_lowercase();
    let decomposed = match decompose_word(&lower) {
        Some(d) => d,
        None => return false,
    };

    if decomposed.vowels.is_empty() {
        return true;
    }

    // Đuôi từ không được có cụm phụ âm tiếng Anh (st, sk, ct, ft...)
    if has_invalid_coda_consonants(&lower) {
        return false;
    }

    // Luật thanh điệu âm tắc: c, ch, p, t chỉ đi với Sắc hoặc Nặng
    if matches!(decomposed.trail.as_str(), "c" | "ch" | "p" | "t") {
        let tone = decomposed.current_tone();
        if matches!(tone, Tone::Grave | Tone::Hook | Tone::Tilde) {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decompose_simple() {
        let d = decompose_word("toan").unwrap();
        assert_eq!(d.lead, "t");
        assert_eq!(d.vowels.len(), 2);
        assert_eq!(d.trail, "n");
        assert_eq!(d.recompose(), "toan");
    }

    #[test]
    fn test_decompose_qu() {
        let d = decompose_word("quát").unwrap();
        assert_eq!(d.lead, "qu");
        assert_eq!(d.vowels.len(), 1);
        assert_eq!(d.trail, "t");
        assert_eq!(d.recompose(), "quát");
    }

    #[test]
    fn test_decompose_gi() {
        let d = decompose_word("gió").unwrap();
        assert_eq!(d.lead, "gi");
        assert_eq!(d.vowels.len(), 1);
        assert_eq!(d.trail, "");
        assert_eq!(d.recompose(), "gió");
    }

    #[test]
    fn test_tone_placement_modern_vs_classic() {
        let mut d = decompose_word("hoa").unwrap();
        d.apply_tone(Tone::Acute, true); // modern
        assert_eq!(d.recompose(), "hóa");

        let mut d2 = decompose_word("hoa").unwrap();
        d2.apply_tone(Tone::Acute, false); // classic
        assert_eq!(d2.recompose(), "hoá");
    }

    #[test]
    fn test_tone_placement_with_trail() {
        let mut d = decompose_word("toan").unwrap();
        d.apply_tone(Tone::Acute, true);
        assert_eq!(d.recompose(), "toán");

        let mut d2 = decompose_word("toan").unwrap();
        d2.apply_tone(Tone::Acute, false);
        assert_eq!(d2.recompose(), "toán"); // Có âm cuối luôn ở nguyên âm thứ hai!
    }

    #[test]
    fn test_english_detection() {
        assert!(is_english_foreign_word("fast"));
        assert!(is_english_foreign_word("system"));
        assert!(is_english_foreign_word("release"));
        assert!(is_english_foreign_word("status"));
        assert!(!is_english_foreign_word("nghiêng"));
        assert!(!is_english_foreign_word("hoàng"));
    }
}
