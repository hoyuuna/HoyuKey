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
    /// Tự động điều chỉnh dấu thanh khi sửa nguyên âm trong từ (khí -> khái)
    pub modify_tone_in_word: bool,
    /// Khởi động cùng Windows khi đăng nhập máy tính
    pub start_with_windows: bool,
    /// Tự động viết hoa chữ cái đầu dòng
    pub auto_caps_line: bool,
    /// Tự động viết hoa chữ cái sau dấu chấm
    pub auto_caps_sentence: bool,
    /// Tự động thêm dấu chấm câu khi xuống dòng hoặc gửi
    pub auto_period_enter: bool,
    /// Tương thích trình duyệt (Chrome, Edge, Brave, Cốc Cốc...)
    pub browser_compatibility: bool,
    /// Danh sách tiến trình ứng dụng cần tương thích trình duyệt
    pub browser_apps: String,
    /// Ghi nhớ trạng thái E / V riêng cho từng ứng dụng khi chuyển cửa sổ
    pub remember_app_state: bool,
    /// Ép giới hạn RAM chạy nền
    pub limit_ram: bool,
    /// Mức RAM tối đa (MB) khi chạy nền (mốc an toàn tối thiểu 1 MB, mặc định 2 MB)
    pub max_ram_mb: u32,
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
            modify_tone_in_word: true,
            start_with_windows: false,
            auto_caps_line: false,
            auto_caps_sentence: false,
            auto_period_enter: false,
            browser_compatibility: true,
            browser_apps: "chrome.exe, msedge.exe, brave.exe, coccoc.exe, opera.exe".to_string(),
            remember_app_state: false,
            limit_ram: false,
            max_ram_mb: 1,
        }
    }
}

