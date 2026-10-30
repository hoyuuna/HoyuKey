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
    /// Bỏ dấu & thêm mũ Telex tự do bất kỳ lúc nào trong từ (hienej -> hiện, dặtd -> đặt)
    pub free_marking: bool,
    /// Nhận diện từ tiếng Anh thông minh (không can thiệp khi gõ các từ như fix, file, wifi, animation)
    pub smart_english: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            input_method: InputMethod::Telex,
            modern_tone: true,
            switch_key: SwitchKey::CtrlShift,
            auto_restore: true,
            free_marking: true,
            smart_english: true,
        }
    }
}
