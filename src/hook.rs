use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicPtr, Ordering};
use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, GetKeyboardState, SendInput, ToUnicode, INPUT, INPUT_KEYBOARD,
    KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VK_BACK, VK_CONTROL, VK_DOWN, VK_END,
    VK_ESCAPE, VK_HOME, VK_LEFT, VK_LWIN, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT,
    VK_RWIN, VK_SHIFT, VK_TAB, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId,
    PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx, GUITHREADINFO, HHOOK, KBDLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_MBUTTONDOWN,
    WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use crate::config::SwitchKey;
use crate::engine::KeyAction;
use crate::tray::WM_APP_UPDATE_TRAY;
use crate::GLOBAL_STATE;

pub const HOYU_MAGIC_EXTRA_INFO: usize = 0x484F5955; // 'HOYU'

static KBD_HOOK_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(null_mut());
static MOUSE_HOOK_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(null_mut());

// Theo dõi các phím bị chặn KeyDown để chặn luôn KeyUp tương ứng, tránh desync phím khi gõ nhanh trên trình duyệt
const INIT_BOOL: AtomicBool = AtomicBool::new(false);
static BLOCKED_VK: [AtomicBool; 256] = [INIT_BOOL; 256];

// Theo dõi cửa sổ đang active (foreground), đổi cửa sổ -> tự động reset buffer
static LAST_FOREGROUND_WINDOW: AtomicIsize = AtomicIsize::new(0);

// Theo dõi trạng thái phím tắt Ctrl + Shift (chuyển chế độ khi nhả phím)
static CTRL_SHIFT_ACTIVE: AtomicBool = AtomicBool::new(false);
static CTRL_SHIFT_DIRTY: AtomicBool = AtomicBool::new(false);

pub fn install_hook() -> bool {
    unsafe {
        let kbd_hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_keyboard_proc),
            null_mut(),
            0,
        );

        let mouse_hook = SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(low_level_mouse_proc),
            null_mut(),
            0,
        );

        if kbd_hook != null_mut() {
            KBD_HOOK_HANDLE.store(kbd_hook as _, Ordering::SeqCst);
        }
        if mouse_hook != null_mut() {
            MOUSE_HOOK_HANDLE.store(mouse_hook as _, Ordering::SeqCst);
        }

        kbd_hook != null_mut()
    }
}

pub fn uninstall_hook() {
    let kbd = KBD_HOOK_HANDLE.swap(null_mut(), Ordering::SeqCst) as HHOOK;
    if kbd != null_mut() {
        unsafe {
            UnhookWindowsHookEx(kbd);
        }
    }

    let mouse = MOUSE_HOOK_HANDLE.swap(null_mut(), Ordering::SeqCst) as HHOOK;
    if mouse != null_mut() {
        unsafe {
            UnhookWindowsHookEx(mouse);
        }
    }
}

unsafe extern "system" fn low_level_mouse_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let msg = wparam as u32;
        if msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN || msg == WM_MBUTTONDOWN {
            // Người dùng click chuột ra vị trí khác -> kết thúc từ cũ ngay lập tức
            reset_engine();
        }
    }
    CallNextHookEx(null_mut(), code, wparam, lparam)
}

unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    let kbd = *(lparam as *const KBDLLHOOKSTRUCT);

    // 1. Nếu là phím do chính HoyuKey tạo ra -> cho qua ngay lập tức để tránh vòng lặp!
    if kbd.dwExtraInfo == HOYU_MAGIC_EXTRA_INFO {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    // 2. Tự động kiểm tra thay đổi cửa sổ làm việc (chuyển tab trình duyệt, chuyển app)
    let current_fg = GetForegroundWindow() as isize;
    let prev_fg = LAST_FOREGROUND_WINDOW.swap(current_fg, Ordering::SeqCst);
    if prev_fg != 0 && prev_fg != current_fg {
        let mut state = GLOBAL_STATE.lock().unwrap();
        state.engine.on_window_change();
    }

    let vk = kbd.vkCode as u16;
    let vk_idx = (kbd.vkCode as usize) & 0xFF;

    let is_ctrl_key = vk == VK_CONTROL || vk == 0xA2 || vk == 0xA3;
    let is_shift_key = vk == VK_SHIFT || vk == 0xA0 || vk == 0xA1;

    let switch_key = {
        let state = GLOBAL_STATE.lock().unwrap();
        state.config.switch_key
    };

    let is_down = (wparam as u32 == WM_KEYDOWN) || (wparam as u32 == WM_SYSKEYDOWN);
    let is_up = (wparam as u32 == WM_KEYUP) || (wparam as u32 == WM_SYSKEYUP);

    // 3. Xử lý nhả phím (KeyUp):
    if is_up {
        // Nếu dùng phím tắt Ctrl + Shift: chuyển chế độ khi người dùng nhả phím (chuẩn Windows)
        if switch_key == SwitchKey::CtrlShift && (is_ctrl_key || is_shift_key) {
            if CTRL_SHIFT_ACTIVE.swap(false, Ordering::SeqCst) {
                if !CTRL_SHIFT_DIRTY.swap(false, Ordering::SeqCst) {
                    toggle_vietnamese_mode();
                }
            }
        }

        // Chặn KeyUp nếu KeyDown trước đó đã bị bộ gõ chặn (tránh desync trên Chrome/Edge)
        if BLOCKED_VK[vk_idx].swap(false, Ordering::SeqCst) {
            return 1;
        }
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    if !is_down {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    // 4. Kiểm tra phím tắt chuyển đổi chế độ V / E
    match switch_key {
        SwitchKey::CtrlShift => {
            if is_ctrl_key || is_shift_key {
                let ctrl_down = (GetAsyncKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
                let shift_down = (GetAsyncKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;
                if ctrl_down && shift_down {
                    CTRL_SHIFT_ACTIVE.store(true, Ordering::SeqCst);
                    CTRL_SHIFT_DIRTY.store(false, Ordering::SeqCst);
                }
            } else {
                // Người dùng bấm phím thứ 3 trong lúc giữ Ctrl+Shift (ví dụ Ctrl+Shift+S) -> hủy chuyển chế độ
                CTRL_SHIFT_DIRTY.store(true, Ordering::SeqCst);
                CTRL_SHIFT_ACTIVE.store(false, Ordering::SeqCst);
            }
        }
        SwitchKey::AltZ => {
            let is_z = vk == 0x5A || vk == b'Z' as u16 || vk == b'z' as u16;
            let alt_down = (GetAsyncKeyState(VK_MENU as i32) as u16 & 0x8000) != 0;
            if is_z && alt_down {
                toggle_vietnamese_mode();
                BLOCKED_VK[vk_idx].store(true, Ordering::SeqCst);
                return 1;
            }
        }
    }

    // 5. Nếu đang giữ Ctrl, Alt hoặc Windows -> phím tắt app khác (Ctrl+C, Ctrl+A, Alt+Tab...),
    // xóa buffer từ hiện tại và cho phím đi qua tự nhiên
    let ctrl_pressed = (GetKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
    let alt_pressed = (GetKeyState(VK_MENU as i32) as u16 & 0x8000) != 0;
    let win_pressed = (GetKeyState(VK_LWIN as i32) as u16 & 0x8000) != 0
        || (GetKeyState(VK_RWIN as i32) as u16 & 0x8000) != 0;

    if ctrl_pressed || alt_pressed || win_pressed {
        reset_engine();
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    // 6. Xử lý Backspace & các phím điều hướng ngắt từ
    match kbd.vkCode as u16 {
        VK_BACK => {
            let mut state = GLOBAL_STATE.lock().unwrap();
            let bs_action = state.engine.handle_backspace();
            drop(state);

            if let Some(KeyAction::Transform { backspaces, text }) = bs_action {
                send_transform_batch(backspaces, &text);
                return 1;
            } else {
                return CallNextHookEx(null_mut(), code, wparam, lparam);
            }
        }
        VK_RETURN => {
            let auto_dot = {
                let mut state = GLOBAL_STATE.lock().unwrap();
                let should_dot = state.config.auto_period_enter
                    && state.engine.should_auto_period_before_enter();
                state.engine.on_enter();
                should_dot
            };

            if auto_dot {
                send_transform_batch(0, ".");
            }

            return CallNextHookEx(null_mut(), code, wparam, lparam);
        }
        VK_ESCAPE | VK_TAB | VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME
        | VK_END | VK_PRIOR | VK_NEXT => {
            reset_engine();
            return CallNextHookEx(null_mut(), code, wparam, lparam);
        }
        _ => {}
    }

    // 7. Chuyển đổi mã phím thành ký tự Unicode
    if let Some(ch) = vk_to_char(kbd.vkCode, kbd.scanCode) {
        let action = {
            let mut state = GLOBAL_STATE.lock().unwrap();
            let config = state.config.clone();
            state.engine.process_char(ch, &config)
        };

        match action {
            KeyAction::Transform { backspaces, text } => {
                // Ghi nhận phím này bị chặn để chặn luôn KeyUp của nó
                BLOCKED_VK[vk_idx].store(true, Ordering::SeqCst);
                // Gửi Backspace và chuỗi Unicode mới
                send_transform_batch(backspaces, &text);
                return 1;
            }
            KeyAction::PassThrough | KeyAction::Reset | KeyAction::Backspace => {
                return CallNextHookEx(null_mut(), code, wparam, lparam);
            }
        }
    }

    CallNextHookEx(null_mut(), code, wparam, lparam)
}


pub fn toggle_vietnamese_mode() {
    let (hwnd, enabled) = {
        let mut state = GLOBAL_STATE.lock().unwrap();
        state.config.enabled = !state.config.enabled;
        state.config.save();
        state.engine.reset();
        (state.main_hwnd, state.config.enabled)
    };

    if hwnd != 0 {
        unsafe {
            PostMessageW(hwnd as HWND, WM_APP_UPDATE_TRAY, 0, 0);
        }
    }

    crate::ui::notify_mode_changed(enabled);
}

fn reset_engine() {
    let mut state = GLOBAL_STATE.lock().unwrap();
    state.engine.reset();
}

fn vk_to_char(vk: u32, scan_code: u32) -> Option<char> {
    unsafe {
        // 1. Phím chữ cái A-Z (chiếm 99% khi gõ tiếng Việt):
        // Xử lý trực tiếp từ Virtual Key mà KHÔNG gọi ToUnicode làm hỏng dead-key state của Windows/Chrome!
        if (0x41..=0x5A).contains(&vk) {
            let shift = (GetKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;
            let caps = (GetKeyState(0x14) as u16 & 0x0001) != 0; // VK_CAPITAL = 0x14
            let upper = shift ^ caps;
            let ch = ((vk as u8 - b'A') + if upper { b'A' } else { b'a' }) as char;
            return Some(ch);
        }

        // 2. Phím số 0-9 khi không giữ Shift:
        if (0x30..=0x39).contains(&vk) {
            let shift = (GetKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;
            if !shift {
                return Some((vk as u8) as char);
            }
        }

        // 3. Các phím ký tự khác (dấu câu, ký hiệu đặc biệt):
        let mut key_state = [0u8; 256];
        GetKeyboardState(key_state.as_mut_ptr());

        let mut utf16_buf = [0u16; 8];
        let len = ToUnicode(
            vk,
            scan_code,
            key_state.as_ptr(),
            utf16_buf.as_mut_ptr(),
            utf16_buf.len() as i32,
            4, // wFlags = 4: không làm thay đổi trạng thái bàn phím hệ thống
        );

        if len > 0 {
            char::decode_utf16(utf16_buf[..len as usize].iter().copied())
                .next()?
                .ok()
        } else {
            None
        }
    }
}

unsafe fn is_focused_in_web_content() -> bool {
    let mut gui_info: GUITHREADINFO = std::mem::zeroed();
    gui_info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
    if GetGUIThreadInfo(0, &mut gui_info) != 0 && !gui_info.hwndFocus.is_null() {
        let mut class_buf = [0u16; 256];
        let len = GetClassNameW(gui_info.hwndFocus, class_buf.as_mut_ptr(), class_buf.len() as i32);
        if len > 0 {
            let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
            if class_name.contains("RenderWidgetHost") {
                return true;
            }
        }
    }
    false
}

unsafe fn is_foreground_browser_app(apps_list: &str) -> bool {
    let hwnd = GetForegroundWindow();
    if hwnd.is_null() {
        return false;
    }

    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == 0 {
        return false;
    }

    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if process.is_null() {
        return false;
    }

    let mut path_buf = [0u16; 1024];
    let mut size = path_buf.len() as u32;
    let ok = QueryFullProcessImageNameW(
        process,
        0,
        path_buf.as_mut_ptr(),
        &mut size,
    );
    CloseHandle(process);

    if ok != 0 && size > 0 {
        let full_path = String::from_utf16_lossy(&path_buf[..size as usize]);
        let exe_name = std::path::Path::new(&full_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        for app in apps_list.split(',') {
            let app = app.trim().to_ascii_lowercase();
            if !app.is_empty() && (exe_name == app || exe_name == format!("{}.exe", app)) {
                return true;
            }
        }
    }

    false
}

/// Gửi chuỗi Backspace và Unicode Text nguyên tử (Atomic SendInput).
/// Gửi chuẩn Win32 API tương thích hoàn hảo với mọi ứng dụng, trình duyệt Chrome, Edge, Firefox, Word, Excel.
unsafe fn send_transform_batch(backspaces: usize, text: &str) {
    if backspaces == 0 && text.is_empty() {
        return;
    }

    let (is_browser, has_space_before) = {
        let state = GLOBAL_STATE.lock().unwrap();
        if state.config.browser_compatibility && backspaces > 0 {
            (is_foreground_browser_app(&state.config.browser_apps), state.engine.has_space_before)
        } else {
            (false, false)
        }
    };

    // Chỉ nuốt vùng chọn inline autocomplete khi:
    // 1. Ứng dụng là trình duyệt (Chrome, Edge...)
    // 2. Không phải đang gõ trong nội dung trang web (Google Docs, web search, text input...)
    // 3. Chưa có dấu cách phía trước (!has_space_before)
    // 4. Và là biến đổi ở ký tự đầu tiên (backspaces == 1)
    let need_omnibox_fix = is_browser && !has_space_before && backspaces == 1 && !is_focused_in_web_content();
    let total_backspaces = if need_omnibox_fix { backspaces + 1 } else { backspaces };
    let mut inputs = Vec::with_capacity(total_backspaces * 2 + text.encode_utf16().count() * 2);

    // 1. Gửi chính xác số lượng phím Backspace cần xóa
    for _ in 0..total_backspaces {
        let mut down: INPUT = std::mem::zeroed();
        down.r#type = INPUT_KEYBOARD;
        down.Anonymous.ki = KEYBDINPUT {
            wVk: VK_BACK,
            wScan: 0,
            dwFlags: 0,
            time: 0,
            dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
        };
        inputs.push(down);

        let mut up: INPUT = std::mem::zeroed();
        up.r#type = INPUT_KEYBOARD;
        up.Anonymous.ki = KEYBDINPUT {
            wVk: VK_BACK,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
        };
        inputs.push(up);
    }

    // 2. Bơm chuỗi Unicode tiếng Việt mới
    for c in text.encode_utf16() {
        let mut down: INPUT = std::mem::zeroed();
        down.r#type = INPUT_KEYBOARD;
        down.Anonymous.ki = KEYBDINPUT {
            wVk: 0,
            wScan: c,
            dwFlags: KEYEVENTF_UNICODE,
            time: 0,
            dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
        };
        inputs.push(down);

        let mut up: INPUT = std::mem::zeroed();
        up.r#type = INPUT_KEYBOARD;
        up.Anonymous.ki = KEYBDINPUT {
            wVk: 0,
            wScan: c,
            dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
        };
        inputs.push(up);
    }

    // Gửi toàn bộ atomic trong 1 lệnh duy nhất, hệ điều hành xử lý tức thì không có độ trễ
    if !inputs.is_empty() {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
    }
}
