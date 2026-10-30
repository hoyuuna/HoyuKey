use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicPtr, Ordering};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardState, MapVirtualKeyW, SendInput, ToUnicode, INPUT, INPUT_KEYBOARD,
    KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VK_BACK, VK_CONTROL, VK_DOWN, VK_END,
    VK_ESCAPE, VK_HOME, VK_LEFT, VK_LWIN, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT,
    VK_RWIN, VK_SHIFT, VK_TAB, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
    HHOOK, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN,
    WM_MBUTTONDOWN, WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_SYSKEYUP,
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
        reset_engine();
    }

    let is_down = (wparam as u32 == WM_KEYDOWN) || (wparam as u32 == WM_SYSKEYDOWN);
    let is_up = (wparam as u32 == WM_KEYUP) || (wparam as u32 == WM_SYSKEYUP);

    // 3. Nếu là sự kiện KeyUp của phím mà KeyDown trước đó đã bị bộ gõ chặn:
    // Chặn luôn KeyUp này để trình duyệt (Chrome, Edge...) không bị desync trạng thái phím!
    let vk_idx = (kbd.vkCode as usize) & 0xFF;
    if is_up {
        if BLOCKED_VK[vk_idx].swap(false, Ordering::SeqCst) {
            return 1;
        }
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    if !is_down {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    // 4. Kiểm tra phím tắt chuyển đổi chế độ V / E (Ctrl+Shift hoặc Alt+Z)
    if check_hotkey(kbd.vkCode as u16) {
        toggle_vietnamese_mode();
        return CallNextHookEx(null_mut(), code, wparam, lparam);
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
        VK_RETURN | VK_ESCAPE | VK_TAB | VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME
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

fn check_hotkey(vk: u16) -> bool {
    let state = GLOBAL_STATE.lock().unwrap();
    let switch_key = state.config.switch_key;
    drop(state);

    unsafe {
        match switch_key {
            SwitchKey::CtrlShift => {
                let ctrl_down = (GetKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
                let shift_down = (GetKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;

                (vk == VK_SHIFT && ctrl_down) || (vk == VK_CONTROL && shift_down)
            }
            SwitchKey::AltZ => {
                let alt_down = (GetKeyState(VK_MENU as i32) as u16 & 0x8000) != 0;
                (vk == 0x5A || vk == b'Z' as u16) && alt_down
            }
        }
    }
}

pub fn toggle_vietnamese_mode() {
    let (hwnd, _enabled) = {
        let mut state = GLOBAL_STATE.lock().unwrap();
        state.config.enabled = !state.config.enabled;
        state.engine.reset();
        (state.main_hwnd, state.config.enabled)
    };

    if hwnd != 0 {
        unsafe {
            PostMessageW(hwnd as HWND, WM_APP_UPDATE_TRAY, 0, 0);
        }
    }
}

fn reset_engine() {
    let mut state = GLOBAL_STATE.lock().unwrap();
    state.engine.reset();
}

fn vk_to_char(vk: u32, scan_code: u32) -> Option<char> {
    unsafe {
        let mut key_state = [0u8; 256];
        GetKeyboardState(key_state.as_mut_ptr());

        let mut utf16_buf = [0u16; 8];
        let len = ToUnicode(
            vk,
            scan_code,
            key_state.as_ptr(),
            utf16_buf.as_mut_ptr(),
            utf16_buf.len() as i32,
            0,
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

/// Gửi chuỗi Backspace và Unicode Text nguyên tử (Atomic SendInput).
/// Sử dụng kỹ thuật Zero-Width Space (U+200B) đệm để phá vỡ vùng chọn gợi ý (inline autocomplete selection)
/// của Google Chrome / Chromium / Edge / Excel, xóa dứt điểm lỗi `ar` -> `aả`, `dd` -> `dđ`, `oo` -> `oô`!
unsafe fn send_transform_batch(backspaces: usize, text: &str) {
    let backspace_scan = MapVirtualKeyW(VK_BACK as u32, 0) as u16;
    let mut inputs = Vec::new();

    if backspaces > 0 {
        // 1. Gửi ký tự Zero-Width Space (U+200B) đệm:
        // Nếu trình duyệt đang có vùng bôi đen gợi ý (Omnibox, Search box), ký tự này sẽ thế chỗ và ngắt ngay selection!
        const ZWS_CHAR: u16 = 0x200B;
        let mut zws_down: INPUT = std::mem::zeroed();
        zws_down.r#type = INPUT_KEYBOARD;
        zws_down.Anonymous.ki = KEYBDINPUT {
            wVk: 0,
            wScan: ZWS_CHAR,
            dwFlags: KEYEVENTF_UNICODE,
            time: 0,
            dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
        };
        inputs.push(zws_down);

        let mut zws_up: INPUT = std::mem::zeroed();
        zws_up.r#type = INPUT_KEYBOARD;
        zws_up.Anonymous.ki = KEYBDINPUT {
            wVk: 0,
            wScan: ZWS_CHAR,
            dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
        };
        inputs.push(zws_up);

        // 2. Gửi (backspaces + 1) phím Backspace:
        // Phím đầu tiên xóa ZWS, các phím sau xóa sạch các ký tự gốc cần biến đổi
        for _ in 0..=backspaces {
            let mut down: INPUT = std::mem::zeroed();
            down.r#type = INPUT_KEYBOARD;
            down.Anonymous.ki = KEYBDINPUT {
                wVk: VK_BACK,
                wScan: backspace_scan,
                dwFlags: 0,
                time: 0,
                dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
            };
            inputs.push(down);

            let mut up: INPUT = std::mem::zeroed();
            up.r#type = INPUT_KEYBOARD;
            up.Anonymous.ki = KEYBDINPUT {
                wVk: VK_BACK,
                wScan: backspace_scan,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
            };
            inputs.push(up);
        }
    }

    // 3. Bơm chuỗi Unicode tiếng Việt mới
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
