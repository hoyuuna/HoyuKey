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

/// Báº£ng ghi nhá»› tráº¡ng thÃ¡i Tiáº¿ng Viá»‡t (E / V) theo tá»«ng tiáº¿n trÃ¬nh á»©ng dá»¥ng
static APP_INPUT_STATES: Mutex<Option<HashMap<String, bool>>> = Mutex::new(None);

// Theo dÃµi cÃ¡c phÃ­m bá»‹ cháº·n KeyDown Ä‘á»ƒ cháº·n luÃ´n KeyUp tÆ°Æ¡ng á»©ng, trÃ¡nh desync phÃ­m khi gÃµ nhanh trÃªn trÃ¬nh duyá»‡t
const INIT_BOOL: AtomicBool = AtomicBool::new(false);
static BLOCKED_VK: [AtomicBool; 256] = [INIT_BOOL; 256];

// Theo dÃµi cá»­a sá»• Ä‘ang active (foreground), Ä‘á»•i cá»­a sá»• -> tá»± Ä‘á»™ng reset buffer
static LAST_FOREGROUND_WINDOW: AtomicIsize = AtomicIsize::new(0);

// Theo dÃµi tráº¡ng thÃ¡i phÃ­m táº¯t Ctrl + Shift (chuyá»ƒn cháº¿ Ä‘á»™ khi nháº£ phÃ­m)
static CTRL_IS_DOWN: AtomicBool = AtomicBool::new(false);
static SHIFT_IS_DOWN: AtomicBool = AtomicBool::new(false);
static CTRL_SHIFT_READY: AtomicBool = AtomicBool::new(false);
static OTHER_KEY_OR_MOUSE_PRESSED: AtomicBool = AtomicBool::new(false);

// Theo dÃµi phÃ­m Ä‘ang giá»¯ (auto-repeat) Ä‘á»ƒ tá»‘i Æ°u vÃ  trÃ¡nh ngháº½n luá»“ng phÃ­m Windows
static LAST_PHYSICAL_VK: AtomicU32 = AtomicU32::new(0);
static KEY_REPEAT_COUNT: AtomicU32 = AtomicU32::new(0);

// Caching tráº¡ng thÃ¡i á»©ng dá»¥ng trÃ¬nh duyá»‡t: 0ms latency, khÃ´ng má»Ÿ process handle trong hook
static IS_CURRENT_APP_BROWSER: AtomicBool = AtomicBool::new(false);

pub fn install_hook() -> bool {
    crate::automation::init_background_caret_loader();
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

/// Callback WinEvent báº¯t sá»± kiá»‡n chuyá»ƒn cá»­a sá»• Foreground ngay láº­p tá»©c khi Alt+Tab hoáº·c click chuá»™t
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

/// Láº¥y tÃªn file thá»±c thi (.exe) tá»« HWND
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

/// Xá»­ lÃ½ khi cá»­a sá»• lÃ m viá»‡c thay Ä‘á»•i (chuyá»ƒn app)
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

    // Bá» qua chÃ­nh cá»­a sá»• HoyuKey dashboard
    // Đổi cửa sổ làm việc: reset ngay trạng thái phím tắt modifier để tránh desync
    CTRL_IS_DOWN.store(false, Ordering::SeqCst);
    SHIFT_IS_DOWN.store(false, Ordering::SeqCst);
    CTRL_SHIFT_READY.store(false, Ordering::SeqCst);
    OTHER_KEY_OR_MOUSE_PRESSED.store(false, Ordering::SeqCst);

    let is_ignored_app = matches!(
        app_name.as_str(),
        "hoyukey.exe"
            | "explorer.exe"
            | "shellexperiencehost.exe"
            | "searchhost.exe"
            | "startmenuexperiencehost.exe"
            | "taskmgr.exe"
            | "lockapp.exe"
            | "systemsettings.exe"
            | "applicationframehost.exe"
    );
    if is_ignored_app || !remember_enabled {
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
        // Láº§n Ä‘áº§u vÃ o app nÃ y: ghi nhá»› tráº¡ng thÃ¡i hiá»‡n táº¡i
        map.insert(app_name, current_enabled);
    }
}

