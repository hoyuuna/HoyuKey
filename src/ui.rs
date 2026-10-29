use std::ptr::null_mut;
use std::sync::atomic::{AtomicIsize, Ordering};
use windows_sys::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreatePen,
    CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, Ellipse, EndPaint, InvalidateRect,
    RoundRect, ScreenToClient, SelectObject, SetBkMode, SetTextColor, DT_CENTER, DT_LEFT,
    DT_SINGLELINE, DT_VCENTER, HFONT, PAINTSTRUCT, PS_SOLID, SRCCOPY, TRANSPARENT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, GetCursorPos, GetSystemMetrics, LoadCursorW,
    PostMessageW, PostQuitMessage, RegisterClassW, SetCursor, SetForegroundWindow, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, IDC_ARROW, IDC_HAND, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_RESTORE,
    SW_SHOW, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_LBUTTONDOWN, WM_PAINT, WM_SETCURSOR,
    WNDCLASSW, WS_CAPTION, WS_EX_DLGMODALFRAME, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU,
};

use crate::config::{Config, InputMethod, SwitchKey};
use crate::tray::WM_APP_UPDATE_TRAY;
use crate::GLOBAL_STATE;

pub static SETTINGS_HWND: AtomicIsize = AtomicIsize::new(0);

// Shadcn UI Dark Palette (Zinc / Neutral)
const COLOR_BG: u32 = 0x0B0909;          // #09090b (BGR)
const COLOR_CARD_BG: u32 = 0x151212;     // #121215
const COLOR_CARD_BORDER: u32 = 0x2A2727; // #27272a
const COLOR_BORDER_SUBTLE: u32 = 0x332E2E;
const COLOR_TEXT_PRIMARY: u32 = 0xF5F4F4;  // #f4f4f5
const COLOR_TEXT_MUTED: u32 = 0x968C8C;    // #8c8c96
const COLOR_ACCENT_BLUE: u32 = 0xF6823B;   // #3b82f6 (BGR)
const COLOR_TAB_ACTIVE_BG: u32 = 0xFAFAFA;
const COLOR_TAB_ACTIVE_TEXT: u32 = 0x0B0909;
const COLOR_TAB_INACTIVE_BG: u32 = 0x1B1818;
const COLOR_TAB_INACTIVE_TEXT: u32 = 0xAAAAA1;
const COLOR_STATUS_GREEN: u32 = 0x5EC522;  // #22c55e (BGR)
const COLOR_BTN_DANGER: u32 = 0x4444EF;    // #ef4444 (BGR)

pub fn create_settings_window(hinstance: HMODULE) -> HWND {
    let class_name = to_wide("HoyuKey_ShadcnSettingsClass");
    let window_title = to_wide("HoyuKey - Bảng điều khiển");

    unsafe {
        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(settings_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null_mut(),
            lpszClassName: class_name.as_ptr(),
        };

        RegisterClassW(&wnd_class);

        let width = 496;
        let height = 540;
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

        SETTINGS_HWND.store(hwnd as isize, Ordering::SeqCst);
        hwnd
    }
}

