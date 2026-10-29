use std::ptr::null_mut;
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use windows_sys::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    AlphaBlend, BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateDIBSection,
    CreateFontW, CreatePen, CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, Ellipse, EndPaint,
    InvalidateRect, RoundRect, ScreenToClient, SelectObject, SetBkColor, SetBkMode,
    SetTextColor, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
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

// Timer ID cho animation 60 FPS
const TIMER_ID_ANIM: usize = 1001;

// Tiến trình hoạt họa các công tắc và thanh trượt (0 đến 1000 <=> 0.0 đến 1.0)
static MODERN_TONE_PROGRESS: AtomicU32 = AtomicU32::new(1000);
static AUTO_RESTORE_PROGRESS: AtomicU32 = AtomicU32::new(1000);
static INPUT_METHOD_PROGRESS: AtomicU32 = AtomicU32::new(0); // 0 = Telex, 1000 = VNI
static SWITCH_KEY_PROGRESS: AtomicU32 = AtomicU32::new(0);    // 0 = Ctrl+Shift, 1000 = Alt+Z

// Nhúng logo 48x48 BGRA từ icon/logo.png
const LOGO_BGRA_48: &[u8] = include_bytes!("../icon/logo_48.bin");

// Bảng màu Shadcn UI Dark Mode (Zinc Palette chuẩn, tương phản cao, dịu mắt)
const COLOR_BG: u32 = 0x0C0909;             // #09090c
const COLOR_CARD_BG: u32 = 0x161313;        // #131316
const COLOR_CARD_BORDER: u32 = 0x2A2424;    // #24242a viền tinh tế 1px
const COLOR_TEXT_PRIMARY: u32 = 0xF8F6F6;   // #f6f6f8 (trắng sáng sắc nét)
const COLOR_TEXT_MUTED: u32 = 0x9D9494;     // #94949d (xám zinc chuẩn)
const COLOR_STATUS_GREEN: u32 = 0x5EC522;   // #22c55e
const COLOR_BTN_DANGER: u32 = 0x5555F4;     // #f45555

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

        // Kích thước chuẩn lưới thiết kế: 540 x 660
        let width = 540;
        let height = 660;
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
        INPUT_METHOD_PROGRESS.store(if state.config.input_method == InputMethod::Vni { 1000 } else { 0 }, Ordering::SeqCst);
        SWITCH_KEY_PROGRESS.store(if state.config.switch_key == SwitchKey::AltZ { 1000 } else { 0 }, Ordering::SeqCst);
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
                let target_method = if state.config.input_method == InputMethod::Vni { 1000.0 } else { 0.0 };
                let target_switch = if state.config.switch_key == SwitchKey::AltZ { 1000.0 } else { 0.0 };
                drop(state);

                let curr_tone = MODERN_TONE_PROGRESS.load(Ordering::SeqCst) as f32;
                let curr_restore = AUTO_RESTORE_PROGRESS.load(Ordering::SeqCst) as f32;
                let curr_method = INPUT_METHOD_PROGRESS.load(Ordering::SeqCst) as f32;
                let curr_switch = SWITCH_KEY_PROGRESS.load(Ordering::SeqCst) as f32;

                let next_tone = step_anim(curr_tone, target_tone);
                let next_restore = step_anim(curr_restore, target_restore);
                let next_method = step_anim(curr_method, target_method);
                let next_switch = step_anim(curr_switch, target_switch);

                MODERN_TONE_PROGRESS.store(next_tone.round() as u32, Ordering::SeqCst);
                AUTO_RESTORE_PROGRESS.store(next_restore.round() as u32, Ordering::SeqCst);
                INPUT_METHOD_PROGRESS.store(next_method.round() as u32, Ordering::SeqCst);
                SWITCH_KEY_PROGRESS.store(next_switch.round() as u32, Ordering::SeqCst);

                InvalidateRect(hwnd, null_mut(), 0);

                if next_tone == target_tone
                    && next_restore == target_restore
                    && next_method == target_method
                    && next_switch == target_switch
                {
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

fn step_anim(current: f32, target: f32) -> f32 {
    let diff = target - current;
    if diff.abs() > 5.0 {
        current + diff * 0.35
    } else {
        target
    }
}

fn is_clickable_area(x: i32, y: i32) -> bool {
    // Kiểu gõ Telex / VNI (căn chuẩn mép phải 492)
    (x >= 296 && x <= 492 && y >= 100 && y <= 146)
    // Switch Bỏ dấu kiểu mới
    || (x >= 440 && x <= 494 && y >= 198 && y <= 234)
    // Switch Tự động khôi phục phím
    || (x >= 440 && x <= 494 && y >= 270 && y <= 306)
    // Phím tắt Ctrl+Shift / Alt+Z (căn chuẩn mép phải 492)
    || (x >= 280 && x <= 492 && y >= 348 && y <= 394)
    // Nút Thoát
    || (x >= 28 && x <= 168 && y >= 530 && y <= 572)
    // Nút Đóng
    || (x >= 352 && x <= 512 && y >= 530 && y <= 572)
}

fn handle_mouse_click(hwnd: HWND, x: i32, y: i32) {
    let mut state = GLOBAL_STATE.lock().unwrap();
    let mut changed = false;

    // 1. Click Tab Kiểu gõ (Telex / VNI)
    if y >= 100 && y <= 146 {
        if x >= 296 && x <= 392 {
            if state.config.input_method != InputMethod::Telex {
                state.config.input_method = InputMethod::Telex;
                changed = true;
            }
        } else if x >= 396 && x <= 492 {
            if state.config.input_method != InputMethod::Vni {
                state.config.input_method = InputMethod::Vni;
                changed = true;
            }
        }
    }

    // 2. Click Switch Bỏ dấu kiểu mới
    if x >= 440 && x <= 494 && y >= 198 && y <= 234 {
        state.config.modern_tone = !state.config.modern_tone;
        changed = true;
    }

    // 3. Click Switch Tự động khôi phục phím
    if x >= 440 && x <= 494 && y >= 270 && y <= 306 {
        state.config.auto_restore = !state.config.auto_restore;
        changed = true;
    }

    // 4. Click Tab Phím tắt (Ctrl+Shift / Alt+Z)
    if y >= 348 && y <= 394 {
        if x >= 280 && x <= 384 {
            if state.config.switch_key != SwitchKey::CtrlShift {
                state.config.switch_key = SwitchKey::CtrlShift;
                changed = true;
            }
        } else if x >= 388 && x <= 492 {
            if state.config.switch_key != SwitchKey::AltZ {
                state.config.switch_key = SwitchKey::AltZ;
                changed = true;
            }
        }
    }

    // 5. Nút Thoát HoyuKey
    if x >= 28 && x <= 168 && y >= 530 && y <= 572 {
        drop(state);
        unsafe {
            PostQuitMessage(0);
        }
        return;
    }

    // 6. Nút Đóng (Ẩn về khay)
    if x >= 352 && x <= 512 && y >= 530 && y <= 572 {
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
    // 1. Nền tổng thể #09090c
    let bg_brush = CreateSolidBrush(COLOR_BG);
    let bg_rect = RECT { left: 0, top: 0, right: w, bottom: h };
    windows_sys::Win32::Graphics::Gdi::FillRect(dc, &bg_rect, bg_brush);
    DeleteObject(bg_brush);

    SetBkMode(dc, TRANSPARENT as _);
    SetBkColor(dc, COLOR_CARD_BG);

    // Typography chất lượng cao: Segoe UI với CLEARTYPE_NATURAL_QUALITY
    let font_title = create_font(22, 700);
    let font_bold = create_font(14, 600);
    let font_body = create_font(13, 400);
    let font_small = create_font(12, 400);
    let font_badge = create_font(11, 600);

    // 2. Header
    draw_bgra_image(dc, 28, 22, 48, 48, LOGO_BGRA_48);

    SelectObject(dc, font_title);
    SetTextColor(dc, COLOR_TEXT_PRIMARY);
    draw_text(dc, "HoyuKey", 88, 22);

    draw_rounded_card(dc, 192, 27, 88, 20, 10, COLOR_CARD_BG, COLOR_CARD_BORDER);
    SelectObject(dc, font_badge);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_centered_text(dc, "Pre-release", 192, 27, 88, 20);

    SelectObject(dc, font_body);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Bộ gõ Tiếng Việt thế hệ mới cho PC", 88, 49);

    let card_x = 28;
    let card_w = 484; // Căn thẳng hàng hoàn hảo mép trái 28, mép phải 512

    // 3. Card 1: Kiểu gõ chính (Telex / VNI)
    draw_rounded_card(dc, card_x, 86, card_w, 76, 10, COLOR_CARD_BG, COLOR_CARD_BORDER);
    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_TEXT_PRIMARY);
    draw_text(dc, "Kiểu gõ chính", 48, 104);
    SelectObject(dc, font_small);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Chọn phương thức gõ Telex hoặc VNI", 48, 126);

    let method_progress = INPUT_METHOD_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
    draw_animated_tabs(
        dc,
        296, // Căn chuẩn mép phải = 296 + 196 = 492
        102,
        &["Telex", "VNI"],
        94,
        method_progress,
        font_bold,
    );

    // 4. Card 2: Tùy chọn gõ dấu
    draw_rounded_card(dc, card_x, 174, card_w, 148, 10, COLOR_CARD_BG, COLOR_CARD_BORDER);

    // Row 1: Bỏ dấu kiểu mới
    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_TEXT_PRIMARY);
    draw_text(dc, "Bỏ dấu kiểu mới", 48, 194);
    SelectObject(dc, font_small);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Đặt dấu ở âm thứ hai: hóa, hòa thay vì hoá, hoà", 48, 216);

    let tone_progress = MODERN_TONE_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
    draw_animated_switch(dc, 444, 202, tone_progress); // Căn chuẩn mép phải 444 + 48 = 492

    // Đường ngăn cách căn chuẩn mép trái 48, mép phải 492
    draw_divider(dc, 48, 248, 444);

    // Row 2: Tự động khôi phục phím
    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_TEXT_PRIMARY);
    draw_text(dc, "Tự động khôi phục phím", 48, 266);
    SelectObject(dc, font_small);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Khôi phục ký tự gốc khi gõ lặp dấu hoặc từ sai", 48, 288);

    let restore_progress = AUTO_RESTORE_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
    draw_animated_switch(dc, 444, 274, restore_progress); // Căn chuẩn mép phải 444 + 48 = 492

    // 5. Card 3: Phím tắt chuyển đổi
    draw_rounded_card(dc, card_x, 334, card_w, 76, 10, COLOR_CARD_BG, COLOR_CARD_BORDER);
    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_TEXT_PRIMARY);
    draw_text(dc, "Phím tắt chuyển đổi", 48, 352);
    SelectObject(dc, font_small);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Đổi nhanh giữa Tiếng Việt (V) và Anh (E)", 48, 374);

    let switch_progress = SWITCH_KEY_PROGRESS.load(Ordering::SeqCst) as f32 / 1000.0;
    draw_animated_tabs(
        dc,
        280, // Căn chuẩn mép phải = 280 + 212 = 492
        350,
        &["Ctrl + Shift", "Alt + Z"],
        102,
        switch_progress,
        font_bold,
    );

    // 6. Card 4: Trạng thái hệ thống
    draw_rounded_card(dc, card_x, 422, card_w, 72, 10, COLOR_CARD_BG, COLOR_CARD_BORDER);
    let dot_brush = CreateSolidBrush(if config.enabled { COLOR_STATUS_GREEN } else { COLOR_TEXT_MUTED });
    let dot_pen = CreatePen(PS_SOLID as _, 1, if config.enabled { COLOR_STATUS_GREEN } else { COLOR_TEXT_MUTED });
    let old_brush = SelectObject(dc, dot_brush);
    let old_pen = SelectObject(dc, dot_pen);
    Ellipse(dc, 48, 453, 58, 463);
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
    draw_text(dc, status_str, 68, 438);

    SelectObject(dc, font_small);
    SetTextColor(dc, COLOR_TEXT_MUTED);
    draw_text(dc, "Nhấn phím tắt hoặc click khay hệ thống để đổi chế độ", 68, 462);

    // 7. Footer Buttons: Căn thẳng hàng tuyệt đối với hai cạnh của Cards
    // Cạnh trái: 28, Cạnh phải: 512
    draw_rounded_card(dc, 28, 530, 140, 42, 8, COLOR_CARD_BG, COLOR_CARD_BORDER);
    SelectObject(dc, font_bold);
    SetTextColor(dc, COLOR_BTN_DANGER);
    draw_centered_text(dc, "Thoát HoyuKey", 28, 530, 140, 42);

    draw_rounded_card(dc, 352, 530, 160, 42, 8, 0xFAFAFA, 0xFAFAFA);
    SelectObject(dc, font_bold);
    SetTextColor(dc, 0x0C0909);
    draw_centered_text(dc, "Đóng (Ẩn về khay)", 352, 530, 160, 42);

    DeleteObject(font_title);
    DeleteObject(font_bold);
    DeleteObject(font_body);
    DeleteObject(font_small);
    DeleteObject(font_badge);
}

