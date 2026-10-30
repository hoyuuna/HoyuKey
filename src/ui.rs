use std::ptr::null_mut;
use std::sync::atomic::{AtomicIsize, AtomicU32, AtomicUsize, Ordering};
use windows_sys::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
    EndPaint, InvalidateRect, ScreenToClient, SelectObject, PAINTSTRUCT, SRCCOPY,
};
use windows_sys::Win32::Graphics::GdiPlus::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, GetClientRect, GetCursorPos,
    GetSystemMetrics, KillTimer, LoadCursorW, PostMessageW, PostQuitMessage, RegisterClassW,
    SendMessageW, SetCursor, SetForegroundWindow, SetTimer, ShowWindow, CS_HREDRAW, CS_VREDRAW,
    IDC_ARROW, IDC_HAND, ICON_BIG, ICON_SMALL, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_RESTORE,
    SW_SHOW, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_LBUTTONDOWN, WM_PAINT, WM_SETCURSOR,
    WM_SETICON, WM_TIMER, WNDCLASSW, WS_CAPTION, WS_EX_DLGMODALFRAME, WS_MINIMIZEBOX,
    WS_OVERLAPPED, WS_SYSMENU,
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

// Nhúng logo Premultiplied ARGB chuẩn màu
const LOGO_BGRA_128: &[u8] = include_bytes!("../icon/logo_128.bin");
const LOGO_BGRA_32: &[u8] = include_bytes!("../icon/logo_32.bin");

// Bảng màu chuẩn Shadcn UI Dark Mode (Zinc Palette)
const ARGB_BG: u32 = 0xFF09090B;              // #09090b
const ARGB_CARD_BG: u32 = 0xFF141417;         // #141417
const ARGB_CARD_BORDER: u32 = 0xFF27272A;     // #27272a
const ARGB_DIVIDER: u32 = 0xFF202024;         // #202024
const ARGB_TEXT_PRIMARY: u32 = 0xFFF4F4F5;    // #f4f4f5 (trắng sáng mịn)
const ARGB_TEXT_MUTED: u32 = 0xFFA1A1AA;      // #a1a1aa (xám zinc chuẩn)
const ARGB_STATUS_GREEN: u32 = 0xFF22C55E;    // #22c55e
const ARGB_STATUS_MUTED: u32 = 0xFF71717A;    // #71717a
const ARGB_PILL_TRACK: u32 = 0xFF1C1C20;      // #1c1c20
const ARGB_WHITE: u32 = 0xFFFAFAFA;           // #fafafa
const ARGB_TEXT_DARK: u32 = 0xFF09090B;       // #09090b
const ARGB_DANGER_TEXT: u32 = 0xFFEF4444;     // #ef4444
const ARGB_DANGER_BORDER: u32 = 0xFF3F1D1D;   // #3f1d1d

