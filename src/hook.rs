use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicPtr, Ordering};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, GetKeyboardState, SendInput, ToUnicode, INPUT, INPUT_KEYBOARD,
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
        reset_engine();
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


pub fn toggle_vietnamese_mode() {
    let (hwnd, enabled) = {
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

/// Gửi chuỗi Backspace và Unicode Text nguyên tử (Atomic SendInput).
/// Gửi chuẩn Win32 API tương thích hoàn hảo với mọi ứng dụng, trình duyệt Chrome, Edge, Firefox, Word, Excel.
unsafe fn send_transform_batch(backspaces: usize, text: &str) {
    if backspaces == 0 && text.is_empty() {
        return;
    }

    let mut inputs = Vec::with_capacity(backspaces * 2 + text.encode_utf16().count() * 2);

    // 1. Gửi chính xác số lượng phím Backspace cần xóa
    for _ in 0..backspaces {
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
