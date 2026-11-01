use std::ptr::null_mut;
use std::sync::atomic::{AtomicI32, AtomicIsize, AtomicU32, AtomicU8, AtomicUsize, Ordering};
use std::sync::Mutex;
use windows_sys::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    AddFontMemResourceEx, AddFontResourceExW, BeginPaint, BitBlt, CreateCompatibleBitmap,
    CreateCompatibleDC, DeleteDC, DeleteObject, EndPaint, InvalidateRect, ScreenToClient,
    SelectObject, StretchBlt, SetStretchBltMode, GetDIBits, SetDIBits, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HALFTONE, PAINTSTRUCT, SRCCOPY,
};
use windows_sys::Win32::Graphics::GdiPlus::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetCursorPos,
    GetSystemMetrics, KillTimer, LoadCursorW, PostMessageW, PostQuitMessage, RegisterClassW,
    SendMessageW, SetCursor, SetForegroundWindow, SetTimer, ShowWindow, CS_HREDRAW, CS_VREDRAW,
    IDC_ARROW, IDC_HAND, ICON_BIG, ICON_SMALL, SM_CXSCREEN, SM_CYSCREEN, SW_RESTORE,
    SW_SHOW, WM_CHAR, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_NCMOUSEMOVE,
    WM_PAINT, WM_SETCURSOR, WM_SETICON, WM_TIMER, WNDCLASSW, WS_CAPTION, WS_EX_DLGMODALFRAME,
    WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU,
};

const WM_MOUSELEAVE: u32 = 0x02A3;
const WM_MOUSEWHEEL: u32 = 0x020A;

// Quản lý cuộn nội dung (Scrollable content)
static SCROLL_Y: AtomicI32 = AtomicI32::new(0);
static TARGET_SCROLL_Y: AtomicI32 = AtomicI32::new(0);

use crate::config::{Config, InputMethod, SwitchKey};
use crate::tray::WM_APP_UPDATE_TRAY;
use crate::GLOBAL_STATE;

pub static SETTINGS_HWND: AtomicIsize = AtomicIsize::new(0);
static GDIP_TOKEN: AtomicUsize = AtomicUsize::new(0);

// Timer ID cho animation 60 FPS
const TIMER_ID_ANIM: usize = 1001;

// Tiến trình hoạt họa các công tắc và thanh trượt (0 đến 1000 <=> 0.0 đến 1.0)
static MODERN_TONE_PROGRESS: AtomicU32 = AtomicU32::new(1000);
static FREE_MARKING_PROGRESS: AtomicU32 = AtomicU32::new(1000);
static SMART_ENGLISH_PROGRESS: AtomicU32 = AtomicU32::new(1000);
static MODIFY_TONE_PROGRESS: AtomicU32 = AtomicU32::new(1000);
static AUTO_RESTORE_PROGRESS: AtomicU32 = AtomicU32::new(1000);
static SIMPLE_TELEX_PROGRESS: AtomicU32 = AtomicU32::new(0);
static DYNAMIC_CARET_PROGRESS: AtomicU32 = AtomicU32::new(1000);
static INPUT_METHOD_PROGRESS: AtomicU32 = AtomicU32::new(0); // 0 = Telex, 1000 = VNI
static SWITCH_KEY_PROGRESS: AtomicU32 = AtomicU32::new(0);    // 0 = Ctrl+Shift, 1000 = Alt+Z
const V_BGRA_32: &[u8] = include_bytes!("../icon/v_32.bin");
const E_BGRA_32: &[u8] = include_bytes!("../icon/e_32.bin");

static START_WITH_WINDOWS_PROGRESS: AtomicU32 = AtomicU32::new(0);
static NOTIFY_STARTUP_PROGRESS: AtomicU32 = AtomicU32::new(1000);
static NOTIFY_TOGGLE_PROGRESS: AtomicU32 = AtomicU32::new(0);
static MODE_PROGRESS: AtomicU32 = AtomicU32::new(1000);
static AUTO_CAPS_LINE_PROGRESS: AtomicU32 = AtomicU32::new(0);
static AUTO_CAPS_SENTENCE_PROGRESS: AtomicU32 = AtomicU32::new(0);
static AUTO_PERIOD_ENTER_PROGRESS: AtomicU32 = AtomicU32::new(0);
static BROWSER_COMPAT_PROGRESS: AtomicU32 = AtomicU32::new(1000);
static BROWSER_INPUT_ANIM: AtomicU32 = AtomicU32::new(1000);
static BROWSER_INPUT_FOCUSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static REMEMBER_APP_PROGRESS: AtomicU32 = AtomicU32::new(0);
static RAM_LIMIT_PROGRESS: AtomicU32 = AtomicU32::new(0);
static RAM_LIMIT_ANIM: AtomicU32 = AtomicU32::new(0);

// Quản lý trang và hiệu ứng chuyển trang (Smooth Page Transition Animation)
static CURRENT_PAGE: AtomicU8 = AtomicU8::new(0);
static TARGET_PAGE: AtomicU8 = AtomicU8::new(0);
static TRANSITION_PROGRESS: AtomicU32 = AtomicU32::new(1000); // 1000 = đứng yên
static TRANSITION_DIR: AtomicI32 = AtomicI32::new(0);         // 1: sang trái, -1: sang phải

// Quản lý hover item
static HOVER_ITEM: AtomicI32 = AtomicI32::new(-1);

// Quản lý hoạt họa Modal Popup (Bật ra mượt mà 0 đến 1000)
static MODAL_ANIM_PROGRESS: AtomicU32 = AtomicU32::new(0);

pub fn trigger_modal_open(hwnd: HWND) {
    MODAL_ANIM_PROGRESS.store(0, Ordering::SeqCst);
    unsafe {
        SetTimer(hwnd, TIMER_ID_ANIM, 16, None);
        InvalidateRect(hwnd, null_mut(), 0);
    }
}

pub fn close_modal_anim(hwnd: HWND) {
    MODAL_ANIM_PROGRESS.store(0, Ordering::SeqCst);
    unsafe {
        InvalidateRect(hwnd, null_mut(), 0);
    }
}

// Nhúng trực tiếp font Google Sans
const FONT_GOOGLE_SANS_REGULAR: &[u8] = include_bytes!("../fonts/GoogleSans-Regular.ttf");
const FONT_GOOGLE_SANS_BOLD: &[u8] = include_bytes!("../fonts/GoogleSans-Bold.ttf");

// Nhúng logo Premultiplied ARGB
const LOGO_BGRA_128: &[u8] = include_bytes!("../icon/logo_128.bin");
const LOGO_BGRA_32: &[u8] = include_bytes!("../icon/logo_32.bin");

// Bảng màu Shadcn UI Zinc Dark
const ARGB_BG: u32 = 0xFF09090B;              // #09090b
const ARGB_CARD_BG: u32 = 0xFF141417;         // #141417
const ARGB_CARD_HOVER: u32 = 0xFF1E1E23;      // #1e1e23
const ARGB_CARD_BORDER: u32 = 0xFF27272A;     // #27272a
const ARGB_DIVIDER: u32 = 0xFF202024;         // #202024
const ARGB_TEXT_PRIMARY: u32 = 0xFFF4F4F5;    // #f4f4f5
const ARGB_TEXT_MUTED: u32 = 0xFFA1A1AA;      // #a1a1aa
const _ARGB_STATUS_GREEN: u32 = 0xFF22C55E;    // #22c55e
const ARGB_STATUS_MUTED: u32 = 0xFF71717A;    // #71717a
const ARGB_PILL_TRACK: u32 = 0xFF1C1C20;      // #1c1c20
const ARGB_WHITE: u32 = 0xFFFAFAFA;           // #fafafa
const ARGB_WHITE_HOVER: u32 = 0xFFE4E4E7;     // #e4e4e7
const ARGB_TEXT_DARK: u32 = 0xFF09090B;       // #09090b
const ARGB_DANGER_TEXT: u32 = 0xFFEF4444;     // #ef4444
const ARGB_DANGER_BORDER: u32 = 0xFF3F1D1D;   // #3f1d1d
const ARGB_DANGER_HOVER: u32 = 0xFF2D1515;
const ARGB_ALERT_BG: u32 = 0x22F59E0B;
const ARGB_ALERT_BORDER: u32 = 0x55F59E0B;
const ARGB_ALERT_TEXT: u32 = 0xFFFBBF24;    // #2d1515
const ARGB_MODAL_BG: u32 = 0xFF16161A;        // Nền modal
const ARGB_MODAL_BORDER: u32 = 0xFF2E2E34;    // Viền modal

pub fn init_ui_subsystems() {
    unsafe {
        if GDIP_TOKEN.load(Ordering::SeqCst) == 0 {
            let mut token: usize = 0;
            let input = GdiplusStartupInput {
                GdiplusVersion: 1,
                DebugEventCallback: 0,
                SuppressBackgroundThread: 0,
                SuppressExternalCodecs: 0,
            };
            if GdiplusStartup(&mut token, &input, null_mut()) == 0 {
                GDIP_TOKEN.store(token, Ordering::SeqCst);
            }
        }

        static FONTS_LOADED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !FONTS_LOADED.swap(true, Ordering::SeqCst) {
            let mut c_fonts: u32 = 0;
            AddFontMemResourceEx(
                FONT_GOOGLE_SANS_REGULAR.as_ptr() as _,
                FONT_GOOGLE_SANS_REGULAR.len() as u32,
                null_mut(),
                &mut c_fonts,
            );
            AddFontMemResourceEx(
                FONT_GOOGLE_SANS_BOLD.as_ptr() as _,
                FONT_GOOGLE_SANS_BOLD.len() as u32,
                null_mut(),
                &mut c_fonts,
            );

            let p1 = to_wide("fonts\\GoogleSans-Regular.ttf");
            let p2 = to_wide("fonts\\GoogleSans-Bold.ttf");
            AddFontResourceExW(p1.as_ptr(), 0x10, null_mut());
            AddFontResourceExW(p2.as_ptr(), 0x10, null_mut());
        }
    }
}

pub fn shutdown_ui_subsystems() {
    unsafe {
        let mut lock = CACHED_FONTS.lock().unwrap();
        if let Some(fonts) = lock.take() {
            GdipDeleteFont(fonts.font_title);
            GdipDeleteFont(fonts.font_sub_title);
            GdipDeleteFont(fonts.font_modal_title);
            GdipDeleteFont(fonts.font_bold);
            GdipDeleteFont(fonts.font_desc);
            GdipDeleteFont(fonts.font_section);
            GdipDeleteFont(fonts.font_tab);
            GdipDeleteFont(fonts.font_btn);
            if !fonts.family.is_null() {
                GdipDeleteFontFamily(fonts.family);
            }
        }
    }
    let token = GDIP_TOKEN.swap(0, Ordering::SeqCst);
    if token != 0 {
        unsafe {
            GdiplusShutdown(token);
        }
    }
}

pub fn create_settings_window(hinstance: HMODULE) -> HWND {
    init_ui_subsystems();

    let class_name = to_wide("HoyuKey_ShadcnSettingsClass");
    let window_title = to_wide("HoyuKey Dashboard");

    let app_icon = crate::tray::create_hicon_from_bgra(LOGO_BGRA_32, 32);

    unsafe {
        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(settings_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: app_icon,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null_mut(),
            lpszClassName: class_name.as_ptr(),
        };

        static CLASS_REGISTERED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !CLASS_REGISTERED.swap(true, Ordering::SeqCst) {
            RegisterClassW(&wnd_class);
        }

        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 520,
            bottom: 660,
        };
        AdjustWindowRectEx(
            &mut rect,
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            0,
            WS_EX_DLGMODALFRAME,
        );

        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let x = (screen_w - width) / 2;
        let y = (screen_h - height) / 2;

        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            class_name.as_ptr(),
            window_title.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            x,
            y,
            width,
            height,
            null_mut(),
            null_mut(),
            hinstance,
            null_mut(),
        );

        SendMessageW(hwnd, WM_SETICON, ICON_BIG as _, app_icon as _);
        SendMessageW(hwnd, WM_SETICON, ICON_SMALL as _, app_icon as _);

        SETTINGS_HWND.store(hwnd as isize, Ordering::SeqCst);
        hwnd
    }
}

pub fn close_settings_window(hwnd: HWND) {
    unsafe {
        KillTimer(hwnd, TIMER_ID_ANIM);
        DestroyWindow(hwnd);
    }
    SETTINGS_HWND.store(0, Ordering::SeqCst);
    shutdown_ui_subsystems();
    let config = GLOBAL_STATE.lock().unwrap().config.clone();
    crate::apply_ram_limiter(&config);
}

pub fn show_settings_window() {
    let mut hwnd = SETTINGS_HWND.load(Ordering::SeqCst) as HWND;
    if hwnd.is_null() {
        let hinstance = unsafe { GetModuleHandleW(null_mut()) };
        hwnd = create_settings_window(hinstance);
    }
    if hwnd != null_mut() {
        let mut state = GLOBAL_STATE.lock().unwrap();
        state.config.start_with_windows = crate::autostart::is_autostart_enabled();
        MODERN_TONE_PROGRESS.store(if state.config.modern_tone { 1000 } else { 0 }, Ordering::SeqCst);
        FREE_MARKING_PROGRESS.store(if state.config.free_marking { 1000 } else { 0 }, Ordering::SeqCst);
        SMART_ENGLISH_PROGRESS.store(if state.config.smart_english { 1000 } else { 0 }, Ordering::SeqCst);
        MODIFY_TONE_PROGRESS.store(if state.config.modify_tone_in_word { 1000 } else { 0 }, Ordering::SeqCst);
        AUTO_RESTORE_PROGRESS.store(if state.config.auto_restore { 1000 } else { 0 }, Ordering::SeqCst);
        let is_telex = state.config.input_method == InputMethod::Telex;
        SIMPLE_TELEX_PROGRESS.store(if is_telex && state.config.simple_telex { 1000 } else { 0 }, Ordering::SeqCst);
        DYNAMIC_CARET_PROGRESS.store(if state.config.dynamic_caret { 1000 } else { 0 }, Ordering::SeqCst);
        INPUT_METHOD_PROGRESS.store(if state.config.input_method == InputMethod::Vni { 1000 } else { 0 }, Ordering::SeqCst);
        SWITCH_KEY_PROGRESS.store(if state.config.switch_key == SwitchKey::AltZ { 1000 } else { 0 }, Ordering::SeqCst);
        START_WITH_WINDOWS_PROGRESS.store(if state.config.start_with_windows { 1000 } else { 0 }, Ordering::SeqCst);
        NOTIFY_STARTUP_PROGRESS.store(if state.config.notify_on_startup { 1000 } else { 0 }, Ordering::SeqCst);
        NOTIFY_TOGGLE_PROGRESS.store(if state.config.notify_on_toggle { 1000 } else { 0 }, Ordering::SeqCst);
        MODE_PROGRESS.store(if state.config.enabled { 1000 } else { 0 }, Ordering::SeqCst);
        AUTO_CAPS_LINE_PROGRESS.store(if state.config.auto_caps_line { 1000 } else { 0 }, Ordering::SeqCst);
        AUTO_CAPS_SENTENCE_PROGRESS.store(if state.config.auto_caps_sentence { 1000 } else { 0 }, Ordering::SeqCst);
        AUTO_PERIOD_ENTER_PROGRESS.store(if state.config.auto_period_enter { 1000 } else { 0 }, Ordering::SeqCst);
        BROWSER_COMPAT_PROGRESS.store(if state.config.browser_compatibility { 1000 } else { 0 }, Ordering::SeqCst);
        BROWSER_INPUT_ANIM.store(if state.config.browser_compatibility { 1000 } else { 0 }, Ordering::SeqCst);
        BROWSER_INPUT_FOCUSED.store(false, Ordering::SeqCst);
        REMEMBER_APP_PROGRESS.store(if state.config.remember_app_state { 1000 } else { 0 }, Ordering::SeqCst);
        RAM_LIMIT_PROGRESS.store(if state.config.limit_ram { 1000 } else { 0 }, Ordering::SeqCst);
        RAM_LIMIT_ANIM.store(if state.config.limit_ram { 1000 } else { 0 }, Ordering::SeqCst);
        HOVER_ITEM.store(-1, Ordering::SeqCst);
        CURRENT_PAGE.store(0, Ordering::SeqCst);
        TARGET_PAGE.store(0, Ordering::SeqCst);
        TRANSITION_PROGRESS.store(1000, Ordering::SeqCst);
        SCROLL_Y.store(0, Ordering::SeqCst);
        TARGET_SCROLL_Y.store(0, Ordering::SeqCst);
        MODAL_ANIM_PROGRESS.store(0, Ordering::SeqCst);
        drop(state);

        unsafe {
            ShowWindow(hwnd, SW_SHOW);
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
            InvalidateRect(hwnd, null_mut(), 0);
        }
    }
}

