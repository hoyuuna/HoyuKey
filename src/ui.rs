use std::ptr::null_mut;
use std::sync::atomic::{AtomicI32, AtomicIsize, AtomicU32, AtomicU8, AtomicUsize, Ordering};
use windows_sys::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    AddFontMemResourceEx, AddFontResourceExW, BeginPaint, BitBlt, CreateCompatibleBitmap,
    CreateCompatibleDC, DeleteDC, DeleteObject, EndPaint, InvalidateRect, ScreenToClient,
    SelectObject, PAINTSTRUCT, SRCCOPY,
};
use windows_sys::Win32::Graphics::GdiPlus::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, GetClientRect, GetCursorPos,
    GetSystemMetrics, KillTimer, LoadCursorW, PostMessageW, PostQuitMessage, RegisterClassW,
    SendMessageW, SetCursor, SetForegroundWindow, SetTimer, ShowWindow, CS_HREDRAW, CS_VREDRAW,
    IDC_ARROW, IDC_HAND, ICON_BIG, ICON_SMALL, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_RESTORE,
    SW_SHOW, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_PAINT,
    WM_SETCURSOR, WM_SETICON, WM_TIMER, WNDCLASSW, WS_CAPTION, WS_EX_DLGMODALFRAME,
    WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU,
};

use crate::config::{Config, InputMethod, SwitchKey};
use crate::tray::WM_APP_UPDATE_TRAY;
use crate::GLOBAL_STATE;

pub static SETTINGS_HWND: AtomicIsize = AtomicIsize::new(0);
static GDIP_TOKEN: AtomicUsize = AtomicUsize::new(0);

// Timer ID cho animation 60 FPS
const TIMER_ID_ANIM: usize = 1001;

// Tiến trình hoạt họa các công tắc và thanh trượt (0 đến 1000 <=> 0.0 đến 1.0)
static MODERN_TONE_PROGRESS: AtomicU32 = AtomicU32::new(1000);
static AUTO_RESTORE_PROGRESS: AtomicU32 = AtomicU32::new(1000);
static INPUT_METHOD_PROGRESS: AtomicU32 = AtomicU32::new(0); // 0 = Telex, 1000 = VNI
static SWITCH_KEY_PROGRESS: AtomicU32 = AtomicU32::new(0);    // 0 = Ctrl+Shift, 1000 = Alt+Z

// Quản lý trang giao diện (Phone Settings style)
// 0: Trang chủ (Home)
// 1: Cài đặt đánh máy (TypingSettings)
// 2: Cài đặt HoyuKey (AppSettings)
// 3: Giới thiệu (About)
static CURRENT_PAGE: AtomicU8 = AtomicU8::new(0);

// Quản lý hover item (-1: không có, 10: status card, 11: menu typing, 12: menu app, 13: menu about, 20: back btn, 30: exit btn, 31: close btn)
static HOVER_ITEM: AtomicI32 = AtomicI32::new(-1);

// Nhúng trực tiếp font Google Sans vào file binary để chạy độc lập mọi nơi
const FONT_GOOGLE_SANS_REGULAR: &[u8] = include_bytes!("../fonts/GoogleSans-Regular.ttf");
const FONT_GOOGLE_SANS_BOLD: &[u8] = include_bytes!("../fonts/GoogleSans-Bold.ttf");

// Nhúng logo Premultiplied ARGB chuẩn màu
const LOGO_BGRA_128: &[u8] = include_bytes!("../icon/logo_128.bin");
const LOGO_BGRA_32: &[u8] = include_bytes!("../icon/logo_32.bin");

