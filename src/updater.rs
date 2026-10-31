use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::InvalidateRect;
use windows_sys::Win32::Networking::WinInet::{
    InternetCloseHandle, InternetOpenUrlW, InternetOpenW, InternetReadFile,
    INTERNET_FLAG_IGNORE_CERT_CN_INVALID, INTERNET_FLAG_IGNORE_CERT_DATE_INVALID,
    INTERNET_FLAG_RELOAD, INTERNET_FLAG_SECURE, INTERNET_OPEN_TYPE_DIRECT,
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

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub download_url: String,
    pub body: String,
    pub is_prerelease: bool,
}

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
        match fetch_all_releases() {
            Ok(releases) => {
                if let Some(first) = releases.first() {
                    *CHANGELOG_BODY.lock().unwrap() = first.body.clone();
                } else {
                    *CHANGELOG_BODY.lock().unwrap() =
                        "• Chưa có nhật ký thay đổi nào trên GitHub.".to_string();
                }
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
        match fetch_all_releases() {
            Ok(releases) => {
                // Cập nhật nhật ký nếu có
                if let Some(first) = releases.first() {
                    *CHANGELOG_BODY.lock().unwrap() = first.body.clone();
                }

                // Chỉ thông báo cập nhật đối với các bản phát hành chính thức (không phải pre-release)
                let official_release = releases.iter().find(|r| !r.is_prerelease);

                if let Some(rel) = official_release {
                    let clean_latest = rel.tag_name.trim_start_matches('v').trim();
                    let clean_current = CURRENT_VERSION.trim_start_matches('v').trim();

                    if is_newer_version(clean_latest, clean_current) && !rel.download_url.is_empty() {
                        *LATEST_VERSION_TAG.lock().unwrap() = rel.tag_name.clone();
                        *LATEST_DOWNLOAD_URL.lock().unwrap() = rel.download_url.clone();
                        MODAL_STATE.store(2, Ordering::SeqCst);
                    } else {
                        MODAL_STATE.store(4, Ordering::SeqCst);
                    }
                } else {
                    // Chưa có bản official release nào mới hơn
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

pub const UPDATE_URL: &str = "https://api.github.com/repos/hoyuuna/HoyuKey/releases";

fn fetch_all_releases() -> Result<Vec<ReleaseInfo>, String> {
    let json_bytes = http_get(UPDATE_URL)?;
    let json_str = String::from_utf8_lossy(&json_bytes);

    let releases = parse_releases(&json_str);
    if releases.is_empty() {
        // Dự phòng nếu API trả về một object đơn lẻ thay vì mảng []
        if let Some(single) = parse_single_release(&json_str) {
            return Ok(vec![single]);
        }
        return Err("Không tìm thấy thông tin phiên bản từ API".to_string());
    }

    Ok(releases)
}

fn parse_releases(json: &str) -> Vec<ReleaseInfo> {
    let mut list = Vec::new();
    let mut depth = 0;
    let mut start_idx = 0;
    let mut in_string = false;
    let mut escape = false;

    for (i, b) in json.bytes().enumerate() {
        if escape {
            escape = false;
            continue;
        }
        if b == b'\\' {
            escape = true;
            continue;
        }
        if b == b'"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }

        if b == b'{' {
            if depth == 0 {
                start_idx = i;
            }
            depth += 1;
        } else if b == b'}' {
            depth -= 1;
            if depth == 0 {
                let block = &json[start_idx..=i];
                if let Some(rel) = parse_single_release(block) {
                    list.push(rel);
                }
            }
        }
    }

    list
}

fn parse_single_release(block: &str) -> Option<ReleaseInfo> {
    let tag_name = extract_json_field(block, "version")
        .or_else(|| extract_json_field(block, "tag_name"))?;

    let download_url = extract_json_field(block, "download_url")
        .or_else(|| extract_download_url(block))
        .unwrap_or_default();

    let body = extract_json_body_by_key(block, "\"changelog\":")
        .or_else(|| extract_json_body_by_key(block, "\"body\":"))
        .unwrap_or_else(|| {
            "• Phiên bản mới của HoyuKey với nhiều cải tiến và sửa lỗi.".to_string()
        });

    let is_prerelease = if let Some(idx) = block.find("\"prerelease\":") {
        let sub = block[idx + 13..].trim_start();
        sub.starts_with("true")
    } else {
        false
    };

    Some(ReleaseInfo {
        tag_name,
        download_url,
        body,
        is_prerelease,
    })
}

fn extract_json_body_by_key(json: &str, key: &str) -> Option<String> {
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
            | INTERNET_FLAG_IGNORE_CERT_DATE_INVALID
            | INTERNET_FLAG_SECURE;

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

#[link(name = "urlmon")]
extern "system" {
    fn URLDownloadToFileW(
        p_caller: *mut std::ffi::c_void,
        sz_url: *const u16,
        sz_file_name: *const u16,
        dw_reserved: u32,
        lpfn_cb: *mut std::ffi::c_void,
    ) -> i32;
}

fn download_file(url: &str, dest: &std::path::Path) -> Result<(), String> {
    let _ = std::fs::remove_file(dest);

    // Cách 1: Ưu tiên curl.exe có sẵn trên Windows 10/11
    let mut cmd = std::process::Command::new("curl.exe");
    cmd.args([
        "-L", // Follow redirects (GitHub -> Azure CDN)
        "-s", // Silent
        "-f", // Fail fast on HTTP errors
        "-o",
        &dest.to_string_lossy(),
        "-H",
        "User-Agent: HoyuKey-Updater/1.0",
        url,
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    if let Ok(st) = cmd.status() {
        if st.success() {
            if let Ok(meta) = std::fs::metadata(dest) {
                if meta.len() > 100_000 {
                    return Ok(());
                }
            }
        }
    }

    // Cách 2: Dự phòng URLDownloadToFileW từ urlmon.dll
    let wide_url = to_wide(url);
    let wide_dest = to_wide(&dest.to_string_lossy());
    let hr = unsafe {
        URLDownloadToFileW(
            null_mut(),
            wide_url.as_ptr(),
            wide_dest.as_ptr(),
            0,
            null_mut(),
        )
    };

    if hr == 0 {
        if let Ok(meta) = std::fs::metadata(dest) {
            if meta.len() > 100_000 {
                return Ok(());
            }
        }
    }

    // Cách 3: Dự phòng cuối cùng bằng WinInet http_get
    let data = http_get(url)?;
    if data.len() < 100_000 {
        return Err("File tải về bị lỗi kích thước".to_string());
    }
    std::fs::write(dest, &data).map_err(|e| e.to_string())?;

    Ok(())
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

    let _ = std::fs::remove_file(&update_exe);
    let _ = std::fs::remove_file(&bat_file);

    // 1. Tải file mới vào cùng thư mục app
    if let Err(_) = download_file(download_url, &update_exe) {
        MODAL_STATE.store(5, Ordering::SeqCst);
        unsafe { InvalidateRect(hwnd, null_mut(), 0); }
        return;
    }

    // 2. Tạo script cập nhật tự động (chuẩn mực, lành tính, không dùng các lệnh lạ)
    let exe_name = current_exe.file_name().unwrap_or_default().to_string_lossy();
    let bat_content = format!(
        "@echo off\r\n\
chcp 65001 >nul\r\n\
timeout /t 1 /nobreak >nul\r\n\
:retry\r\n\
move /y \"HoyuKey_new.exe\" \"{}\" >nul 2>&1\r\n\
if exist \"HoyuKey_new.exe\" (\r\n\
    timeout /t 1 /nobreak >nul\r\n\
    goto retry\r\n\
)\r\n\
start \"\" \"{}\"\r\n\
del \"%~f0\" >nul 2>&1\r\n",
        exe_name, exe_name
    );

    if let Err(_) = std::fs::write(&bat_file, bat_content) {
        MODAL_STATE.store(5, Ordering::SeqCst);
        unsafe { InvalidateRect(hwnd, null_mut(), 0); }
        return;
    }

    // 3. Chạy script ngầm và thoát ứng dụng
    let mut cmd = std::process::Command::new("cmd");
    cmd.args(["/C", &bat_file.to_string_lossy()]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let _ = cmd.spawn();

    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::PostQuitMessage(0);
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_releases_with_prerelease() {
        let sample_json = r#"[
            {
                "tag_name": "v1.0.0",
                "prerelease": true,
                "body": "HoyuKey v1.0.0\nPhiên bản tự động",
                "assets": [
                    { "browser_download_url": "https://github.com/hoyuuna/HoyuKey/releases/download/v1.0.0/HoyuKey.exe" }
                ]
            },
            {
                "tag_name": "v0.9.0",
                "prerelease": false,
                "body": "Bản chính thức v0.9.0",
                "assets": [
                    { "browser_download_url": "https://github.com/hoyuuna/HoyuKey/releases/download/v0.9.0/HoyuKey.exe" }
                ]
            }
        ]"#;

        let list = parse_releases(sample_json);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].tag_name, "v1.0.0");
        assert!(list[0].is_prerelease);
        assert_eq!(list[1].tag_name, "v0.9.0");
        assert!(!list[1].is_prerelease);

        let official = list.iter().find(|r| !r.is_prerelease).unwrap();
        assert_eq!(official.tag_name, "v0.9.0");
    }
}