pub fn notify_mode_changed(_enabled: bool) {
    let hwnd = SETTINGS_HWND.load(Ordering::SeqCst) as HWND;
    if hwnd != null_mut() {
        unsafe {
            SetTimer(hwnd, TIMER_ID_ANIM, 16, None);
            InvalidateRect(hwnd, null_mut(), 0);
        }
    }
}

unsafe extern "system" fn settings_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            let w = rect.right - rect.left;
            let h = rect.bottom - rect.top;

            let mem_dc = CreateCompatibleDC(hdc);
            let mem_bitmap = CreateCompatibleBitmap(hdc, w, h);
            let old_bm = SelectObject(mem_dc, mem_bitmap);

            let config = {
                let state = GLOBAL_STATE.lock().unwrap();
                state.config.clone()
            };

            paint_shadcn_ui(mem_dc, w, h, &config);

            BitBlt(hdc, 0, 0, w, h, mem_dc, 0, 0, SRCCOPY);

            SelectObject(mem_dc, old_bm);
            DeleteObject(mem_bitmap);
            DeleteDC(mem_dc);

            EndPaint(hwnd, &ps);
            0
        }
        WM_ERASEBKGND => 1,
        WM_MOUSEMOVE => {
            let mut tme = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut tme);

            let x = (lparam & 0xFFFF) as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i32;
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            let w = rect.right - rect.left;

            let current_page = CURRENT_PAGE.load(Ordering::SeqCst);
            let new_hover = get_hovered_item(x, y, w, current_page);
            let old_hover = HOVER_ITEM.swap(new_hover, Ordering::SeqCst);

            if new_hover != old_hover {
                InvalidateRect(hwnd, null_mut(), 0);
            }
            0
        }
        WM_MOUSELEAVE => {
            let old_hover = HOVER_ITEM.swap(-1, Ordering::SeqCst);
            if old_hover != -1 {
                InvalidateRect(hwnd, null_mut(), 0);
            }
            0
        }
        WM_NCMOUSEMOVE => {
            let old_hover = HOVER_ITEM.swap(-1, Ordering::SeqCst);
            if old_hover != -1 {
                InvalidateRect(hwnd, null_mut(), 0);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_TIMER => {
            if wparam as usize == TIMER_ID_ANIM {
                let config = {
                    let state = GLOBAL_STATE.lock().unwrap();
                    state.config.clone()
                };

                let mut still_animating = false;

                // 1. Page transition slide animation (nhanh, dứt khoát chỉ trong 5-6 frame)
                let curr_trans = TRANSITION_PROGRESS.load(Ordering::SeqCst);
                if curr_trans < 1000 {
                    still_animating = true;
                    let diff = 1000 - curr_trans as i32;
                    let next = if diff <= 40 {
                        1000
                    } else {
                        let step = (diff / 2).max(140);
                        (curr_trans as i32 + step).min(1000) as u32
                    };
                    TRANSITION_PROGRESS.store(next, Ordering::SeqCst);
                    if next == 1000 {
                        let target = TARGET_PAGE.load(Ordering::SeqCst);
                        CURRENT_PAGE.store(target, Ordering::SeqCst);
                    }
                }

                // 2. Helper cập nhật mượt mà, dứt khoát cho tất cả các switch
                let mut advance_anim = |target: u32, current_atom: &AtomicU32| {
                    let curr = current_atom.load(Ordering::SeqCst);
                    if curr != target {
                        still_animating = true;
                        let diff = target as i32 - curr as i32;
                        let next = if diff.abs() <= 35 {
                            target
                        } else {
                            let step = (diff / 2).clamp(-250, 250);
                            let step = if step.abs() < 60 { if diff > 0 { 60 } else { -60 } } else { step };
                            (curr as i32 + step).clamp(0, 1000) as u32
                        };
                        current_atom.store(next, Ordering::SeqCst);
                    }
                };

                advance_anim(if config.enabled { 1000 } else { 0 }, &MODE_PROGRESS);
                advance_anim(if config.modern_tone { 1000 } else { 0 }, &MODERN_TONE_PROGRESS);
                advance_anim(if config.free_marking { 1000 } else { 0 }, &FREE_MARKING_PROGRESS);
                advance_anim(if config.smart_english { 1000 } else { 0 }, &SMART_ENGLISH_PROGRESS);
                advance_anim(if config.modify_tone_in_word { 1000 } else { 0 }, &MODIFY_TONE_PROGRESS);
                advance_anim(if config.auto_restore { 1000 } else { 0 }, &AUTO_RESTORE_PROGRESS);
                let is_telex = config.input_method == InputMethod::Telex;
                advance_anim(if is_telex && config.simple_telex { 1000 } else { 0 }, &SIMPLE_TELEX_PROGRESS);
                advance_anim(if config.dynamic_caret { 1000 } else { 0 }, &DYNAMIC_CARET_PROGRESS);
                advance_anim(if config.input_method == InputMethod::Vni { 1000 } else { 0 }, &INPUT_METHOD_PROGRESS);
                advance_anim(if config.switch_key == SwitchKey::AltZ { 1000 } else { 0 }, &SWITCH_KEY_PROGRESS);
                advance_anim(if config.start_with_windows { 1000 } else { 0 }, &START_WITH_WINDOWS_PROGRESS);
                advance_anim(if config.notify_on_startup { 1000 } else { 0 }, &NOTIFY_STARTUP_PROGRESS);
                advance_anim(if config.notify_on_toggle { 1000 } else { 0 }, &NOTIFY_TOGGLE_PROGRESS);
                advance_anim(if config.auto_caps_line { 1000 } else { 0 }, &AUTO_CAPS_LINE_PROGRESS);
                advance_anim(if config.auto_caps_sentence { 1000 } else { 0 }, &AUTO_CAPS_SENTENCE_PROGRESS);
                advance_anim(if config.auto_period_enter { 1000 } else { 0 }, &AUTO_PERIOD_ENTER_PROGRESS);
                advance_anim(if config.browser_compatibility { 1000 } else { 0 }, &BROWSER_COMPAT_PROGRESS);
                advance_anim(if config.browser_compatibility { 1000 } else { 0 }, &BROWSER_INPUT_ANIM);
                advance_anim(if config.remember_app_state { 1000 } else { 0 }, &REMEMBER_APP_PROGRESS);
                advance_anim(if config.limit_ram { 1000 } else { 0 }, &RAM_LIMIT_PROGRESS);
                advance_anim(if config.limit_ram { 1000 } else { 0 }, &RAM_LIMIT_ANIM);

                // 3. Easing cuộn nội dung mượt mà 60 FPS (tự động kẹp theo max_scroll động)
                let page = CURRENT_PAGE.load(Ordering::SeqCst);
                let max_scroll = get_max_scroll_for_page(page);
                let target_scroll = TARGET_SCROLL_Y.load(Ordering::SeqCst).min(max_scroll);
                if target_scroll != TARGET_SCROLL_Y.load(Ordering::SeqCst) {
                    TARGET_SCROLL_Y.store(target_scroll, Ordering::SeqCst);
                }
                let curr_scroll = SCROLL_Y.load(Ordering::SeqCst);
                if curr_scroll != target_scroll {
                    still_animating = true;
                    let diff = target_scroll - curr_scroll;
                    let step = if diff.abs() <= 2 {
                        diff
                    } else {
                        (diff as f32 * 0.35).round() as i32
                    };
                    SCROLL_Y.store(curr_scroll + step, Ordering::SeqCst);
                }

                // 4. Modal Popup Pop-out Animation
                let modal_state = crate::updater::MODAL_STATE.load(Ordering::SeqCst);
                if modal_state > 0 {
                    let curr_modal_anim = MODAL_ANIM_PROGRESS.load(Ordering::SeqCst);
                    if curr_modal_anim < 1000 {
                        still_animating = true;
                        let diff = 1000 - curr_modal_anim;
                        let next = if diff <= 40 {
                            1000
                        } else {
                            let step = (diff / 2).max(80);
                            (curr_modal_anim + step).min(1000)
                        };
                        MODAL_ANIM_PROGRESS.store(next, Ordering::SeqCst);
                    }
                }

                InvalidateRect(hwnd, null_mut(), 0);

                if !still_animating {
                    KillTimer(hwnd, TIMER_ID_ANIM);
                }
            }
            0
        }
        WM_LBUTTONDOWN => {
            let x = (lparam & 0xFFFF) as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i32;
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            let w = rect.right - rect.left;
            handle_mouse_click(hwnd, x, y, w);
            0
        }
        WM_SETCURSOR => {
            let mut pt: POINT = std::mem::zeroed();
            GetCursorPos(&mut pt);
            ScreenToClient(hwnd, &mut pt);

            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            let w = rect.right - rect.left;

            let current_page = CURRENT_PAGE.load(Ordering::SeqCst);
            if get_hovered_item(pt.x, pt.y, w, current_page) != -1 {
                SetCursor(LoadCursorW(null_mut(), IDC_HAND));
                1
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        WM_CHAR => {
            let page = CURRENT_PAGE.load(Ordering::SeqCst);
            if page == 5 && BROWSER_INPUT_FOCUSED.load(Ordering::SeqCst) {
                let ch_code = wparam as u16;
                let mut state = GLOBAL_STATE.lock().unwrap();
                if ch_code == 0x08 { // Backspace
                    state.config.browser_apps.pop();
                    state.config.save();
                    drop(state);
                    InvalidateRect(hwnd, null_mut(), 0);
                    return 0;
                } else if ch_code == 0x0D || ch_code == 0x1B { // Enter or Esc
                    BROWSER_INPUT_FOCUSED.store(false, Ordering::SeqCst);
                    drop(state);
                    InvalidateRect(hwnd, null_mut(), 0);
                    return 0;
                } else if ch_code >= 32 && ch_code < 127 { // Printable ASCII
                    if let Some(ch) = char::from_u32(ch_code as u32) {
                        state.config.browser_apps.push(ch);
                        state.config.save();
                        drop(state);
                        InvalidateRect(hwnd, null_mut(), 0);
                        return 0;
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_MOUSEWHEEL => {
            let page = CURRENT_PAGE.load(Ordering::SeqCst);
            let max_scroll = get_max_scroll_for_page(page);
            if max_scroll > 0 {
                let delta = ((wparam >> 16) & 0xFFFF) as i16;
                let current_target = TARGET_SCROLL_Y.load(Ordering::SeqCst);
                let scroll_step = 42;
                let new_target = if delta < 0 {
                    (current_target + scroll_step).min(max_scroll)
                } else {
                    (current_target - scroll_step).max(0)
                };
                TARGET_SCROLL_Y.store(new_target, Ordering::SeqCst);
                SetTimer(hwnd, TIMER_ID_ANIM, 16, None);
                InvalidateRect(hwnd, null_mut(), 0);
            }
            0
        }
        WM_CLOSE => {
            close_settings_window(hwnd);
            0
        }
        WM_DESTROY => {
            SETTINGS_HWND.store(0, Ordering::SeqCst);
            shutdown_ui_subsystems();
            let config = GLOBAL_STATE.lock().unwrap().config.clone();
            crate::apply_ram_limiter(&config);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}


fn get_max_scroll_for_page(page: u8) -> i32 {
    match page {
        0 => 80,
        1 => 118,
        2 => {
            let anim_p = RAM_LIMIT_ANIM.load(Ordering::SeqCst) as f32 / 1000.0;
            let anim_ease = 1.0 - (1.0 - anim_p).powi(3);
            let extra_h = (82.0 * anim_ease).round() as i32;
            let content_bottom = 558 + extra_h;
            (content_bottom - 576).max(0)
        }
        _ => 0,
    }
}

fn get_layout_metrics(w: i32) -> (i32, i32, i32, i32, i32, i32) {
    let margin_x = 24;
    let card_w = w - (margin_x * 2);
    let card_x = margin_x;
    let card_right = card_x + card_w;
    let right_ctrl = card_right - 20;
    let left_text = card_x + 20;
    (card_x, card_w, card_right, right_ctrl, left_text, margin_x)
}

fn get_hovered_item(x: i32, y: i32, client_w: i32, page: u8) -> i32 {
    // 0. Nếu đang hiện Modal Custom: Chỉ nhận diện click trên Modal
    let modal_state = crate::updater::MODAL_STATE.load(Ordering::SeqCst);
    if modal_state > 0 {
        let modal_w = 400;
        let modal_h = 230;
        let modal_x = (client_w - modal_w) / 2;
        let modal_y = (660 - modal_h) / 2;
        let btn_y = modal_y + 165;

        if modal_state == 2 {
            // Nút Để sau: ID 51
            if x >= modal_x + 24 && x <= modal_x + 24 + 166 && y >= btn_y && y <= btn_y + 42 {
                return 51;
            }
            // Nút Cập nhật ngay: ID 50
            if x >= modal_x + 210 && x <= modal_x + 210 + 166 && y >= btn_y && y <= btn_y + 42 {
                return 50;
            }
        } else if modal_state == 4 || modal_state == 5 {
            // Nút Đã hiểu: ID 50
            let btn_x = modal_x + (modal_w - 160) / 2;
            if x >= btn_x && x <= btn_x + 160 && y >= btn_y && y <= btn_y + 42 {
                return 50;
            }
        }
        return -1;
    }

    let (card_x, card_w, card_right, right_ctrl, _, _) = get_layout_metrics(client_w);

    // 1. Fixed Bottom Dock (y >= 584): 2 nút Thoát & Đóng vừa vặn, không thừa cằm
    let btn_w = (card_w - 12) / 2;
    if y >= 598 && y <= 646 {
        if x >= card_x && x <= card_x + btn_w {
            return 30; // Thoát HoyuKey
        }
        let btn2_x = card_x + btn_w + 12;
        if x >= btn2_x && x <= card_right {
            return 31; // Đóng
        }
    }
    if y >= 584 {
        return -1;
    }

    if page == 0 {
        // --- TRANG CHỦ (Hỗ trợ cuộn chuột scroll_y) ---
        let scroll_y = SCROLL_Y.load(Ordering::SeqCst);
        let y_content = y + scroll_y;

        // Status Card: y = 86..158
        if x >= card_x && x <= card_right && y_content >= 86 && y_content <= 158 {
            return 10;
        }
        // Menu item 1: Cài đặt đánh máy: y = 212..274
        if x >= card_x && x <= card_right && y_content >= 212 && y_content <= 274 {
            return 11;
        }
        // Menu item 2: Hỗ trợ viết: y = 286..348
        if x >= card_x && x <= card_right && y_content >= 286 && y_content <= 348 {
            return 14;
        }
        // Menu item 3: Nâng cao: y = 360..422 (ngay trên Cài đặt HoyuKey)
        if x >= card_x && x <= card_right && y_content >= 360 && y_content <= 422 {
            return 15;
        }
        // Menu item 4: Cài đặt HoyuKey: y = 434..496
        if x >= card_x && x <= card_right && y_content >= 434 && y_content <= 496 {
            return 12;
        }
        // Menu item 5: Giới thiệu: y = 508..570
        if x >= card_x && x <= card_right && y_content >= 508 && y_content <= 570 {
            return 13;
        }
    } else {
        // --- CÁC TRANG CON ---
        // Nút Back tròn ở đầu: (card_x, 20) kích thước 40x40 (Cố định, không cuộn)
        if x >= card_x && x <= card_x + 40 && y >= 20 && y <= 60 {
            return 20;
        }
        if y < 70 {
            return -1;
        }

        let scroll_y = SCROLL_Y.load(Ordering::SeqCst);
        let y_content = y + scroll_y;

        if page == 1 {
            // Cài đặt đánh máy:
            let tab_x = right_ctrl - 200;
            if x >= tab_x && x <= right_ctrl && y_content >= 95 && y_content <= 145 {
                return 40;
            }
            if x >= right_ctrl - 52 && x <= right_ctrl {
                if y_content >= 180 && y_content <= 224 {
                    return 41;
                }
                if y_content >= 252 && y_content <= 296 {
                    return 42;
                }
                if y_content >= 324 && y_content <= 368 {
                    return 43;
                }
                if y_content >= 396 && y_content <= 440 {
                    return 44;
                }
                if y_content >= 468 && y_content <= 512 {
                    return 45;
                }
                if y_content >= 540 && y_content <= 584 {
                    let state = GLOBAL_STATE.lock().unwrap();
                    if state.config.input_method == InputMethod::Telex {
                        return 47;
                    }
                }
            }
        } else if page == 2 {
            // Cài đặt HoyuKey:
            let sw_tab_x = card_x + 20;
            let sw_total_w = card_w - 40;
            if x >= sw_tab_x && x <= sw_tab_x + sw_total_w && y_content >= 152 && y_content <= 196 {
                return 46;
            }
            if x >= right_ctrl - 52 && x <= right_ctrl {
                if y_content >= 234 && y_content <= 274 {
                    return 47;
                }
                if y_content >= 346 && y_content <= 386 {
                    return 49; // Thông báo khởi động
                }
                if y_content >= 418 && y_content <= 458 {
                    return 50; // Thông báo chuyển ngôn ngữ
                }
                if y_content >= 498 && y_content <= 538 {
                    return 48; // Điều chỉnh RAM chạy nền
                }
            }
            let anim_p = RAM_LIMIT_ANIM.load(Ordering::SeqCst);
            if anim_p > 200 {
                let pill_y = 598;
                let pill_h = 34;
                if y_content >= pill_y && y_content <= pill_y + pill_h {
                    let total_w = card_w - 40;
                    let btn_w = (total_w - 18) / 4;
                    for idx in 0..4 {
                        let bx = (card_x + 20) + idx as i32 * (btn_w + 6);
                        if x >= bx && x <= bx + btn_w {
                            return 481 + idx as i32;
                        }
                    }
                }
            }
        } else if page == 3 {
            // Giới thiệu: Nút Kiểm tra cập nhật
            let btn_update_w = 240;
            let btn_update_x = card_x + (card_w - btn_update_w) / 2;
            let btn_update_y = 250;
            let btn_update_h = 42;
            if x >= btn_update_x && x <= btn_update_x + btn_update_w && y_content >= btn_update_y && y_content <= btn_update_y + btn_update_h {
                return 25;
            }
        } else if page == 4 {
            // Hỗ trợ viết:
            if x >= right_ctrl - 52 && x <= right_ctrl {
                if y_content >= 96 && y_content <= 140 {
                    return 61;
                }
                if y_content >= 170 && y_content <= 214 {
                    return 62;
                }
                if y_content >= 244 && y_content <= 288 {
                    return 63;
                }
            }
        } else if page == 5 {
            // Nâng cao:
            if x >= right_ctrl - 52 && x <= right_ctrl && y_content >= 96 && y_content <= 140 {
                return 71;
            }
            let input_anim = BROWSER_INPUT_ANIM.load(Ordering::SeqCst);
            if input_anim > 200 {
                let box_x = card_x + 20;
                let box_w = card_w - 40;
                let box_y = 160;
                let box_h = 42;
                if x >= box_x && x <= box_x + box_w && y_content >= box_y && y_content <= box_y + box_h {
                    return 72;
                }
            }

            let anim_p = BROWSER_INPUT_ANIM.load(Ordering::SeqCst) as f32 / 1000.0;
            let anim_ease = 1.0 - (1.0 - anim_p).powi(3);
            let card1_h = (68.0 + 64.0 * anim_ease).round() as i32;
            let card2_y = 86 + card1_h + 14;
            if x >= right_ctrl - 52 && x <= right_ctrl && y_content >= card2_y + 10 && y_content <= card2_y + 54 {
                return 73;
            }
        }
    }

    -1
}

fn handle_mouse_click(hwnd: HWND, x: i32, y: i32, client_w: i32) {
    // 0. Nếu đang hiện Modal Custom: Bắt sự kiện click nút Modal
    let modal_state = crate::updater::MODAL_STATE.load(Ordering::SeqCst);
    if modal_state > 0 {
        let modal_w = 400;
        let modal_h = 230;
        let modal_x = (client_w - modal_w) / 2;
        let modal_y = (660 - modal_h) / 2;
        let btn_y = modal_y + 165;

        if modal_state == 2 {
            // Click Nút Để sau:
            if x >= modal_x + 24 && x <= modal_x + 24 + 166 && y >= btn_y && y <= btn_y + 42 {
                crate::updater::close_modal(hwnd);
                return;
            }
            // Click Nút Cập nhật ngay:
            if x >= modal_x + 210 && x <= modal_x + 210 + 166 && y >= btn_y && y <= btn_y + 42 {
                crate::updater::start_downloading_update(hwnd);
                return;
            }
        } else if modal_state == 4 || modal_state == 5 {
            let btn_x = modal_x + (modal_w - 160) / 2;
            if x >= btn_x && x <= btn_x + 160 && y >= btn_y && y <= btn_y + 42 {
                crate::updater::close_modal(hwnd);
                return;
            }
        }
        return;
    }

    let (card_x, card_w, card_right, right_ctrl, _, _) = get_layout_metrics(client_w);
    let page = CURRENT_PAGE.load(Ordering::SeqCst);

    // 1. Nút Thoát và Đóng (Fixed Bottom Dock ở mọi trang, vừa vặn không thừa cằm)
    let btn_w = (card_w - 12) / 2;
    if y >= 598 && y <= 646 {
        if x >= card_x && x <= card_x + btn_w {
            unsafe {
                PostQuitMessage(0);
            }
            return;
        }
        let btn2_x = card_x + btn_w + 12;
        if x >= btn2_x && x <= card_right {
            close_settings_window(hwnd);
            return;
        }
    }
    if y >= 584 {
        return;
    }

    if TRANSITION_PROGRESS.load(Ordering::SeqCst) < 1000 {
        return;
    }

    if page == 0 {
        let scroll_y = SCROLL_Y.load(Ordering::SeqCst);
        let y_content = y + scroll_y;

        // --- XỬ LÝ TRANG CHỦ ---
        if x >= card_x && x <= card_right && y_content >= 86 && y_content <= 158 {
            crate::hook::toggle_vietnamese_mode();
            unsafe {
                InvalidateRect(hwnd, null_mut(), 0);
            }
            return;
        }

        if x >= card_x && x <= card_right && y_content >= 212 && y_content <= 274 {
            start_page_transition(hwnd, 1, 1);
            return;
        }

        if x >= card_x && x <= card_right && y_content >= 286 && y_content <= 348 {
            start_page_transition(hwnd, 4, 1);
            return;
        }

        if x >= card_x && x <= card_right && y_content >= 360 && y_content <= 422 {
            start_page_transition(hwnd, 5, 1);
            return;
        }

        if x >= card_x && x <= card_right && y_content >= 434 && y_content <= 496 {
            start_page_transition(hwnd, 2, 1);
            return;
        }

        if x >= card_x && x <= card_right && y_content >= 508 && y_content <= 570 {
            start_page_transition(hwnd, 3, 1);
            return;
        }
    } else {
        // --- XỬ LÝ CÁC TRANG CON ---
        // Nút Back tròn cố định ở (card_x, 20)
        if x >= card_x && x <= card_x + 40 && y >= 20 && y <= 60 {
            start_page_transition(hwnd, 0, -1);
            return;
        }
        if y < 70 {
            return;
        }

        let scroll_y = SCROLL_Y.load(Ordering::SeqCst);
        let y_content = y + scroll_y;

        let mut state = GLOBAL_STATE.lock().unwrap();
        let mut changed = false;

        if page == 1 {
            let tab_x = right_ctrl - 200;
            if y_content >= 95 && y_content <= 145 {
                if x >= tab_x && x <= tab_x + 96 {
                    if state.config.input_method != InputMethod::Telex {
                        state.config.input_method = InputMethod::Telex;
                        changed = true;
                    }
                } else if x >= tab_x + 104 && x <= right_ctrl {
                    if state.config.input_method != InputMethod::Vni {
                        state.config.input_method = InputMethod::Vni;
                        state.config.simple_telex = false;
                        changed = true;
                    }
                }
            }

            if x >= right_ctrl - 52 && x <= right_ctrl {
                if y_content >= 180 && y_content <= 224 {
                    state.config.modern_tone = !state.config.modern_tone;
                    changed = true;
                } else if y_content >= 252 && y_content <= 296 {
                    state.config.free_marking = !state.config.free_marking;
                    changed = true;
                } else if y_content >= 324 && y_content <= 368 {
                    state.config.modify_tone_in_word = !state.config.modify_tone_in_word;
                    changed = true;
                } else if y_content >= 396 && y_content <= 440 {
                    state.config.smart_english = !state.config.smart_english;
                    changed = true;
                } else if y_content >= 468 && y_content <= 512 {
                    state.config.auto_restore = !state.config.auto_restore;
                    changed = true;
                } else if y_content >= 540 && y_content <= 584 {
                    if state.config.input_method == InputMethod::Telex {
                        state.config.simple_telex = !state.config.simple_telex;
                        changed = true;
                    }
                } else if y_content >= 612 && y_content <= 656 {
                    state.config.dynamic_caret = !state.config.dynamic_caret;
                    changed = true;
                }
            }
        } else if page == 2 {
            let sw_tab_x = card_x + 20;
            let sw_total_w = card_w - 40;
            let sw_tab_w = (sw_total_w - 8) / 2;
            if y_content >= 152 && y_content <= 196 {
                if x >= sw_tab_x && x <= sw_tab_x + sw_tab_w + 4 {
                    if state.config.switch_key != SwitchKey::CtrlShift {
                        state.config.switch_key = SwitchKey::CtrlShift;
                        changed = true;
                    }
                } else if x >= sw_tab_x + sw_tab_w + 4 && x <= sw_tab_x + sw_total_w {
                    if state.config.switch_key != SwitchKey::AltZ {
                        state.config.switch_key = SwitchKey::AltZ;
                        changed = true;
                    }
                }
            } else if x >= right_ctrl - 52 && x <= right_ctrl && y_content >= 234 && y_content <= 274 {
                let new_val = !state.config.start_with_windows;
                state.config.start_with_windows = new_val;
                crate::autostart::set_autostart(new_val);
                changed = true;
            } else if x >= right_ctrl - 52 && x <= right_ctrl && y_content >= 346 && y_content <= 386 {
                state.config.notify_on_startup = !state.config.notify_on_startup;
                changed = true;
            } else if x >= right_ctrl - 52 && x <= right_ctrl && y_content >= 418 && y_content <= 458 {
                state.config.notify_on_toggle = !state.config.notify_on_toggle;
                changed = true;
            } else if x >= right_ctrl - 52 && x <= right_ctrl && y_content >= 498 && y_content <= 538 {
                state.config.limit_ram = !state.config.limit_ram;
                crate::apply_ram_limiter(&state.config);
                changed = true;
            } else {
                let anim_p = RAM_LIMIT_ANIM.load(Ordering::SeqCst);
                if anim_p > 200 {
                    let pill_y = 598;
                    let pill_h = 34;
                    if y_content >= pill_y && y_content <= pill_y + pill_h {
                        let total_w = card_w - 40;
                        let btn_w = (total_w - 18) / 4;
                        let presets = [0, 1, 2, 4];
                        for (idx, &mb) in presets.iter().enumerate() {
                            let bx = (card_x + 20) + idx as i32 * (btn_w + 6);
                            if x >= bx && x <= bx + btn_w {
                                if state.config.max_ram_mb != mb {
                                    state.config.max_ram_mb = mb;
                                    crate::apply_ram_limiter(&state.config);
                                    changed = true;
                                }
                                break;
                            }
                        }
                    }
                }
            }
        } else if page == 3 {
            // Nút Kiểm tra cập nhật
            let btn_update_w = 240;
            let btn_update_x = card_x + (card_w - btn_update_w) / 2;
            let btn_update_y = 250;
            let btn_update_h = 42;
            if x >= btn_update_x && x <= btn_update_x + btn_update_w && y_content >= btn_update_y && y_content <= btn_update_y + btn_update_h {
                drop(state);
                crate::updater::start_checking_update(hwnd);
                return;
            }
        } else if page == 4 {
            // Hỗ trợ viết
            if x >= right_ctrl - 52 && x <= right_ctrl {
                if y_content >= 96 && y_content <= 140 {
                    state.config.auto_caps_line = !state.config.auto_caps_line;
                    changed = true;
                } else if y_content >= 170 && y_content <= 214 {
                    state.config.auto_caps_sentence = !state.config.auto_caps_sentence;
                    changed = true;
                } else if y_content >= 244 && y_content <= 288 {
                    state.config.auto_period_enter = !state.config.auto_period_enter;
                    changed = true;
                }
            }
        } else if page == 5 {
            // Hỗ trợ Tương thích trình duyệt trong Nâng cao
            if x >= right_ctrl - 52 && x <= right_ctrl && y_content >= 96 && y_content <= 140 {
                state.config.browser_compatibility = !state.config.browser_compatibility;
                changed = true;
            }

            let anim_p = BROWSER_INPUT_ANIM.load(Ordering::SeqCst) as f32 / 1000.0;
            let anim_ease = 1.0 - (1.0 - anim_p).powi(3);
            let card1_h = (68.0 + 64.0 * anim_ease).round() as i32;
            let card2_y = 86 + card1_h + 14;
            if x >= right_ctrl - 52 && x <= right_ctrl && y_content >= card2_y + 10 && y_content <= card2_y + 54 {
                state.config.remember_app_state = !state.config.remember_app_state;
                changed = true;
            }

            let input_anim = BROWSER_INPUT_ANIM.load(Ordering::SeqCst);
            if input_anim > 200 {
                let box_x = card_x + 20;
                let box_w = card_w - 40;
                let box_y = 160;
                let box_h = 42;
                if x >= box_x && x <= box_x + box_w && y_content >= box_y && y_content <= box_y + box_h {
                    BROWSER_INPUT_FOCUSED.store(true, Ordering::SeqCst);
                    unsafe { InvalidateRect(hwnd, null_mut(), 0); }
                } else {
                    if BROWSER_INPUT_FOCUSED.swap(false, Ordering::SeqCst) {
                        unsafe { InvalidateRect(hwnd, null_mut(), 0); }
                    }
                }
            } else {
                if BROWSER_INPUT_FOCUSED.swap(false, Ordering::SeqCst) {
                    unsafe { InvalidateRect(hwnd, null_mut(), 0); }
                }
            }
        }

        if changed {
            state.config.save();
            let main_hwnd = state.main_hwnd;
            drop(state);
            unsafe {
                SetTimer(hwnd, TIMER_ID_ANIM, 16, None);
                InvalidateRect(hwnd, null_mut(), 0);
                if main_hwnd != 0 {
                    PostMessageW(main_hwnd as HWND, WM_APP_UPDATE_TRAY, 0, 0);
                }
            }
        }
    }
}

fn start_page_transition(hwnd: HWND, target_page: u8, direction: i32) {
    HOVER_ITEM.store(-1, Ordering::SeqCst);
    SCROLL_Y.store(0, Ordering::SeqCst);
    TARGET_SCROLL_Y.store(0, Ordering::SeqCst);
    if target_page == 3 {
        crate::updater::ensure_changelog_loaded(hwnd);
    }
    TARGET_PAGE.store(target_page, Ordering::SeqCst);
    TRANSITION_DIR.store(direction, Ordering::SeqCst);
    TRANSITION_PROGRESS.store(0, Ordering::SeqCst);
    unsafe {
        SetTimer(hwnd, TIMER_ID_ANIM, 16, None);
        InvalidateRect(hwnd, null_mut(), 0);
    }
}

#[derive(Clone, Copy)]
struct CachedFonts {
    family: *mut GpFontFamily,
    font_title: *mut GpFont,
    font_sub_title: *mut GpFont,
    font_modal_title: *mut GpFont,
    font_bold: *mut GpFont,
    font_desc: *mut GpFont,
    font_section: *mut GpFont,
    font_tab: *mut GpFont,
    font_btn: *mut GpFont,
}

unsafe impl Send for CachedFonts {}
unsafe impl Sync for CachedFonts {}

static CACHED_FONTS: Mutex<Option<CachedFonts>> = Mutex::new(None);

unsafe fn get_cached_fonts() -> CachedFonts {
    let mut lock = CACHED_FONTS.lock().unwrap();
    if let Some(fonts) = *lock {
        return fonts;
    }

    let mut family: *mut GpFontFamily = null_mut();
    let family_name = to_wide("Google Sans");
    GdipCreateFontFamilyFromName(family_name.as_ptr(), null_mut(), &mut family);
    if family.is_null() {
        let fallback = to_wide("Segoe UI Variable Text");
        GdipCreateFontFamilyFromName(fallback.as_ptr(), null_mut(), &mut family);
    }
    if family.is_null() {
        let fallback2 = to_wide("Segoe UI");
        GdipCreateFontFamilyFromName(fallback2.as_ptr(), null_mut(), &mut family);
    }

    let fonts = CachedFonts {
        family,
        font_title: create_gdip_font(family, 24.0, FontStyleBold),
        font_sub_title: create_gdip_font(family, 21.0, FontStyleBold),
        font_modal_title: create_gdip_font(family, 18.0, FontStyleBold),
        font_bold: create_gdip_font(family, 15.0, FontStyleBold),
        font_desc: create_gdip_font(family, 12.0, FontStyleRegular),
        font_section: create_gdip_font(family, 11.0, FontStyleBold),
        font_tab: create_gdip_font(family, 12.0, FontStyleBold),
        font_btn: create_gdip_font(family, 14.0, FontStyleBold),
    };

    *lock = Some(fonts);
    fonts
}

unsafe fn paint_shadcn_ui(dc: windows_sys::Win32::Graphics::Gdi::HDC, w: i32, h: i32, config: &Config) {
    let mut graphics: *mut GpGraphics = null_mut();
    GdipCreateFromHDC(dc, &mut graphics);
    if graphics.is_null() {
        return;
    }

    GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);
    GdipSetTextRenderingHint(graphics, TextRenderingHintClearTypeGridFit);

    // Tô nền Zinc #09090b
    let mut bg_brush: *mut GpSolidFill = null_mut();
    GdipCreateSolidFill(ARGB_BG, &mut bg_brush);
    let mut bg_path: *mut GpPath = null_mut();
    GdipCreatePath(FillModeAlternate, &mut bg_path);
    GdipAddPathLine(bg_path, 0.0, 0.0, w as f32, 0.0);
    GdipAddPathLine(bg_path, w as f32, 0.0, w as f32, h as f32);
    GdipAddPathLine(bg_path, w as f32, h as f32, 0.0, h as f32);
    GdipClosePathFigure(bg_path);
    GdipFillPath(graphics, bg_brush as *mut GpBrush, bg_path);
    GdipDeletePath(bg_path);
    GdipDeleteBrush(bg_brush as *mut GpBrush);

    let fonts = get_cached_fonts();
    let font_title = fonts.font_title;
    let font_sub_title = fonts.font_sub_title;
    let font_modal_title = fonts.font_modal_title;
    let font_bold = fonts.font_bold;
    let font_desc = fonts.font_desc;
    let font_section = fonts.font_section;
    let font_tab = fonts.font_tab;
    let font_btn = fonts.font_btn;

    let (card_x, card_w, _card_right, right_ctrl, left_text, _) = get_layout_metrics(w);

    let current_page = CURRENT_PAGE.load(Ordering::SeqCst);
    let target_page = TARGET_PAGE.load(Ordering::SeqCst);
    let trans_raw = TRANSITION_PROGRESS.load(Ordering::SeqCst);
    let trans_dir = TRANSITION_DIR.load(Ordering::SeqCst);
    let hover_id = HOVER_ITEM.load(Ordering::SeqCst);

    let progress = trans_raw as f32 / 1000.0;
    let ease = 1.0 - (1.0 - progress).powi(3);
    let slide_distance = (card_w + 48) as f32;

    // Đặt vùng Clip cho nội dung cuộn (0..584), giữ dock đáy cố định hoàn toàn
    GdipSetClipRectI(graphics, 0, 0, w, 584, CombineModeReplace);

    let scroll_y = SCROLL_Y.load(Ordering::SeqCst) as f32;

    if trans_raw >= 1000 {
        render_page_elements(
            graphics,
            current_page,
            0.0,
            scroll_y,
            card_x,
            card_w,
            right_ctrl,
            left_text,
            config,
            hover_id,
            font_title,
            font_sub_title,
            font_bold,
            font_desc,
            font_section,
            font_tab,
            font_btn,
        );
    } else {
        if trans_dir == 1 {
            let offset_curr = -slide_distance * ease;
            let offset_target = slide_distance * (1.0 - ease);

            render_page_elements(
                graphics,
                current_page,
                offset_curr,
                scroll_y,
                card_x,
                card_w,
                right_ctrl,
                left_text,
                config,
                hover_id,
                font_title,
                font_sub_title,
                font_bold,
                font_desc,
                font_section,
                font_tab,
                font_btn,
            );

            render_page_elements(
                graphics,
                target_page,
                offset_target,
                0.0,
                card_x,
                card_w,
                right_ctrl,
                left_text,
                config,
                hover_id,
                font_title,
                font_sub_title,
                font_bold,
                font_desc,
                font_section,
                font_tab,
                font_btn,
            );
        } else {
            let offset_curr = slide_distance * ease;
            let offset_target = -slide_distance * (1.0 - ease);

            render_page_elements(
                graphics,
                current_page,
                offset_curr,
                scroll_y,
                card_x,
                card_w,
                right_ctrl,
                left_text,
                config,
                hover_id,
                font_title,
                font_sub_title,
                font_bold,
                font_desc,
                font_section,
                font_tab,
                font_btn,
            );

            render_page_elements(
                graphics,
                target_page,
                offset_target,
                0.0,
                card_x,
                card_w,
                right_ctrl,
                left_text,
                config,
                hover_id,
                font_title,
                font_sub_title,
                font_bold,
                font_desc,
                font_section,
                font_tab,
                font_btn,
            );
        }
    }

    // Bỏ clip để vẽ Fixed Bottom Dock độc lập
    GdipResetClip(graphics);

    // Fixed Bottom Dock 2 nút Thoát & Đóng (y = 584..h) - Gọn gàng, vừa vặn cân đối
    let dock_y = 584;
    let mut dock_brush: *mut GpSolidFill = null_mut();
    GdipCreateSolidFill(ARGB_BG, &mut dock_brush);
    GdipFillRectangleI(graphics, dock_brush as *mut GpBrush, 0, dock_y, w, h - dock_y);
    GdipDeleteBrush(dock_brush as *mut GpBrush);

    gdiplus_draw_line(graphics, 0, dock_y, w, dock_y, ARGB_DIVIDER, 1.0);

    let btn_w = (card_w - 12) / 2;
    let btn_y = 600;
    let btn_h = 44;

    let exit_bg = if hover_id == 30 { ARGB_DANGER_HOVER } else { ARGB_CARD_BG };
    gdiplus_draw_rounded_card(graphics, card_x, btn_y, btn_w, btn_h, 8, exit_bg, ARGB_DANGER_BORDER, 1.0);
    draw_gdip_text_centered(graphics, font_btn, "Thoát", card_x, btn_y, btn_w, btn_h, ARGB_DANGER_TEXT);

    let btn_close_x = card_x + btn_w + 12;
    let close_bg = if hover_id == 31 { ARGB_WHITE_HOVER } else { ARGB_WHITE };
    gdiplus_draw_rounded_card(graphics, btn_close_x, btn_y, btn_w, btn_h, 8, close_bg, ARGB_WHITE, 1.0);
    draw_gdip_text_centered(graphics, font_btn, "Đóng", btn_close_x, btn_y, btn_w, btn_h, ARGB_TEXT_DARK);

    // ========================================================
    // 🌟 CUSTOM MODAL POPUP (REAL BLUR NỀN, BO GÓC, POP-OUT ANIMATION)
    // ========================================================
    let modal_state = crate::updater::MODAL_STATE.load(Ordering::SeqCst);
    if modal_state > 0 {
        // 1. Commit and delete current GDI+ graphics to flush DC
        GdipDeleteGraphics(graphics);

        // 2. Thực hiện Real Gaussian/Box Blur lên toàn bộ giao diện phía sau
        apply_screen_blur(dc, w, h);

        // 3. Khởi tạo lại GDI+ Graphics trên DC đã làm mờ thật sự
        GdipCreateFromHDC(dc, &mut graphics);
        GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);
        GdipSetInterpolationMode(graphics, InterpolationModeHighQualityBicubic);
        GdipSetPixelOffsetMode(graphics, PixelOffsetModeHighQuality);
        GdipSetTextRenderingHint(graphics, TextRenderingHintClearTypeGridFit);

        // 4. Animation Fade-in mượt mà (thuần Alpha, không phóng to thu nhỏ)
        let anim_p = MODAL_ANIM_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
        let ease = 1.0 - (1.0 - anim_p).powi(3);

        // Lớp kính mờ tối (Dark Frosted Glass Tint)
        let overlay_alpha = (0x99 as f32 * ease).round() as u32;
        let overlay_color = (overlay_alpha << 24) | 0x09090b;

        let mut overlay_brush: *mut GpSolidFill = null_mut();
        GdipCreateSolidFill(overlay_color, &mut overlay_brush);
        let mut overlay_path: *mut GpPath = null_mut();
        GdipCreatePath(FillModeAlternate, &mut overlay_path);
        GdipAddPathLine(overlay_path, 0.0, 0.0, w as f32, 0.0);
        GdipAddPathLine(overlay_path, w as f32, 0.0, w as f32, h as f32);
        GdipAddPathLine(overlay_path, w as f32, h as f32, 0.0, h as f32);
        GdipClosePathFigure(overlay_path);
        GdipFillPath(graphics, overlay_brush as *mut GpBrush, overlay_path);
        GdipDeletePath(overlay_path);
        GdipDeleteBrush(overlay_brush as *mut GpBrush);

        // Helper áp dụng độ mờ Fade cho mọi màu sắc trong Modal
        let fade_color = |c: u32| -> u32 {
            let a = (((c >> 24) & 0xFF) as f32 * ease).round() as u32;
            (a << 24) | (c & 0x00FFFFFF)
        };

        // 5. Card Modal bo góc cố định ở giữa màn hình (Fade-in thanh lịch)
        let modal_w = 400;
        let modal_h = 230;
        let modal_x = (w - modal_w) / 2;
        let modal_y = (h - modal_h) / 2;

        gdiplus_draw_rounded_card(
            graphics,
            modal_x,
            modal_y,
            modal_w,
            modal_h,
            14,
            fade_color(ARGB_MODAL_BG),
            fade_color(ARGB_MODAL_BORDER),
            1.5,
        );

        let modal_btn_y = modal_y + 165;
        let btn_h = 42;

        match modal_state {
            1 => {
                // Đang kiểm tra cập nhật...
                draw_gdip_text_centered(graphics, font_modal_title, "Đang kiểm tra cập nhật...", modal_x, modal_y + 60, modal_w, 32, fade_color(ARGB_TEXT_PRIMARY));
                draw_gdip_text_centered(graphics, font_desc, "Đang kết nối máy chủ GitHub, vui lòng chờ.", modal_x, modal_y + 100, modal_w, 24, fade_color(ARGB_TEXT_MUTED));
            }
            2 => {
                // Phát hiện bản mới!
                let tag = crate::updater::LATEST_VERSION_TAG.lock().unwrap().clone();
                draw_gdip_text_centered(graphics, font_modal_title, "Có bản cập nhật mới", modal_x, modal_y + 35, modal_w, 32, fade_color(ARGB_TEXT_PRIMARY));

                let desc = format!("Bản {} đã sẵn sàng. Bạn có muốn tải và cài đặt ngay không?", tag);
                draw_gdip_text_multiline(graphics, font_desc, &desc, modal_x + 30, modal_y + 75, modal_w - 60, 60, fade_color(ARGB_TEXT_MUTED));

                // Nút "Để sau" (trái): ID 51
                let btn_w = 166;
                let cancel_x = modal_x + 24;
                let cancel_bg = if hover_id == 51 { ARGB_CARD_HOVER } else { ARGB_CARD_BG };
                gdiplus_draw_rounded_card(graphics, cancel_x, modal_btn_y, btn_w, btn_h, 8, fade_color(cancel_bg), fade_color(ARGB_CARD_BORDER), 1.0);
                draw_gdip_text_centered(graphics, font_btn, "Để sau", cancel_x, modal_btn_y, btn_w, btn_h, fade_color(ARGB_TEXT_MUTED));

                // Nút "Cập nhật ngay" (phải): ID 50
                let update_x = modal_x + 210;
                let update_bg = if hover_id == 50 { ARGB_WHITE_HOVER } else { ARGB_WHITE };
                gdiplus_draw_rounded_card(graphics, update_x, modal_btn_y, btn_w, btn_h, 8, fade_color(update_bg), fade_color(ARGB_WHITE), 1.0);
                draw_gdip_text_centered(graphics, font_btn, "Cập nhật ngay", update_x, modal_btn_y, btn_w, btn_h, fade_color(ARGB_TEXT_DARK));
            }
            3 => {
                // Đang tải cập nhật...
                draw_gdip_text_centered(graphics, font_modal_title, "Đang tải bản cập nhật...", modal_x, modal_y + 60, modal_w, 32, fade_color(ARGB_TEXT_PRIMARY));
                draw_gdip_text_centered(graphics, font_desc, "HoyuKey sẽ tự động khởi động lại sau khi tải xong.", modal_x, modal_y + 100, modal_w, 24, fade_color(ARGB_TEXT_MUTED));
            }
            4 => {
                // Bản mới nhất!
                draw_gdip_text_centered(graphics, font_modal_title, "Bạn đang ở bản mới nhất", modal_x, modal_y + 40, modal_w, 32, fade_color(ARGB_TEXT_PRIMARY));
                let desc = format!("HoyuKey v{} đã là phiên bản mới nhất.", crate::updater::CURRENT_VERSION);
                draw_gdip_text_centered(graphics, font_desc, &desc, modal_x, modal_y + 85, modal_w, 24, fade_color(ARGB_TEXT_MUTED));

                // Nút "Đã hiểu": ID 50
                let btn_w = 160;
                let ok_btn_x = modal_x + (modal_w - btn_w) / 2;
                let ok_bg = if hover_id == 50 { ARGB_WHITE_HOVER } else { ARGB_WHITE };
                gdiplus_draw_rounded_card(graphics, ok_btn_x, modal_btn_y, btn_w, btn_h, 8, fade_color(ok_bg), fade_color(ARGB_WHITE), 1.0);
                draw_gdip_text_centered(graphics, font_btn, "Đã hiểu", ok_btn_x, modal_btn_y, btn_w, btn_h, fade_color(ARGB_TEXT_DARK));
            }
            5 => {
                // Lỗi kết nối - Dùng màu chữ ARGB_TEXT_PRIMARY đồng nhất, sang trọng, không dùng màu đỏ
                draw_gdip_text_centered(graphics, font_modal_title, "Không kiểm tra được cập nhật", modal_x, modal_y + 40, modal_w, 32, fade_color(ARGB_TEXT_PRIMARY));
                draw_gdip_text_centered(graphics, font_desc, "Vui lòng kiểm tra lại kết nối mạng Internet.", modal_x, modal_y + 85, modal_w, 24, fade_color(ARGB_TEXT_MUTED));

                let btn_w = 160;
                let ok_btn_x = modal_x + (modal_w - btn_w) / 2;
                let ok_bg = if hover_id == 50 { ARGB_WHITE_HOVER } else { ARGB_WHITE };
                gdiplus_draw_rounded_card(graphics, ok_btn_x, modal_btn_y, btn_w, btn_h, 8, fade_color(ok_bg), fade_color(ARGB_WHITE), 1.0);
                draw_gdip_text_centered(graphics, font_btn, "Đã hiểu", ok_btn_x, modal_btn_y, btn_w, btn_h, fade_color(ARGB_TEXT_DARK));
            }
            _ => {}
        }
    }

    GdipDeleteGraphics(graphics);
}

unsafe fn render_page_elements(
    graphics: *mut GpGraphics,
    page: u8,
    offset_x: f32,
    scroll_y: f32,
    base_card_x: i32,
    card_w: i32,
    base_right_ctrl: i32,
    base_left_text: i32,
    config: &Config,
    hover_id: i32,
    font_title: *mut GpFont,
    font_sub_title: *mut GpFont,
    font_bold: *mut GpFont,
    font_desc: *mut GpFont,
    font_section: *mut GpFont,
    font_tab: *mut GpFont,
    font_btn: *mut GpFont,
) {
    let card_x = (base_card_x as f32 + offset_x).round() as i32;
    let right_ctrl = (base_right_ctrl as f32 + offset_x).round() as i32;
    let left_text = (base_left_text as f32 + offset_x).round() as i32;
    let sy = scroll_y.round() as i32;

    if page == 0 {
        // ========================================================
        // 🏠 TRANG CHỦ (HOME DASHBOARD) - CÓ THỂ CUỘN CHUỘT MƯỢT MÀ
        // ========================================================


        // 1. Header tinh gọn: Logo 48x48 + Tiêu đề "DASHBOARD"
        gdiplus_draw_image(graphics, card_x, 22 - sy, 48, 48, LOGO_BGRA_128, 128, 128);
        draw_gdip_text(graphics, font_title, "DASHBOARD", card_x + 60, 26 - sy, ARGB_TEXT_PRIMARY, false);

        // 2. Card Trạng thái hệ thống (y = 86..158, cao 72px)
        let mode_p = MODE_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
        let mode_ease = 1.0 - (1.0 - mode_p).powi(3);

        let status_bg = if hover_id == 10 { ARGB_CARD_HOVER } else { ARGB_CARD_BG };
        gdiplus_draw_rounded_card(graphics, card_x, 86 - sy, card_w, 72, 10, status_bg, ARGB_CARD_BORDER, 1.0);

        // Icon E và V ở vị trí bên trái (chỉ có icon thay đổi với animation mượt mà)
        let icon_x = left_text;
        let icon_y = 104 - sy;
        let icon_size = 36;

        gdiplus_draw_image_alpha(graphics, icon_x, icon_y, icon_size, icon_size, E_BGRA_32, 32, 32, 1.0 - mode_ease);
        gdiplus_draw_image_alpha(graphics, icon_x, icon_y, icon_size, icon_size, V_BGRA_32, 32, 32, mode_ease);

        // Tiêu đề & mô tả trạng thái (đặt ngay bên phải icon)
        let text_x = icon_x + icon_size + 16;
        let status_str = if mode_ease >= 0.5 {
            "Tiếng Việt đang bật"
        } else {
            "Tiếng Việt đang tắt"
        };
        draw_gdip_text(graphics, font_bold, status_str, text_x, 100 - sy, ARGB_TEXT_PRIMARY, false);
        draw_gdip_text(graphics, font_desc, "Bấm để đổi chế độ hoặc dùng phím tắt", text_x, 124 - sy, ARGB_TEXT_MUTED, false);

        // 3. Tiêu đề nhóm CÀI ĐẶT (cách status card 26px, cách Card 1 28px - không còn bị dính sát!)
        draw_gdip_text(graphics, font_section, "CÀI ĐẶT", card_x + 4, 184 - sy, ARGB_STATUS_MUTED, false);

        // 4. Menu item 1: Cài đặt đánh máy (y = 212..274, h = 62)
        let menu1_bg = if hover_id == 11 { ARGB_CARD_HOVER } else { ARGB_CARD_BG };
        gdiplus_draw_rounded_card(graphics, card_x, 212 - sy, card_w, 62, 10, menu1_bg, ARGB_CARD_BORDER, 1.0);
        gdiplus_draw_rounded_card(graphics, left_text, 223 - sy, 40, 40, 20, ARGB_PILL_TRACK, ARGB_CARD_BORDER, 1.0);
        draw_vector_keyboard_icon(graphics, left_text + 20, 243 - sy, ARGB_TEXT_PRIMARY);

        draw_gdip_text(graphics, font_bold, "Cài đặt đánh máy", left_text + 52, 222 - sy, ARGB_TEXT_PRIMARY, false);
        draw_gdip_text(graphics, font_desc, "Kiểu gõ, bỏ dấu tự do, nhận diện tiếng Anh", left_text + 52, 246 - sy, ARGB_TEXT_MUTED, false);
        let arrow1_color = if hover_id == 11 { ARGB_TEXT_PRIMARY } else { ARGB_TEXT_MUTED };
        draw_vector_chevron_right(graphics, right_ctrl - 12, 243 - sy, arrow1_color);

        // 5. Menu item 2: Hỗ trợ viết (y = 286..348, h = 62)
        let menu2_bg = if hover_id == 14 { ARGB_CARD_HOVER } else { ARGB_CARD_BG };
        gdiplus_draw_rounded_card(graphics, card_x, 286 - sy, card_w, 62, 10, menu2_bg, ARGB_CARD_BORDER, 1.0);
        gdiplus_draw_rounded_card(graphics, left_text, 297 - sy, 40, 40, 20, ARGB_PILL_TRACK, ARGB_CARD_BORDER, 1.0);
        draw_vector_pen_icon(graphics, left_text + 20, 317 - sy, ARGB_TEXT_PRIMARY);

        draw_gdip_text(graphics, font_bold, "Hỗ trợ viết", left_text + 52, 296 - sy, ARGB_TEXT_PRIMARY, false);
        draw_gdip_text(graphics, font_desc, "Viết hoa đầu dòng, sau dấu chấm, tự thêm chấm", left_text + 52, 320 - sy, ARGB_TEXT_MUTED, false);
        let arrow2_color = if hover_id == 14 { ARGB_TEXT_PRIMARY } else { ARGB_TEXT_MUTED };
        draw_vector_chevron_right(graphics, right_ctrl - 12, 317 - sy, arrow2_color);

        // 6. Menu item 3: Nâng cao (y = 360..422, h = 62)
        let menu_adv_bg = if hover_id == 15 { ARGB_CARD_HOVER } else { ARGB_CARD_BG };
        gdiplus_draw_rounded_card(graphics, card_x, 360 - sy, card_w, 62, 10, menu_adv_bg, ARGB_CARD_BORDER, 1.0);
        gdiplus_draw_rounded_card(graphics, left_text, 371 - sy, 40, 40, 20, ARGB_PILL_TRACK, ARGB_CARD_BORDER, 1.0);
        draw_vector_gear_icon(graphics, left_text + 20, 391 - sy, ARGB_TEXT_PRIMARY);

        draw_gdip_text(graphics, font_bold, "Nâng cao", left_text + 52, 370 - sy, ARGB_TEXT_PRIMARY, false);
        draw_gdip_text(graphics, font_desc, "Tương thích trình duyệt và các ứng dụng", left_text + 52, 394 - sy, ARGB_TEXT_MUTED, false);
        let arrow_adv_color = if hover_id == 15 { ARGB_TEXT_PRIMARY } else { ARGB_TEXT_MUTED };
        draw_vector_chevron_right(graphics, right_ctrl - 12, 391 - sy, arrow_adv_color);

        // 7. Menu item 4: Cài đặt HoyuKey (y = 434..496, h = 62)
        let menu3_bg = if hover_id == 12 { ARGB_CARD_HOVER } else { ARGB_CARD_BG };
        gdiplus_draw_rounded_card(graphics, card_x, 434 - sy, card_w, 62, 10, menu3_bg, ARGB_CARD_BORDER, 1.0);
        gdiplus_draw_rounded_card(graphics, left_text, 445 - sy, 40, 40, 20, ARGB_PILL_TRACK, ARGB_CARD_BORDER, 1.0);
        draw_vector_sliders_icon(graphics, left_text + 20, 465 - sy, ARGB_TEXT_PRIMARY);

        draw_gdip_text(graphics, font_bold, "Cài đặt HoyuKey", left_text + 52, 444 - sy, ARGB_TEXT_PRIMARY, false);
        draw_gdip_text(graphics, font_desc, "Phím tắt chuyển đổi, khởi động cùng máy", left_text + 52, 468 - sy, ARGB_TEXT_MUTED, false);
        let arrow3_color = if hover_id == 12 { ARGB_TEXT_PRIMARY } else { ARGB_TEXT_MUTED };
        draw_vector_chevron_right(graphics, right_ctrl - 12, 465 - sy, arrow3_color);

        // 8. Menu item 5: Giới thiệu (y = 508..570, h = 62)
        let menu4_bg = if hover_id == 13 { ARGB_CARD_HOVER } else { ARGB_CARD_BG };
        gdiplus_draw_rounded_card(graphics, card_x, 508 - sy, card_w, 62, 10, menu4_bg, ARGB_CARD_BORDER, 1.0);
        gdiplus_draw_rounded_card(graphics, left_text, 519 - sy, 40, 40, 20, ARGB_PILL_TRACK, ARGB_CARD_BORDER, 1.0);
        draw_vector_info_icon(graphics, left_text + 20, 539 - sy, ARGB_TEXT_PRIMARY);

        draw_gdip_text(graphics, font_bold, "Giới thiệu", left_text + 52, 518 - sy, ARGB_TEXT_PRIMARY, false);
        draw_gdip_text(graphics, font_desc, "Phiên bản, kiểm tra cập nhật và nhật ký", left_text + 52, 542 - sy, ARGB_TEXT_MUTED, false);
        let arrow4_color = if hover_id == 13 { ARGB_TEXT_PRIMARY } else { ARGB_TEXT_MUTED };
        draw_vector_chevron_right(graphics, right_ctrl - 12, 539 - sy, arrow4_color);

    } else {
        // ========================================================
        // 📱 CÁC TRANG CON (SUBPAGES) - CÁC THÀNH PHẦN CUỘN NẰM PHÍA SAU
        // ========================================================
        if page == 1 {
            // Card 1: Kiểu gõ chính (y = 86..162, cao 76px)
            gdiplus_draw_rounded_card(graphics, card_x, 86 - sy, card_w, 76, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);
            draw_gdip_text(graphics, font_bold, "Kiểu gõ chính", left_text, 99 - sy, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Telex hoặc VNI", left_text, 125 - sy, ARGB_TEXT_MUTED, false);

            let method_progress = INPUT_METHOD_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_tabs(
                graphics,
                right_ctrl - 200,
                102 - sy,
                &["Telex", "VNI"],
                96,
                36,
                method_progress,
                font_tab,
            );

            // Card 2: 7 Tùy chọn đánh máy thông minh (y = 174..674, cao 500px)
            gdiplus_draw_rounded_card(graphics, card_x, 174 - sy, card_w, 500, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);

            // 1. Bỏ dấu kiểu mới
            draw_gdip_text(graphics, font_bold, "Bỏ dấu kiểu mới", left_text, 188 - sy, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Đặt dấu ở âm thứ hai (hòa, thúy thay vì hoà, thuý)", left_text, 212 - sy, ARGB_TEXT_MUTED, false);
            let tone_progress = MODERN_TONE_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_switch(graphics, right_ctrl - 50, 194 - sy, tone_progress);

            gdiplus_draw_line(graphics, left_text, 246 - sy, right_ctrl, 246 - sy, ARGB_DIVIDER, 1.0);

            // 2. Bỏ dấu tự do
            draw_gdip_text(graphics, font_bold, "Bỏ dấu tự do", left_text, 260 - sy, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Gõ dấu hoặc thêm mũ ở bất kỳ vị trí nào trong từ", left_text, 284 - sy, ARGB_TEXT_MUTED, false);
            let free_marking_progress = FREE_MARKING_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_switch(graphics, right_ctrl - 50, 266 - sy, free_marking_progress);

            gdiplus_draw_line(graphics, left_text, 318 - sy, right_ctrl, 318 - sy, ARGB_DIVIDER, 1.0);

            // 3. Tự chỉnh dấu khi sửa từ
            draw_gdip_text(graphics, font_bold, "Tự chỉnh dấu khi sửa từ", left_text, 332 - sy, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Tự động đổi vị trí dấu khi chèn ký tự vào giữa từ", left_text, 356 - sy, ARGB_TEXT_MUTED, false);
            let modify_progress = MODIFY_TONE_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_switch(graphics, right_ctrl - 50, 338 - sy, modify_progress);

            gdiplus_draw_line(graphics, left_text, 390 - sy, right_ctrl, 390 - sy, ARGB_DIVIDER, 1.0);

            // 4. Nhận diện tiếng Anh
            draw_gdip_text(graphics, font_bold, "Nhận diện tiếng Anh", left_text, 404 - sy, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Giữ nguyên từ tiếng Anh thông dụng (file, fix, link, game)", left_text, 428 - sy, ARGB_TEXT_MUTED, false);
            let smart_english_progress = SMART_ENGLISH_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_switch(graphics, right_ctrl - 50, 410 - sy, smart_english_progress);

            gdiplus_draw_line(graphics, left_text, 462 - sy, right_ctrl, 462 - sy, ARGB_DIVIDER, 1.0);

            // 5. Khôi phục phím khi gõ sai
            draw_gdip_text(graphics, font_bold, "Khôi phục phím khi gõ sai", left_text, 476 - sy, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Hoàn lại ký tự ban đầu khi nhập từ không hợp lệ", left_text, 500 - sy, ARGB_TEXT_MUTED, false);
            let restore_progress = AUTO_RESTORE_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_switch(graphics, right_ctrl - 50, 482 - sy, restore_progress);

            gdiplus_draw_line(graphics, left_text, 534 - sy, right_ctrl, 534 - sy, ARGB_DIVIDER, 1.0);

            // 6. Telex giản lược (mặc định tắt, tự động disable khi chọn VNI)
            let is_vni = config.input_method == InputMethod::Vni;
            let telex_title_color = if is_vni { ARGB_STATUS_MUTED } else { ARGB_TEXT_PRIMARY };
            let telex_desc_color = if is_vni { 0x50808080 } else { ARGB_TEXT_MUTED };
            draw_gdip_text(graphics, font_bold, "Telex giản lược", left_text, 548 - sy, telex_title_color, false);
            draw_gdip_text(graphics, font_desc, "w đơn lẻ ra w, uow ra uơ, không hoàn tác dấu lặp", left_text, 572 - sy, telex_desc_color, false);
            let simple_progress = if is_vni { 0.0 } else { SIMPLE_TELEX_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0 };
            draw_animated_switch(graphics, right_ctrl - 50, 554 - sy, simple_progress);

            gdiplus_draw_line(graphics, left_text, 606 - sy, right_ctrl, 606 - sy, ARGB_DIVIDER, 1.0);

            // 7. Sửa dấu tự do tại con trỏ (UI Automation)
            draw_gdip_text(graphics, font_bold, "Sửa dấu tự do tại con trỏ", left_text, 620 - sy, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Nhận diện từ khi click chuột/mũi tên để sửa dấu (UI Automation)", left_text, 644 - sy, ARGB_TEXT_MUTED, false);
            let caret_progress = DYNAMIC_CARET_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_switch(graphics, right_ctrl - 50, 626 - sy, caret_progress);

        } else if page == 2 {
            // Card 1: Phím tắt chuyển đổi chế độ (cao 122px, y = 86..208)
            gdiplus_draw_rounded_card(graphics, card_x, 86 - sy, card_w, 122, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);
            draw_gdip_text(graphics, font_bold, "Phím tắt chuyển đổi", left_text, 102 - sy, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Chuyển nhanh giữa Tiếng Việt và Tiếng Anh", left_text, 126 - sy, ARGB_TEXT_MUTED, false);

            let switch_progress = SWITCH_KEY_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            let sw_total_w = card_w - 40;
            let sw_tab_w = (sw_total_w - 8) / 2;
            draw_animated_tabs(
                graphics,
                left_text,
                152 - sy,
                &["Ctrl + Shift", "Alt + Z"],
                sw_tab_w,
                36,
                switch_progress,
                font_tab,
            );

            // Card 2: Khởi động cùng Windows (cao 76px, y = 220..296)
            gdiplus_draw_rounded_card(graphics, card_x, 220 - sy, card_w, 76, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);
            draw_gdip_text(graphics, font_bold, "Khởi động cùng Windows", left_text, 234 - sy, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Tự động chạy HoyuKey khi đăng nhập máy tính", left_text, 258 - sy, ARGB_TEXT_MUTED, false);
            let autostart_progress = START_WITH_WINDOWS_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_switch(graphics, right_ctrl - 50, 241 - sy, autostart_progress);

            // Mục nhỏ "THÔNG BÁO"
            draw_gdip_text(graphics, font_section, "THÔNG BÁO", card_x + 4, 310 - sy, ARGB_STATUS_MUTED, false);

            // Card Thông báo: 2 tùy chọn (cao 144px, y = 332..476)
            gdiplus_draw_rounded_card(graphics, card_x, 332 - sy, card_w, 144, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);

            // 1. Thông báo khởi động
            draw_gdip_text(graphics, font_bold, "Thông báo khi khởi động", left_text, 346 - sy, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "HoyuKey đã kích hoạt và sẵn sàng!", left_text, 370 - sy, ARGB_TEXT_MUTED, false);
            let notify_startup_p = NOTIFY_STARTUP_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_switch(graphics, right_ctrl - 50, 353 - sy, notify_startup_p);

            gdiplus_draw_line(graphics, left_text, 404 - sy, right_ctrl, 404 - sy, ARGB_DIVIDER, 1.0);

            // 2. Thông báo chuyển ngôn ngữ
            draw_gdip_text(graphics, font_bold, "Thông báo chuyển ngôn ngữ", left_text, 418 - sy, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Hiện thông báo mỗi khi bật hoặc tắt Tiếng Việt", left_text, 442 - sy, ARGB_TEXT_MUTED, false);
            let notify_toggle_p = NOTIFY_TOGGLE_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_switch(graphics, right_ctrl - 50, 425 - sy, notify_toggle_p);

            // Card 3: Điều chỉnh RAM chạy nền (Memory Limiter - y = 488..)
            let anim_p = RAM_LIMIT_ANIM.load(Ordering::SeqCst) as f32 / 1000.0;
            let anim_ease = 1.0 - (1.0 - anim_p).powi(3);
            let card3_h = (70.0 + 82.0 * anim_ease).round() as i32;
            let card3_y = 488 - sy;
            gdiplus_draw_rounded_card(graphics, card_x, card3_y, card_w, card3_h, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);

            draw_gdip_text(graphics, font_bold, "Điều chỉnh RAM chạy nền", left_text, card3_y + 14, ARGB_TEXT_PRIMARY, false);
            draw_vector_warning_icon(graphics, left_text + 195, card3_y + 24, ARGB_ALERT_TEXT);
            draw_gdip_text(graphics, font_desc, "Em <3 RAM hãy cho em RAM~", left_text, card3_y + 38, ARGB_TEXT_MUTED, false);
            let ram_limit_p = RAM_LIMIT_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_switch(graphics, right_ctrl - 50, card3_y + 20, ram_limit_p);

            if anim_ease > 0.02 {
                let alpha_u8 = (255.0 * anim_ease).clamp(0.0, 255.0) as u32;
                let divider_color = (alpha_u8 << 24) | (ARGB_DIVIDER & 0x00FFFFFF);
                gdiplus_draw_line(graphics, left_text, card3_y + 64, right_ctrl, card3_y + 64, divider_color, 1.0);

                // 1. Inline Alert Cảnh báo
                let alert_y = card3_y + 72;
                let alert_h = 30;
                let alert_w = card_w - 40;
                let alert_bg = (alpha_u8.min(34) << 24) | (ARGB_ALERT_BG & 0x00FFFFFF);
                let alert_border = (alpha_u8.min(90) << 24) | (ARGB_ALERT_BORDER & 0x00FFFFFF);
                gdiplus_draw_rounded_card(graphics, left_text, alert_y, alert_w, alert_h, 6, alert_bg, alert_border, 1.0);

                let alert_text_color = (alpha_u8 << 24) | (ARGB_ALERT_TEXT & 0x00FFFFFF);
                draw_gdip_text_centered(graphics, font_desc, "⚠ Chỉ sử dụng nếu bạn biết bạn đang làm gì!", left_text, alert_y, alert_w, alert_h, alert_text_color);

                // 2. Các mốc cấu hình (Segmented presets: 512 KB, 1 MB, 2 MB, ĐÙ)
                let pill_y = alert_y + alert_h + 8;
                let pill_h = 32;
                let presets: [(u32, &str); 4] = [(0, "512 KB"), (1, "1 MB"), (2, "2 MB"), (4, "ĐÙ")];
                let current_limit = {
                    let state = GLOBAL_STATE.lock().unwrap();
                    state.config.max_ram_mb
                };
                let btn_w = (alert_w - 18) / 4;

                for (idx, &(mb, label)) in presets.iter().enumerate() {
                    let bx = left_text + idx as i32 * (btn_w + 6);
                    let is_active = current_limit == mb;
                    let (bg, border, text_color) = if is_active {
                        (
                            (alpha_u8 << 24) | (ARGB_WHITE & 0x00FFFFFF),
                            (alpha_u8 << 24) | (ARGB_WHITE & 0x00FFFFFF),
                            (alpha_u8 << 24) | (ARGB_TEXT_DARK & 0x00FFFFFF),
                        )
                    } else {
                        let is_hover = hover_id == (481 + idx as i32);
                        let bg_color = if is_hover { ARGB_CARD_HOVER } else { ARGB_PILL_TRACK };
                        (
                            (alpha_u8 << 24) | (bg_color & 0x00FFFFFF),
                            (alpha_u8 << 24) | (ARGB_CARD_BORDER & 0x00FFFFFF),
                            (alpha_u8 << 24) | (ARGB_TEXT_MUTED & 0x00FFFFFF),
                        )
                    };
                    gdiplus_draw_rounded_card(graphics, bx, pill_y, btn_w, pill_h, 6, bg, border, 1.0);
                    draw_gdip_text_centered(graphics, font_btn, label, bx, pill_y, btn_w, pill_h, text_color);
                }
            }

        } else if page == 3 {
            // Card 1: Logo, Tên & Nút Kiểm tra Cập nhật (y = 86..304, cao 218px)
            gdiplus_draw_rounded_card(graphics, card_x, 86 - sy, card_w, 218, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);

            let center_x = card_x + card_w / 2;
            gdiplus_draw_image(graphics, center_x - 30, 104 - sy, 60, 60, LOGO_BGRA_128, 128, 128);

            draw_gdip_text_centered(graphics, font_title, "HoyuKey", card_x, 172 - sy, card_w, 30, ARGB_TEXT_PRIMARY);

            let ver_text = format!("Phiên bản: v{}", crate::updater::CURRENT_VERSION);
            draw_gdip_text_centered(graphics, font_desc, &ver_text, card_x, 206 - sy, card_w, 22, ARGB_TEXT_MUTED);

            // Nút "Kiểm tra cập nhật" hiện đại
            let btn_update_w = 240;
            let btn_update_x = card_x + (card_w - btn_update_w) / 2;
            let btn_update_y = 250 - sy;
            let btn_update_h = 42;

            let update_btn_bg = if hover_id == 25 { ARGB_WHITE_HOVER } else { ARGB_WHITE };
            gdiplus_draw_rounded_card(graphics, btn_update_x, btn_update_y, btn_update_w, btn_update_h, 8, update_btn_bg, ARGB_WHITE, 1.0);
            draw_gdip_text_centered(graphics, font_btn, "Kiểm tra cập nhật", btn_update_x, btn_update_y, btn_update_w, btn_update_h, ARGB_TEXT_DARK);

            // Card 2: Changelog Nhật ký thay đổi (Lấy động trực tiếp từ GitHub Releases)
            let cl_y = 316 - sy;
            let cl_h = 244;
            gdiplus_draw_rounded_card(graphics, card_x, cl_y, card_w, cl_h, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);
            draw_gdip_text(graphics, font_section, "NHẬT KÝ THAY ĐỔI", left_text, cl_y + 16, ARGB_STATUS_MUTED, false);

            let raw_changelog = crate::updater::CHANGELOG_BODY.lock().unwrap().clone();
            let changelog_text = if raw_changelog.is_empty() {
                "• Đang tải nhật ký thay đổi từ GitHub...".to_string()
            } else {
                raw_changelog
            };

            let mut log_y = cl_y + 42;
            for line in changelog_text.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let display_line = if trimmed.starts_with('#') {
                    trimmed.trim_start_matches('#').trim().to_string()
                } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                    format!("• {}", &trimmed[2..])
                } else if trimmed.starts_with("•") {
                    trimmed.to_string()
                } else {
                    format!("• {}", trimmed)
                };
                draw_gdip_text(graphics, font_desc, &display_line, left_text, log_y, ARGB_TEXT_PRIMARY, false);
                log_y += 26;
                if log_y > cl_y + cl_h - 26 {
                    break;
                }
            }
        } else if page == 4 {
            // Card 3 tùy chọn hỗ trợ soạn thảo (cao 214px, y = 86..300)
            gdiplus_draw_rounded_card(graphics, card_x, 86 - sy, card_w, 214, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);

            // 1. Tự động viết hoa đầu dòng
            draw_gdip_text(graphics, font_bold, "Tự động viết hoa đầu dòng", left_text, 100 - sy, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Viết hoa chữ cái đầu tiên khi mở đầu dòng mới", left_text, 124 - sy, ARGB_TEXT_MUTED, false);
            let caps_line_p = AUTO_CAPS_LINE_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_switch(graphics, right_ctrl - 50, 106 - sy, caps_line_p);

            gdiplus_draw_line(graphics, left_text, 158 - sy, right_ctrl, 158 - sy, ARGB_DIVIDER, 1.0);

            // 2. Tự động viết hoa sau dấu chấm
            draw_gdip_text(graphics, font_bold, "Tự động viết hoa sau dấu chấm", left_text, 172 - sy, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Viết hoa chữ cái đầu tiên sau các dấu ngắt câu (. ? !)", left_text, 196 - sy, ARGB_TEXT_MUTED, false);
            let caps_sent_p = AUTO_CAPS_SENTENCE_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_switch(graphics, right_ctrl - 50, 178 - sy, caps_sent_p);

            gdiplus_draw_line(graphics, left_text, 230 - sy, right_ctrl, 230 - sy, ARGB_DIVIDER, 1.0);

            // 3. Tự động chấm câu khi xuống dòng
            draw_gdip_text(graphics, font_bold, "Tự động chấm câu khi xuống dòng", left_text, 244 - sy, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Thêm dấu chấm nếu chưa có khi nhấn Enter hoặc gửi", left_text, 268 - sy, ARGB_TEXT_MUTED, false);
            let period_enter_p = AUTO_PERIOD_ENTER_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_switch(graphics, right_ctrl - 50, 250 - sy, period_enter_p);
        } else if page == 5 {
            let anim_p = BROWSER_INPUT_ANIM.load(Ordering::SeqCst) as f32 / 1000.0;
            let anim_ease = 1.0 - (1.0 - anim_p).powi(3);

            let card_h = (68.0 + 64.0 * anim_ease).round() as i32;
            gdiplus_draw_rounded_card(graphics, card_x, 86 - sy, card_w, card_h, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);

            // 1. Tương thích trình duyệt
            draw_gdip_text(graphics, font_bold, "Tương thích trình duyệt", left_text, 98 - sy, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Khắc phục lỗi gõ trên thanh địa chỉ và gợi ý trình duyệt", left_text, 122 - sy, ARGB_TEXT_MUTED, false);
            let compat_p = BROWSER_COMPAT_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_switch(graphics, right_ctrl - 50, 104 - sy, compat_p);

            if anim_ease > 0.02 {
                let alpha_u8 = (255.0 * anim_ease).clamp(0.0, 255.0) as u32;
                let divider_color = (alpha_u8 << 24) | (ARGB_DIVIDER & 0x00FFFFFF);
                gdiplus_draw_line(graphics, left_text, 146 - sy, right_ctrl, 146 - sy, divider_color, 1.0);

                let is_focused = BROWSER_INPUT_FOCUSED.load(Ordering::SeqCst);
                let box_border = if is_focused { ARGB_WHITE } else { ARGB_CARD_BORDER };
                let box_border_color = (alpha_u8 << 24) | (box_border & 0x00FFFFFF);
                let box_bg_color = (alpha_u8 << 24) | (ARGB_PILL_TRACK & 0x00FFFFFF);
                let box_y = 158 - sy;
                let box_w = card_w - 40;
                let box_h = 42;

                gdiplus_draw_rounded_card(graphics, left_text, box_y, box_w, box_h, 6, box_bg_color, box_border_color, 1.0);

                let browser_apps_text = {
                    let state = GLOBAL_STATE.lock().unwrap();
                    state.config.browser_apps.clone()
                };

                let display_text = if is_focused {
                    format!("{}|", browser_apps_text)
                } else {
                    browser_apps_text
                };

                let text_color = (alpha_u8 << 24) | (ARGB_TEXT_PRIMARY & 0x00FFFFFF);
                draw_gdip_text_centered(graphics, font_desc, &display_text, left_text + 8, box_y, box_w - 16, box_h, text_color);
            }

            // 2. Ghi nhớ trạng thái theo ứng dụng
            let card2_y = 86 + card_h + 14 - sy;
            let card2_h = 68;
            gdiplus_draw_rounded_card(graphics, card_x, card2_y, card_w, card2_h, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);

            draw_gdip_text(graphics, font_bold, "Ghi nhớ trạng thái theo ứng dụng", left_text, card2_y + 12, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Tự động khôi phục chế độ E / V riêng cho từng cửa sổ ứng dụng", left_text, card2_y + 36, ARGB_TEXT_MUTED, false);
            let remember_p = REMEMBER_APP_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_switch(graphics, right_ctrl - 50, card2_y + 18, remember_p);
        }

        // ========================================================
        // 🔙 STICKY HEADER CHO TẤT CẢ CÁC TRANG CON
        // Nút Back tròn đặc màu ARGB_CARD_BG / HOVER cố định ở (card_x, 20)
        // Tiêu đề trang con: FULL VIẾT HOA, nền trong suốt chữ vẽ trực tiếp
        // ========================================================
        let back_bg = if hover_id == 20 { ARGB_CARD_HOVER } else { ARGB_CARD_BG };
        gdiplus_draw_rounded_card(graphics, card_x, 20, 40, 40, 20, back_bg, ARGB_CARD_BORDER, 1.0);
        draw_vector_chevron_left(graphics, card_x + 20, 40, ARGB_TEXT_PRIMARY);

        let sub_title = match page {
            1 => "CÀI ĐẶT ĐÁNH MÁY",
            2 => "CÀI ĐẶT HOYUKEY",
            3 => "GIỚI THIỆU",
            4 => "HỖ TRỢ VIẾT",
            5 => "NÂNG CAO",
            _ => "",
        };
        draw_gdip_text(graphics, font_sub_title, sub_title, card_x + 54, 23, ARGB_TEXT_PRIMARY, false);
    }
}

// ========================================================
// 🎨 CÁC HÀM VẼ ICON VECTOR HIỆN ĐẠI (GDI+ PATH & PENS)
// ========================================================

unsafe fn draw_vector_warning_icon(graphics: *mut GpGraphics, cx: i32, cy: i32, color_argb: u32) {
    let mut pen: *mut GpPen = null_mut();
    GdipCreatePen1(color_argb, 1.4, UnitPixel, &mut pen);
    if pen.is_null() {
        return;
    }
    GdipSetPenStartCap(pen, LineCapRound);
    GdipSetPenEndCap(pen, LineCapRound);
    GdipSetPenLineJoin(pen, LineJoinRound);

    let fx = cx as f32;
    let fy = cy as f32;

    let mut path: *mut GpPath = null_mut();
    GdipCreatePath(FillModeAlternate, &mut path);
    GdipAddPathLine(path, fx, fy - 6.0, fx + 6.5, fy + 5.5);
    GdipAddPathLine(path, fx + 6.5, fy + 5.5, fx - 6.5, fy + 5.5);
    GdipClosePathFigure(path);
    GdipDrawPath(graphics, pen, path);
    GdipDeletePath(path);

    GdipDrawLine(graphics, pen, fx, fy - 2.5, fx, fy + 1.0);
    let mut brush: *mut GpSolidFill = null_mut();
    GdipCreateSolidFill(color_argb, &mut brush);
    GdipFillEllipse(graphics, brush as *mut GpBrush, fx - 0.75, fy + 3.0, 1.5, 1.5);
    GdipDeleteBrush(brush as *mut GpBrush);

    GdipDeletePen(pen);
}


unsafe fn draw_vector_gear_icon(graphics: *mut GpGraphics, cx: i32, cy: i32, color_argb: u32) {
    let fx = cx as f32;
    let fy = cy as f32;

    let mut pen: *mut GpPen = null_mut();
    GdipCreatePen1(color_argb, 1.6, UnitPixel, &mut pen);
    if pen.is_null() {
        return;
    }
    GdipSetPenStartCap(pen, LineCapRound);
    GdipSetPenEndCap(pen, LineCapRound);
    GdipSetPenLineJoin(pen, LineJoinRound);

    // 1. Lỗ trục tròn ở tâm
    GdipDrawEllipse(graphics, pen, fx - 3.2, fy - 3.2, 6.4, 6.4);

    // 2. Vành bánh răng ngoài với 6 răng khối dập nổi Lucide chuẩn
    let mut gear_path: *mut GpPath = null_mut();
    GdipCreatePath(FillModeAlternate, &mut gear_path);

    let r_inner = 6.0;
    let r_outer = 9.2;
    let tooth_half = 14.0 * std::f32::consts::PI / 180.0;
    let gap_half = 16.0 * std::f32::consts::PI / 180.0;

    for i in 0..6 {
        let ca = (i as f32) * (std::f32::consts::PI / 3.0);
        let a1 = ca - tooth_half;
        let a2 = ca + tooth_half;
        let a3 = ca + (std::f32::consts::PI / 6.0) - (gap_half / 2.0);
        let a4 = ca + (std::f32::consts::PI / 6.0) + (gap_half / 2.0);

        let p1_x = fx + r_outer * a1.cos();
        let p1_y = fy + r_outer * a1.sin();
        let p2_x = fx + r_outer * a2.cos();
        let p2_y = fy + r_outer * a2.sin();
        let p3_x = fx + r_inner * a3.cos();
        let p3_y = fy + r_inner * a3.sin();
        let p4_x = fx + r_inner * a4.cos();
        let p4_y = fy + r_inner * a4.sin();

        GdipAddPathLine(gear_path, p1_x, p1_y, p2_x, p2_y);
        GdipAddPathLine(gear_path, p2_x, p2_y, p3_x, p3_y);
        GdipAddPathLine(gear_path, p3_x, p3_y, p4_x, p4_y);
    }
    GdipClosePathFigure(gear_path);
    GdipDrawPath(graphics, pen, gear_path);

    GdipDeletePath(gear_path);
    GdipDeletePen(pen);
}

unsafe fn draw_vector_pen_icon(graphics: *mut GpGraphics, cx: i32, cy: i32, color_argb: u32) {
    let mut pen: *mut GpPen = null_mut();
    GdipCreatePen1(color_argb, 1.6, UnitPixel, &mut pen);
    if pen.is_null() {
        return;
    }
    GdipSetPenStartCap(pen, LineCapRound);
    GdipSetPenEndCap(pen, LineCapRound);
    GdipSetPenLineJoin(pen, LineJoinRound);

    let fx = cx as f32;
    let fy = cy as f32;
    // Vẽ biểu tượng chiếc bút nghiêng thanh lịch
    GdipDrawLine(graphics, pen, fx - 6.0, fy + 6.0, fx - 3.0, fy + 6.0);
    GdipDrawLine(graphics, pen, fx - 6.0, fy + 6.0, fx - 6.0, fy + 3.0);
    GdipDrawLine(graphics, pen, fx - 6.0, fy + 3.0, fx + 3.0, fy - 6.0);
    GdipDrawLine(graphics, pen, fx - 3.0, fy + 6.0, fx + 6.0, fy - 3.0);
    GdipDrawLine(graphics, pen, fx + 3.0, fy - 6.0, fx + 6.0, fy - 3.0);
    GdipDeletePen(pen);
}

unsafe fn draw_vector_chevron_left(graphics: *mut GpGraphics, cx: i32, cy: i32, color_argb: u32) {
    let mut path: *mut GpPath = null_mut();
    GdipCreatePath(FillModeAlternate, &mut path);

    let fx = cx as f32;
    let fy = cy as f32;
    GdipAddPathLine(path, fx + 2.5, fy - 6.5, fx - 3.5, fy);
    GdipAddPathLine(path, fx - 3.5, fy, fx + 2.5, fy + 6.5);

    let mut pen: *mut GpPen = null_mut();
    GdipCreatePen1(color_argb, 2.2, UnitPixel, &mut pen);
    GdipSetPenStartCap(pen, LineCapRound);
    GdipSetPenEndCap(pen, LineCapRound);
    GdipSetPenLineJoin(pen, LineJoinRound);

    GdipDrawPath(graphics, pen, path);

    GdipDeletePen(pen);
    GdipDeletePath(path);
}

unsafe fn draw_vector_chevron_right(graphics: *mut GpGraphics, cx: i32, cy: i32, color_argb: u32) {
    let mut path: *mut GpPath = null_mut();
    GdipCreatePath(FillModeAlternate, &mut path);

    let fx = cx as f32;
    let fy = cy as f32;
    GdipAddPathLine(path, fx - 2.5, fy - 5.5, fx + 2.5, fy);
    GdipAddPathLine(path, fx + 2.5, fy, fx - 2.5, fy + 5.5);

    let mut pen: *mut GpPen = null_mut();
    GdipCreatePen1(color_argb, 2.0, UnitPixel, &mut pen);
    GdipSetPenStartCap(pen, LineCapRound);
    GdipSetPenEndCap(pen, LineCapRound);
    GdipSetPenLineJoin(pen, LineJoinRound);

    GdipDrawPath(graphics, pen, path);

    GdipDeletePen(pen);
    GdipDeletePath(path);
}

unsafe fn draw_vector_keyboard_icon(graphics: *mut GpGraphics, cx: i32, cy: i32, color_argb: u32) {
    let fx = cx as f32;
    let fy = cy as f32;

    let path = create_rounded_path(fx - 11.0, fy - 8.0, 22.0, 16.0, 3.0);
    let mut pen: *mut GpPen = null_mut();
    GdipCreatePen1(color_argb, 1.5, UnitPixel, &mut pen);
    GdipDrawPath(graphics, pen, path);
    GdipDeletePath(path);

    let mut brush: *mut GpSolidFill = null_mut();
    GdipCreateSolidFill(color_argb, &mut brush);
    GdipFillEllipse(graphics, brush as *mut GpBrush, fx - 7.0, fy - 4.5, 2.0, 2.0);
    GdipFillEllipse(graphics, brush as *mut GpBrush, fx - 2.0, fy - 4.5, 2.0, 2.0);
    GdipFillEllipse(graphics, brush as *mut GpBrush, fx + 3.0, fy - 4.5, 2.0, 2.0);
    GdipFillEllipse(graphics, brush as *mut GpBrush, fx - 5.0, fy - 1.0, 2.0, 2.0);
    GdipFillEllipse(graphics, brush as *mut GpBrush, fx + 1.0, fy - 1.0, 2.0, 2.0);

    let mut space_path: *mut GpPath = null_mut();
    GdipCreatePath(FillModeAlternate, &mut space_path);
    GdipAddPathLine(space_path, fx - 5.0, fy + 3.5, fx + 5.0, fy + 3.5);
    GdipSetPenStartCap(pen, LineCapRound);
    GdipSetPenEndCap(pen, LineCapRound);
    GdipDrawPath(graphics, pen, space_path);
    GdipDeletePath(space_path);

    GdipDeleteBrush(brush as *mut GpBrush);
    GdipDeletePen(pen);
}

unsafe fn draw_vector_sliders_icon(graphics: *mut GpGraphics, cx: i32, cy: i32, color_argb: u32) {
    let fx = cx as f32;
    let fy = cy as f32;

    let mut pen: *mut GpPen = null_mut();
    GdipCreatePen1(color_argb, 1.5, UnitPixel, &mut pen);
    GdipSetPenStartCap(pen, LineCapRound);
    GdipSetPenEndCap(pen, LineCapRound);

    let mut brush: *mut GpSolidFill = null_mut();
    GdipCreateSolidFill(color_argb, &mut brush);

    GdipDrawLine(graphics, pen, fx - 9.0, fy - 5.5, fx + 9.0, fy - 5.5);
    GdipFillEllipse(graphics, brush as *mut GpBrush, fx - 6.0, fy - 8.0, 5.0, 5.0);

    GdipDrawLine(graphics, pen, fx - 9.0, fy, fx + 9.0, fy);
    GdipFillEllipse(graphics, brush as *mut GpBrush, fx + 1.0, fy - 2.5, 5.0, 5.0);

    GdipDrawLine(graphics, pen, fx - 9.0, fy + 5.5, fx + 9.0, fy + 5.5);
    GdipFillEllipse(graphics, brush as *mut GpBrush, fx - 4.0, fy + 3.0, 5.0, 5.0);

    GdipDeleteBrush(brush as *mut GpBrush);
    GdipDeletePen(pen);
}

unsafe fn draw_vector_info_icon(graphics: *mut GpGraphics, cx: i32, cy: i32, color_argb: u32) {
    let fx = cx as f32;
    let fy = cy as f32;

    let mut pen: *mut GpPen = null_mut();
    GdipCreatePen1(color_argb, 1.5, UnitPixel, &mut pen);
    GdipDrawEllipse(graphics, pen, fx - 9.0, fy - 9.0, 18.0, 18.0);

    let mut brush: *mut GpSolidFill = null_mut();
    GdipCreateSolidFill(color_argb, &mut brush);
    GdipFillEllipse(graphics, brush as *mut GpBrush, fx - 1.2, fy - 5.2, 2.4, 2.4);

    GdipSetPenStartCap(pen, LineCapRound);
    GdipSetPenEndCap(pen, LineCapRound);
    GdipDrawLine(graphics, pen, fx, fy - 1.5, fx, fy + 4.5);

    GdipDeleteBrush(brush as *mut GpBrush);
    GdipDeletePen(pen);
}

unsafe fn draw_animated_switch(
    graphics: *mut GpGraphics,
    x: i32,
    y: i32,
    progress: f32,
) {
    let w = 50;
    let h = 28;
    let radius = 14;

    let r_track = (24.0 + (255.0 - 24.0) * progress).round() as u32;
    let g_track = (24.0 + (255.0 - 24.0) * progress).round() as u32;
    let b_track = (27.0 + (255.0 - 27.0) * progress).round() as u32;
    let color_track = 0xFF000000 | (r_track << 16) | (g_track << 8) | b_track;

    let r_border = (63.0 + (255.0 - 63.0) * progress).round() as u32;
    let g_border = (63.0 + (255.0 - 63.0) * progress).round() as u32;
    let b_border = (70.0 + (255.0 - 70.0) * progress).round() as u32;
    let color_border = 0xFF000000 | (r_border << 16) | (g_border << 8) | b_border;

    gdiplus_draw_rounded_card(graphics, x, y, w, h, radius, color_track, color_border, 1.0);

    let knob_diameter = 22.0;
    let knob_min_x = x as f32 + 3.0;
    let knob_max_x = (x + w) as f32 - knob_diameter - 3.0;
    let knob_curr_x = knob_min_x + (knob_max_x - knob_min_x) * progress;
    let knob_y = y as f32 + 3.0;

    let r_knob = (250.0 + (9.0 - 250.0) * progress).round() as u32;
    let g_knob = (250.0 + (9.0 - 250.0) * progress).round() as u32;
    let b_knob = (250.0 + (12.0 - 250.0) * progress).round() as u32;
    let color_knob = 0xFF000000 | (r_knob << 16) | (g_knob << 8) | b_knob;

    gdiplus_fill_ellipse(graphics, knob_curr_x, knob_y, knob_diameter, knob_diameter, color_knob);
}

unsafe fn draw_animated_tabs(
    graphics: *mut GpGraphics,
    x: i32,
    y: i32,
    tabs: &[&str; 2],
    tab_w: i32,
    tab_h: i32,
    progress: f32,
    font: *mut GpFont,
) {
    let total_w = tab_w * 2 + 8;
    let total_h = tab_h + 8;

    gdiplus_draw_rounded_card(graphics, x, y, total_w, total_h, 8, ARGB_PILL_TRACK, ARGB_CARD_BORDER, 1.0);

    let pill_min_x = (x + 4) as f32;
    let pill_max_x = (x + 4 + tab_w) as f32;
    let pill_curr_x = pill_min_x + (pill_max_x - pill_min_x) * progress;
    let pill_y = y + 4;

    gdiplus_draw_rounded_card(graphics, pill_curr_x.round() as i32, pill_y, tab_w, tab_h, 6, ARGB_WHITE, ARGB_WHITE, 1.0);

    let t1_color = if progress < 0.5 { ARGB_TEXT_DARK } else { ARGB_TEXT_MUTED };
    draw_gdip_text_centered(graphics, font, tabs[0], x + 4, y + 4, tab_w, tab_h, t1_color);

    let t2_color = if progress >= 0.5 { ARGB_TEXT_DARK } else { ARGB_TEXT_MUTED };
    draw_gdip_text_centered(graphics, font, tabs[1], x + 4 + tab_w, y + 4, tab_w, tab_h, t2_color);
}

unsafe fn create_rounded_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> *mut GpPath {
    let mut path: *mut GpPath = null_mut();
    GdipCreatePath(FillModeAlternate, &mut path);
    let d = r * 2.0;

    GdipAddPathArc(path, x, y, d, d, 180.0, 90.0);
    GdipAddPathLine(path, x + r, y, x + w - r, y);
    GdipAddPathArc(path, x + w - d, y, d, d, 270.0, 90.0);
    GdipAddPathLine(path, x + w, y + r, x + w, y + h - r);
    GdipAddPathArc(path, x + w - d, y + h - d, d, d, 0.0, 90.0);
    GdipAddPathLine(path, x + w - r, y + h, x + r, y + h);
    GdipAddPathArc(path, x, y + h - d, d, d, 90.0, 90.0);
    GdipAddPathLine(path, x, y + h - r, x, y + r);

    GdipClosePathFigure(path);
    path
}

unsafe fn gdiplus_draw_rounded_card(
    graphics: *mut GpGraphics,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    radius: i32,
    bg_argb: u32,
    border_argb: u32,
    border_width: f32,
) {
    let path = create_rounded_path(x as f32, y as f32, w as f32, h as f32, radius as f32);

    let mut brush: *mut GpSolidFill = null_mut();
    GdipCreateSolidFill(bg_argb, &mut brush);
    GdipFillPath(graphics, brush as *mut GpBrush, path);
    GdipDeleteBrush(brush as *mut GpBrush);

    if border_width > 0.0 {
        let mut pen: *mut GpPen = null_mut();
        GdipCreatePen1(border_argb, border_width, UnitPixel, &mut pen);
        GdipDrawPath(graphics, pen, path);
        GdipDeletePen(pen);
    }

    GdipDeletePath(path);
}

unsafe fn gdiplus_fill_ellipse(
    graphics: *mut GpGraphics,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    bg_argb: u32,
) {
    let mut brush: *mut GpSolidFill = null_mut();
    GdipCreateSolidFill(bg_argb, &mut brush);
    GdipFillEllipse(graphics, brush as *mut GpBrush, x, y, w, h);
    GdipDeleteBrush(brush as *mut GpBrush);
}

unsafe fn gdiplus_draw_line(
    graphics: *mut GpGraphics,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    color_argb: u32,
    width: f32,
) {
    let mut pen: *mut GpPen = null_mut();
    GdipCreatePen1(color_argb, width, UnitPixel, &mut pen);
    GdipDrawLineI(graphics, pen, x1, y1, x2, y2);
    GdipDeletePen(pen);
}

unsafe fn gdiplus_draw_image(
    graphics: *mut GpGraphics,
    dest_x: i32,
    dest_y: i32,
    dest_w: i32,
    dest_h: i32,
    bytes: &[u8],
    src_w: i32,
    src_h: i32,
) {
    const PIXEL_FORMAT_32BPP_PARGB: i32 = 0x000E200B;
    let mut bitmap: *mut GpBitmap = null_mut();
    GdipCreateBitmapFromScan0(
        src_w,
        src_h,
        src_w * 4,
        PIXEL_FORMAT_32BPP_PARGB,
        bytes.as_ptr() as _,
        &mut bitmap,
    );
    if !bitmap.is_null() {
        GdipDrawImageRectRectI(
            graphics,
            bitmap as *mut GpImage,
            dest_x,
            dest_y,
            dest_w,
            dest_h,
            0,
            0,
            src_w,
            src_h,
            UnitPixel,
            null_mut(),
            0,
            null_mut(),
        );
        GdipDisposeImage(bitmap as *mut GpImage);
    }
}

unsafe fn gdiplus_draw_image_alpha(
    graphics: *mut GpGraphics,
    dest_x: i32,
    dest_y: i32,
    dest_w: i32,
    dest_h: i32,
    bytes: &[u8],
    src_w: i32,
    src_h: i32,
    alpha: f32,
) {
    if alpha <= 0.01 {
        return;
    }
    const PIXEL_FORMAT_32BPP_PARGB: i32 = 0x000E200B;
    let mut bitmap: *mut GpBitmap = null_mut();
    GdipCreateBitmapFromScan0(
        src_w,
        src_h,
        src_w * 4,
        PIXEL_FORMAT_32BPP_PARGB,
        bytes.as_ptr() as _,
        &mut bitmap,
    );
    if bitmap.is_null() {
        return;
    }

    if (alpha - 1.0).abs() < 0.01 {
        GdipDrawImageRectRectI(
            graphics,
            bitmap as *mut GpImage,
            dest_x,
            dest_y,
            dest_w,
            dest_h,
            0,
            0,
            src_w,
            src_h,
            UnitPixel,
            null_mut(),
            0,
            null_mut(),
        );
    } else {
        let mut image_attr: *mut GpImageAttributes = null_mut();
        GdipCreateImageAttributes(&mut image_attr);

        let mut cm: ColorMatrix = std::mem::zeroed();
        cm.m[0] = 1.0;
        cm.m[6] = 1.0;
        cm.m[12] = 1.0;
        cm.m[18] = alpha.clamp(0.0, 1.0);
        cm.m[24] = 1.0;

        GdipSetImageAttributesColorMatrix(
            image_attr,
            ColorAdjustTypeDefault,
            1,
            &cm,
            null_mut(),
            ColorMatrixFlagsDefault,
        );

        GdipDrawImageRectRectI(
            graphics,
            bitmap as *mut GpImage,
            dest_x,
            dest_y,
            dest_w,
            dest_h,
            0,
            0,
            src_w,
            src_h,
            UnitPixel,
            image_attr,
            0,
            null_mut(),
        );

        GdipDisposeImageAttributes(image_attr);
    }

    GdipDisposeImage(bitmap as *mut GpImage);
}

unsafe fn _gdiplus_draw_card_glow(
    graphics: *mut GpGraphics,
    card_x: i32,
    card_y: i32,
    card_w: i32,
    card_h: i32,
    radius: i32,
    color_rgb: u32,
    alpha: u32,
) {
    if alpha == 0 {
        return;
    }
    let card_path = create_rounded_path(card_x as f32, card_y as f32, card_w as f32, card_h as f32, radius as f32);

    let mut state: u32 = 0;
    GdipSaveGraphics(graphics, &mut state);
    GdipSetClipPath(graphics, card_path, CombineModeIntersect);

    let glow_w = 260;
    let rect = Rect {
        X: card_x,
        Y: card_y,
        Width: glow_w,
        Height: card_h,
    };

    let start_color = (alpha << 24) | (color_rgb & 0x00FFFFFF);
    let end_color = color_rgb & 0x00FFFFFF;

    let mut line_brush: *mut GpLineGradient = null_mut();
    GdipCreateLineBrushFromRectI(
        &rect,
        start_color,
        end_color,
        LinearGradientModeHorizontal,
        WrapModeClamp,
        &mut line_brush,
    );

    if !line_brush.is_null() {
        GdipFillRectangleI(graphics, line_brush as *mut GpBrush, card_x, card_y, glow_w, card_h);
        GdipDeleteBrush(line_brush as *mut GpBrush);
    }

    GdipRestoreGraphics(graphics, state);
    GdipDeletePath(card_path);
}

unsafe fn create_gdip_font(family: *mut GpFontFamily, size: f32, style: i32) -> *mut GpFont {
    let mut font: *mut GpFont = null_mut();
    GdipCreateFont(family, size, style, UnitPixel, &mut font);
    font
}

unsafe fn draw_gdip_text(
    graphics: *mut GpGraphics,
    font: *mut GpFont,
    text: &str,
    x: i32,
    y: i32,
    color_argb: u32,
    _bold: bool,
) {
    let wide = to_wide(text);
    let rect = RectF {
        X: x as f32,
        Y: y as f32,
        Width: 440.0,
        Height: 36.0,
    };

    let mut brush: *mut GpSolidFill = null_mut();
    GdipCreateSolidFill(color_argb, &mut brush);

    GdipDrawString(
        graphics,
        wide.as_ptr(),
        wide.len() as i32 - 1,
        font,
        &rect,
        null_mut(),
        brush as *mut GpBrush,
    );

    GdipDeleteBrush(brush as *mut GpBrush);
}

unsafe fn draw_gdip_text_centered(
    graphics: *mut GpGraphics,
    font: *mut GpFont,
    text: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color_argb: u32,
) {
    let wide = to_wide(text);
    let rect = RectF {
        X: x as f32,
        Y: y as f32,
        Width: w as f32,
        Height: h as f32,
    };

    let mut format: *mut GpStringFormat = null_mut();
    GdipCreateStringFormat(0x00001000, 0, &mut format);
    GdipSetStringFormatAlign(format, StringAlignmentCenter);
    GdipSetStringFormatLineAlign(format, StringAlignmentCenter);

    let mut brush: *mut GpSolidFill = null_mut();
    GdipCreateSolidFill(color_argb, &mut brush);

    GdipDrawString(
        graphics,
        wide.as_ptr(),
        wide.len() as i32 - 1,
        font,
        &rect,
        format,
        brush as *mut GpBrush,
    );

    GdipDeleteBrush(brush as *mut GpBrush);
    GdipDeleteStringFormat(format);
}

unsafe fn draw_gdip_text_multiline(
    graphics: *mut GpGraphics,
    font: *mut GpFont,
    text: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color_argb: u32,
) {
    let wide = to_wide(text);
    let rect = RectF {
        X: x as f32,
        Y: y as f32,
        Width: w as f32,
        Height: h as f32,
    };

    let mut format: *mut GpStringFormat = null_mut();
    GdipCreateStringFormat(0x00001000, 0, &mut format);
    GdipSetStringFormatAlign(format, StringAlignmentCenter);

    let mut brush: *mut GpSolidFill = null_mut();
    GdipCreateSolidFill(color_argb, &mut brush);

    GdipDrawString(
        graphics,
        wide.as_ptr(),
        wide.len() as i32 - 1,
        font,
        &rect,
        format,
        brush as *mut GpBrush,
    );

    GdipDeleteBrush(brush as *mut GpBrush);
    GdipDeleteStringFormat(format);
}

fn fast_box_blur_32(pixels: &mut [u32], w: usize, h: usize, radius: usize) {
    if w == 0 || h == 0 || radius == 0 {
        return;
    }
    let mut temp = vec![0u32; pixels.len()];

    for _pass in 0..2 {
        for y in 0..h {
            let row_offset = y * w;
            for x in 0..w {
                let x_start = x.saturating_sub(radius);
                let x_end = (x + radius).min(w - 1);
                let count = (x_end - x_start + 1) as u32;

                let mut r_sum = 0u32;
                let mut g_sum = 0u32;
                let mut b_sum = 0u32;

                for ix in x_start..=x_end {
                    let px = pixels[row_offset + ix];
                    r_sum += (px >> 16) & 0xFF;
                    g_sum += (px >> 8) & 0xFF;
                    b_sum += px & 0xFF;
                }

                temp[row_offset + x] = 0xFF000000
                    | ((r_sum / count) << 16)
                    | ((g_sum / count) << 8)
                    | (b_sum / count);
            }
        }

        for x in 0..w {
            for y in 0..h {
                let y_start = y.saturating_sub(radius);
                let y_end = (y + radius).min(h - 1);
                let count = (y_end - y_start + 1) as u32;

                let mut r_sum = 0u32;
                let mut g_sum = 0u32;
                let mut b_sum = 0u32;

                for iy in y_start..=y_end {
                    let px = temp[iy * w + x];
                    r_sum += (px >> 16) & 0xFF;
                    g_sum += (px >> 8) & 0xFF;
                    b_sum += px & 0xFF;
                }

                pixels[y * w + x] = 0xFF000000
                    | ((r_sum / count) << 16)
                    | ((g_sum / count) << 8)
                    | (b_sum / count);
            }
        }
    }
}

unsafe fn apply_screen_blur(dc: windows_sys::Win32::Graphics::Gdi::HDC, w: i32, h: i32) {
    let scale_down = 4;
    let sw = (w / scale_down).max(1);
    let sh = (h / scale_down).max(1);

    let s_dc = CreateCompatibleDC(dc);
    let s_bm = CreateCompatibleBitmap(dc, sw, sh);
    let old_bm = SelectObject(s_dc, s_bm);

    SetStretchBltMode(s_dc, HALFTONE);
    StretchBlt(s_dc, 0, 0, sw, sh, dc, 0, 0, w, h, SRCCOPY);

    let mut bmi: BITMAPINFO = std::mem::zeroed();
    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = sw;
    bmi.bmiHeader.biHeight = -sh;
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;

    let pixel_count = (sw * sh) as usize;
    let mut pixels: Vec<u32> = vec![0; pixel_count];

    GetDIBits(
        s_dc,
        s_bm,
        0,
        sh as u32,
        pixels.as_mut_ptr() as *mut _,
        &mut bmi,
        DIB_RGB_COLORS,
    );

    fast_box_blur_32(&mut pixels, sw as usize, sh as usize, 3);

    SetDIBits(
        s_dc,
        s_bm,
        0,
        sh as u32,
        pixels.as_ptr() as *const _,
        &bmi,
        DIB_RGB_COLORS,
    );

    SetStretchBltMode(dc, HALFTONE);
    StretchBlt(dc, 0, 0, w, h, s_dc, 0, 0, sw, sh, SRCCOPY);

    SelectObject(s_dc, old_bm);
    DeleteObject(s_bm);
    DeleteDC(s_dc);
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