impl Config {
    /// Thư mục chứa file thực thi (.exe)
    pub fn exe_dir() -> Option<std::path::PathBuf> {
        std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()))
    }

    /// Đường dẫn config lưu tại %APPDATA%/HoyuKey/config.json (dùng cho phiên bản cũ hoặc dự phòng)
    pub fn appdata_config_path() -> Option<std::path::PathBuf> {
        let appdata = std::env::var("APPDATA").ok()?;
        let dir = std::path::PathBuf::from(appdata).join("HoyuKey");
        if !dir.exists() {
            let _ = std::fs::create_dir_all(&dir);
        }
        Some(dir.join("config.json"))
    }

    /// Đường dẫn config chính: Luôn ưu tiên lưu cùng thư mục với file thực thi (.exe)
    pub fn config_path() -> Option<std::path::PathBuf> {
        if let Some(dir) = Self::exe_dir() {
            return Some(dir.join("config.json"));
        }
        Self::appdata_config_path()
    }

    /// Nạp cấu hình với hỗ trợ tương thích ngược toàn diện giữa các phiên bản
    pub fn load() -> Self {
        let mut config = Self::default();

        let exe_cfg = Self::exe_dir().map(|d| d.join("config.json"));
        let appdata_cfg = Self::appdata_config_path();

        let mut loaded = false;

        // 1. Ưu tiên nạp từ file config.json đặt ngay cạnh file .exe
        if let Some(ref path) = exe_cfg {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    config.merge_from_json(&content);
                    loaded = true;
                }
            }
        }

        // 2. Tương thích ngược: Nếu ở thư mục file .exe chưa có config.json,
        // kiểm tra nếu có config cũ từ %APPDATA%/HoyuKey/config.json thì nạp và tự động migrate sang thư mục file .exe
        if !loaded {
            if let Some(ref path) = appdata_cfg {
                if path.exists() {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        config.merge_from_json(&content);
                        loaded = true;
                        // Tự động lưu sang cạnh file .exe để từ nay chạy portable độc lập
                        config.save();
                    }
                }
            }
        }

        // 3. Nếu là lần đầu khởi chạy (chưa có ở cả 2 nơi), khởi tạo file config.json ngay cạnh file .exe
        if !loaded {
            config.save();
        }

        config.start_with_windows = crate::autostart::is_autostart_enabled();
        config
    }

    /// Lưu cấu hình: Ưu tiên lưu ngay cạnh file .exe, tự động dự phòng sang AppData nếu thư mục file .exe bị cấm ghi
    pub fn save(&self) {
        if let Some(path) = Self::config_path() {
            let json = self.to_json_string();
            if let Err(_) = std::fs::write(&path, &json) {
                // Dự phòng: Ghi vào AppData nếu thư mục exe bị write-protected
                if let Some(appdata_path) = Self::appdata_config_path() {
                    let _ = std::fs::write(appdata_path, json);
                }
            }
        }
    }

    pub fn to_json_string(&self) -> String {
        let escaped_browser_apps = self.browser_apps.replace('\\', "\\\\").replace('"', "\\\"");
        format!(
            "{{\n  \"enabled\": {},\n  \"input_method\": \"{}\",\n  \"modern_tone\": {},\n  \"switch_key\": \"{}\",\n  \"auto_restore\": {},\n  \"free_marking\": {},\n  \"smart_english\": {},\n  \"modify_tone_in_word\": {},\n  \"start_with_windows\": {},\n  \"auto_caps_line\": {},\n  \"auto_caps_sentence\": {},\n  \"auto_period_enter\": {},\n  \"browser_compatibility\": {},\n  \"browser_apps\": \"{}\",\n  \"remember_app_state\": {},\n  \"limit_ram\": {},\n  \"max_ram_mb\": {}\n}}\n",
            self.enabled,
            match self.input_method {
                InputMethod::Telex => "Telex",
                InputMethod::Vni => "VNI",
            },
            self.modern_tone,
            match self.switch_key {
                SwitchKey::CtrlShift => "CtrlShift",
                SwitchKey::AltZ => "AltZ",
            },
            self.auto_restore,
            self.free_marking,
            self.smart_english,
            self.modify_tone_in_word,
            self.start_with_windows,
            self.auto_caps_line,
            self.auto_caps_sentence,
            self.auto_period_enter,
            self.browser_compatibility,
            escaped_browser_apps,
            self.remember_app_state,
            self.limit_ram,
            self.max_ram_mb,
        )
    }

    /// Trích xuất và cập nhật cấu hình từ JSON mà không phụ thuộc vào thứ tự hay định dạng (hỗ trợ nhiều dòng, 1 dòng minified, thiếu trường hoặc thừa trường)
    pub fn merge_from_json(&mut self, json: &str) {
        for (key, val) in parse_json_key_values(json) {
            match key.as_str() {
                "enabled" => self.enabled = val == "true",
                "input_method" => {
                    if val.eq_ignore_ascii_case("vni") {
                        self.input_method = InputMethod::Vni;
                    } else {
                        self.input_method = InputMethod::Telex;
                    }
                }
                "modern_tone" => self.modern_tone = val == "true",
                "switch_key" => {
                    if val.eq_ignore_ascii_case("altz") {
                        self.switch_key = SwitchKey::AltZ;
                    } else {
                        self.switch_key = SwitchKey::CtrlShift;
                    }
                }
                "auto_restore" => self.auto_restore = val == "true",
                "free_marking" => self.free_marking = val == "true",
                "smart_english" => self.smart_english = val == "true",
                "modify_tone_in_word" => self.modify_tone_in_word = val == "true",
                "auto_caps_line" => self.auto_caps_line = val == "true",
                "auto_caps_sentence" => self.auto_caps_sentence = val == "true",
                "auto_period_enter" => self.auto_period_enter = val == "true",
                "browser_compatibility" => self.browser_compatibility = val == "true",
                "browser_apps" => self.browser_apps = val,
                "remember_app_state" => self.remember_app_state = val == "true",
                "limit_ram" => self.limit_ram = val == "true",
                "max_ram_mb" => {
                    if let Ok(v) = val.parse::<u32>() {
                        self.max_ram_mb = v.clamp(1, 16);
                    }
                }
                // Tương thích ngược: Bỏ qua các trường lạ/chưa hỗ trợ mà không làm hỏng cấu hình
                _ => {}
            }
        }
    }
}