/// Cáº­p nháº­t tráº¡ng thÃ¡i E / V má»›i cho á»©ng dá»¥ng Ä‘ang active
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
            let is_ignored_app = matches!(
                app_name.as_str(),
                "hoyukey.exe"
                    | "explorer.exe"
                    | "shellexperiencehost.exe"
                    | "searchhost.exe"
                    | "startmenuexperiencehost.exe"
                    | "taskmgr.exe"
                    | "lockapp.exe"
                    | "systemsettings.exe"
                    | "applicationframehost.exe"
            );
            if !is_ignored_app {
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
        OTHER_KEY_OR_MOUSE_PRESSED.store(true, Ordering::SeqCst);
        CTRL_SHIFT_READY.store(false, Ordering::SeqCst);

        let msg = wparam as u32;
        if msg == WM_LBUTTONUP || msg == WM_RBUTTONUP {
            // Khi nháº£ chuá»™t: con trá» Ä‘Ã£ Ä‘Ã¡p xuá»‘ng vá»‹ trÃ­ má»›i -> yÃªu cáº§u náº¡p ná»n tá»« táº¡i con trá»
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

    // 1. Náº¿u lÃ  phÃ­m do chÃ­nh HoyuKey táº¡o ra -> cho qua ngay láº­p tá»©c Ä‘á»ƒ trÃ¡nh vÃ²ng láº·p!
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

    // 2. Xá»­ lÃ½ nháº£ phÃ­m (KeyUp):
    if is_up {
        if LAST_PHYSICAL_VK.load(Ordering::Relaxed) == kbd.vkCode {
            LAST_PHYSICAL_VK.store(0, Ordering::Relaxed);
            KEY_REPEAT_COUNT.store(0, Ordering::Relaxed);
        }

        // Náº¿u dÃ¹ng phÃ­m táº¯t Ctrl + Shift: chuyá»ƒn cháº¿ Ä‘á»™ khi ngÆ°á»i dÃ¹ng nháº£ phÃ­m (chuáº©n Windows)
        if is_ctrl_key || is_shift_key {
            let (switch_key, switch_key_enabled, pause_shortcut, pause_shortcut_apps) = {
                let state = GLOBAL_STATE.lock().unwrap();
                (
                    state.config.switch_key,
                    state.config.switch_key_enabled,
                    state.config.pause_shortcut,
                    state.config.pause_shortcut_apps.clone(),
                )
            };
            let is_shortcut_allowed = switch_key_enabled && (!pause_shortcut || !is_foreground_paused_app(&pause_shortcut_apps));
            if switch_key == SwitchKey::CtrlShift {
                let is_pure_ctrl_shift = !OTHER_KEY_OR_MOUSE_PRESSED.load(Ordering::SeqCst);
                if is_ctrl_key {
                    CTRL_IS_DOWN.store(false, Ordering::SeqCst);
                    if CTRL_SHIFT_READY.swap(false, Ordering::SeqCst) && is_pure_ctrl_shift && is_shortcut_allowed {
                        toggle_vietnamese_mode();
                    }
                } else if is_shift_key {
                    SHIFT_IS_DOWN.store(false, Ordering::SeqCst);
                    if CTRL_SHIFT_READY.swap(false, Ordering::SeqCst) && is_pure_ctrl_shift && is_shortcut_allowed {
                        toggle_vietnamese_mode();
                    }
                }
            }

            if !CTRL_IS_DOWN.load(Ordering::SeqCst) && !SHIFT_IS_DOWN.load(Ordering::SeqCst) {
                OTHER_KEY_OR_MOUSE_PRESSED.store(false, Ordering::SeqCst);
                CTRL_SHIFT_READY.store(false, Ordering::SeqCst);
            }
        } else {
            OTHER_KEY_OR_MOUSE_PRESSED.store(true, Ordering::SeqCst);
            CTRL_SHIFT_READY.store(false, Ordering::SeqCst);
        }

        // Cháº·n KeyUp náº¿u KeyDown tÆ°Æ¡ng á»©ng Ä‘Ã£ bá»‹ bá»™ gÃµ cháº·n (trÃ¡nh desync trÃªn Chrome/Edge)
        if BLOCKED_VK[vk_idx].swap(false, Ordering::SeqCst) {
            return 1;
        }
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    if !is_down {
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    if !is_ctrl_key && !is_shift_key {
        OTHER_KEY_OR_MOUSE_PRESSED.store(true, Ordering::SeqCst);
        CTRL_SHIFT_READY.store(false, Ordering::SeqCst);
    }

    // Báº¥t ká»³ phÃ­m váº­t lÃ½ nÃ o Ä‘Æ°á»£c báº¥m cÅ©ng láº­p tá»©c há»§y bá» cÃ¡c tÃ¡c vá»¥ CÃ¡ch 2 Ä‘ang chá» (náº¿u cÃ³)

    // 3. Theo dÃµi auto-repeat khi ngÆ°á»i dÃ¹ng giá»¯ phÃ­m
    let repeat_count = if LAST_PHYSICAL_VK.load(Ordering::Relaxed) == kbd.vkCode {
        KEY_REPEAT_COUNT.fetch_add(1, Ordering::Relaxed) + 1
    } else {
        LAST_PHYSICAL_VK.store(kbd.vkCode, Ordering::Relaxed);
        KEY_REPEAT_COUNT.store(0, Ordering::Relaxed);
        0
    };

    // Tá»± Ä‘á»™ng kiá»ƒm tra thay Ä‘á»•i cá»­a sá»• lÃ m viá»‡c (chuyá»ƒn tab trÃ¬nh duyá»‡t, chuyá»ƒn app)
    // Chá»‰ kiá»ƒm tra á»Ÿ phÃ­m Ä‘áº§u tiÃªn (khÃ´ng kiá»ƒm tra khi Ä‘ang giá»¯ láº·p láº¡i phÃ­m Ä‘á»ƒ tiáº¿t kiá»‡m CPU)
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

    // 4. Kiá»ƒm tra phÃ­m táº¯t chuyá»ƒn Ä‘á»•i cháº¿ Ä‘á»™ V / E
    if is_ctrl_key || is_shift_key || is_z {
        let (switch_key, switch_key_enabled, pause_shortcut, pause_shortcut_apps) = {
            let state = GLOBAL_STATE.lock().unwrap();
            (
                state.config.switch_key,
                state.config.switch_key_enabled,
                state.config.pause_shortcut,
                state.config.pause_shortcut_apps.clone(),
            )
        };
        let is_shortcut_allowed = switch_key_enabled && (!pause_shortcut || !is_foreground_paused_app(&pause_shortcut_apps));

        if is_shortcut_allowed {
            match switch_key {
                SwitchKey::CtrlShift => {
                    let other_pressed = OTHER_KEY_OR_MOUSE_PRESSED.load(Ordering::SeqCst);
                    if is_ctrl_key {
                        CTRL_IS_DOWN.store(true, Ordering::SeqCst);
                        if SHIFT_IS_DOWN.load(Ordering::SeqCst) && !other_pressed {
                            CTRL_SHIFT_READY.store(true, Ordering::SeqCst);
                        }
                    } else if is_shift_key {
                        SHIFT_IS_DOWN.store(true, Ordering::SeqCst);
                        if CTRL_IS_DOWN.load(Ordering::SeqCst) && !other_pressed {
                            CTRL_SHIFT_READY.store(true, Ordering::SeqCst);
                        }
                    } else {
                        OTHER_KEY_OR_MOUSE_PRESSED.store(true, Ordering::SeqCst);
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
        } else {
            CTRL_SHIFT_READY.store(false, Ordering::SeqCst);
        }
    }

    // 5. Náº¿u Ä‘ang giá»¯ Ctrl, Alt hoáº·c Windows -> phÃ­m táº¯t app khÃ¡c (Ctrl+C, Ctrl+A, Alt+Tab...),
    // xÃ³a buffer tá»« hiá»‡n táº¡i vÃ  cho phÃ­m Ä‘i qua tá»± nhiÃªn
    let ctrl_pressed = (GetKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
    let alt_pressed = (GetKeyState(VK_MENU as i32) as u16 & 0x8000) != 0;
    let win_pressed = (GetKeyState(VK_LWIN as i32) as u16 & 0x8000) != 0
        || (GetKeyState(VK_RWIN as i32) as u16 & 0x8000) != 0;

    if ctrl_pressed || alt_pressed || win_pressed {
        reset_engine();
        BLOCKED_VK[vk_idx].store(false, Ordering::SeqCst);
        return CallNextHookEx(null_mut(), code, wparam, lparam);
    }

    // 6. Xá»­ lÃ½ Backspace & cÃ¡c phÃ­m Ä‘iá»u hÆ°á»›ng ngáº¯t tá»«
    match kbd.vkCode as u16 {
        VK_BACK => {
            BLOCKED_VK[vk_idx].store(false, Ordering::SeqCst);
            {
                let mut state = GLOBAL_STATE.lock().unwrap();
                let simple_telex = state.config.simple_telex;
                state.engine.handle_backspace(simple_telex);
                if state.engine.current_word.is_empty() {
                    state.engine.caret_needs_check = true;
                }
            }
            crate::automation::request_caret_refresh();
            return CallNextHookEx(null_mut(), code, wparam, lparam);
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
                send_replace_batch(0, 0, ".", None);
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
            if kbd.vkCode as u16 == VK_HOME {
                if let Ok(mut state) = GLOBAL_STATE.lock() {
                    state.engine.at_start_of_line = true;
                    state.engine.has_typed_in_line = false;
                    state.engine.line_words.clear();
                }
            }
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

    // 7. Chuyá»ƒn Ä‘á»•i mÃ£ phÃ­m thÃ nh kÃ½ tá»± Unicode
    if let Some(ch) = vk_to_char(kbd.vkCode, kbd.scanCode) {
        // Kiá»ƒm tra náº¡p ná»n con trá» (Background Caret Context - 0ms latency, khÃ´ng block hook)
        if repeat_count == 0 {
            let (
                enabled,
                dynamic_caret,
                is_telex,
                modern_tone,
                free_marking,
                smart_english,
                simple_telex,
                modify_tone_in_word,
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
                    state.config.modify_tone_in_word,
                    state.engine.current_word.is_empty(),
                    state.engine.caret_needs_check,
                    state.engine.has_space_before
                        || state.engine.spaces_after_word > 0
                        || state.engine.last_char_typed.map_or(false, |c| c.is_whitespace() || crate::engine::is_word_boundary(c)),
                    state.engine.at_start_of_line,
                )
            };

            // KÃ­ch hoáº¡t sá»­a dáº¥u táº¡i con trá» CHá»ˆ KHI ngÆ°á»i dÃ¹ng vá»«a click chuá»™t / Ä‘iá»u hÆ°á»›ng con trá» (caret_needs_check)
            if enabled && dynamic_caret && is_empty && caret_needs_check && !at_start_of_line && !is_after_whitespace {
                // CHá»ˆ cÃ¡c phÃ­m dáº¥u thanh (s, f, r, x, j, z) má»›i lÃ  candidate!
                // TUYá»†T Äá»I khÃ´ng bao gá»“m w, a, e, o, d vÃ¬ Ä‘Ã³ lÃ  cÃ¡c chá»¯ cÃ¡i báº¯t Ä‘áº§u tá»« thÃ´ng thÆ°á»ng.
                let is_candidate_key = if is_telex {
                    matches!(ch, 's' | 'f' | 'r' | 'x' | 'j' | 'z' |
                                 'S' | 'F' | 'R' | 'X' | 'J' | 'Z')
                } else {
                    matches!(ch, '1'..='9' | '0')
                };

                if is_candidate_key {
                    // CHá»ˆ Ä‘á»c tá»« cache ná»n (0ms latency, TUYá»†T Äá»I khÃ´ng gá»i COM UIA Ä‘á»“ng bá»™ trong hook!)
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
                                modify_tone_in_word,
                                None,
                            )
                        } else {
                            crate::engine::vni::transform_vni(
                                target_word,
                                ch,
                                modern_tone,
                                free_marking,
                                smart_english,
                                modify_tone_in_word,
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
                                send_replace_batch(deletes, backspaces, &new_word, None);
                                return 1;
                            }
                        }
                    }
                }
            }
            if let Ok(mut state) = GLOBAL_STATE.lock() {
                state.engine.caret_needs_check = false;
            }
        }
        crate::automation::clear_cached_caret_word();

        // Xá»­ lÃ½ biáº¿n Ä‘á»•i kÃ½ tá»± thÃ´ng qua engine (chá»‰ lock 1 láº§n duy nháº¥t, láº¥y káº¿t quáº£ vÃ  má»Ÿ khÃ³a ngay)
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
                send_replace_batch(deletes, backspaces, &text, None);
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

    // Ghi nhá»› tráº¡ng thÃ¡i má»›i cho á»©ng dá»¥ng hiá»‡n táº¡i náº¿u tÃ­nh nÄƒng Ä‘ang báº­t
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
    state.engine.line_words.clear();
}

fn vk_to_char(vk: u32, scan_code: u32) -> Option<char> {
    unsafe {
        // 1. PhÃ­m chá»¯ cÃ¡i A-Z (chiáº¿m 99% khi gÃµ tiáº¿ng Viá»‡t):
        // Xá»­ lÃ½ trá»±c tiáº¿p tá»« Virtual Key mÃ  KHÃ”NG gá»i ToUnicode lÃ m há»ng dead-key state cá»§a Windows/Chrome!
        if (0x41..=0x5A).contains(&vk) {
            let shift = (GetKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;
            let caps = (GetKeyState(0x14) as u16 & 0x0001) != 0; // VK_CAPITAL = 0x14
            let upper = shift ^ caps;
            let ch = ((vk as u8 - b'A') + if upper { b'A' } else { b'a' }) as char;
            return Some(ch);
        }

        // 2. PhÃ­m sá»‘ 0-9 khi khÃ´ng giá»¯ Shift:
        if (0x30..=0x39).contains(&vk) {
            let shift = (GetKeyState(VK_SHIFT as i32) as u16 & 0x8000) != 0;
            if !shift {
                return Some((vk as u8) as char);
            }
        }

        // 3. CÃ¡c phÃ­m kÃ½ tá»± khÃ¡c (dáº¥u cÃ¢u, kÃ½ hiá»‡u Ä‘áº·c biá»‡t):
        let mut key_state = [0u8; 256];
        GetKeyboardState(key_state.as_mut_ptr());

        let mut utf16_buf = [0u16; 8];
        let len = ToUnicode(
            vk,
            scan_code,
            key_state.as_ptr(),
            utf16_buf.as_mut_ptr(),
            utf16_buf.len() as i32,
            4, // wFlags = 4: khÃ´ng lÃ m thay Ä‘á»•i tráº¡ng thÃ¡i bÃ n phÃ­m há»‡ thá»‘ng
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



/// Gá»­i chuá»—i Delete, Backspace vÃ  Unicode Text nguyÃªn tá»­ (Atomic SendInput).
/// Há»— trá»£ sá»­a dáº¥u táº¡i con trá» (xÃ³a Ä‘uÃ´i sau con trá» báº±ng Delete vÃ  xÃ³a Ä‘áº§u trÆ°á»›c con trá» báº±ng Backspace).
unsafe fn send_replace_batch(
    deletes: usize,
    backspaces: usize,
    text: &str,
    prefix_char: Option<u16>,
) {
    if deletes == 0 && backspaces == 0 && text.is_empty() && prefix_char.is_none() {
        return;
    }

    let mut inputs = Vec::with_capacity(
        (deletes + backspaces + if prefix_char.is_some() { 1 } else { 0 }) * 2
            + text.encode_utf16().count() * 2,
    );

    // 0. Gửi ký tự rỗng (chuẩn OpenKey vFixRecommendBrowser 0x202F)
    // Để ghi đè vùng chọn autocomplete trong Omnibox của trình duyệt
    if let Some(ch) = prefix_char {
        let mut down: INPUT = std::mem::zeroed();
        down.r#type = INPUT_KEYBOARD;
        down.Anonymous.ki = KEYBDINPUT {
            wVk: 0,
            wScan: ch,
            dwFlags: KEYEVENTF_UNICODE,
            time: 0,
            dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
        };
        inputs.push(down);

        let mut up: INPUT = std::mem::zeroed();
        up.r#type = INPUT_KEYBOARD;
        up.Anonymous.ki = KEYBDINPUT {
            wVk: 0,
            wScan: ch,
            dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: HOYU_MAGIC_EXTRA_INFO,
        };
        inputs.push(up);
    }

    // 1. Gá»­i phÃ­m Delete Ä‘á»ƒ xÃ³a pháº§n Ä‘uÃ´i phÃ­a sau con trá» (náº¿u con trá» Ä‘ang á»Ÿ giá»¯a tá»«)
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

    // 2. Gá»­i phÃ­m Backspace Ä‘á»ƒ xÃ³a pháº§n Ä‘áº§u phÃ­a trÆ°á»›c con trá»
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

    // 3. BÆ¡m chuá»—i Unicode tiáº¿ng Viá»‡t má»›i
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

    // 4. Náº¿u trÆ°á»›c Ä‘Ã³ Ä‘Ã£ xÃ³a pháº§n Ä‘uÃ´i báº±ng Delete (con trá» á»Ÿ giá»¯a tá»«):
    // Gá»­i láº¡i Ä‘Ãºng sá»‘ phÃ­m Left Arrow Ä‘á»ƒ Ä‘Æ°a con trá» quay vá» Ä‘Ãºng vá»‹ trÃ­ giá»¯a tá»« ban Ä‘áº§u!
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

#[allow(dead_code)]
static LAST_CHECKED_HWND: AtomicIsize = AtomicIsize::new(0);
#[allow(dead_code)]
static LAST_CHECKED_IS_BROWSER: AtomicBool = AtomicBool::new(false);

#[allow(dead_code)]
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

    let mut is_browser = false;

    // 1. Kiểm tra qua OpenProcess và QueryFullProcessImageNameW
    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if !process.is_null() {
        let mut path_buf = [0u16; 1024];
        let mut size = path_buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, 0, path_buf.as_mut_ptr(), &mut size);
        CloseHandle(process);

        if ok != 0 && size > 0 {
            let full_path = String::from_utf16_lossy(&path_buf[..size as usize]);
            let exe_name = std::path::Path::new(&full_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();

            // Khớp với danh sách do người dùng cấu hình
            let matched_config = browser_apps.split(',').any(|app| {
                let app = app.trim().to_ascii_lowercase();
                !app.is_empty() && (exe_name == app || exe_name == format!("{}.exe", app))
            });

            // Tự động nhận diện các trình duyệt phổ biến nếu danh sách bị xóa hoặc thiếu
            let matched_builtin = exe_name.contains("chrome")
                || exe_name.contains("edge")
                || exe_name.contains("brave")
                || exe_name.contains("coccoc")
                || exe_name.contains("opera")
                || exe_name.contains("firefox")
                || exe_name.contains("vivaldi")
                || exe_name.contains("arc")
                || exe_name.contains("zen")
                || exe_name.contains("browser");

            is_browser = matched_config || matched_builtin;
        }
    }

    // 2. Fallback nếu OpenProcess thất bại (ví dụ trình duyệt chạy quyền Admin hoặc sandbox bảo vệ):
    // Dùng GetClassNameW kiểm tra window class của cửa sổ foreground (không cần quyền process)
    if !is_browser {
        let mut class_buf = [0u16; 256];
        let len = GetClassNameW(hwnd, class_buf.as_mut_ptr(), class_buf.len() as i32);
        if len > 0 {
            let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
            if class_name.contains("Chrome_WidgetWin") || class_name.contains("MozillaWindowClass") {
                is_browser = true;
            }
        }
    }

    LAST_CHECKED_HWND.store(hwnd as isize, Ordering::Relaxed);
    LAST_CHECKED_IS_BROWSER.store(is_browser, Ordering::Relaxed);
    is_browser
}

pub(crate) unsafe fn is_foreground_paused_app(pause_apps: &str) -> bool {
    if pause_apps.trim().is_empty() {
        return false;
    }
    let hwnd = GetForegroundWindow();
    if hwnd.is_null() {
        return false;
    }
    if let Some(exe_name) = get_process_name_by_hwnd(hwnd) {
        pause_apps.split(',').any(|app| {
            let app = app.trim().to_ascii_lowercase();
            !app.is_empty() && (exe_name == app || exe_name == format!("{}.exe", app))
        })
    } else {
        false
    }
}

#[allow(dead_code)]
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
            // TrÃªn Chrome, Edge, Brave, Cá»‘c Cá»‘c, Opera:
            // Cá»­a sá»• hiá»ƒn thá»‹ ná»™i dung trang web luÃ´n cÃ³ class name chá»©a "RenderWidgetHost"
            // (vÃ­ dá»¥: Chrome_RenderWidgetHostHWND)
            if class_name.contains("RenderWidgetHost") || class_name.contains("MozillaContentWindowClass") {
                return true;
            }
        }
    }
    false
}

/// Gá»­i chuá»—i Backspace vÃ  Unicode Text nguyÃªn tá»­ (Atomic SendInput).
/// Gá»­i chuáº©n Win32 API tÆ°Æ¡ng thÃ­ch hoÃ n háº£o vá»›i má»i á»©ng dá»¥ng, trÃ¬nh duyá»‡t Chrome, Edge, Firefox, Word, Excel.
/// Tá»± Ä‘á»™ng cache theo HWND cá»­a sá»• foreground Ä‘á»ƒ Ä‘áº¡t tá»‘c Ä‘á»™ 0ms khÃ´ng gá»i OpenProcess láº·p láº¡i.
unsafe fn send_transform_batch(
    backspaces: usize,
    text: &str,
    _has_space_before: bool,
    browser_compatibility: bool,
    browser_apps: &str,
) {
    if backspaces == 0 && text.is_empty() {
        return;
    }

    // Chuẩn OpenKey vFixRecommendBrowser:
    // Khi gõ trong trình duyệt (Chrome, Edge, Brave...) và có ít nhất 1 ký tự cần xóa (backspaces > 0):
    // Gửi ký tự rỗng (0x202F) trước để ghi đè vùng chọn autocomplete trong Omnibox,
    // đồng thời gửi thêm đúng 1 phím Backspace để xóa sạch ký tự rỗng này.
    // Kết quả:
    // - Trong Omnibox: xóa sạch gợi ý và chữ cũ, không bị 'aả'!
    // - Trong web content / Facebook: xóa sạch ký tự rỗng và chữ cũ, không bao giờ ăn mất chữ đi trước!
    let is_browser = browser_compatibility && is_foreground_browser_app(browser_apps);
    if is_browser && backspaces > 0 {
        send_replace_batch(0, backspaces + 1, text, Some(0x202F));
    } else {
        send_replace_batch(0, backspaces, text, None);
    }
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

pub(crate) unsafe fn get_clipboard_text() -> Option<String> {
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

pub(crate) unsafe fn set_clipboard_text(text: &str) -> bool {
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
