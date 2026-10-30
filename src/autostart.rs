use std::env;
use std::ptr::null_mut;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ,
};

const RUN_KEY_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const APP_VALUE_NAME: &str = "HoyuKey";

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Kiểm tra xem ứng dụng đã được cấu hình khởi động cùng Windows trong Registry chưa
pub fn is_autostart_enabled() -> bool {
    unsafe {
        let mut hkey: HKEY = null_mut();
        let subkey = to_wide(RUN_KEY_PATH);
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut hkey) != 0 {
            return false;
        }

        let val_name = to_wide(APP_VALUE_NAME);
        let mut val_type: u32 = 0;
        let mut data_len: u32 = 0;

        let res = RegQueryValueExW(
            hkey,
            val_name.as_ptr(),
            null_mut(),
            &mut val_type,
            null_mut(),
            &mut data_len,
        );

        RegCloseKey(hkey);
        res == 0 && data_len > 0
    }
}

/// Bật hoặc tắt khởi động cùng Windows bằng cách ghi/xóa giá trị trong Registry
pub fn set_autostart(enable: bool) -> bool {
    unsafe {
        let mut hkey: HKEY = null_mut();
        let subkey = to_wide(RUN_KEY_PATH);
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_SET_VALUE, &mut hkey) != 0 {
            return false;
        }

        let val_name = to_wide(APP_VALUE_NAME);

        let success = if enable {
            if let Ok(exe_path) = env::current_exe() {
                let cmd = format!("\"{}\"", exe_path.to_string_lossy());
                let wide_cmd = to_wide(&cmd);
                let byte_len = (wide_cmd.len() * 2) as u32;

                RegSetValueExW(
                    hkey,
                    val_name.as_ptr(),
                    0,
                    REG_SZ,
                    wide_cmd.as_ptr() as *const u8,
                    byte_len,
                ) == 0
            } else {
                false
            }
        } else {
            let _ = RegDeleteValueW(hkey, val_name.as_ptr());
            true
        };

        RegCloseKey(hkey);
        success
    }
}