pub fn show_settings_window() {
    let hwnd = SETTINGS_HWND.load(Ordering::SeqCst) as HWND;
    if hwnd != null_mut() {
        unsafe {
            ShowWindow(hwnd, SW_RESTORE);
            ShowWindow(hwnd, SW_SHOW);
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
        WM_ERASEBKGND => 1, // Chống chớp màn hình khi re-render
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            let w = rect.right - rect.left;
            let h = rect.bottom - rect.top;

            // Double Buffering
            let mem_dc = CreateCompatibleDC(hdc);
            let mem_bm = CreateCompatibleBitmap(hdc, w, h);
            let old_bm = SelectObject(mem_dc, mem_bm);

            let config = {
                let state = GLOBAL_STATE.lock().unwrap();
                state.config.clone()
            };

            paint_shadcn_ui(mem_dc, w, h, &config);

            BitBlt(hdc, 0, 0, w, h, mem_dc, 0, 0, SRCCOPY);

            SelectObject(mem_dc, old_bm);
            DeleteObject(mem_bm);
            DeleteDC(mem_dc);
            EndPaint(hwnd, &ps);
            0
        }
        WM_LBUTTONDOWN => {
            let x = (lparam & 0xFFFF) as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i32;
            handle_mouse_click(hwnd, x, y);
            0
        }
        WM_SETCURSOR => {
            let mut pt: POINT = std::mem::zeroed();
            GetCursorPos(&mut pt);
            ScreenToClient(hwnd, &mut pt);

            if is_clickable_area(pt.x, pt.y) {
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
            SETTINGS_HWND.store(0, Ordering::SeqCst);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn is_clickable_area(x: i32, y: i32) -> bool {
    // Kiểu gõ Telex / VNI
    (x >= 280 && x <= 456 && y >= 86 && y <= 126)
    // Switch Bỏ dấu kiểu mới
    || (x >= 400 && x <= 455 && y >= 168 && y <= 204)
    // Switch Tự động khôi phục phím
    || (x >= 400 && x <= 455 && y >= 226 && y <= 262)
    // Phím tắt Ctrl+Shift / Alt+Z
    || (x >= 260 && x <= 456 && y >= 300 && y <= 340)
    // Nút Thoát
    || (x >= 24 && x <= 150 && y >= 445 && y <= 485)
    // Nút Đóng
    || (x >= 310 && x <= 456 && y >= 445 && y <= 485)
}

fn handle_mouse_click(hwnd: HWND, x: i32, y: i32) {
    let mut state = GLOBAL_STATE.lock().unwrap();
    let mut changed = false;

    // 1. Click Tab Kiểu gõ
    if y >= 86 && y <= 126 {
        if x >= 280 && x <= 366 {
            state.config.input_method = InputMethod::Telex;
            changed = true;
        } else if x >= 370 && x <= 456 {
            state.config.input_method = InputMethod::Vni;
            changed = true;
        }
    }

    // 2. Click Switch Bỏ dấu kiểu mới
    if x >= 390 && x <= 455 && y >= 165 && y <= 205 {
        state.config.modern_tone = !state.config.modern_tone;
        changed = true;
    }

    // 3. Click Switch Tự động khôi phục
    if x >= 390 && x <= 455 && y >= 222 && y <= 262 {
        state.config.auto_restore = !state.config.auto_restore;
        changed = true;
    }

    // 4. Click Tab Phím tắt
    if y >= 300 && y <= 340 {
        if x >= 260 && x <= 356 {
            state.config.switch_key = SwitchKey::CtrlShift;
            changed = true;
        } else if x >= 360 && x <= 456 {
            state.config.switch_key = SwitchKey::AltZ;
            changed = true;
        }
    }

    // 5. Nút Thoát HoyuKey
    if x >= 24 && x <= 150 && y >= 445 && y <= 485 {
        drop(state);
        unsafe {
            PostQuitMessage(0);
        }
        return;
    }

    // 6. Nút Đóng (Ẩn về khay)
    if x >= 310 && x <= 456 && y >= 445 && y <= 485 {
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
            InvalidateRect(hwnd, null_mut(), 0);
            if main_hwnd != 0 {
                PostMessageW(main_hwnd as HWND, WM_APP_UPDATE_TRAY, 0, 0);
            }
        }
    }
}

unsafe fn paint_shadcn_ui(dc: windows_sys::Win32::Graphics::Gdi::HDC, w: i32, h: i32, config: &Config) {
    // 1. Nền tổng thể #09090b
    let bg_brush = CreateSolidBrush(COLOR_BG);
    let bg_rect = RECT { left: 0, top: 0, right: w, bottom: h };
    windows_sys::Win32::Graphics::Gdi::FillRect(dc, &bg_rect, bg_brush);
    DeleteObject(bg_brush);

    SetBkMode(dc, TRANSPARENT as _);

    // Fonts
    let font_title = create_font(20, 700);
    let font_subtitle = create_font(13, 400);
    let font_bold = create_font(14, 600);
    let font_body = create_font(13, 400);
    let font_small = create_font(11, 400);
    let font_badge = create_font(11, 600);

    // 2. Header
    // Icon badge "HK"
    draw_rounded_card(dc, 24, 20, 42, 42, 8, COLOR_CARD_BG, COLOR_CARD_BORDER);
    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_ACCENT_BLUE);
    draw_centered_text(dc, "HK", 24, 20, 42, 42);

    // Tiêu đề HoyuKey
    SelectObject(dc, font_title);
    SetTextColor(dc, COLOR_TEXT_PRIMARY);
    draw_text(dc, "HoyuKey", 76, 20);

    // Badge "Pre-release"
    draw_rounded_card(dc, 168, 24, 88, 20, 10, COLOR_CARD_BG, COLOR_CARD_BORDER);
    SelectObject(dc, font_badge);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_centered_text(dc, "Pre-release", 168, 24, 88, 20);

    // Subtitle
    SelectObject(dc, font_subtitle);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Bộ gõ Tiếng Việt thế hệ mới cho PC", 76, 45);

    // 3. Card 1: Kiểu gõ (Telex / VNI)
    draw_rounded_card(dc, 24, 76, 432, 60, 8, COLOR_CARD_BG, COLOR_CARD_BORDER);
    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_TEXT_PRIMARY);
    draw_text(dc, "Kiểu gõ chính", 40, 87);
    SelectObject(dc, font_small);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Chọn cách nhập dấu thanh và dấu mũ", 40, 107);

    // Segmented Tabs cho Telex & VNI
    draw_segmented_tabs(
        dc,
        280,
        88,
        &[("Telex", config.input_method == InputMethod::Telex), ("VNI", config.input_method == InputMethod::Vni)],
        font_bold,
    );

    // 4. Card 2: Tùy chọn gõ dấu
    draw_rounded_card(dc, 24, 148, 432, 126, 8, COLOR_CARD_BG, COLOR_CARD_BORDER);

    // Item 1: Bỏ dấu kiểu mới
    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_TEXT_PRIMARY);
    draw_text(dc, "Bỏ dấu kiểu mới", 40, 162);
    SelectObject(dc, font_small);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Đặt dấu ở âm thứ hai: hóa, hòa thay vì hoá, hoà", 40, 182);
    draw_switch(dc, 408, 168, config.modern_tone);

    // Divider
    draw_divider(dc, 40, 210, 400);

    // Item 2: Tự động khôi phục phím
    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_TEXT_PRIMARY);
    draw_text(dc, "Tự động khôi phục phím", 40, 222);
    SelectObject(dc, font_small);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Hoàn tác ký tự gốc khi gõ lặp dấu hoặc gõ từ sai", 40, 242);
    draw_switch(dc, 408, 228, config.auto_restore);

    // 5. Card 3: Phím tắt chuyển đổi
    draw_rounded_card(dc, 24, 286, 432, 60, 8, COLOR_CARD_BG, COLOR_CARD_BORDER);
    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_TEXT_PRIMARY);
    draw_text(dc, "Phím tắt chuyển đổi", 40, 297);
    SelectObject(dc, font_small);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Chuyển nhanh giữa Tiếng Việt (V) và Anh (E)", 40, 317);

    draw_segmented_tabs(
        dc,
        260,
        298,
        &[
            ("Ctrl + Shift", config.switch_key == SwitchKey::CtrlShift),
            ("Alt + Z", config.switch_key == SwitchKey::AltZ),
        ],
        font_bold,
    );

    // 6. Card 4: Trạng thái hệ thống
    draw_rounded_card(dc, 24, 358, 432, 60, 8, COLOR_CARD_BG, COLOR_CARD_BORDER);
    // Indicator dot
    let dot_brush = CreateSolidBrush(if config.enabled { COLOR_STATUS_GREEN } else { COLOR_TEXT_MUTED });
    let dot_pen = CreatePen(PS_SOLID as _, 1, if config.enabled { COLOR_STATUS_GREEN } else { COLOR_TEXT_MUTED });
    let old_brush = SelectObject(dc, dot_brush);
    let old_pen = SelectObject(dc, dot_pen);
    Ellipse(dc, 40, 381, 50, 391);
    SelectObject(dc, old_brush);
    SelectObject(dc, old_pen);
    DeleteObject(dot_brush);
    DeleteObject(dot_pen);

    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_TEXT_PRIMARY);
    let status_str = if config.enabled {
        "Trạng thái: Đang bật Tiếng Việt [V]"
    } else {
        "Trạng thái: Đang tắt gõ Tiếng Việt [E]"
    };
    draw_text(dc, status_str, 58, 372);

    SelectObject(dc, font_small);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Nhấn phím tắt hoặc click biểu tượng góc màn hình để đổi chế độ", 58, 392);

    // 7. Footer Buttons
    // Nút Thoát (Thoát HoyuKey)
    draw_rounded_card(dc, 24, 436, 126, 36, 6, COLOR_CARD_BG, COLOR_CARD_BORDER);
    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_BTN_DANGER);
    draw_centered_text(dc, "Thoát HoyuKey", 24, 436, 126, 36);

    // Nút Đóng (Ẩn về khay) - Shadcn Primary Button
    draw_rounded_card(dc, 306, 436, 150, 36, 6, COLOR_TAB_ACTIVE_BG, COLOR_TAB_ACTIVE_BG);
    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_TAB_ACTIVE_TEXT);
    draw_centered_text(dc, "Đóng (Ẩn về khay)", 306, 436, 150, 36);

    // Cleanup fonts
    DeleteObject(font_title);
    DeleteObject(font_subtitle);
    DeleteObject(font_bold);
    DeleteObject(font_body);
    DeleteObject(font_small);
    DeleteObject(font_badge);
}