// Bảng màu chuẩn Shadcn UI Dark Mode (Zinc Palette)
const ARGB_BG: u32 = 0xFF09090B;              // #09090b
const ARGB_CARD_BG: u32 = 0xFF141417;         // #141417
const ARGB_CARD_HOVER: u32 = 0xFF1E1E23;      // #1e1e23 (hover xám tinh tế)
const ARGB_CARD_BORDER: u32 = 0xFF27272A;     // #27272a
const ARGB_DIVIDER: u32 = 0xFF202024;         // #202024
const ARGB_TEXT_PRIMARY: u32 = 0xFFF4F4F5;    // #f4f4f5 (trắng sáng mịn)
const ARGB_TEXT_MUTED: u32 = 0xFFA1A1AA;      // #a1a1aa (xám zinc chuẩn)
const ARGB_STATUS_GREEN: u32 = 0xFF22C55E;    // #22c55e
const ARGB_STATUS_MUTED: u32 = 0xFF71717A;    // #71717a
const ARGB_PILL_TRACK: u32 = 0xFF1C1C20;      // #1c1c20
const ARGB_WHITE: u32 = 0xFFFAFAFA;           // #fafafa
const ARGB_WHITE_HOVER: u32 = 0xFFE4E4E7;     // #e4e4e7
const ARGB_TEXT_DARK: u32 = 0xFF09090B;       // #09090b
const ARGB_DANGER_TEXT: u32 = 0xFFEF4444;     // #ef4444
const ARGB_DANGER_BORDER: u32 = 0xFF3F1D1D;   // #3f1d1d
const ARGB_DANGER_HOVER: u32 = 0xFF2D1515;    // #2d1515

pub fn init_ui_subsystems() {
    unsafe {
        // 1. Khởi tạo GDI+
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

        // 2. Nạp font Google Sans trực tiếp từ bộ nhớ (độc lập, không cần cài đặt)
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

        // Nạp thêm từ file thư mục fonts nếu có sẵn
        let p1 = to_wide("fonts\\GoogleSans-Regular.ttf");
        let p2 = to_wide("fonts\\GoogleSans-Bold.ttf");
        AddFontResourceExW(p1.as_ptr(), 0x10, null_mut());
        AddFontResourceExW(p2.as_ptr(), 0x10, null_mut());
    }
}

