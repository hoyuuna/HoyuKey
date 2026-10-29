use std::ptr::null_mut;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use windows_sys::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    AlphaBlend, BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateDIBSection,
    CreateFontW, CreatePen, CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, Ellipse, EndPaint,
    InvalidateRect, RoundRect, ScreenToClient, SelectObject, SetBkMode, SetTextColor,
    AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
    DIB_RGB_COLORS, DT_CENTER, DT_LEFT, DT_SINGLELINE, DT_VCENTER, HFONT, PAINTSTRUCT, PS_SOLID,
    SRCCOPY, TRANSPARENT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, GetCursorPos, GetSystemMetrics, KillTimer,
    LoadCursorW, PostMessageW, PostQuitMessage, RegisterClassW, SetCursor, SetForegroundWindow,
    SetTimer, ShowWindow, CS_HREDRAW, CS_VREDRAW, IDC_ARROW, IDC_HAND, SM_CXSCREEN, SM_CYSCREEN,
    SW_HIDE, SW_RESTORE, SW_SHOW, WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_LBUTTONDOWN, WM_PAINT,
    WM_SETCURSOR, WM_TIMER, WNDCLASSW, WS_CAPTION, WS_EX_DLGMODALFRAME, WS_MINIMIZEBOX,
    WS_OVERLAPPED, WS_SYSMENU,
};

use crate::config::{Config, InputMethod, SwitchKey};
use crate::tray::WM_APP_UPDATE_TRAY;
use crate::GLOBAL_STATE;

pub static SETTINGS_HWND: AtomicIsize = AtomicIsize::new(0);

// Timer ID cho animation công tắc gạt
const TIMER_ID_ANIM: usize = 1001;

// Tiến trình hoạt họa công tắc (0 đến 1000 tương ứng 0.0 đến 1.0)
static MODERN_TONE_PROGRESS: AtomicU32 = AtomicU32::new(1000);
static AUTO_RESTORE_PROGRESS: AtomicU32 = AtomicU32::new(1000);

// Nhúng logo 48x48 BGRA từ icon/logo.png do người dùng cung cấp
const LOGO_BGRA_48: &[u8] = include_bytes!("../icon/logo_48.bin");