unsafe fn draw_rounded_card(
    dc: windows_sys::Win32::Graphics::Gdi::HDC,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    radius: i32,
    bg_color: u32,
    border_color: u32,
) {
    let brush = CreateSolidBrush(bg_color);
    let pen = CreatePen(PS_SOLID as _, 1, border_color);
    let old_brush = SelectObject(dc, brush);
    let old_pen = SelectObject(dc, pen);

    RoundRect(dc, x, y, x + w, y + h, radius * 2, radius * 2);

    SelectObject(dc, old_brush);
    SelectObject(dc, old_pen);
    DeleteObject(brush);
    DeleteObject(pen);
}

unsafe fn draw_switch(
    dc: windows_sys::Win32::Graphics::Gdi::HDC,
    x: i32,
    y: i32,
    is_on: bool,
) {
    let w = 42;
    let h = 24;

    let bg_color = if is_on { COLOR_ACCENT_BLUE } else { COLOR_BORDER_SUBTLE };
    draw_rounded_card(dc, x, y, w, h, 12, bg_color, bg_color);

    // Tròn nút knob
    let knob_brush = CreateSolidBrush(0xFFFFFF);
    let knob_pen = CreatePen(PS_SOLID as _, 1, 0xFFFFFF);
    let old_brush = SelectObject(dc, knob_brush);
    let old_pen = SelectObject(dc, knob_pen);

    let knob_x = if is_on { x + 20 } else { x + 3 };
    let knob_y = y + 3;
    Ellipse(dc, knob_x, knob_y, knob_x + 18, knob_y + 18);

    SelectObject(dc, old_brush);
    SelectObject(dc, old_pen);
    DeleteObject(knob_brush);
    DeleteObject(knob_pen);
}

