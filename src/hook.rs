use std::ptr::null_mut;
use std::sync::atomic::{AtomicIsize, AtomicPtr, AtomicU16, Ordering};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardState, MapVirtualKeyW, SendInput, ToUnicode, INPUT, INPUT_KEYBOARD,
    KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VK_BACK, VK_CONTROL, VK_DOWN, VK_END,
    VK_ESCAPE, VK_HOME, VK_LEFT, VK_LWIN, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT,
    VK_RWIN, VK_SHIFT, VK_TAB, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
    HHOOK, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN,
    WM_KEYUP, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use crate::config::SwitchKey;
use crate::engine::KeyAction;
use crate::tray::WM_APP_UPDATE_TRAY;
use crate::GLOBAL_STATE;

pub const HOYU_MAGIC_EXTRA_INFO: usize = 0x484F5955; // 'HOYU'

static KBD_HOOK_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(null_mut());
static MOUSE_HOOK_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(null_mut());

// Lưu phím vừa bị chặn KeyDown để chặn luôn KeyUp tương ứng, tránh desync phím trên trình duyệt
static LAST_BLOCKED_VK: AtomicU16 = AtomicU16::new(0);

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
    if is_up {
        let blocked_vk = LAST_BLOCKED_VK.load(Ordering::SeqCst);
        if blocked_vk != 0 && blocked_vk == kbd.vkCode as u16 {
            LAST_BLOCKED_VK.store(0, Ordering::SeqCst);
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
                LAST_BLOCKED_VK.store(kbd.vkCode as u16, Ordering::SeqCst);
                // Gửi toàn bộ Backspace và chuỗi Unicode mới trong MỘT lượt SendInput duy nhất
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

/// Gửi nguyên tử (Atomic) toàn bộ chuỗi Backspace và Unicode Text trong 1 lần SendInput duy nhất.
/// Thiết lập scan code chuẩn cho Backspace (0x0E) để mọi trình duyệt (Chrome, Edge...) nhận diện tức thì.
unsafe fn send_transform_batch(backspaces: usize, text: &str) {
    let backspace_scan = MapVirtualKeyW(VK_BACK as u32, 0) as u16;
    let mut inputs = Vec::with_capacity(backspaces * 2 + text.encode_utf16().count() * 2);

    for _ in 0..backspaces {
        // Backspace Down
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

        // Backspace Up
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

    for c in text.encode_utf16() {
        // Unicode Char Down
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

        // Unicode Char Up
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

    if !inputs.is_empty() {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
    }
}