// Bảng màu Shadcn UI Dark Mode (Zinc Palette)
const COLOR_BG: u32 = 0x0B0909;             // #09090b
const COLOR_CARD_BG: u32 = 0x161414;        // #141416
const COLOR_CARD_BORDER: u32 = 0x2A2727;    // #27272a
const COLOR_TEXT_PRIMARY: u32 = 0xF5F4F4;   // #f4f4f5
const COLOR_TEXT_MUTED: u32 = 0x9B9191;     // #91919b
const COLOR_TEXT_SUBTLE: u32 = 0x736B6B;    // #6b6b73
const COLOR_STATUS_GREEN: u32 = 0x5EC522;   // #22c55e
const COLOR_BTN_DANGER: u32 = 0x4444EF;     // #ef4444

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

        // Kích thước rộng rãi, thoáng đãng: 536 x 650
        let width = 536;
        let height = 650;
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
        let state = GLOBAL_STATE.lock().unwrap();
        MODERN_TONE_PROGRESS.store(if state.config.modern_tone { 1000 } else { 0 }, Ordering::SeqCst);
        AUTO_RESTORE_PROGRESS.store(if state.config.auto_restore { 1000 } else { 0 }, Ordering::SeqCst);
        drop(state);

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
        WM_ERASEBKGND => 1,
        WM_TIMER => {
            if wparam == TIMER_ID_ANIM {
                let state = GLOBAL_STATE.lock().unwrap();
                let target_tone = if state.config.modern_tone { 1000.0 } else { 0.0 };
                let target_restore = if state.config.auto_restore { 1000.0 } else { 0.0 };
                drop(state);

                let curr_tone = MODERN_TONE_PROGRESS.load(Ordering::SeqCst) as f32;
                let curr_restore = AUTO_RESTORE_PROGRESS.load(Ordering::SeqCst) as f32;

                let diff_tone = target_tone - curr_tone;
                let diff_restore = target_restore - curr_restore;

                let next_tone = if diff_tone.abs() > 10.0 { curr_tone + diff_tone * 0.35 } else { target_tone };
                let next_restore = if diff_restore.abs() > 10.0 { curr_restore + diff_restore * 0.35 } else { target_restore };

                MODERN_TONE_PROGRESS.store(next_tone.round() as u32, Ordering::SeqCst);
                AUTO_RESTORE_PROGRESS.store(next_restore.round() as u32, Ordering::SeqCst);

                InvalidateRect(hwnd, null_mut(), 0);

                if next_tone == target_tone && next_restore == target_restore {
                    KillTimer(hwnd, TIMER_ID_ANIM);
                }
            }
            0
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            let w = rect.right - rect.left;
            let h = rect.bottom - rect.top;

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
            KillTimer(hwnd, TIMER_ID_ANIM);
            SETTINGS_HWND.store(0, Ordering::SeqCst);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn is_clickable_area(x: i32, y: i32) -> bool {
    // Kiểu gõ Telex / VNI
    (x >= 300 && x <= 486 && y >= 96 && y <= 144)
    // Switch Bỏ dấu kiểu mới
    || (x >= 420 && x <= 480 && y >= 195 && y <= 235)
    // Switch Tự động khôi phục phím
    || (x >= 420 && x <= 480 && y >= 268 && y <= 308)
    // Phím tắt Ctrl+Shift / Alt+Z
    || (x >= 270 && x <= 486 && y >= 356 && y <= 404)
    // Nút Thoát
    || (x >= 28 && x <= 168 && y >= 535 && y <= 578)
    // Nút Đóng
    || (x >= 332 && x <= 492 && y >= 535 && y <= 578)
}

fn handle_mouse_click(hwnd: HWND, x: i32, y: i32) {
    let mut state = GLOBAL_STATE.lock().unwrap();
    let mut changed = false;
    let mut trigger_anim = false;

    // 1. Click Tab Kiểu gõ (Telex / VNI)
    if y >= 96 && y <= 144 {
        if x >= 300 && x <= 390 {
            state.config.input_method = InputMethod::Telex;
            changed = true;
        } else if x >= 394 && x <= 486 {
            state.config.input_method = InputMethod::Vni;
            changed = true;
        }
    }

    // 2. Click Switch Bỏ dấu kiểu mới
    if x >= 420 && x <= 480 && y >= 195 && y <= 235 {
        state.config.modern_tone = !state.config.modern_tone;
        changed = true;
        trigger_anim = true;
    }

    // 3. Click Switch Tự động khôi phục phím
    if x >= 420 && x <= 480 && y >= 268 && y <= 308 {
        state.config.auto_restore = !state.config.auto_restore;
        changed = true;
        trigger_anim = true;
    }

    // 4. Click Tab Phím tắt (Ctrl+Shift / Alt+Z)
    if y >= 356 && y <= 404 {
        if x >= 270 && x <= 374 {
            state.config.switch_key = SwitchKey::CtrlShift;
            changed = true;
        } else if x >= 378 && x <= 486 {
            state.config.switch_key = SwitchKey::AltZ;
            changed = true;
        }
    }

    // 5. Nút Thoát HoyuKey
    if x >= 28 && x <= 168 && y >= 535 && y <= 578 {
        drop(state);
        unsafe {
            PostQuitMessage(0);
        }
        return;
    }

    // 6. Nút Đóng (Ẩn về khay)
    if x >= 332 && x <= 492 && y >= 535 && y <= 578 {
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
            if trigger_anim {
                SetTimer(hwnd, TIMER_ID_ANIM, 16, None);
            }
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

    // Chuẩn hóa typography với Segoe UI chất lượng cao
    let font_title = create_font(21, 700);
    let font_bold = create_font(14, 600);
    let font_body = create_font(13, 400);
    let font_small = create_font(12, 400);
    let font_badge = create_font(11, 600);

    // 2. Header
    // Vẽ logo.png 48x48 của người dùng bằng AlphaBlend
    draw_bgra_image(dc, 28, 22, 48, 48, LOGO_BGRA_48);

    // Tiêu đề
    SelectObject(dc, font_title);
    SetTextColor(dc, COLOR_TEXT_PRIMARY);
    draw_text(dc, "HoyuKey", 88, 22);

    // Badge "Pre-release"
    draw_rounded_card(dc, 186, 26, 86, 20, 10, COLOR_CARD_BG, COLOR_CARD_BORDER);
    SelectObject(dc, font_badge);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_centered_text(dc, "Pre-release", 186, 26, 86, 20);

    // Subtitle
    SelectObject(dc, font_body);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Bộ gõ Tiếng Việt thế hệ mới cho PC", 88, 48);

    let card_w = 464;

    // 3. Card 1: Kiểu gõ chính (Telex / VNI)
    draw_rounded_card(dc, 28, 86, card_w, 76, 10, COLOR_CARD_BG, COLOR_CARD_BORDER);
    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_TEXT_PRIMARY);
    draw_text(dc, "Kiểu gõ chính", 48, 102);
    SelectObject(dc, font_small);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Chọn phương thức gõ Telex hoặc VNI", 48, 126);

    // Segmented Tabs
    draw_segmented_tabs(
        dc,
        300,
        100,
        &[("Telex", config.input_method == InputMethod::Telex), ("VNI", config.input_method == InputMethod::Vni)],
        font_bold,
    );

    // 4. Card 2: Tùy chọn gõ dấu
    draw_rounded_card(dc, 28, 174, card_w, 148, 10, COLOR_CARD_BG, COLOR_CARD_BORDER);

    // Row 1: Bỏ dấu kiểu mới
    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_TEXT_PRIMARY);
    draw_text(dc, "Bỏ dấu kiểu mới", 48, 192);
    SelectObject(dc, font_small);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Đặt dấu ở âm thứ hai: hóa, hòa thay vì hoá, hoà", 48, 214);

    let tone_progress = MODERN_TONE_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
    draw_animated_switch(dc, 424, 200, tone_progress);

    // Đường ngăn cách tinh tế
    draw_divider(dc, 48, 248, card_w - 40);

    // Row 2: Tự động khôi phục phím
    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_TEXT_PRIMARY);
    draw_text(dc, "Tự động khôi phục phím", 48, 264);
    SelectObject(dc, font_small);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Khôi phục ký tự gốc khi gõ lặp dấu hoặc từ sai", 48, 286);

    let restore_progress = AUTO_RESTORE_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
    draw_animated_switch(dc, 424, 272, restore_progress);

    // 5. Card 3: Phím tắt chuyển đổi
    draw_rounded_card(dc, 28, 334, card_w, 76, 10, COLOR_CARD_BG, COLOR_CARD_BORDER);
    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_TEXT_PRIMARY);
    draw_text(dc, "Phím tắt chuyển đổi", 48, 350);
    SelectObject(dc, font_small);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Đổi nhanh giữa Tiếng Việt (V) và Anh (E)", 48, 374);

    draw_segmented_tabs(
        dc,
        276,
        348,
        &[
            ("Ctrl + Shift", config.switch_key == SwitchKey::CtrlShift),
            ("Alt + Z", config.switch_key == SwitchKey::AltZ),
        ],
        font_bold,
    );

    // 6. Card 4: Trạng thái hệ thống
    draw_rounded_card(dc, 28, 422, card_w, 72, 10, COLOR_CARD_BG, COLOR_CARD_BORDER);
    // Chấm tròn trạng thái
    let dot_brush = CreateSolidBrush(if config.enabled { COLOR_STATUS_GREEN } else { COLOR_TEXT_SUBTLE });
    let dot_pen = CreatePen(PS_SOLID as _, 1, if config.enabled { COLOR_STATUS_GREEN } else { COLOR_TEXT_SUBTLE });
    let old_brush = SelectObject(dc, dot_brush);
    let old_pen = SelectObject(dc, dot_pen);
    Ellipse(dc, 48, 452, 60, 464);
    SelectObject(dc, old_brush);
    SelectObject(dc, old_pen);
    DeleteObject(dot_brush);
    DeleteObject(dot_pen);

    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_TEXT_PRIMARY);
    let status_str = if config.enabled {
        "Trạng thái: Đang bật Tiếng Việt [V]"
    } else {
        "Trạng thái: Đang tắt Tiếng Việt [E]"
    };
    draw_text(dc, status_str, 70, 438);

    SelectObject(dc, font_small);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Nhấn phím tắt hoặc click khay hệ thống để đổi chế độ", 70, 462);

    // 7. Footer Buttons
    // Nút Thoát HoyuKey (nền tối, viền subtle, chữ đỏ cảnh báo)
    draw_rounded_card(dc, 28, 526, 140, 42, 8, COLOR_CARD_BG, COLOR_CARD_BORDER);
    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_BTN_DANGER);
    draw_centered_text(dc, "Thoát HoyuKey", 28, 526, 140, 42);

    // Nút Đóng (Ẩn về khay) - Chuẩn nút chính Shadcn: Nền trắng tinh khôi, chữ đen đậm
    draw_rounded_card(dc, 332, 526, 160, 42, 8, 0xFAFAFA, 0xFAFAFA);
    SelectObject(dc, font_bold);
    SetTextColor(dc, 0x09090B);
    draw_centered_text(dc, "Đóng (Ẩn về khay)", 332, 526, 160, 42);

    // Dọn dẹp fonts
    DeleteObject(font_title);
    DeleteObject(font_bold);
    DeleteObject(font_body);
    DeleteObject(font_small);
    DeleteObject(font_badge);
}

