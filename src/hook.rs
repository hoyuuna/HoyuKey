use std::collections::HashMap;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicPtr, AtomicU32, Ordering};
use std::sync::Mutex;
use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, GetKeyboardState, SendInput, ToUnicode, INPUT, INPUT_KEYBOARD,
    KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END,
    VK_ESCAPE, VK_HOME, VK_LEFT, VK_LWIN, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT,
    VK_RWIN, VK_SHIFT, VK_TAB, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, GetWindowThreadProcessId,
    PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx, EVENT_SYSTEM_FOREGROUND,
    HHOOK, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WINEVENT_OUTOFCONTEXT, WM_KEYDOWN,
    WM_KEYUP, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

#[link(name = "user32")]
extern "system" {
    fn SetWinEventHook(
        event_min: u32,
        event_max: u32,
        hmod_win_event_proc: *mut std::ffi::c_void,
        pfn_win_event_proc: Option<
            unsafe extern "system" fn(
                h_win_event_hook: *mut std::ffi::c_void,
                event: u32,
                hwnd: HWND,
                id_object: i32,
                id_child: i32,
                id_event_thread: u32,
                dwms_event_time: u32,
            ),
        >,
        id_process: u32,
        id_thread: u32,
        dw_flags: u32,
    ) -> *mut std::ffi::c_void;

    fn UnhookWinEvent(h_win_event_hook: *mut std::ffi::c_void) -> i32;
}

use crate::config::SwitchKey;
use crate::engine::KeyAction;
use crate::tray::WM_APP_UPDATE_TRAY;
use crate::GLOBAL_STATE;

pub const HOYU_MAGIC_EXTRA_INFO: usize = 0x484F5955; // 'HOYU'

static KBD_HOOK_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(null_mut());
static MOUSE_HOOK_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(null_mut());
static WIN_EVENT_HOOK: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(null_mut());

/// Bảng ghi nhớ trạng thái Tiếng Việt (E / V) theo từng tiến trình ứng dụng
static APP_INPUT_STATES: Mutex<Option<HashMap<String, bool>>> = Mutex::new(None);

// Theo dõi các phím bị chặn KeyDown để chặn luôn KeyUp tương ứng, tránh desync phím khi gõ nhanh trên trình duyệt
const INIT_BOOL: AtomicBool = AtomicBool::new(false);
static BLOCKED_VK: [AtomicBool; 256] = [INIT_BOOL; 256];

// Theo dõi cửa sổ đang active (foreground), đổi cửa sổ -> tự động reset buffer
static LAST_FOREGROUND_WINDOW: AtomicIsize = AtomicIsize::new(0);

// Theo dõi trạng thái phím tắt Ctrl + Shift (chuyển chế độ khi nhả phím)
static CTRL_IS_DOWN: AtomicBool = AtomicBool::new(false);
static SHIFT_IS_DOWN: AtomicBool = AtomicBool::new(false);
static CTRL_SHIFT_READY: AtomicBool = AtomicBool::new(false);

// Theo dõi phím đang giữ (auto-repeat) để tối ưu và tránh nghẽn luồng phím Windows
static LAST_PHYSICAL_VK: AtomicU32 = AtomicU32::new(0);
static KEY_REPEAT_COUNT: AtomicU32 = AtomicU32::new(0);

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

        let win_event = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            null_mut(),
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        );

        if kbd_hook != null_mut() {
            KBD_HOOK_HANDLE.store(kbd_hook as _, Ordering::SeqCst);
        }
        if mouse_hook != null_mut() {
            MOUSE_HOOK_HANDLE.store(mouse_hook as _, Ordering::SeqCst);
        }
        if win_event != null_mut() {
            WIN_EVENT_HOOK.store(win_event, Ordering::SeqCst);
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

    let win_event = WIN_EVENT_HOOK.swap(null_mut(), Ordering::SeqCst);
    if win_event != null_mut() {
        unsafe {
            UnhookWinEvent(win_event);
        }
    }
}