unsafe fn draw_segmented_tabs(
    dc: windows_sys::Win32::Graphics::Gdi::HDC,
    x: i32,
    y: i32,
    tabs: &[(&str, bool)],
    font: HFONT,
) {
    let total_w = tabs.len() as i32 * 88 + 8;
    let total_h = 36;

    // Nền segmented container
    draw_rounded_card(dc, x, y, total_w, total_h, 6, COLOR_TAB_INACTIVE_BG, COLOR_CARD_BORDER);

    SelectObject(dc, font);

    for (i, &(text, is_active)) in tabs.iter().enumerate() {
        let tab_x = x + 4 + (i as i32 * 88);
        let tab_y = y + 4;
        let tab_w = 84;
        let tab_h = 28;

        if is_active {
            draw_rounded_card(dc, tab_x, tab_y, tab_w, tab_h, 5, COLOR_TAB_ACTIVE_BG, COLOR_TAB_ACTIVE_BG);
            SetTextColor(dc, COLOR_TAB_ACTIVE_TEXT);
        } else {
            SetTextColor(dc, COLOR_TAB_INACTIVE_TEXT);
        }

        draw_centered_text(dc, text, tab_x, tab_y, tab_w, tab_h);
    }
}

unsafe fn draw_divider(
    dc: windows_sys::Win32::Graphics::Gdi::HDC,
    x1: i32,
    y: i32,
    w: i32,
) {
    let pen = CreatePen(PS_SOLID as _, 1, COLOR_CARD_BORDER);
    let old_pen = SelectObject(dc, pen);
    windows_sys::Win32::Graphics::Gdi::MoveToEx(dc, x1, y, null_mut());
    windows_sys::Win32::Graphics::Gdi::LineTo(dc, x1 + w, y);
    SelectObject(dc, old_pen);
    DeleteObject(pen);
}

unsafe fn draw_text(
    dc: windows_sys::Win32::Graphics::Gdi::HDC,
    text: &str,
    x: i32,
    y: i32,
) {
    let wide = to_wide(text);
    let mut rect = RECT {
        left: x,
        top: y,
        right: x + 400,
        bottom: y + 30,
    };
    DrawTextW(
        dc,
        wide.as_ptr(),
        wide.len() as i32 - 1,
        &mut rect,
        DT_LEFT | DT_SINGLELINE,
    );
}

unsafe fn draw_centered_text(
    dc: windows_sys::Win32::Graphics::Gdi::HDC,
    text: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    let wide = to_wide(text);
    let mut rect = RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    };
    DrawTextW(
        dc,
        wide.as_ptr(),
        wide.len() as i32 - 1,
        &mut rect,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
}

unsafe fn create_font(size: i32, weight: i32) -> HFONT {
    let face_name = to_wide("Segoe UI");
    CreateFontW(
        -size,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        1, // DEFAULT_CHARSET
        0,
        0,
        4, // CLEARTYPE_QUALITY
        0,
        face_name.as_ptr(),
    )
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
