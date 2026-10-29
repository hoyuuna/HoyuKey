use std::ptr::null_mut;
use std::sync::atomic::{AtomicPtr, Ordering};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardState, SendInput, ToUnicode, INPUT, INPUT_KEYBOARD, KEYBDINPUT,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VK_BACK, VK_CONTROL, VK_DOWN, VK_END, VK_ESCAPE,
    VK_HOME, VK_LEFT, VK_LWIN, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_RWIN,
    VK_SHIFT, VK_TAB, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK,
    KBDLLHOOKSTRUCT, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

use crate::config::SwitchKey;
use crate::engine::KeyAction;
use crate::tray::WM_APP_UPDATE_TRAY;
use crate::GLOBAL_STATE;

pub const HOYU_MAGIC_EXTRA_INFO: usize = 0x484F5955; // 'HOYU'

static HOOK_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(null_mut());

pub fn install_hook() -> bool {
    unsafe {
        let hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_keyboard_proc),
            null_mut(),
            0,
        );

        if hook != null_mut() {
            HOOK_HANDLE.store(hook as _, Ordering::SeqCst);
            true
        } else {
            false
        }
    }
}

pub fn uninstall_hook() {
    let hook = HOOK_HANDLE.swap(null_mut(), Ordering::SeqCst) as HHOOK;
    if hook != null_mut() {
        unsafe {
            UnhookWindowsHookEx(hook);
        }
    }
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

    let is_down = (wparam as u32 == WM_KEYDOWN) || (wparam as u32 == WM_SYSKEYDOWN);

    // 2. Kiểm tra phím tắt chuyển đổi chế độ V / E (Ctrl+Shift hoặc Alt+Z)
    if is_down && check_hotkey(kbd.vkCode as u16) {
        toggle_vietnamese_mode();
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    if !is_down {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    // 3. Nếu đang giữ Ctrl, Alt hoặc Windows -> đây là phím tắt của app khác (Ctrl+C, Alt+F4...),
    // xóa buffer từ hiện tại và cho phím đi qua tự nhiên
    let ctrl_pressed = (GetKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
    let alt_pressed = (GetKeyState(VK_MENU as i32) as u16 & 0x8000) != 0;
    let win_pressed = (GetKeyState(VK_LWIN as i32) as u16 & 0x8000) != 0
        || (GetKeyState(VK_RWIN as i32) as u16 & 0x8000) != 0;

    if ctrl_pressed || alt_pressed || win_pressed {
        reset_engine();
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    // 4. Xử lý Backspace & các phím điều hướng ngắt từ
    match kbd.vkCode as u16 {
        VK_BACK => {
            let mut state = GLOBAL_STATE.lock().unwrap();
            state.engine.handle_backspace();
            return CallNextHookEx(null_mut(), code, wparam, lparam);
        }
        VK_RETURN | VK_ESCAPE | VK_TAB | VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME
        | VK_END | VK_PRIOR | VK_NEXT => {
            reset_engine();
            return CallNextHookEx(null_mut(), code, wparam, lparam);
        }
        _ => {}
    }

    // 5. Chuyển đổi mã phím thành ký tự Unicode
    if let Some(ch) = vk_to_char(kbd.vkCode, kbd.scanCode) {
        let action = {
            let mut state = GLOBAL_STATE.lock().unwrap();
            let config = state.config.clone();
            state.engine.process_char(ch, &config)
        };

        match action {
            KeyAction::Transform { backspaces, text } => {
                // Xóa ký tự cũ trên màn hình
                send_backspaces(backspaces);
                // Gõ từ tiếng Việt mới
                send_unicode_str(&text);
                // Chặn phím gốc để không bị lặp chữ
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
        if GetKeyboardState(key_state.as_mut_ptr()) == 0 {
            return None;
        }

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

unsafe fn send_backspaces(count: usize) {
    if count == 0 {
        return;
    }

    let mut inputs = Vec::with_capacity(count * 2);
    for _ in 0..count {
        // Key down
        let mut input_down: INPUT = std::mem::zeroed();
        input_down.r#type = INPUT_KEYBOARD;
        input_down.Anonymous.ki = KEYBDINPUT {
            wVk: VK_BACK,
            wScan: 0,
            dwFlags: 0,
            time: 0,
            dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
        };
        inputs.push(input_down);

        // Key up
        let mut input_up: INPUT = std::mem::zeroed();
        input_up.r#type = INPUT_KEYBOARD;
        input_up.Anonymous.ki = KEYBDINPUT {
            wVk: VK_BACK,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
        };
        inputs.push(input_up);
    }

    SendInput(
        inputs.len() as u32,
        inputs.as_ptr(),
        std::mem::size_of::<INPUT>() as i32,
    );
}

unsafe fn send_unicode_str(s: &str) {
    let mut inputs = Vec::new();
    for c in s.encode_utf16() {
        // Key down
        let mut input_down: INPUT = std::mem::zeroed();
        input_down.r#type = INPUT_KEYBOARD;
        input_down.Anonymous.ki = KEYBDINPUT {
            wVk: 0,
            wScan: c,
            dwFlags: KEYEVENTF_UNICODE,
            time: 0,
            dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
        };
        inputs.push(input_down);

        // Key up
        let mut input_up: INPUT = std::mem::zeroed();
        input_up.r#type = INPUT_KEYBOARD;
        input_up.Anonymous.ki = KEYBDINPUT {
            wVk: 0,
            wScan: c,
            dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
        };
        inputs.push(input_up);
    }

    if !inputs.is_empty() {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
    }
}