/// Callback WinEvent bắt sự kiện chuyển cửa sổ Foreground ngay lập tức khi Alt+Tab hoặc click chuột
unsafe extern "system" fn win_event_proc(
    _h_win_event_hook: *mut std::ffi::c_void,
    event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    if event == EVENT_SYSTEM_FOREGROUND && !hwnd.is_null() {
        on_focus_window_changed(hwnd);
    }
}

/// Lấy tên file thực thi (.exe) từ HWND
pub fn get_process_name_by_hwnd(hwnd: HWND) -> Option<String> {
    if hwnd.is_null() {
        return None;
    }
    unsafe {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }

        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }

        let mut path_buf = [0u16; 1024];
        let mut size = path_buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, 0, path_buf.as_mut_ptr(), &mut size);
        CloseHandle(process);

        if ok != 0 && size > 0 {
            let full_path = String::from_utf16_lossy(&path_buf[..size as usize]);
            let exe_name = std::path::Path::new(&full_path)
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_ascii_lowercase());
            return exe_name;
        }
    }
    None
}

/// Xử lý khi cửa sổ làm việc thay đổi (chuyển app)
pub fn on_focus_window_changed(new_hwnd: HWND) {
    if new_hwnd.is_null() {
        return;
    }

    let (remember_enabled, current_enabled, main_hwnd) = {
        let state = match GLOBAL_STATE.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        (
            state.config.remember_app_state,
            state.config.enabled,
            state.main_hwnd,
        )
    };

    if !remember_enabled {
        return;
    }

    let app_name = match get_process_name_by_hwnd(new_hwnd) {
        Some(name) => name,
        None => return,
    };

    // Bỏ qua chính cửa sổ HoyuKey dashboard
    if app_name == "hoyukey.exe" {
        return;
    }

    let mut map_lock = APP_INPUT_STATES.lock().unwrap();
    let map = map_lock.get_or_insert_with(HashMap::new);

    if let Some(&saved_mode) = map.get(&app_name) {
        if saved_mode != current_enabled {
            {
                let mut state = GLOBAL_STATE.lock().unwrap();
                state.config.enabled = saved_mode;
                state.engine.reset();
            }

            if main_hwnd != 0 {
                unsafe {
                    PostMessageW(main_hwnd as HWND, WM_APP_UPDATE_TRAY, 0, 0);
                }
            }
            crate::ui::notify_mode_changed(saved_mode);
        }
    } else {
        // Lần đầu vào app này: ghi nhớ trạng thái hiện tại
        map.insert(app_name, current_enabled);
    }
}

