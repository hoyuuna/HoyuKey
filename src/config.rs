#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMethod {
    Telex,
    Vni,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchKey {
    CtrlShift,
    AltZ,
}

#[derive(Debug, Clone)]
pub struct Config {
    /// Bật / Tắt gõ tiếng Việt (V / E)
    pub enabled: bool,
    /// Kiểu gõ (Telex / VNI)
    pub input_method: InputMethod,
    /// Bỏ dấu kiểu mới (òa, úy thay vì oà, uý)
    pub modern_tone: bool,
    /// Phím tắt chuyển đổi chế độ V / E
    pub switch_key: SwitchKey,
    /// Tự động khôi phục từ khi gõ từ không hợp lệ
    pub auto_restore: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            input_method: InputMethod::Telex,
            modern_tone: true,
            switch_key: SwitchKey::CtrlShift,
            auto_restore: true,
        }
    }
}
