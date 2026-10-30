use std::ptr::null_mut;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Networking::WinInet::{
    InternetCloseHandle, InternetOpenUrlW, InternetOpenW, InternetReadFile,
    INTERNET_FLAG_IGNORE_CERT_CN_INVALID, INTERNET_FLAG_IGNORE_CERT_DATE_INVALID,
    INTERNET_FLAG_RELOAD, INTERNET_OPEN_TYPE_DIRECT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDYES, MB_ICONINFORMATION, MB_ICONQUESTION, MB_ICONWARNING, MB_OK, MB_YESNO,
};

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn check_for_updates_interactive(hwnd: HWND) {
    let hwnd_raw = hwnd as isize;
    std::thread::spawn(move || {
        let hwnd = hwnd_raw as HWND;
        match fetch_latest_release_info() {
            Ok((latest_tag, download_url)) => {
                let clean_latest = latest_tag.trim_start_matches('v').trim();
                let clean_current = CURRENT_VERSION.trim_start_matches('v').trim();

                if is_newer_version(clean_latest, clean_current) {
                    let prompt = format!(
                        "Phát hiện bản cập nhật mới: {}\n(Phiên bản hiện tại: v{})\n\nBạn có muốn tải xuống và cập nhật ngay không?",
                        latest_tag, clean_current
                    );
                    let res = unsafe {
                        MessageBoxW(
                            hwnd,
                            to_wide(&prompt).as_ptr(),
                            to_wide("HoyuKey Cập nhật").as_ptr(),
                            MB_YESNO | MB_ICONQUESTION,
                        )
                    };

                    if res == IDYES {
                        perform_update(hwnd, &download_url);
                    }
                } else {
                    let msg = format!(
                        "Bạn đang sử dụng phiên bản mới nhất (v{})!",
                        clean_current
                    );
                    unsafe {
                        MessageBoxW(
                            hwnd,
                            to_wide(&msg).as_ptr(),
                            to_wide("HoyuKey - Thông báo").as_ptr(),
                            MB_OK | MB_ICONINFORMATION,
                        );
                    }
                }
            }
            Err(_) => {
                unsafe {
                    MessageBoxW(
                        hwnd,
                        to_wide("Không thể kết nối đến GitHub để kiểm tra cập nhật.\nVui lòng kiểm tra lại kết nối mạng Internet!").as_ptr(),
                        to_wide("HoyuKey - Lỗi kết nối").as_ptr(),
                        MB_OK | MB_ICONWARNING,
                    );
                }
            }
        }
    });
}

fn fetch_latest_release_info() -> Result<(String, String), String> {
    let url = "https://api.github.com/repos/hoyuuna/HoyuKey/releases/latest";
    let json_bytes = http_get(url)?;
    let json_str = String::from_utf8_lossy(&json_bytes);

    // Trích xuất "tag_name": "v0.x.x"
    let tag_name = extract_json_field(&json_str, "tag_name")
        .ok_or_else(|| "Không tìm thấy tag_name".to_string())?;

    // Trích xuất "browser_download_url": "https://...HoyuKey.exe"
    let download_url = extract_download_url(&json_str)
        .ok_or_else(|| "Không tìm thấy file tải về HoyuKey.exe".to_string())?;

    Ok((tag_name, download_url))
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
        Err(_) => return,
    };
    let current_dir = match current_exe.parent() {
        Some(d) => d.to_path_buf(),
        None => return,
    };

    let update_exe = current_dir.join("HoyuKey_new.exe");
    let bat_file = current_dir.join("update_hoyukey.bat");

    // 1. Tải file mới
    let data = match http_get(download_url) {
        Ok(d) => d,
        Err(e) => {
            let msg = format!("Lỗi khi tải bản cập nhật: {}", e);
            unsafe {
                MessageBoxW(
                    hwnd,
                    to_wide(&msg).as_ptr(),
                    to_wide("HoyuKey - Lỗi tải về").as_ptr(),
                    MB_OK | MB_ICONWARNING,
                );
            }
            return;
        }
    };

    if let Err(_) = std::fs::write(&update_exe, &data) {
        return;
    }

    // 2. Tạo script cập nhật tự động
    let exe_name = current_exe.file_name().unwrap_or_default().to_string_lossy();
    let bat_content = format!(
        "@echo off\r\ntimeout /t 1 /nobreak >nul\r\nmove /y \"HoyuKey_new.exe\" \"{}\"\r\nstart \"\" \"{}\"\r\ndel \"%~f0\"\r\n",
        exe_name, exe_name
    );

    if let Err(_) = std::fs::write(&bat_file, bat_content) {
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
