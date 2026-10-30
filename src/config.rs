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
        }
    }
}

impl Config {
    pub fn config_path() -> Option<std::path::PathBuf> {
        let appdata = std::env::var("APPDATA").ok()?;
        let dir = std::path::PathBuf::from(appdata).join("HoyuKey");
        if !dir.exists() {
            let _ = std::fs::create_dir_all(&dir);
        }
        Some(dir.join("config.json"))
    }

    pub fn load() -> Self {
        let mut config = Self::default();
        if let Some(path) = Self::config_path() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                config.merge_from_json(&content);
            } else {
                config.save();
            }
        }
        config.start_with_windows = crate::autostart::is_autostart_enabled();
        config
    }

    pub fn save(&self) {
        if let Some(path) = Self::config_path() {
            let json = self.to_json_string();
            let _ = std::fs::write(path, json);
        }
    }

    pub fn to_json_string(&self) -> String {
        format!(
            "{{\n  \"enabled\": {},\n  \"input_method\": \"{}\",\n  \"modern_tone\": {},\n  \"switch_key\": \"{}\",\n  \"auto_restore\": {},\n  \"free_marking\": {},\n  \"smart_english\": {},\n  \"modify_tone_in_word\": {},\n  \"start_with_windows\": {},\n  \"auto_caps_line\": {},\n  \"auto_caps_sentence\": {},\n  \"auto_period_enter\": {},\n  \"browser_compatibility\": {},\n  \"browser_apps\": \"{}\"\n}}\n",
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
            self.browser_apps.replace('"', "\\\""),
        )
    }

    pub fn merge_from_json(&mut self, json: &str) {
        for line in json.lines() {
            let line = line.trim().trim_end_matches(',');
            if let Some((key, val)) = line.split_once(':') {
                let key = key.trim().trim_matches('"');
                let val = val.trim().trim_matches('"');
                match key {
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
                    "browser_apps" => self.browser_apps = val.to_string(),
                    _ => {}
                }
            }
        }
    }
}
