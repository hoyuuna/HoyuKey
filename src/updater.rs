use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::InvalidateRect;
use windows_sys::Win32::Networking::WinInet::{
    InternetCloseHandle, InternetOpenUrlW, InternetOpenW, InternetReadFile,
    INTERNET_FLAG_IGNORE_CERT_CN_INVALID, INTERNET_FLAG_IGNORE_CERT_DATE_INVALID,
    INTERNET_FLAG_RELOAD, INTERNET_OPEN_TYPE_DIRECT,
};

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

// 0: Không hiện modal
// 1: Đang kiểm tra cập nhật...
// 2: Phát hiện bản cập nhật mới (Hỏi Cập nhật ngay / Để sau)
// 3: Đang tải xuống và cài đặt...
// 4: Bạn đang ở phiên bản mới nhất!
// 5: Lỗi kết nối mạng
pub static MODAL_STATE: AtomicU8 = AtomicU8::new(0);

pub static LATEST_VERSION_TAG: Mutex<String> = Mutex::new(String::new());
pub static LATEST_DOWNLOAD_URL: Mutex<String> = Mutex::new(String::new());
pub static CHANGELOG_BODY: Mutex<String> = Mutex::new(String::new());
static IS_FETCHING_CHANGELOG: AtomicBool = AtomicBool::new(false);

pub fn ensure_changelog_loaded(hwnd: HWND) {
    {
        let log = CHANGELOG_BODY.lock().unwrap();
        if !log.is_empty() {
            return;
        }
    }

    if IS_FETCHING_CHANGELOG.swap(true, Ordering::SeqCst) {
        return;
    }

    *CHANGELOG_BODY.lock().unwrap() = "• Đang tải nhật ký thay đổi từ GitHub...".to_string();
    let hwnd_raw = hwnd as isize;

    std::thread::spawn(move || {
        let hwnd = hwnd_raw as HWND;
        match fetch_latest_release_info() {
            Ok((_tag, _url, body)) => {
                *CHANGELOG_BODY.lock().unwrap() = body;
            }
            Err(_) => {
                *CHANGELOG_BODY.lock().unwrap() =
                    "• Chưa thể tải nhật ký cập nhật do chưa có kết nối mạng.\n• Nhật ký sẽ tự động hiển thị khi có kết nối.".to_string();
            }
        }
        IS_FETCHING_CHANGELOG.store(false, Ordering::SeqCst);
        unsafe {
            InvalidateRect(hwnd, null_mut(), 0);
        }
    });
}

pub fn start_checking_update(hwnd: HWND) {
    MODAL_STATE.store(1, Ordering::SeqCst);
    crate::ui::trigger_modal_open(hwnd);

    let hwnd_raw = hwnd as isize;
    std::thread::spawn(move || {
        let hwnd = hwnd_raw as HWND;
        match fetch_latest_release_info() {
            Ok((latest_tag, download_url, body)) => {
                *CHANGELOG_BODY.lock().unwrap() = body;
                let clean_latest = latest_tag.trim_start_matches('v').trim();
                let clean_current = CURRENT_VERSION.trim_start_matches('v').trim();

                if is_newer_version(clean_latest, clean_current) {
                    *LATEST_VERSION_TAG.lock().unwrap() = latest_tag;
                    *LATEST_DOWNLOAD_URL.lock().unwrap() = download_url;
                    MODAL_STATE.store(2, Ordering::SeqCst);
                } else {
                    MODAL_STATE.store(4, Ordering::SeqCst);
                }
            }
            Err(_) => {
                MODAL_STATE.store(5, Ordering::SeqCst);
            }
        }
        unsafe {
            InvalidateRect(hwnd, null_mut(), 0);
        }
    });
}

pub fn start_downloading_update(hwnd: HWND) {
    MODAL_STATE.store(3, Ordering::SeqCst);
    crate::ui::trigger_modal_open(hwnd);

    let download_url = LATEST_DOWNLOAD_URL.lock().unwrap().clone();
    let hwnd_raw = hwnd as isize;

    std::thread::spawn(move || {
        let hwnd = hwnd_raw as HWND;
        perform_update(hwnd, &download_url);
    });
}

pub fn close_modal(hwnd: HWND) {
    MODAL_STATE.store(0, Ordering::SeqCst);
    crate::ui::close_modal_anim(hwnd);
}

fn fetch_latest_release_info() -> Result<(String, String, String), String> {
    let url = "https://api.github.com/repos/hoyuuna/HoyuKey/releases/latest";
    let json_bytes = http_get(url)?;
    let json_str = String::from_utf8_lossy(&json_bytes);

    let tag_name = extract_json_field(&json_str, "tag_name")
        .ok_or_else(|| "Không tìm thấy tag_name".to_string())?;

    let download_url = extract_download_url(&json_str)
        .ok_or_else(|| "Không tìm thấy file tải về HoyuKey.exe".to_string())?;

    let body = extract_json_body(&json_str).unwrap_or_else(|| {
        "• Bản cập nhật mới nhất cho HoyuKey với nhiều tối ưu và sửa lỗi.".to_string()
    });

    Ok((tag_name, download_url, body))
}