pub fn shutdown_ui_subsystems() {
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

        RegisterClassW(&wnd_class);

        // Kích thước chuẩn Client Area: 520 x 660
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

pub fn show_settings_window() {
    let hwnd = SETTINGS_HWND.load(Ordering::SeqCst) as HWND;
    if hwnd != null_mut() {
        let state = GLOBAL_STATE.lock().unwrap();
        MODERN_TONE_PROGRESS.store(if state.config.modern_tone { 1000 } else { 0 }, Ordering::SeqCst);
        AUTO_RESTORE_PROGRESS.store(if state.config.auto_restore { 1000 } else { 0 }, Ordering::SeqCst);
        INPUT_METHOD_PROGRESS.store(if state.config.input_method == InputMethod::Vni { 1000 } else { 0 }, Ordering::SeqCst);
        SWITCH_KEY_PROGRESS.store(if state.config.switch_key == SwitchKey::AltZ { 1000 } else { 0 }, Ordering::SeqCst);
        drop(state);

        unsafe {
            ShowWindow(hwnd, SW_SHOW);
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
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
        WM_TIMER => {
            if wparam as usize == TIMER_ID_ANIM {
                let config = {
                    let state = GLOBAL_STATE.lock().unwrap();
                    state.config.clone()
                };

                let mut still_animating = false;

                // 1. Modern Tone Switch animation
                let target_tone = if config.modern_tone { 1000 } else { 0 };
                let curr_tone = MODERN_TONE_PROGRESS.load(Ordering::SeqCst);
                if curr_tone != target_tone {
                    still_animating = true;
                    let diff = (target_tone as i32 - curr_tone as i32) / 3;
                    let step = if diff == 0 { if target_tone > curr_tone { 1 } else { -1 } } else { diff };
                    let next = (curr_tone as i32 + step).clamp(0, 1000) as u32;
                    MODERN_TONE_PROGRESS.store(next, Ordering::SeqCst);
                }

                // 2. Auto Restore Switch animation
                let target_restore = if config.auto_restore { 1000 } else { 0 };
                let curr_restore = AUTO_RESTORE_PROGRESS.load(Ordering::SeqCst);
                if curr_restore != target_restore {
                    still_animating = true;
                    let diff = (target_restore as i32 - curr_restore as i32) / 3;
                    let step = if diff == 0 { if target_restore > curr_restore { 1 } else { -1 } } else { diff };
                    let next = (curr_restore as i32 + step).clamp(0, 1000) as u32;
                    AUTO_RESTORE_PROGRESS.store(next, Ordering::SeqCst);
                }

                // 3. Input Method Tab animation (Telex / VNI)
                let target_method = if config.input_method == InputMethod::Vni { 1000 } else { 0 };
                let curr_method = INPUT_METHOD_PROGRESS.load(Ordering::SeqCst);
                if curr_method != target_method {
                    still_animating = true;
                    let diff = (target_method as i32 - curr_method as i32) / 3;
                    let step = if diff == 0 { if target_method > curr_method { 1 } else { -1 } } else { diff };
                    let next = (curr_method as i32 + step).clamp(0, 1000) as u32;
                    INPUT_METHOD_PROGRESS.store(next, Ordering::SeqCst);
                }

                // 4. Switch Key Tab animation (Ctrl+Shift / Alt+Z)
                let target_switch = if config.switch_key == SwitchKey::AltZ { 1000 } else { 0 };
                let curr_switch = SWITCH_KEY_PROGRESS.load(Ordering::SeqCst);
                if curr_switch != target_switch {
                    still_animating = true;
                    let diff = (target_switch as i32 - curr_switch as i32) / 3;
                    let step = if diff == 0 { if target_switch > curr_switch { 1 } else { -1 } } else { diff };
                    let next = (curr_switch as i32 + step).clamp(0, 1000) as u32;
                    SWITCH_KEY_PROGRESS.store(next, Ordering::SeqCst);
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
                HOVER_ITEM.store(-1, Ordering::SeqCst);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        WM_CLOSE => {
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        WM_DESTROY => {
            shutdown_ui_subsystems();
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
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
    let (card_x, card_w, card_right, right_ctrl, _, _) = get_layout_metrics(client_w);

    // 1. Fixed Bottom Buttons (luôn có ở mọi trang): y = 590..636
    let btn_w = (card_w - 12) / 2;
    if y >= 590 && y <= 636 {
        if x >= card_x && x <= card_x + btn_w {
            return 30; // Thoát HoyuKey
        }
        let btn2_x = card_x + btn_w + 12;
        if x >= btn2_x && x <= card_right {
            return 31; // Đóng
        }
    }

    if page == 0 {
        // --- TRANG CHỦ ---
        // Status Card: y = 86..162
        if x >= card_x && x <= card_right && y >= 86 && y <= 162 {
            return 10;
        }
        // Menu item 1: Cài đặt đánh máy: y = 196..270
        if x >= card_x && x <= card_right && y >= 196 && y <= 270 {
            return 11;
        }
        // Menu item 2: Cài đặt HoyuKey: y = 282..356
        if x >= card_x && x <= card_right && y >= 282 && y <= 356 {
            return 12;
        }
        // Menu item 3: Giới thiệu: y = 368..442
        if x >= card_x && x <= card_right && y >= 368 && y <= 442 {
            return 13;
        }
    } else {
        // --- CÁC TRANG CON ---
        // Nút Back tròn ở đầu: (card_x, 20) kích thước 40x40
        if x >= card_x && x <= card_x + 40 && y >= 20 && y <= 60 {
            return 20;
        }

        if page == 1 {
            // Cài đặt đánh máy:
            // Tab Kiểu gõ (Telex / VNI): right_ctrl - 200 .. right_ctrl, y = 105..145
            let tab_x = right_ctrl - 200;
            if x >= tab_x && x <= right_ctrl && y >= 105 && y <= 145 {
                return 40;
            }
            // Switch Bỏ dấu kiểu mới: right_ctrl - 50 .. right_ctrl, y = 206..236
            if x >= right_ctrl - 52 && x <= right_ctrl && y >= 204 && y <= 236 {
                return 41;
            }
            // Switch Tự động khôi phục phím: right_ctrl - 50 .. right_ctrl, y = 282..312
            if x >= right_ctrl - 52 && x <= right_ctrl && y >= 280 && y <= 312 {
                return 42;
            }
        } else if page == 2 {
            // Cài đặt HoyuKey:
            // Tab Phím tắt (Ctrl+Shift / Alt+Z): right_ctrl - 216 .. right_ctrl, y = 105..145
            let sw_tab_x = right_ctrl - 216;
            if x >= sw_tab_x && x <= right_ctrl && y >= 105 && y <= 145 {
                return 43;
            }
        }
    }

    -1
}

fn handle_mouse_click(hwnd: HWND, x: i32, y: i32, client_w: i32) {
    let (card_x, card_w, card_right, right_ctrl, _, _) = get_layout_metrics(client_w);
    let page = CURRENT_PAGE.load(Ordering::SeqCst);

    // 1. Nút Thoát và Đóng (Fixed Bottom Dock ở mọi trang)
    let btn_w = (card_w - 12) / 2;
    if y >= 590 && y <= 636 {
        // Nút Thoát:
        if x >= card_x && x <= card_x + btn_w {
            unsafe {
                PostQuitMessage(0);
            }
            return;
        }
        // Nút Đóng:
        let btn2_x = card_x + btn_w + 12;
        if x >= btn2_x && x <= card_right {
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
            return;
        }
    }

    if page == 0 {
        // --- XỬ LÝ TRANG CHỦ ---
        // Click Card Trạng thái: bật/tắt tiếng Việt nhanh
        if x >= card_x && x <= card_right && y >= 86 && y <= 162 {
            crate::hook::toggle_vietnamese_mode();
            unsafe {
                InvalidateRect(hwnd, null_mut(), 0);
            }
            return;
        }

        // Click Menu 1: Cài đặt đánh máy
        if x >= card_x && x <= card_right && y >= 196 && y <= 270 {
            CURRENT_PAGE.store(1, Ordering::SeqCst);
            unsafe {
                InvalidateRect(hwnd, null_mut(), 0);
            }
            return;
        }

        // Click Menu 2: Cài đặt HoyuKey
        if x >= card_x && x <= card_right && y >= 282 && y <= 356 {
            CURRENT_PAGE.store(2, Ordering::SeqCst);
            unsafe {
                InvalidateRect(hwnd, null_mut(), 0);
            }
            return;
        }

        // Click Menu 3: Giới thiệu
        if x >= card_x && x <= card_right && y >= 368 && y <= 442 {
            CURRENT_PAGE.store(3, Ordering::SeqCst);
            unsafe {
                InvalidateRect(hwnd, null_mut(), 0);
            }
            return;
        }
    } else {
        // --- XỬ LÝ CÁC TRANG CON ---
        // Click Nút Back tròn quay về Trang chủ:
        if x >= card_x && x <= card_x + 40 && y >= 20 && y <= 60 {
            CURRENT_PAGE.store(0, Ordering::SeqCst);
            unsafe {
                InvalidateRect(hwnd, null_mut(), 0);
            }
            return;
        }

        let mut state = GLOBAL_STATE.lock().unwrap();
        let mut changed = false;

        if page == 1 {
            // Tab Kiểu gõ chính:
            let tab_x = right_ctrl - 200;
            if y >= 105 && y <= 145 {
                if x >= tab_x && x <= tab_x + 96 {
                    if state.config.input_method != InputMethod::Telex {
                        state.config.input_method = InputMethod::Telex;
                        changed = true;
                    }
                } else if x >= tab_x + 104 && x <= right_ctrl {
                    if state.config.input_method != InputMethod::Vni {
                        state.config.input_method = InputMethod::Vni;
                        changed = true;
                    }
                }
            }

            // Switch Bỏ dấu kiểu mới:
            if x >= right_ctrl - 52 && x <= right_ctrl && y >= 204 && y <= 236 {
                state.config.modern_tone = !state.config.modern_tone;
                changed = true;
            }

            // Switch Tự động khôi phục phím:
            if x >= right_ctrl - 52 && x <= right_ctrl && y >= 280 && y <= 312 {
                state.config.auto_restore = !state.config.auto_restore;
                changed = true;
            }
        } else if page == 2 {
            // Tab Phím tắt chuyển đổi:
            let sw_tab_x = right_ctrl - 216;
            if y >= 105 && y <= 145 {
                if x >= sw_tab_x && x <= sw_tab_x + 104 {
                    if state.config.switch_key != SwitchKey::CtrlShift {
                        state.config.switch_key = SwitchKey::CtrlShift;
                        changed = true;
                    }
                } else if x >= sw_tab_x + 112 && x <= right_ctrl {
                    if state.config.switch_key != SwitchKey::AltZ {
                        state.config.switch_key = SwitchKey::AltZ;
                        changed = true;
                    }
                }
            }
        }

        if changed {
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

unsafe fn paint_shadcn_ui(dc: windows_sys::Win32::Graphics::Gdi::HDC, w: i32, h: i32, config: &Config) {
    let mut graphics: *mut GpGraphics = null_mut();
    GdipCreateFromHDC(dc, &mut graphics);
    if graphics.is_null() {
        return;
    }

    // Thiết lập kết xuất đồ họa: Anti-Aliasing, Bicubic Interpolation, ClearType Text
    GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias);
    GdipSetInterpolationMode(graphics, InterpolationModeHighQualityBicubic);
    GdipSetPixelOffsetMode(graphics, PixelOffsetModeHighQuality);
    GdipSetTextRenderingHint(graphics, TextRenderingHintClearTypeGridFit);

    // Tô nền màu Zinc #09090b
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

    // Font Family Google Sans hiện đại, hình học bo tròn mềm mại
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

    // Typography to rõ, chuẩn mắt nhìn 4K & High-DPI
    let font_title = create_gdip_font(family, 24.0, FontStyleBold);
    let font_sub_title = create_gdip_font(family, 21.0, FontStyleBold);
    let font_bold = create_gdip_font(family, 15.0, FontStyleBold);
    let font_desc = create_gdip_font(family, 12.0, FontStyleRegular);
    let font_section = create_gdip_font(family, 11.0, FontStyleBold);
    let font_tab = create_gdip_font(family, 14.0, FontStyleBold);
    let font_btn = create_gdip_font(family, 14.0, FontStyleBold);

    // Lưới căn chỉnh đối xứng hoàn hảo hai bên
    let (card_x, card_w, _card_right, right_ctrl, left_text, _) = get_layout_metrics(w);

    let current_page = CURRENT_PAGE.load(Ordering::SeqCst);
    let hover_id = HOVER_ITEM.load(Ordering::SeqCst);

    if current_page == 0 {
        // ========================================================
        // 🏠 TRANG CHỦ (HOME DASHBOARD)
        // ========================================================

        // 1. Header: Logo HK to 52x52 + Tiêu đề "HoyuKey Dashboard"
        gdiplus_draw_image(graphics, card_x, 20, 52, 52, LOGO_BGRA_128, 128, 128);
        draw_gdip_text(graphics, font_title, "HoyuKey Dashboard", card_x + 66, 17, ARGB_TEXT_PRIMARY, false);
        draw_gdip_text(graphics, font_desc, "Bộ gõ Tiếng Việt thế hệ mới cho PC", card_x + 66, 48, ARGB_TEXT_MUTED, false);

        // 2. Card Trạng thái hệ thống (ở trên cùng theo đúng yêu cầu)
        let status_bg = if hover_id == 10 { ARGB_CARD_HOVER } else { ARGB_CARD_BG };
        gdiplus_draw_rounded_card(graphics, card_x, 86, card_w, 76, 10, status_bg, ARGB_CARD_BORDER, 1.0);

        let status_color = if config.enabled { ARGB_STATUS_GREEN } else { ARGB_STATUS_MUTED };
        gdiplus_fill_ellipse(graphics, left_text as f32, 118.0, 12.0, 12.0, status_color);

        let status_str = if config.enabled {
            "Trạng thái: Đang bật Tiếng Việt [V]"
        } else {
            "Trạng thái: Đang tắt Tiếng Việt [E]"
        };
        draw_gdip_text(graphics, font_bold, status_str, left_text + 22, 102, ARGB_TEXT_PRIMARY, false);
        draw_gdip_text(graphics, font_desc, "Nhấp vào đây hoặc dùng phím tắt để đổi chế độ", left_text + 22, 126, ARGB_TEXT_MUTED, false);

        // Badge [V] / [E] ở mép phải
        let badge_bg = if config.enabled { 0xFF1C2C20 } else { 0xFF222227 };
        let badge_text_color = if config.enabled { ARGB_STATUS_GREEN } else { ARGB_TEXT_MUTED };
        gdiplus_draw_rounded_card(graphics, right_ctrl - 44, 104, 44, 40, 8, badge_bg, ARGB_CARD_BORDER, 1.0);
        draw_gdip_text_centered(graphics, font_bold, if config.enabled { "V" } else { "E" }, right_ctrl - 44, 104, 44, 40, badge_text_color);

        // 3. Tiêu đề nhóm: DANH MỤC CÀI ĐẶT
        draw_gdip_text(graphics, font_section, "DANH MỤC CÀI ĐẶT", card_x + 4, 175, ARGB_STATUS_MUTED, false);

        // 4. Menu item 1: Cài đặt đánh máy (Typing Settings)
        let menu1_bg = if hover_id == 11 { ARGB_CARD_HOVER } else { ARGB_CARD_BG };
        gdiplus_draw_rounded_card(graphics, card_x, 196, card_w, 74, 10, menu1_bg, ARGB_CARD_BORDER, 1.0);
        // Icon tròn
        gdiplus_draw_rounded_card(graphics, left_text, 213, 40, 40, 20, ARGB_PILL_TRACK, ARGB_CARD_BORDER, 1.0);
        draw_gdip_text_centered(graphics, font_bold, "⌨", left_text, 213, 40, 40, ARGB_WHITE);

        draw_gdip_text(graphics, font_bold, "Cài đặt đánh máy", left_text + 52, 211, ARGB_TEXT_PRIMARY, false);
        draw_gdip_text(graphics, font_desc, "Kiểu gõ Telex / VNI, bỏ dấu mới, khôi phục phím", left_text + 52, 235, ARGB_TEXT_MUTED, false);
        // Mũi tên >
        draw_gdip_text_centered(graphics, font_bold, ">", right_ctrl - 24, 211, 24, 44, ARGB_TEXT_MUTED);

        // 5. Menu item 2: Cài đặt HoyuKey (App Settings)
        let menu2_bg = if hover_id == 12 { ARGB_CARD_HOVER } else { ARGB_CARD_BG };
        gdiplus_draw_rounded_card(graphics, card_x, 282, card_w, 74, 10, menu2_bg, ARGB_CARD_BORDER, 1.0);
        // Icon tròn
        gdiplus_draw_rounded_card(graphics, left_text, 299, 40, 40, 20, ARGB_PILL_TRACK, ARGB_CARD_BORDER, 1.0);
        draw_gdip_text_centered(graphics, font_bold, "⚙", left_text, 299, 40, 40, ARGB_WHITE);

        draw_gdip_text(graphics, font_bold, "Cài đặt HoyuKey", left_text + 52, 297, ARGB_TEXT_PRIMARY, false);
        draw_gdip_text(graphics, font_desc, "Phím tắt chuyển đổi, hệ thống khởi động", left_text + 52, 321, ARGB_TEXT_MUTED, false);
        // Mũi tên >
        draw_gdip_text_centered(graphics, font_bold, ">", right_ctrl - 24, 297, 24, 44, ARGB_TEXT_MUTED);

        // 6. Menu item 3: Giới thiệu (About)
        let menu3_bg = if hover_id == 13 { ARGB_CARD_HOVER } else { ARGB_CARD_BG };
        gdiplus_draw_rounded_card(graphics, card_x, 368, card_w, 74, 10, menu3_bg, ARGB_CARD_BORDER, 1.0);
        // Icon tròn
        gdiplus_draw_rounded_card(graphics, left_text, 385, 40, 40, 20, ARGB_PILL_TRACK, ARGB_CARD_BORDER, 1.0);
        draw_gdip_text_centered(graphics, font_bold, "ℹ", left_text, 385, 40, 40, ARGB_WHITE);

        draw_gdip_text(graphics, font_bold, "Giới thiệu", left_text + 52, 383, ARGB_TEXT_PRIMARY, false);
        draw_gdip_text(graphics, font_desc, "Phiên bản v0.1, tác giả Hoyuuna, mã nguồn mở", left_text + 52, 407, ARGB_TEXT_MUTED, false);
        // Mũi tên >
        draw_gdip_text_centered(graphics, font_bold, ">", right_ctrl - 24, 383, 24, 44, ARGB_TEXT_MUTED);

    } else {
        // ========================================================
        // 📱 CÁC TRANG CON (SUBPAGES - Y NHƯ PHONE SETTINGS)
        // ========================================================

        // Nút Back tròn 40x40 ở trên cùng bên trái
        let back_bg = if hover_id == 20 { ARGB_CARD_HOVER } else { ARGB_CARD_BG };
        gdiplus_draw_rounded_card(graphics, card_x, 20, 40, 40, 20, back_bg, ARGB_CARD_BORDER, 1.0);
        draw_gdip_text_centered(graphics, font_bold, "<", card_x, 20, 40, 40, ARGB_WHITE);

        if current_page == 1 {
            // --- Trang Cài đặt đánh máy ---
            draw_gdip_text(graphics, font_sub_title, "Cài đặt đánh máy", card_x + 54, 23, ARGB_TEXT_PRIMARY, false);

            // Card 1: Kiểu gõ chính (Telex / VNI) - chiều cao 86px
            gdiplus_draw_rounded_card(graphics, card_x, 86, card_w, 86, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);
            draw_gdip_text(graphics, font_bold, "Kiểu gõ chính", left_text, 105, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Chọn phương thức gõ Telex hoặc VNI", left_text, 131, ARGB_TEXT_MUTED, false);

            let method_progress = INPUT_METHOD_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_tabs(
                graphics,
                right_ctrl - 200,
                105,
                &["Telex", "VNI"],
                96,
                40,
                method_progress,
                font_tab,
            );

            // Card 2: Tùy chọn gõ dấu - chiều cao 164px
            gdiplus_draw_rounded_card(graphics, card_x, 186, card_w, 164, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);

            // Row 1: Bỏ dấu kiểu mới
            draw_gdip_text(graphics, font_bold, "Bỏ dấu kiểu mới", left_text, 201, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Đặt dấu ở âm thứ hai: hóa, hòa thay vì hoá, hoà", left_text, 226, ARGB_TEXT_MUTED, false);

            let tone_progress = MODERN_TONE_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_switch(graphics, right_ctrl - 50, 208, tone_progress);

            // Divider
            gdiplus_draw_line(graphics, left_text, 268, right_ctrl, 268, ARGB_DIVIDER, 1.0);

            // Row 2: Tự động khôi phục phím
            draw_gdip_text(graphics, font_bold, "Tự động khôi phục phím", left_text, 283, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Khôi phục ký tự gốc khi gõ lặp dấu hoặc từ sai", left_text, 308, ARGB_TEXT_MUTED, false);

            let restore_progress = AUTO_RESTORE_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_switch(graphics, right_ctrl - 50, 288, restore_progress);

        } else if current_page == 2 {
            // --- Trang Cài đặt HoyuKey ---
            draw_gdip_text(graphics, font_sub_title, "Cài đặt HoyuKey", card_x + 54, 23, ARGB_TEXT_PRIMARY, false);

            // Card 1: Phím tắt chuyển đổi
            gdiplus_draw_rounded_card(graphics, card_x, 86, card_w, 86, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);
            draw_gdip_text(graphics, font_bold, "Phím tắt chuyển đổi", left_text, 105, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Đổi nhanh giữa Tiếng Việt (V) và Anh (E)", left_text, 131, ARGB_TEXT_MUTED, false);

            let switch_progress = SWITCH_KEY_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
            draw_animated_tabs(
                graphics,
                right_ctrl - 216,
                105,
                &["Ctrl + Shift", "Alt + Z"],
                104,
                40,
                switch_progress,
                font_tab,
            );

            // Card 2: Trạng thái khởi động
            gdiplus_draw_rounded_card(graphics, card_x, 186, card_w, 86, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);
            draw_gdip_text(graphics, font_bold, "Khởi động cùng Windows", left_text, 205, ARGB_TEXT_PRIMARY, false);
            draw_gdip_text(graphics, font_desc, "Tự động kích hoạt HoyuKey khi bật máy tính", left_text, 231, ARGB_TEXT_MUTED, false);
            draw_animated_switch(graphics, right_ctrl - 50, 212, 1.0);

        } else if current_page == 3 {
            // --- Trang Giới thiệu ---
            draw_gdip_text(graphics, font_sub_title, "Giới thiệu", card_x + 54, 23, ARGB_TEXT_PRIMARY, false);

            // Card thông tin
            gdiplus_draw_rounded_card(graphics, card_x, 86, card_w, 360, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);

            let center_x = card_x + card_w / 2;
            gdiplus_draw_image(graphics, center_x - 32, 114, 64, 64, LOGO_BGRA_128, 128, 128);

            draw_gdip_text_centered(graphics, font_title, "HoyuKey", card_x, 190, card_w, 30, ARGB_TEXT_PRIMARY);
            draw_gdip_text_centered(graphics, font_desc, "Phiên bản: 0.1 (Tự động cập nhật)", card_x, 222, card_w, 24, ARGB_TEXT_MUTED);

            gdiplus_draw_line(graphics, left_text + 40, 260, right_ctrl - 40, 260, ARGB_DIVIDER, 1.0);

            draw_gdip_text_centered(graphics, font_bold, "Tác giả: Hoyuuna", card_x, 276, card_w, 26, ARGB_TEXT_PRIMARY);
            draw_gdip_text_centered(graphics, font_desc, "Bộ gõ Tiếng Việt thế hệ mới cho PC viết bằng Rust.", card_x, 304, card_w, 24, ARGB_TEXT_MUTED);
            draw_gdip_text_centered(graphics, font_desc, "Hiệu năng cao, bảo mật tuyệt đối, mã nguồn mở.", card_x, 328, card_w, 24, ARGB_TEXT_MUTED);
            draw_gdip_text_centered(graphics, font_bold, "https://github.com/hoyuuna/HoyuKey", card_x, 368, card_w, 26, 0xFF60A5FA);
        }
    }

    // ========================================================
    // ⚓ CỐ ĐỊNH 2 NÚT Ở DƯỚI CÙNG (FIXED BOTTOM DOCK)
    // Kéo dài ra không còn khoảng thừa ở giữa!
    // ========================================================
    let btn_w = (card_w - 12) / 2; // Chia đều hai bên, khoảng cách giữa đúng 12px
    let btn_y = 590;
    let btn_h = 46;

    // Nút Thoát (trái): mép trái card_x, rộng btn_w
    let exit_bg = if hover_id == 30 { ARGB_DANGER_HOVER } else { ARGB_CARD_BG };
    gdiplus_draw_rounded_card(graphics, card_x, btn_y, btn_w, btn_h, 8, exit_bg, ARGB_DANGER_BORDER, 1.0);
    draw_gdip_text_centered(graphics, font_btn, "Thoát HoyuKey", card_x, btn_y, btn_w, btn_h, ARGB_DANGER_TEXT);

    // Nút Đóng (phải): mép phải card_right, rộng btn_w
    let btn_close_x = card_x + btn_w + 12;
    let close_bg = if hover_id == 31 { ARGB_WHITE_HOVER } else { ARGB_WHITE };
    gdiplus_draw_rounded_card(graphics, btn_close_x, btn_y, btn_w, btn_h, 8, close_bg, ARGB_WHITE, 1.0);
    draw_gdip_text_centered(graphics, font_btn, "Đóng", btn_close_x, btn_y, btn_w, btn_h, ARGB_TEXT_DARK);

    // Dọn dẹp tài nguyên GDI+
    GdipDeleteFont(font_title);
    GdipDeleteFont(font_sub_title);
    GdipDeleteFont(font_bold);
    GdipDeleteFont(font_desc);
    GdipDeleteFont(font_section);
    GdipDeleteFont(font_tab);
    GdipDeleteFont(font_btn);
    if !family.is_null() {
        GdipDeleteFontFamily(family);
    }
    GdipDeleteGraphics(graphics);
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

    // Rãnh: OFF là Đen Zinc #18181b, ON là Trắng tinh #ffffff
    let r_track = (24.0 + (255.0 - 24.0) * progress).round() as u32;
    let g_track = (24.0 + (255.0 - 24.0) * progress).round() as u32;
    let b_track = (27.0 + (255.0 - 27.0) * progress).round() as u32;
    let color_track = 0xFF000000 | (r_track << 16) | (g_track << 8) | b_track;

    let r_border = (63.0 + (255.0 - 63.0) * progress).round() as u32;
    let g_border = (63.0 + (255.0 - 63.0) * progress).round() as u32;
    let b_border = (70.0 + (255.0 - 70.0) * progress).round() as u32;
    let color_border = 0xFF000000 | (r_border << 16) | (g_border << 8) | b_border;

    gdiplus_draw_rounded_card(graphics, x, y, w, h, radius, color_track, color_border, 1.0);

    // Núm gạt: to 22px
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

    // Top-left arc
    GdipAddPathArc(path, x, y, d, d, 180.0, 90.0);
    // Top line
    GdipAddPathLine(path, x + r, y, x + w - r, y);
    // Top-right arc
    GdipAddPathArc(path, x + w - d, y, d, d, 270.0, 90.0);
    // Right line
    GdipAddPathLine(path, x + w, y + r, x + w, y + h - r);
    // Bottom-right arc
    GdipAddPathArc(path, x + w - d, y + h - d, d, d, 0.0, 90.0);
    // Bottom line
    GdipAddPathLine(path, x + w - r, y + h, x + r, y + h);
    // Bottom-left arc
    GdipAddPathArc(path, x, y + h - d, d, d, 90.0, 90.0);
    // Left line
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
        Width: 420.0,
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
    GdipCreateStringFormat(0, 0, &mut format);
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

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