/// Tokenizer JSON siêu nhẹ độc lập (zero-dependency), chịu lỗi cao, hỗ trợ cả JSON đa dòng lẫn 1 dòng
fn parse_json_key_values(json: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();
    let chars: Vec<char> = json.chars().collect();
    let n = chars.len();
    let mut i = 0;

    while i < n {
        if chars[i] == '"' {
            i += 1;
            let mut key = String::new();
            while i < n && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < n {
                    key.push(chars[i + 1]);
                    i += 2;
                } else {
                    key.push(chars[i]);
                    i += 1;
                }
            }
            if i < n && chars[i] == '"' {
                i += 1;
            }

            while i < n && chars[i] != ':' && chars[i] != '}' {
                i += 1;
            }

            if i < n && chars[i] == ':' {
                i += 1;
                while i < n && chars[i].is_whitespace() {
                    i += 1;
                }
                if i < n {
                    if chars[i] == '"' {
                        i += 1;
                        let mut val = String::new();
                        while i < n && chars[i] != '"' {
                            if chars[i] == '\\' && i + 1 < n {
                                val.push(chars[i + 1]);
                                i += 2;
                            } else {
                                val.push(chars[i]);
                                i += 1;
                            }
                        }
                        if i < n && chars[i] == '"' {
                            i += 1;
                        }
                        results.push((key, val));
                    } else {
                        let mut val = String::new();
                        while i < n && chars[i] != ',' && chars[i] != '}' && !chars[i].is_whitespace() {
                            val.push(chars[i]);
                            i += 1;
                        }
                        results.push((key, val));
                    }
                }
            }
        } else {
            i += 1;
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backward_compatibility_old_version() {
        // Giả lập file config của bản cũ (thiếu browser_compatibility, browser_apps, auto_caps...)
        let old_json = r#"{
            "enabled": false,
            "input_method": "VNI",
            "modern_tone": false,
            "switch_key": "AltZ",
            "auto_restore": false
        }"#;

        let mut config = Config::default();
        config.merge_from_json(old_json);

        // Các trường cũ phải được nạp chính xác
        assert_eq!(config.enabled, false);
        assert_eq!(config.input_method, InputMethod::Vni);
        assert_eq!(config.modern_tone, false);
        assert_eq!(config.switch_key, SwitchKey::AltZ);
        assert_eq!(config.auto_restore, false);

        // Các trường mới chưa có trong bản cũ phải tự động giữ giá trị default tối ưu của bản mới
        assert_eq!(config.browser_compatibility, true);
        assert!(config.browser_apps.contains("chrome.exe"));
        assert_eq!(config.smart_english, true);
        assert_eq!(config.remember_app_state, false);
        assert_eq!(config.limit_ram, false);
        assert_eq!(config.max_ram_mb, 2);
    }

    #[test]
    fn test_ram_limit_config() {
        let json_with_ram = r#"{"limit_ram": true, "max_ram_mb": 4}"#;
        let mut config = Config::default();
        config.merge_from_json(json_with_ram);
        assert_eq!(config.limit_ram, true);
        assert_eq!(config.max_ram_mb, 4);

        // Kiểm tra mốc an toàn tối thiểu 1 MB (không được nhỏ hơn 1)
        let json_unsafe = r#"{"limit_ram": true, "max_ram_mb": 0}"#;
        let mut config2 = Config::default();
        config2.merge_from_json(json_unsafe);
        assert_eq!(config2.max_ram_mb, 1);
    }

    #[test]
    fn test_remember_app_state_parsing() {
        let json_with_remember = r#"{"remember_app_state": true}"#;
        let mut config = Config::default();
        config.merge_from_json(json_with_remember);
        assert_eq!(config.remember_app_state, true);

        let serialized = config.to_json_string();
        assert!(serialized.contains("\"remember_app_state\": true"));
    }

    #[test]
    fn test_forward_compatibility_future_fields() {
        // Giả lập file config có chứa trường mới trong tương lai
        let future_json = r#"{
            "enabled": true,
            "future_ai_feature": true,
            "unknown_new_setting": "awesome",
            "browser_apps": "custom_browser.exe"
        }"#;

        let mut config = Config::default();
        config.merge_from_json(future_json);

        assert_eq!(config.enabled, true);
        assert_eq!(config.browser_apps, "custom_browser.exe");
    }

    #[test]
    fn test_minified_one_line_json() {
        let minified = r#"{"enabled":false,"input_method":"VNI","browser_compatibility":false}"#;
        let mut config = Config::default();
        config.merge_from_json(minified);

        assert_eq!(config.enabled, false);
        assert_eq!(config.input_method, InputMethod::Vni);
        assert_eq!(config.browser_compatibility, false);
    }
}