/// Vẽ công tắc gạt với 2 màu ĐEN & TRẮNG tương phản cực đỉnh cùng hoạt họa chuyển động mượt mà
unsafe fn draw_animated_switch(
    dc: windows_sys::Win32::Graphics::Gdi::HDC,
    x: i32,
    y: i32,
    progress: f32, // 0.0 (OFF) -> 1.0 (ON)
) {
    let w = 48;
    let h = 26;
    let radius = 13;

    // 1. Màu nền của rãnh (Track):
    // OFF (0.0): Màu đen/charcoal #18181b (24, 24, 27)
    // ON (1.0): Màu trắng tinh khôi #ffffff (255, 255, 255)
    let r_track = (24.0 + (255.0 - 24.0) * progress).round() as u8;
    let g_track = (24.0 + (255.0 - 24.0) * progress).round() as u8;
    let b_track = (27.0 + (255.0 - 27.0) * progress).round() as u8;
    let color_track = (b_track as u32) << 16 | (g_track as u32) << 8 | (r_track as u32);

    // Viền track: OFF có viền mỏng #3f3f46, ON viền hòa vào màu trắng
    let r_border = (63.0 + (255.0 - 63.0) * progress).round() as u8;
    let g_border = (63.0 + (255.0 - 63.0) * progress).round() as u8;
    let b_border = (70.0 + (255.0 - 70.0) * progress).round() as u8;
    let color_border = (b_border as u32) << 16 | (g_border as u32) << 8 | (r_border as u32);

    draw_rounded_card(dc, x, y, w, h, radius, color_track, color_border);

    // 2. Chấm tròn (Thumb / Knob):
    // OFF (0.0): Chấm màu TRẮNG #fafafa (250, 250, 250) nằm bên trái
    // ON (1.0): Chấm màu ĐEN #09090b (9, 9, 11) nằm bên phải
    let knob_diameter = 20;
    let knob_min_x = x + 3;
    let knob_max_x = x + w - knob_diameter - 3;
    let knob_curr_x = (knob_min_x as f32 + (knob_max_x - knob_min_x) as f32 * progress).round() as i32;
    let knob_y = y + 3;

    let r_knob = (250.0 + (9.0 - 250.0) * progress).round() as u8;
    let g_knob = (250.0 + (9.0 - 250.0) * progress).round() as u8;
    let b_knob = (250.0 + (11.0 - 250.0) * progress).round() as u8;
    let color_knob = (b_knob as u32) << 16 | (g_knob as u32) << 8 | (r_knob as u32);

    let knob_brush = CreateSolidBrush(color_knob);
    let knob_pen = CreatePen(PS_SOLID as _, 1, color_knob);
    let old_brush = SelectObject(dc, knob_brush);
    let old_pen = SelectObject(dc, knob_pen);

    Ellipse(dc, knob_curr_x, knob_y, knob_curr_x + knob_diameter, knob_y + knob_diameter);

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
    let tab_w = 88;
    let tab_h = 36;
    let total_w = tabs.len() as i32 * tab_w + 8;
    let total_h = tab_h + 8;

    // Nền container
    draw_rounded_card(dc, x, y, total_w, total_h, 8, 0x1B1818, COLOR_CARD_BORDER);

    SelectObject(dc, font);

    for (i, &(text, is_active)) in tabs.iter().enumerate() {
        let tx = x + 4 + (i as i32 * tab_w);
        let ty = y + 4;

        if is_active {
            draw_rounded_card(dc, tx, ty, tab_w, tab_h, 6, 0xFAFAFA, 0xFAFAFA);
            SetTextColor(dc, 0x09090B);
        } else {
            SetTextColor(dc, 0xA1A1AA);
        }

        draw_centered_text(dc, text, tx, ty, tab_w, tab_h);
    }
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

/// Vẽ ảnh 32-bit BGRA có hỗ trợ trong suốt alpha bằng AlphaBlend
unsafe fn draw_bgra_image(
    hdc_dest: windows_sys::Win32::Graphics::Gdi::HDC,
    dest_x: i32,
    dest_y: i32,
    w: i32,
    h: i32,
    bgra_bytes: &[u8],
) {
    let mut bmi: BITMAPINFO = std::mem::zeroed();
    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = w;
    bmi.bmiHeader.biHeight = -h; // Top-down
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;

    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let mem_dc = CreateCompatibleDC(hdc_dest);
    let hbitmap = CreateDIBSection(
        hdc_dest,
        &bmi,
        DIB_RGB_COLORS,
        &mut bits,
        std::ptr::null_mut(),
        0,
    );

    if hbitmap != std::ptr::null_mut() && !bits.is_null() {
        std::ptr::copy_nonoverlapping(bgra_bytes.as_ptr(), bits as *mut u8, bgra_bytes.len());
        let old_bm = SelectObject(mem_dc, hbitmap);

        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };

        AlphaBlend(
            hdc_dest, dest_x, dest_y, w, h,
            mem_dc, 0, 0, w, h,
            blend,
        );

        SelectObject(mem_dc, old_bm);
        DeleteObject(hbitmap);
    }
    DeleteDC(mem_dc);
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