/// Cập nhật trạng thái E / V mới cho ứng dụng đang active
pub fn record_current_app_mode(new_enabled: bool) {
    let remember_enabled = {
        let state = match GLOBAL_STATE.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        state.config.remember_app_state
    };

    if !remember_enabled {
        return;
    }

    unsafe {
        let fg = GetForegroundWindow();
        if let Some(app_name) = get_process_name_by_hwnd(fg) {
            if app_name != "hoyukey.exe" {
                let mut map_lock = APP_INPUT_STATES.lock().unwrap();
                let map = map_lock.get_or_insert_with(HashMap::new);
                map.insert(app_name, new_enabled);
            }
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
            // Khi click chuột: người dùng chuyển con trỏ sang vị trí mới,
            // reset sạch bộ đệm để dynamic caret nhận diện chính xác từ tại vị trí chuột mới
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

    let vk = kbd.vkCode as u16;
    let vk_idx = (kbd.vkCode as usize) & 0xFF;

    let is_ctrl_key = vk == VK_CONTROL || vk == 0xA2 || vk == 0xA3;
    let is_shift_key = vk == VK_SHIFT || vk == 0xA0 || vk == 0xA1;
    let is_z = vk == 0x5A || vk == b'Z' as u16 || vk == b'z' as u16;

    let is_down = (wparam as u32 == WM_KEYDOWN) || (wparam as u32 == WM_SYSKEYDOWN);
    let is_up = (wparam as u32 == WM_KEYUP) || (wparam as u32 == WM_SYSKEYUP);

    // 2. Xử lý nhả phím (KeyUp):
    if is_up {
        if LAST_PHYSICAL_VK.load(Ordering::Relaxed) == kbd.vkCode {
            LAST_PHYSICAL_VK.store(0, Ordering::Relaxed);
            KEY_REPEAT_COUNT.store(0, Ordering::Relaxed);
        }

        // Nếu dùng phím tắt Ctrl + Shift: chuyển chế độ khi người dùng nhả phím (chuẩn Windows)
        if is_ctrl_key || is_shift_key {
            let switch_key = {
                let state = GLOBAL_STATE.lock().unwrap();
                state.config.switch_key
            };
            if switch_key == SwitchKey::CtrlShift {
                if is_ctrl_key {
                    CTRL_IS_DOWN.store(false, Ordering::SeqCst);
                    if CTRL_SHIFT_READY.swap(false, Ordering::SeqCst) {
                        toggle_vietnamese_mode();
                    }
                } else if is_shift_key {
                    SHIFT_IS_DOWN.store(false, Ordering::SeqCst);
                    if CTRL_SHIFT_READY.swap(false, Ordering::SeqCst) {
                        toggle_vietnamese_mode();
                    }
                }
            }
        }

        // Chặn KeyUp nếu KeyDown tương ứng đã bị bộ gõ chặn (tránh desync trên Chrome/Edge)
        if BLOCKED_VK[vk_idx].swap(false, Ordering::SeqCst) {
            return 1;
        }
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    if !is_down {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    // 3. Theo dõi auto-repeat khi người dùng giữ phím
    let repeat_count = if LAST_PHYSICAL_VK.load(Ordering::Relaxed) == kbd.vkCode {
        KEY_REPEAT_COUNT.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        LAST_PHYSICAL_VK.store(kbd.vkCode, Ordering::Relaxed);
        KEY_REPEAT_COUNT.store(0, Ordering::Relaxed);
        0
    };

    // Tự động kiểm tra thay đổi cửa sổ làm việc (chuyển tab trình duyệt, chuyển app)
    // Chỉ kiểm tra ở phím đầu tiên (không kiểm tra khi đang giữ lặp lại phím để tiết kiệm CPU)
    if repeat_count == 0 {
        let current_fg = GetForegroundWindow() as isize;
        let prev_fg = LAST_FOREGROUND_WINDOW.swap(current_fg, Ordering::SeqCst);
        if prev_fg != 0 && prev_fg != current_fg {
            {
                let mut state = GLOBAL_STATE.lock().unwrap();
                state.engine.on_window_change();
            }
            on_focus_window_changed(current_fg as HWND);
        }
    }

    // 4. Kiểm tra phím tắt chuyển đổi chế độ V / E
    if is_ctrl_key || is_shift_key || is_z {
        let switch_key = {
            let state = GLOBAL_STATE.lock().unwrap();
            state.config.switch_key
        };
        match switch_key {
            SwitchKey::CtrlShift => {
                if is_ctrl_key {
                    CTRL_IS_DOWN.store(true, Ordering::SeqCst);
                    if SHIFT_IS_DOWN.load(Ordering::SeqCst) {
                        CTRL_SHIFT_READY.store(true, Ordering::SeqCst);
                    }
                } else if is_shift_key {
                    SHIFT_IS_DOWN.store(true, Ordering::SeqCst);
                    if CTRL_IS_DOWN.load(Ordering::SeqCst) {
                        CTRL_SHIFT_READY.store(true, Ordering::SeqCst);
                    }
                } else {
                    // Phím thứ 3 được nhấn -> hủy chuyển chế độ
                    CTRL_SHIFT_READY.store(false, Ordering::SeqCst);
                }
            }
            SwitchKey::AltZ => {
                if is_z {
                    let alt_down = (GetAsyncKeyState(VK_MENU as i32) as u16 & 0x8000) != 0;
                    if alt_down {
                        toggle_vietnamese_mode();
                        BLOCKED_VK[vk_idx].store(true, Ordering::SeqCst);
                        return 1;
                    }
                }
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
        BLOCKED_VK[vk_idx].store(false, Ordering::SeqCst);
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    // 6. Xử lý Backspace & các phím điều hướng ngắt từ
    match kbd.vkCode as u16 {
        VK_BACK => {
            BLOCKED_VK[vk_idx].store(false, Ordering::SeqCst);
            let mut state = GLOBAL_STATE.lock().unwrap();
            let simple_telex = state.config.simple_telex;
            let bs_action = state.engine.handle_backspace(simple_telex);
            drop(state);

            if let Some(KeyAction::Transform { backspaces, text }) = bs_action {
                BLOCKED_VK[vk_idx].store(true, Ordering::SeqCst);
                send_transform_batch(backspaces, &text);
                return 1;
            } else {
                return CallNextHookEx(null_mut(), code, wparam, lparam);
            }
        }
        VK_RETURN => {
            BLOCKED_VK[vk_idx].store(false, Ordering::SeqCst);
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
        VK_LEFT | VK_RIGHT => {
            BLOCKED_VK[vk_idx].store(false, Ordering::SeqCst);
            let dynamic_caret = {
                let state = GLOBAL_STATE.lock().unwrap();
                state.config.dynamic_caret
            };
            if dynamic_caret {
                reset_engine();
            } else {
                let mut state = GLOBAL_STATE.lock().unwrap();
                state.engine.on_arrow_left();
            }
            return CallNextHookEx(null_mut(), code, wparam, lparam);
        }
        VK_ESCAPE | VK_TAB | VK_UP | VK_DOWN | VK_HOME
        | VK_END | VK_PRIOR | VK_NEXT => {
            reset_engine();
            BLOCKED_VK[vk_idx].store(false, Ordering::SeqCst);
            return CallNextHookEx(null_mut(), code, wparam, lparam);
        }
        _ => {}
    }

    // 7. Chuyển đổi mã phím thành ký tự Unicode
    if let Some(ch) = vk_to_char(kbd.vkCode, kbd.scanCode) {
        // Kiểm tra tính năng Sửa dấu tự do tại con trỏ (UI Automation Dynamic Caret)
        // Khi buffer đang rỗng và người dùng gõ một phím dấu/mũ/sửa tiếng Việt:
        if repeat_count == 0 {
            let (enabled, dynamic_caret, is_telex, modern_tone, free_marking, smart_english, simple_telex, is_empty) = {
                let state = GLOBAL_STATE.lock().unwrap();
                (
                    state.config.enabled,
                    state.config.dynamic_caret,
                    state.config.input_method == crate::config::InputMethod::Telex,
                    state.config.modern_tone,
                    state.config.free_marking,
                    state.config.smart_english,
                    state.config.simple_telex,
                    state.engine.current_word.is_empty(),
                )
            };

            if enabled && dynamic_caret && is_empty {
                let is_candidate_key = if is_telex {
                    matches!(ch, 's' | 'f' | 'r' | 'x' | 'j' | 'z' | 'a' | 'e' | 'o' | 'w' | 'd' |
                                 'S' | 'F' | 'R' | 'X' | 'J' | 'Z' | 'A' | 'E' | 'O' | 'W' | 'D')
                } else {
                    matches!(ch, '1'..='9' | '0')
                };

                if is_candidate_key {
                    if let Some(ctx) = crate::automation::get_caret_word_context() {
                        let target_word = &ctx.full_word;

                        let transformed = if is_telex {
                            crate::engine::telex::transform_telex(
                                target_word,
                                ch,
                                modern_tone,
                                free_marking,
                                smart_english,
                                simple_telex,
                            )
                        } else {
                            crate::engine::vni::transform_vni(
                                target_word,
                                ch,
                                modern_tone,
                                free_marking,
                                smart_english,
                            )
                        };

                        if let Some(new_word) = transformed {
                            if new_word != *target_word {
                                let deletes = ctx.post_word.chars().count();
                                let backspaces = ctx.pre_word.chars().count();

                                {
                                    let mut state = GLOBAL_STATE.lock().unwrap();
                                    state.engine.current_word = new_word.clone();
                                    state.engine.raw_keys = new_word.clone();
                                    state.engine.has_transformed = true;
                                }

                                BLOCKED_VK[vk_idx].store(true, Ordering::SeqCst);
                                send_replace_batch(deletes, backspaces, &new_word);
                                return 1;
                            }
                        }
                    }
                }
            }
        }

        // Khi người dùng giữ phím (auto-repeat >= 3):
        // Tuyệt đối không can thiệp biến đổi để tránh nghẽn luồng phím Windows & lag máy
        let action = if repeat_count >= 3 {
            let mut state = GLOBAL_STATE.lock().unwrap();
            state.engine.current_word.push(ch);
            KeyAction::PassThrough
        } else {
            let mut state = GLOBAL_STATE.lock().unwrap();
            let state = &mut *state;
            state.engine.process_char(ch, &state.config)
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
                BLOCKED_VK[vk_idx].store(false, Ordering::SeqCst);
                return CallNextHookEx(null_mut(), code, wparam, lparam);
            }
        }
    }

    BLOCKED_VK[vk_idx].store(false, Ordering::SeqCst);
    CallNextHookEx(null_mut(), code, wparam, lparam)
}


pub fn toggle_vietnamese_mode() {
    let (hwnd, enabled, notify_toggle) = {
        let mut state = GLOBAL_STATE.lock().unwrap();
        state.config.enabled = !state.config.enabled;
        state.config.save();
        state.engine.reset();
        (state.main_hwnd, state.config.enabled, state.config.notify_on_toggle)
    };

    // Ghi nhớ trạng thái mới cho ứng dụng hiện tại nếu tính năng đang bật
    record_current_app_mode(enabled);

    if hwnd != 0 {
        unsafe {
            PostMessageW(hwnd as HWND, WM_APP_UPDATE_TRAY, 0, 0);
        }
    }

    if notify_toggle {
        let msg = if enabled {
            "Đã bật Tiếng Việt"
        } else {
            "Đã tắt Tiếng Việt"
        };
        crate::show_notification("HoyuKey", msg);
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



/// Gửi chuỗi Delete, Backspace và Unicode Text nguyên tử (Atomic SendInput).
/// Hỗ trợ sửa dấu tại con trỏ (xóa đuôi sau con trỏ bằng Delete và xóa đầu trước con trỏ bằng Backspace).
unsafe fn send_replace_batch(deletes: usize, backspaces: usize, text: &str) {
    if deletes == 0 && backspaces == 0 && text.is_empty() {
        return;
    }

    let mut inputs = Vec::with_capacity((deletes + backspaces) * 2 + text.encode_utf16().count() * 2);

    // 1. Gửi phím Delete để xóa phần đuôi phía sau con trỏ (nếu con trỏ đang ở giữa từ)
    for _ in 0..deletes {
        let mut down: INPUT = std::mem::zeroed();
        down.r#type = INPUT_KEYBOARD;
        down.Anonymous.ki = KEYBDINPUT {
            wVk: VK_DELETE,
            wScan: 0,
            dwFlags: 0,
            time: 0,
            dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
        };
        inputs.push(down);

        let mut up: INPUT = std::mem::zeroed();
        up.r#type = INPUT_KEYBOARD;
        up.Anonymous.ki = KEYBDINPUT {
            wVk: VK_DELETE,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
        };
        inputs.push(up);
    }

    // 2. Gửi phím Backspace để xóa phần đầu phía trước con trỏ
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

    if !inputs.is_empty() {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
    }
}

/// Gửi chuỗi Backspace và Unicode Text nguyên tử (Atomic SendInput).
/// Gửi chuẩn Win32 API tương thích hoàn hảo với mọi ứng dụng, trình duyệt Chrome, Edge, Firefox, Word, Excel.
unsafe fn send_transform_batch(backspaces: usize, text: &str) {
    send_replace_batch(0, backspaces, text);
}
