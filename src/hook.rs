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
    CallNextHookEx, GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId,
    PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx, EVENT_SYSTEM_FOREGROUND,
    HHOOK, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WINEVENT_OUTOFCONTEXT, WM_KEYDOWN,
    WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
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

// Caching trạng thái ứng dụng trình duyệt: 0ms latency, không mở process handle trong hook
static IS_CURRENT_APP_BROWSER: AtomicBool = AtomicBool::new(false);

pub fn install_hook() -> bool {
    crate::automation::init_background_caret_loader();
    init_caret_replacement_worker();
    unsafe {
        let fg = GetForegroundWindow();
        if !fg.is_null() {
            on_focus_window_changed(fg);
        }

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

    let (remember_enabled, current_enabled, main_hwnd, browser_compat, browser_apps) = {
        let state = match GLOBAL_STATE.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        (
            state.config.remember_app_state,
            state.config.enabled,
            state.main_hwnd,
            state.config.browser_compatibility,
            state.config.browser_apps.clone(),
        )
    };

    let app_name = match get_process_name_by_hwnd(new_hwnd) {
        Some(name) => name,
        None => return,
    };

    let is_browser = if browser_compat {
        browser_apps.split(',').any(|app| {
            let app = app.trim().to_ascii_lowercase();
            !app.is_empty() && (app_name == app || app_name == format!("{}.exe", app))
        })
    } else {
        false
    };
    IS_CURRENT_APP_BROWSER.store(is_browser, Ordering::Relaxed);

    // Bỏ qua chính cửa sổ HoyuKey dashboard
    if app_name == "hoyukey.exe" {
        return;
    }

    if !remember_enabled {
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
        if msg == WM_LBUTTONUP || msg == WM_RBUTTONUP {
            // Khi nhả chuột: con trỏ đã đáp xuống vị trí mới -> yêu cầu nạp nền từ tại con trỏ
            if let Ok(mut state) = GLOBAL_STATE.lock() {
                state.engine.on_mouse_click();
            }
            crate::automation::request_caret_refresh();
        } else if msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN || msg == WM_MBUTTONDOWN {
            crate::automation::clear_cached_caret_word();
            if let Ok(mut state) = GLOBAL_STATE.lock() {
                state.engine.on_mouse_click();
            }
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

    // Bất kỳ phím vật lý nào được bấm cũng lập tức hủy bỏ các tác vụ Cách 2 đang chờ (nếu có)
    CARET_REPLACEMENT_SEQ.fetch_add(1, Ordering::Relaxed);

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
            let (bs_action, has_space_before, browser_compat, browser_apps) = {
                let mut state = GLOBAL_STATE.lock().unwrap();
                let simple_telex = state.config.simple_telex;
                let browser_compat = state.config.browser_compatibility;
                let browser_apps = state.config.browser_apps.clone();
                let bs_action = state.engine.handle_backspace(simple_telex);
                let has_space_before = state.engine.has_space_before;
                (bs_action, has_space_before, browser_compat, browser_apps)
            };

            if let Some(KeyAction::Transform { backspaces, text }) = bs_action {
                BLOCKED_VK[vk_idx].store(true, Ordering::SeqCst);
                send_transform_batch(backspaces, &text, has_space_before, browser_compat, &browser_apps);
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
                send_replace_batch(0, 0, ".");
            }

            return CallNextHookEx(null_mut(), code, wparam, lparam);
        }
        VK_LEFT => {
            BLOCKED_VK[vk_idx].store(false, Ordering::SeqCst);
            if let Ok(mut state) = GLOBAL_STATE.lock() {
                state.engine.on_arrow_left();
                if state.engine.current_word.is_empty() {
                    crate::automation::request_caret_refresh();
                }
            }
            return CallNextHookEx(null_mut(), code, wparam, lparam);
        }
        VK_RIGHT => {
            BLOCKED_VK[vk_idx].store(false, Ordering::SeqCst);
            if let Ok(mut state) = GLOBAL_STATE.lock() {
                state.engine.on_arrow_right();
                if state.engine.current_word.is_empty() {
                    crate::automation::request_caret_refresh();
                }
            }
            return CallNextHookEx(null_mut(), code, wparam, lparam);
        }
        VK_UP | VK_DOWN => {
            reset_engine();
            if let Ok(mut state) = GLOBAL_STATE.lock() {
                state.engine.caret_needs_check = true;
            }
            crate::automation::request_caret_refresh();
            BLOCKED_VK[vk_idx].store(false, Ordering::SeqCst);
            return CallNextHookEx(null_mut(), code, wparam, lparam);
        }
        VK_ESCAPE | VK_TAB | VK_HOME
        | VK_END | VK_PRIOR | VK_NEXT => {
            reset_engine();
            if matches!(kbd.vkCode as u16, VK_HOME | VK_END | VK_PRIOR | VK_NEXT) {
                if let Ok(mut state) = GLOBAL_STATE.lock() {
                    state.engine.caret_needs_check = true;
                }
            }
            crate::automation::clear_cached_caret_word();
            BLOCKED_VK[vk_idx].store(false, Ordering::SeqCst);
            return CallNextHookEx(null_mut(), code, wparam, lparam);
        }
        _ => {}
    }

    // 7. Chuyển đổi mã phím thành ký tự Unicode
    if let Some(ch) = vk_to_char(kbd.vkCode, kbd.scanCode) {
        // Kiểm tra nạp nền con trỏ (Background Caret Context - 0ms latency, không block hook)
        if repeat_count == 0 {
            let (
                enabled,
                dynamic_caret,
                is_telex,
                modern_tone,
                free_marking,
                smart_english,
                simple_telex,
                is_empty,
                caret_needs_check,
                is_after_whitespace,
                at_start_of_line,
            ) = {
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
                    state.engine.caret_needs_check,
                    state.engine.last_char_typed.map_or(false, |c| c.is_whitespace()),
                    state.engine.at_start_of_line,
                )
            };

            // Kích hoạt sửa dấu tại con trỏ CHỈ KHI người dùng vừa click chuột / điều hướng con trỏ (caret_needs_check)
            if enabled && dynamic_caret && is_empty && caret_needs_check && !at_start_of_line && !is_after_whitespace {
                // CHỈ các phím dấu thanh (s, f, r, x, j, z) mới là candidate!
                // TUYỆT ĐỐI không bao gồm w, a, e, o, d vì đó là các chữ cái bắt đầu từ thông thường.
                let is_candidate_key = if is_telex {
                    matches!(ch, 's' | 'f' | 'r' | 'x' | 'j' | 'z' |
                                 'S' | 'F' | 'R' | 'X' | 'J' | 'Z')
                } else {
                    matches!(ch, '1'..='9' | '0')
                };

                if is_candidate_key {
                    // CHỈ đọc từ cache nền (0ms latency, TUYỆT ĐỐI không gọi COM UIA đồng bộ trong hook!)
                    let caret_word_ctx = crate::automation::get_cached_caret_word();

                    if let Some(ctx) = caret_word_ctx {
                        let target_word = &ctx.full_word;

                        let transformed = if is_telex {
                            crate::engine::telex::transform_telex(
                                target_word,
                                ch,
                                modern_tone,
                                free_marking,
                                smart_english,
                                simple_telex,
                                None,
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
                            if new_word != *target_word && crate::automation::is_modification_behind_caret(&ctx.post_word, &new_word) {
                                let deletes = ctx.post_word.chars().count();
                                let backspaces = ctx.pre_word.chars().count();

                                {
                                    let mut state = GLOBAL_STATE.lock().unwrap();
                                    state.engine.current_word = new_word.clone();
                                    state.engine.raw_keys = new_word.clone();
                                    state.engine.has_transformed = true;
                                    state.engine.caret_offset = Some(new_word.chars().count() - deletes);
                                    state.engine.caret_needs_check = false;
                                }

                                crate::automation::clear_cached_caret_word();
                                BLOCKED_VK[vk_idx].store(true, Ordering::SeqCst);
                                send_replace_batch(deletes, backspaces, &new_word);
                                return 1;
                            }
                        }
                    } else {
                        // Cách 2: Gửi yêu cầu tới dedicated worker thread với debounce 75ms (0ms hook latency, không spawn thread)
                        let seq = CARET_REPLACEMENT_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
                        if let Ok(mut lock) = CARET_REPLACE_QUEUE.lock() {
                            *lock = Some(CaretReplaceRequest {
                                seq,
                                ch,
                                is_telex,
                                modern_tone,
                                free_marking,
                                smart_english,
                                simple_telex,
                            });
                            CARET_REPLACE_CONDVAR.notify_one();
                        }
                    }
                }
            }
            if let Ok(mut state) = GLOBAL_STATE.lock() {
                state.engine.caret_needs_check = false;
            }
        }
        crate::automation::clear_cached_caret_word();

        // Xử lý biến đổi ký tự thông qua engine (chỉ lock 1 lần duy nhất, lấy kết quả và mở khóa ngay)
        let (action, has_space_before, browser_compatibility, browser_apps) = {
            let mut state = GLOBAL_STATE.lock().unwrap();
            let state = &mut *state;
            let action = if repeat_count >= 3 {
                state.engine.current_word.push(ch);
                KeyAction::PassThrough
            } else {
                state.engine.process_char(ch, &state.config)
            };
            let has_space_before = state.engine.has_space_before;
            let browser_compatibility = state.config.browser_compatibility;
            let browser_apps = state.config.browser_apps.clone();
            (action, has_space_before, browser_compatibility, browser_apps)
        };

        match action {
            KeyAction::Transform { backspaces, text } => {
                BLOCKED_VK[vk_idx].store(true, Ordering::SeqCst);
                send_transform_batch(backspaces, &text, has_space_before, browser_compatibility, &browser_apps);
                return 1;
            }
            KeyAction::TransformCaret { deletes, backspaces, text } => {
                BLOCKED_VK[vk_idx].store(true, Ordering::SeqCst);
                send_replace_batch(deletes, backspaces, &text);
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

    // 4. Nếu trước đó đã xóa phần đuôi bằng Delete (con trỏ ở giữa từ):
    // Gửi lại đúng số phím Left Arrow để đưa con trỏ quay về đúng vị trí giữa từ ban đầu!
    for _ in 0..deletes {
        let mut down: INPUT = std::mem::zeroed();
        down.r#type = INPUT_KEYBOARD;
        down.Anonymous.ki = KEYBDINPUT {
            wVk: VK_LEFT,
            wScan: 0,
            dwFlags: 0,
            time: 0,
            dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
        };
        inputs.push(down);

        let mut up: INPUT = std::mem::zeroed();
        up.r#type = INPUT_KEYBOARD;
        up.Anonymous.ki = KEYBDINPUT {
            wVk: VK_LEFT,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
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

use crate::automation::{GetGUIThreadInfo, GUITHREADINFO};

static LAST_CHECKED_HWND: AtomicIsize = AtomicIsize::new(0);
static LAST_CHECKED_IS_BROWSER: AtomicBool = AtomicBool::new(false);

unsafe fn is_foreground_browser_app(browser_apps: &str) -> bool {
    let hwnd = GetForegroundWindow();
    if hwnd.is_null() {
        return false;
    }

    let last = LAST_CHECKED_HWND.load(Ordering::Relaxed);
    if last == hwnd as isize {
        return LAST_CHECKED_IS_BROWSER.load(Ordering::Relaxed);
    }

    let mut pid: u32 = 0;
    let tid = GetWindowThreadProcessId(hwnd, &mut pid);
    if tid == 0 || pid == 0 {
        return false;
    }

    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if process.is_null() {
        return false;
    }

    let mut path_buf = [0u16; 1024];
    let mut size = path_buf.len() as u32;
    let ok = QueryFullProcessImageNameW(process, 0, path_buf.as_mut_ptr(), &mut size);
    CloseHandle(process);

    let is_browser = if ok != 0 && size > 0 {
        let full_path = String::from_utf16_lossy(&path_buf[..size as usize]);
        let exe_name = std::path::Path::new(&full_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        browser_apps.split(',').any(|app| {
            let app = app.trim().to_ascii_lowercase();
            !app.is_empty() && (exe_name == app || exe_name == format!("{}.exe", app))
        })
    } else {
        false
    };

    LAST_CHECKED_HWND.store(hwnd as isize, Ordering::Relaxed);
    LAST_CHECKED_IS_BROWSER.store(is_browser, Ordering::Relaxed);
    is_browser
}

unsafe fn is_focused_in_web_content() -> bool {
    let hwnd = GetForegroundWindow();
    if hwnd.is_null() {
        return false;
    }

    let mut pid: u32 = 0;
    let tid = GetWindowThreadProcessId(hwnd, &mut pid);
    if tid == 0 {
        return false;
    }

    let mut gui_info: GUITHREADINFO = std::mem::zeroed();
    gui_info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;

    if GetGUIThreadInfo(tid, &mut gui_info) != 0 && !gui_info.hwndFocus.is_null() {
        let mut class_buf = [0u16; 256];
        let len = GetClassNameW(gui_info.hwndFocus, class_buf.as_mut_ptr(), class_buf.len() as i32);
        if len > 0 {
            let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
            // Trên Chrome, Edge, Brave, Cốc Cốc, Opera:
            // Cửa sổ hiển thị nội dung trang web luôn có class name chứa "RenderWidgetHost"
            // (ví dụ: Chrome_RenderWidgetHostHWND)
            if class_name.contains("RenderWidgetHost") {
                return true;
            }
        }
    }
    false
}

/// Gửi chuỗi Backspace và Unicode Text nguyên tử (Atomic SendInput).
/// Gửi chuẩn Win32 API tương thích hoàn hảo với mọi ứng dụng, trình duyệt Chrome, Edge, Firefox, Word, Excel.
/// Tự động cache theo HWND cửa sổ foreground để đạt tốc độ 0ms không gọi OpenProcess lặp lại.
unsafe fn send_transform_batch(
    backspaces: usize,
    text: &str,
    has_space_before: bool,
    browser_compatibility: bool,
    browser_apps: &str,
) {
    if backspaces == 0 && text.is_empty() {
        return;
    }

    let is_browser = browser_compatibility && is_foreground_browser_app(browser_apps);
    // Khi người dùng gõ trên thanh địa chỉ trình duyệt (Chrome, Edge, Brave, Cốc Cốc, Opera):
    // Trình duyệt tự động bôi đen (inline selection) gợi ý URL/tìm kiếm đầu tiên.
    // Nếu chưa có dấu cách (!has_space_before) và không phải trong nội dung web (!is_focused_in_web_content):
    // Phím Backspace đầu tiên sẽ xóa vùng chọn gợi ý của trình duyệt, do đó cần thêm 1 Backspace để xóa ký tự gốc.
    let need_omnibox_fix = is_browser && !has_space_before && !is_focused_in_web_content();
    let total_backspaces = if need_omnibox_fix { backspaces + 1 } else { backspaces };

    send_replace_batch(0, total_backspaces, text);
}

const CF_UNICODETEXT: u32 = 13;
const GHND: u32 = 0x0042;

#[link(name = "user32")]
extern "system" {
    fn OpenClipboard(hWndNewOwner: HWND) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn GetClipboardData(uFormat: u32) -> *mut std::ffi::c_void;
    fn SetClipboardData(uFormat: u32, hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut std::ffi::c_void;
    fn GlobalLock(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn GlobalUnlock(hMem: *mut std::ffi::c_void) -> i32;
}

static CARET_REPLACEMENT_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct CaretReplaceRequest {
    seq: u64,
    ch: char,
    is_telex: bool,
    modern_tone: bool,
    free_marking: bool,
    smart_english: bool,
    simple_telex: bool,
}

static CARET_REPLACE_QUEUE: Mutex<Option<CaretReplaceRequest>> = Mutex::new(None);
static CARET_REPLACE_CONDVAR: std::sync::Condvar = std::sync::Condvar::new();

pub fn init_caret_replacement_worker() {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }

    std::thread::Builder::new()
        .name("hoyukey-caret-replace".to_string())
        .spawn(|| {
            loop {
                let req = {
                    let mut lock = CARET_REPLACE_QUEUE.lock().unwrap();
                    while lock.is_none() {
                        lock = CARET_REPLACE_CONDVAR.wait(lock).unwrap();
                    }
                    // Chờ debounce 30ms: phản hồi siêu tốc, gần như tức thì khi bấm phím sửa dấu
                    let (new_lock, timeout_res) = CARET_REPLACE_CONDVAR
                        .wait_timeout(lock, std::time::Duration::from_millis(30))
                        .unwrap();
                    lock = new_lock;
                    if timeout_res.timed_out() {
                        lock.take()
                    } else {
                        continue;
                    }
                };

                if let Some(req) = req {
                    if CARET_REPLACEMENT_SEQ.load(Ordering::SeqCst) == req.seq {
                        perform_post_type_selection_replacement(
                            req.ch,
                            req.is_telex,
                            req.modern_tone,
                            req.free_marking,
                            req.smart_english,
                            req.simple_telex,
                        );
                    }
                }
            }
        })
        .ok();
}

unsafe fn get_clipboard_text() -> Option<String> {
    for _ in 0..8 {
        if OpenClipboard(null_mut()) != 0 {
            let handle = GetClipboardData(CF_UNICODETEXT);
            if !handle.is_null() {
                let ptr = GlobalLock(handle) as *const u16;
                if !ptr.is_null() {
                    let mut len = 0;
                    while *ptr.add(len) != 0 {
                        len += 1;
                    }
                    let slice = std::slice::from_raw_parts(ptr, len);
                    let s = String::from_utf16_lossy(slice);
                    GlobalUnlock(handle);
                    CloseClipboard();
                    return Some(s);
                }
            }
            CloseClipboard();
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    None
}

unsafe fn set_clipboard_text(text: &str) -> bool {
    for _ in 0..8 {
        if OpenClipboard(null_mut()) != 0 {
            EmptyClipboard();
            let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes = utf16.len() * std::mem::size_of::<u16>();
            let handle = GlobalAlloc(GHND, bytes);
            if !handle.is_null() {
                let ptr = GlobalLock(handle) as *mut u16;
                if !ptr.is_null() {
                    std::ptr::copy_nonoverlapping(utf16.as_ptr(), ptr, utf16.len());
                    GlobalUnlock(handle);
                    SetClipboardData(CF_UNICODETEXT, handle);
                }
            }
            CloseClipboard();
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    false
}

unsafe fn send_key_combo(vks: &[u16]) {
    let mut inputs = Vec::new();
    for &vk in vks {
        let mut down: INPUT = std::mem::zeroed();
        down.r#type = INPUT_KEYBOARD;
        down.Anonymous.ki = KEYBDINPUT {
            wVk: vk,
            wScan: 0,
            dwFlags: 0,
            time: 0,
            dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
        };
        inputs.push(down);
    }
    for &vk in vks.iter().rev() {
        let mut up: INPUT = std::mem::zeroed();
        up.r#type = INPUT_KEYBOARD;
        up.Anonymous.ki = KEYBDINPUT {
            wVk: vk,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
        };
        inputs.push(up);
    }
    SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
}

unsafe fn send_single_key(vk: u16) {
    let mut inputs: [INPUT; 2] = std::mem::zeroed();
    inputs[0].r#type = INPUT_KEYBOARD;
    inputs[0].Anonymous.ki = KEYBDINPUT {
        wVk: vk,
        wScan: 0,
        dwFlags: 0,
        time: 0,
        dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
    };
    inputs[1].r#type = INPUT_KEYBOARD;
    inputs[1].Anonymous.ki = KEYBDINPUT {
        wVk: vk,
        wScan: 0,
        dwFlags: KEYEVENTF_KEYUP,
        time: 0,
        dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
    };
    SendInput(2, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
}

fn perform_post_type_selection_replacement(
    ch: char,
    is_telex: bool,
    modern_tone: bool,
    free_marking: bool,
    smart_english: bool,
    simple_telex: bool,
) {
    unsafe {
        // 1. Sao lưu clipboard hiện tại của người dùng
        let old_clipboard = get_clipboard_text();

        // 2. Xóa clipboard để đón dữ liệu mới từ ứng dụng
        if OpenClipboard(null_mut()) != 0 {
            EmptyClipboard();
            CloseClipboard();
        }

        // 3. Gửi Ctrl + Shift + Left để bôi đen từ vừa gõ (bao gồm cả ký tự ch vừa gõ vào)
        send_key_combo(&[VK_CONTROL, VK_SHIFT, VK_LEFT]);
        // Chờ 10ms để ứng dụng (Chrome, Discord, Word...) kịp cập nhật vùng chọn
        std::thread::sleep(std::time::Duration::from_millis(10));

        // 4. Gửi Ctrl + C để copy từ đã bôi đen
        send_key_combo(&[VK_CONTROL, b'C' as u16]);

        // 5. Chờ ứng dụng đưa dữ liệu vào clipboard (tối đa 45ms, kiểm tra mỗi 3ms để đón dữ liệu ngay khi sẵn sàng)
        let mut copied = None;
        for _ in 0..15 {
            std::thread::sleep(std::time::Duration::from_millis(3));
            if let Some(text) = get_clipboard_text() {
                if !text.is_empty() {
                    copied = Some(text);
                    break;
                }
            }
        }

        let mut transformed_word = None;
        if let Some(ref raw_text) = copied {
            let trimmed_end = raw_text.trim_end();
            let ch_lower = ch.to_ascii_lowercase();
            // Chuỗi copy được phải kết thúc bằng ký tự ch vừa gõ
            if trimmed_end.ends_with(ch) || trimmed_end.to_lowercase().ends_with(ch_lower) {
                let mut chars: Vec<char> = trimmed_end.chars().collect();
                if chars.len() >= 2 {
                    chars.pop(); // Bỏ ký tự ch
                    let base_word: String = chars.into_iter().collect();
                    let trimmed = base_word.trim();

                    if !trimmed.is_empty()
                        && trimmed.chars().count() <= 10
                        && trimmed.chars().all(|c| c.is_alphabetic())
                    {
                        let res = if is_telex {
                            crate::engine::telex::transform_telex(
                                trimmed,
                                ch,
                                modern_tone,
                                free_marking,
                                smart_english,
                                simple_telex,
                                None,
                            )
                        } else {
                            crate::engine::vni::transform_vni(
                                trimmed,
                                ch,
                                modern_tone,
                                free_marking,
                                smart_english,
                            )
                        };

                        if let Some(new_w) = res {
                            if new_w != trimmed {
                                let leading_ws: String = base_word.chars().take_while(|c| c.is_whitespace()).collect();
                                let trailing_ws: String = raw_text[trimmed_end.len()..].to_string();
                                let final_replacement = format!("{}{}{}", leading_ws, new_w, trailing_ws);
                                transformed_word = Some(final_replacement);
                            }
                        }
                    }
                }
            }
        }

        if let Some(new_word) = transformed_word {
            // THÀNH CÔNG: Đưa từ mới vào clipboard và gửi Ctrl + V để thay thế vùng chọn nguyên tử
            set_clipboard_text(&new_word);
            send_key_combo(&[VK_CONTROL, b'V' as u16]);
            // Chờ 20ms để ứng dụng dán xong văn bản mới trước khi khôi phục clipboard cũ
            std::thread::sleep(std::time::Duration::from_millis(20));

            // Cập nhật engine
            if let Ok(mut state) = GLOBAL_STATE.lock() {
                let clean = new_word.trim().to_string();
                let len = clean.chars().count();
                state.engine.current_word = clean.clone();
                state.engine.raw_keys = clean.clone();
                state.engine.has_transformed = true;
                state.engine.has_space_before = false;
                state.engine.at_start_of_line = false;
                state.engine.caret_offset = Some(len);
            }
        } else {
            // Không phải từ cần đổi dấu: Hủy bôi đen bằng phím Right Arrow để đưa con trỏ về vị trí cũ
            send_single_key(VK_RIGHT);
        }

        // 6. Khôi phục lại clipboard cũ của người dùng
        if let Some(old) = old_clipboard {
            set_clipboard_text(&old);
        } else if OpenClipboard(null_mut()) != 0 {
            EmptyClipboard();
            CloseClipboard();
        }
    }
}