pub fn init_ui_subsystems() {
    unsafe {
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
    let window_title = to_wide("HoyuKey - Bảng điều khiển");

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

            if is_clickable_area(pt.x, pt.y, w) {
                SetCursor(LoadCursorW(null_mut(), IDC_HAND));
                1
            } else {
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
    let right_ctrl = card_right - 18;
    let left_text = card_x + 18;
    (card_x, card_w, card_right, right_ctrl, left_text, margin_x)
}

fn is_clickable_area(x: i32, y: i32, client_w: i32) -> bool {
    let (card_x, _, card_right, right_ctrl, _, _) = get_layout_metrics(client_w);

    // 1. Tab Kiểu gõ (Telex / VNI): tab_w = 90 -> total = 188
    let tab_x = right_ctrl - 188;
    if x >= tab_x && x <= right_ctrl && y >= 100 && y <= 146 {
        return true;
    }
    // 2. Switch Bỏ dấu kiểu mới: w = 48
    let sw_x = right_ctrl - 48;
    if x >= sw_x && x <= right_ctrl && y >= 198 && y <= 234 {
        return true;
    }
    // 3. Switch Tự động khôi phục phím: w = 48
    if x >= sw_x && x <= right_ctrl && y >= 270 && y <= 306 {
        return true;
    }
    // 4. Tab Phím tắt (Ctrl+Shift / Alt+Z): tab_w = 98 -> total = 204
    let sw_tab_x = right_ctrl - 204;
    if x >= sw_tab_x && x <= right_ctrl && y >= 348 && y <= 394 {
        return true;
    }
    // 5. Nút Thoát HoyuKey: mép trái card_x, w = 140
    if x >= card_x && x <= card_x + 140 && y >= 530 && y <= 572 {
        return true;
    }
    // 6. Nút Đóng: mép phải card_right, w = 160
    if x >= card_right - 160 && x <= card_right && y >= 530 && y <= 572 {
        return true;
    }
    false
}

fn handle_mouse_click(hwnd: HWND, x: i32, y: i32, client_w: i32) {
    let (card_x, _, card_right, right_ctrl, _, _) = get_layout_metrics(client_w);

    let mut state = GLOBAL_STATE.lock().unwrap();
    let mut changed = false;

    // 1. Click Tab Kiểu gõ (Telex / VNI)
    let tab_x = right_ctrl - 188;
    if y >= 100 && y <= 146 {
        if x >= tab_x && x <= tab_x + 90 {
            if state.config.input_method != InputMethod::Telex {
                state.config.input_method = InputMethod::Telex;
                changed = true;
            }
        } else if x >= tab_x + 98 && x <= right_ctrl {
            if state.config.input_method != InputMethod::Vni {
                state.config.input_method = InputMethod::Vni;
                changed = true;
            }
        }
    }

    // 2. Click Switch Bỏ dấu kiểu mới
    let sw_x = right_ctrl - 48;
    if x >= sw_x && x <= right_ctrl && y >= 198 && y <= 234 {
        state.config.modern_tone = !state.config.modern_tone;
        changed = true;
    }

    // 3. Click Switch Tự động khôi phục phím
    if x >= sw_x && x <= right_ctrl && y >= 270 && y <= 306 {
        state.config.auto_restore = !state.config.auto_restore;
        changed = true;
    }

    // 4. Click Tab Phím tắt (Ctrl+Shift / Alt+Z)
    let sw_tab_x = right_ctrl - 204;
    if y >= 348 && y <= 394 {
        if x >= sw_tab_x && x <= sw_tab_x + 98 {
            if state.config.switch_key != SwitchKey::CtrlShift {
                state.config.switch_key = SwitchKey::CtrlShift;
                changed = true;
            }
        } else if x >= sw_tab_x + 106 && x <= right_ctrl {
            if state.config.switch_key != SwitchKey::AltZ {
                state.config.switch_key = SwitchKey::AltZ;
                changed = true;
            }
        }
    }

    // 5. Nút Thoát HoyuKey
    if x >= card_x && x <= card_x + 140 && y >= 530 && y <= 572 {
        drop(state);
        unsafe {
            PostQuitMessage(0);
        }
        return;
    }

    // 6. Nút Đóng (Ẩn về khay)
    if x >= card_right - 160 && x <= card_right && y >= 530 && y <= 572 {
        drop(state);
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
        }
        return;
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

unsafe fn paint_shadcn_ui(dc: windows_sys::Win32::Graphics::Gdi::HDC, w: i32, h: i32, config: &Config) {
    let mut graphics: *mut GpGraphics = null_mut();
    GdipCreateFromHDC(dc, &mut graphics);
    if graphics.is_null() {
        return;
    }

    // Thiết lập kết xuất siêu mượt: Anti-Aliasing, Bicubic Interpolation, ClearType Text
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

    // Font Family Segoe UI hiện đại
    let mut family: *mut GpFontFamily = null_mut();
    let family_name = to_wide("Segoe UI Variable Text");
    GdipCreateFontFamilyFromName(family_name.as_ptr(), null_mut(), &mut family);
    if family.is_null() {
        let fallback = to_wide("Segoe UI");
        GdipCreateFontFamilyFromName(fallback.as_ptr(), null_mut(), &mut family);
    }

    let font_title = create_gdip_font(family, 19.0, FontStyleBold);
    let font_bold = create_gdip_font(family, 12.5, FontStyleBold);
    let font_body = create_gdip_font(family, 11.5, FontStyleRegular);
    let font_desc = create_gdip_font(family, 10.5, FontStyleRegular);
    let font_badge = create_gdip_font(family, 9.5, FontStyleBold);

    // Tính toán lưới căn chỉnh đối xứng hoàn hảo hai bên
    let (card_x, card_w, card_right, right_ctrl, left_text, _) = get_layout_metrics(w);

    // 1. Header: Logo HK 48x48 siêu sắc nét + Tiêu đề
    gdiplus_draw_image(graphics, card_x, 22, 48, 48, LOGO_BGRA_128, 128, 128);

    draw_gdip_text(graphics, font_title, "HoyuKey", card_x + 60, 21, ARGB_TEXT_PRIMARY, false);

    // Badge Pre-release
    gdiplus_draw_rounded_card(graphics, card_x + 160, 26, 84, 20, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);
    draw_gdip_text_centered(graphics, font_badge, "Pre-release", card_x + 160, 26, 84, 20, ARGB_TEXT_MUTED);

    draw_gdip_text(graphics, font_desc, "Bộ gõ Tiếng Việt thế hệ mới cho PC", card_x + 60, 48, ARGB_TEXT_MUTED, false);

    // 2. Card 1: Kiểu gõ chính (Telex / VNI)
    gdiplus_draw_rounded_card(graphics, card_x, 86, card_w, 76, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);
    draw_gdip_text(graphics, font_bold, "Kiểu gõ chính", left_text, 102, ARGB_TEXT_PRIMARY, false);
    draw_gdip_text(graphics, font_desc, "Chọn phương thức gõ Telex hoặc VNI", left_text, 124, ARGB_TEXT_MUTED, false);

    let method_progress = INPUT_METHOD_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
    draw_animated_tabs(
        graphics,
        right_ctrl - 188,
        102,
        &["Telex", "VNI"],
        90,
        method_progress,
        font_bold,
    );

    // 3. Card 2: Tùy chọn gõ dấu
    gdiplus_draw_rounded_card(graphics, card_x, 174, card_w, 148, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);

    // Row 1: Bỏ dấu kiểu mới
    draw_gdip_text(graphics, font_bold, "Bỏ dấu kiểu mới", left_text, 192, ARGB_TEXT_PRIMARY, false);
    draw_gdip_text(graphics, font_desc, "Đặt dấu ở âm thứ hai: hóa, hòa thay vì hoá, hoà", left_text, 214, ARGB_TEXT_MUTED, false);

    let tone_progress = MODERN_TONE_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
    draw_animated_switch(graphics, right_ctrl - 48, 202, tone_progress);

    // Đường phân cách căn chuẩn từ left_text đến right_ctrl
    gdiplus_draw_line(graphics, left_text, 248, right_ctrl, 248, ARGB_DIVIDER, 1.0);

    // Row 2: Tự động khôi phục phím
    draw_gdip_text(graphics, font_bold, "Tự động khôi phục phím", left_text, 264, ARGB_TEXT_PRIMARY, false);
    draw_gdip_text(graphics, font_desc, "Khôi phục ký tự gốc khi gõ lặp dấu hoặc từ sai", left_text, 286, ARGB_TEXT_MUTED, false);

    let restore_progress = AUTO_RESTORE_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
    draw_animated_switch(graphics, right_ctrl - 48, 274, restore_progress);

    // 4. Card 3: Phím tắt chuyển đổi
    gdiplus_draw_rounded_card(graphics, card_x, 334, card_w, 76, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);
    draw_gdip_text(graphics, font_bold, "Phím tắt chuyển đổi", left_text, 350, ARGB_TEXT_PRIMARY, false);
    draw_gdip_text(graphics, font_desc, "Đổi nhanh giữa Tiếng Việt (V) và Anh (E)", left_text, 372, ARGB_TEXT_MUTED, false);

    let switch_progress = SWITCH_KEY_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
    draw_animated_tabs(
        graphics,
        right_ctrl - 204,
        350,
        &["Ctrl + Shift", "Alt + Z"],
        98,
        switch_progress,
        font_bold,
    );

    // 5. Card 4: Trạng thái hệ thống
    gdiplus_draw_rounded_card(graphics, card_x, 422, card_w, 72, 10, ARGB_CARD_BG, ARGB_CARD_BORDER, 1.0);
    let status_color = if config.enabled { ARGB_STATUS_GREEN } else { ARGB_STATUS_MUTED };
    gdiplus_fill_ellipse(graphics, left_text as f32, 452.0, 10.0, 10.0, status_color);

    let status_str = if config.enabled {
        "Trạng thái: Đang bật Tiếng Việt [V]"
    } else {
        "Trạng thái: Đang tắt Tiếng Việt [E]"
    };
    draw_gdip_text(graphics, font_bold, status_str, left_text + 18, 436, ARGB_TEXT_PRIMARY, false);
    draw_gdip_text(graphics, font_desc, "Nhấn phím tắt hoặc click khay hệ thống để đổi chế độ", left_text + 18, 458, ARGB_TEXT_MUTED, false);

    // 6. Footer Buttons: Căn thẳng hàng đối xứng tuyệt đối với hai cạnh Card
    // Nút Thoát: mép trái card_x, rộng 140px
    gdiplus_draw_rounded_card(graphics, card_x, 530, 140, 42, 8, ARGB_CARD_BG, ARGB_DANGER_BORDER, 1.0);
    draw_gdip_text_centered(graphics, font_bold, "Thoát HoyuKey", card_x, 530, 140, 42, ARGB_DANGER_TEXT);

    // Nút Đóng: mép phải card_right, rộng 160px
    let btn_close_x = card_right - 160;
    gdiplus_draw_rounded_card(graphics, btn_close_x, 530, 160, 42, 8, ARGB_WHITE, ARGB_WHITE, 1.0);
    draw_gdip_text_centered(graphics, font_bold, "Đóng (Ẩn về khay)", btn_close_x, 530, 160, 42, ARGB_TEXT_DARK);

    // Dọn dẹp tài nguyên GDI+
    GdipDeleteFont(font_title);
    GdipDeleteFont(font_bold);
    GdipDeleteFont(font_body);
    GdipDeleteFont(font_desc);
    GdipDeleteFont(font_badge);
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
    let w = 48;
    let h = 26;
    let radius = 13;

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

    // Núm gạt: OFF là Trắng #fafafa, ON là Đen #09090b
    let knob_diameter = 20.0;
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
    progress: f32,
    font: *mut GpFont,
) {
    let tab_h = 36;
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
        Width: 400.0,
        Height: 30.0,
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