unsafe fn draw_animated_switch(
    dc: windows_sys::Win32::Graphics::Gdi::HDC,
    x: i32,
    y: i32,
    progress: f32,
) {
    let w = 48;
    let h = 26;
    let radius = 13;

    // Rãnh: OFF là Đen #18181b, ON là Trắng tinh #ffffff
    let r_track = (24.0 + (255.0 - 24.0) * progress).round() as u8;
    let g_track = (24.0 + (255.0 - 24.0) * progress).round() as u8;
    let b_track = (27.0 + (255.0 - 27.0) * progress).round() as u8;
    let color_track = (b_track as u32) << 16 | (g_track as u32) << 8 | (r_track as u32);

    let r_border = (63.0 + (255.0 - 63.0) * progress).round() as u8;
    let g_border = (63.0 + (255.0 - 63.0) * progress).round() as u8;
    let b_border = (70.0 + (255.0 - 70.0) * progress).round() as u8;
    let color_border = (b_border as u32) << 16 | (g_border as u32) << 8 | (r_border as u32);

    draw_rounded_card(dc, x, y, w, h, radius, color_track, color_border);

    // Chấm tròn: OFF là Trắng #fafafa, ON là Đen #09090b
    let knob_diameter = 20;
    let knob_min_x = x + 3;
    let knob_max_x = x + w - knob_diameter - 3;
    let knob_curr_x = (knob_min_x as f32 + (knob_max_x - knob_min_x) as f32 * progress).round() as i32;
    let knob_y = y + 3;

    let r_knob = (250.0 + (9.0 - 250.0) * progress).round() as u8;
    let g_knob = (250.0 + (9.0 - 250.0) * progress).round() as u8;
    let b_knob = (250.0 + (12.0 - 250.0) * progress).round() as u8;
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

unsafe fn draw_animated_tabs(
    dc: windows_sys::Win32::Graphics::Gdi::HDC,
    x: i32,
    y: i32,
    tabs: &[&str; 2],
    tab_w: i32,
    progress: f32,
    font: HFONT,
) {
    let tab_h = 36;
    let total_w = tab_w * 2 + 8;
    let total_h = tab_h + 8;

    draw_rounded_card(dc, x, y, total_w, total_h, 8, 0x1B1818, COLOR_CARD_BORDER);

    let pill_min_x = x + 4;
    let pill_max_x = x + 4 + tab_w;
    let pill_curr_x = (pill_min_x as f32 + (pill_max_x - pill_min_x) as f32 * progress).round() as i32;
    let pill_y = y + 4;

    draw_rounded_card(dc, pill_curr_x, pill_y, tab_w, tab_h, 6, 0xFAFAFA, 0xFAFAFA);

    SelectObject(dc, font);

    let t1_is_selected = progress < 0.5;
    SetTextColor(dc, if t1_is_selected { 0x0C0909 } else { 0xA1A1AA });
    draw_centered_text(dc, tabs[0], x + 4, y + 4, tab_w, tab_h);

    let t2_is_selected = progress >= 0.5;
    SetTextColor(dc, if t2_is_selected { 0x0C0909 } else { 0xA1A1AA });
    draw_centered_text(dc, tabs[1], x + 4 + tab_w, y + 4, tab_w, tab_h);
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
    bmi.bmiHeader.biHeight = -h;
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
        1,
        0,
        0,
        5, // CLEARTYPE_NATURAL_QUALITY
        0,
        face_name.as_ptr(),
    )
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