fn extract_json_body(json: &str) -> Option<String> {
    let key = "\"body\":";
    let start_idx = json.find(key)? + key.len();
    let sub = json[start_idx..].trim_start();
    if !sub.starts_with('"') {
        return None;
    }
    let chars: Vec<char> = sub[1..].chars().collect();
    let mut result = String::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\\' && i + 1 < chars.len() {
            let next = chars[i + 1];
            match next {
                'n' => result.push('\n'),
                'r' => {},
                't' => result.push('\t'),
                '\\' => result.push('\\'),
                '"' => result.push('"'),
                _ => {
                    result.push('\\');
                    result.push(next);
                }
            }
            i += 2;
        } else if ch == '"' {
            break;
        } else {
            result.push(ch);
            i += 1;
        }
    }
    let trimmed = result.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn extract_json_field(json: &str, field: &str) -> Option<String> {
    let key = format!("\"{}\":", field);
    let start_idx = json.find(&key)? + key.len();
    let sub = json[start_idx..].trim_start();
    if !sub.starts_with('"') {
        return None;
    }
    let after_quote = &sub[1..];
    let end_idx = after_quote.find('"')?;
    Some(after_quote[..end_idx].to_string())
}

fn extract_download_url(json: &str) -> Option<String> {
    for line in json.split('\n') {
        if line.contains("browser_download_url") && line.contains(".exe") {
            let parts: Vec<&str> = line.split('"').collect();
            for part in parts {
                if part.starts_with("http") && part.ends_with(".exe") {
                    return Some(part.to_string());
                }
            }
        }
    }
    None
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse_nums = |s: &str| -> Vec<u32> {
        s.split('.')
            .filter_map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok())
            .collect()
    };

    let l = parse_nums(latest);
    let c = parse_nums(current);

    l > c
}

fn http_get(url: &str) -> Result<Vec<u8>, String> {
    unsafe {
        let user_agent = to_wide("HoyuKey-Updater/1.0");
        let h_internet = InternetOpenW(
            user_agent.as_ptr(),
            INTERNET_OPEN_TYPE_DIRECT,
            null_mut(),
            null_mut(),
            0,
        );
        if h_internet.is_null() {
            return Err("InternetOpenW failed".to_string());
        }

        let wide_url = to_wide(url);
        let flags = INTERNET_FLAG_RELOAD
            | INTERNET_FLAG_IGNORE_CERT_CN_INVALID
            | INTERNET_FLAG_IGNORE_CERT_DATE_INVALID;

        let h_url = InternetOpenUrlW(
            h_internet,
            wide_url.as_ptr(),
            null_mut(),
            0,
            flags,
            0,
        );

        if h_url.is_null() {
            InternetCloseHandle(h_internet);
            return Err("InternetOpenUrlW failed".to_string());
        }

        let mut data = Vec::new();
        let mut buffer = [0u8; 8192];
        let mut bytes_read: u32 = 0;

        while InternetReadFile(
            h_url,
            buffer.as_mut_ptr() as _,
            buffer.len() as u32,
            &mut bytes_read,
        ) != 0
        {
            if bytes_read == 0 {
                break;
            }
            data.extend_from_slice(&buffer[..bytes_read as usize]);
        }

        InternetCloseHandle(h_url);
        InternetCloseHandle(h_internet);

        Ok(data)
    }
}

fn perform_update(hwnd: HWND, download_url: &str) {
    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => {
            MODAL_STATE.store(5, Ordering::SeqCst);
            unsafe { InvalidateRect(hwnd, null_mut(), 0); }
            return;
        }
    };
    let current_dir = match current_exe.parent() {
        Some(d) => d.to_path_buf(),
        None => {
            MODAL_STATE.store(5, Ordering::SeqCst);
            unsafe { InvalidateRect(hwnd, null_mut(), 0); }
            return;
        }
    };

    let update_exe = current_dir.join("HoyuKey_new.exe");
    let bat_file = current_dir.join("update_hoyukey.bat");

    // 1. Tải file mới
    let data = match http_get(download_url) {
        Ok(d) => d,
        Err(_) => {
            MODAL_STATE.store(5, Ordering::SeqCst);
            unsafe { InvalidateRect(hwnd, null_mut(), 0); }
            return;
        }
    };

    if let Err(_) = std::fs::write(&update_exe, &data) {
        MODAL_STATE.store(5, Ordering::SeqCst);
        unsafe { InvalidateRect(hwnd, null_mut(), 0); }
        return;
    }

    // 2. Tạo script cập nhật tự động
    let exe_name = current_exe.file_name().unwrap_or_default().to_string_lossy();
    let bat_content = format!(
        "@echo off\r\ntimeout /t 1 /nobreak >nul\r\nmove /y \"HoyuKey_new.exe\" \"{}\"\r\nstart \"\" \"{}\"\r\ndel \"%~f0\"\r\n",
        exe_name, exe_name
    );

    if let Err(_) = std::fs::write(&bat_file, bat_content) {
        MODAL_STATE.store(5, Ordering::SeqCst);
        unsafe { InvalidateRect(hwnd, null_mut(), 0); }
        return;
    }

    // 3. Chạy script và thoát
    let _ = std::process::Command::new("cmd")
        .args(["/C", &bat_file.to_string_lossy()])
        .spawn();

    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::PostQuitMessage(0);
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
