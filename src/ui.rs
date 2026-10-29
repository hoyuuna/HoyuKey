use std::ptr::null_mut;
use std::sync::atomic::{AtomicIsize, Ordering};
use windows_sys::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{GetStockObject, DEFAULT_GUI_FONT, HGDIOBJ};
use windows_sys::Win32::UI::Controls::{BST_CHECKED, BST_UNCHECKED};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetSystemMetrics, PostQuitMessage, RegisterClassW,
    SendMessageW, SetForegroundWindow, ShowWindow, BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX,
    BS_AUTORADIOBUTTON, BS_GROUPBOX, BS_PUSHBUTTON, CS_HREDRAW, CS_VREDRAW, SM_CXSCREEN,
    SM_CYSCREEN, SW_HIDE, SW_SHOW, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_SETFONT,
    WNDCLASSW, WS_CAPTION, WS_CHILD, WS_EX_DLGMODALFRAME, WS_MINIMIZEBOX, WS_OVERLAPPED,
    WS_SYSMENU, WS_VISIBLE,
};

use crate::config::{InputMethod, SwitchKey};
use crate::GLOBAL_STATE;

pub const IDC_RADIO_TELEX: isize = 2001;
pub const IDC_RADIO_VNI: isize = 2002;
pub const IDC_CHK_MODERN_TONE: isize = 2003;
pub const IDC_CHK_AUTO_RESTORE: isize = 2004;
pub const IDC_RADIO_CTRL_SHIFT: isize = 2005;
pub const IDC_RADIO_ALT_Z: isize = 2006;
pub const IDC_BTN_CLOSE: isize = 2007;
pub const IDC_BTN_EXIT: isize = 2008;

pub static SETTINGS_HWND: AtomicIsize = AtomicIsize::new(0);

pub fn create_settings_window(hinstance: HMODULE) -> HWND {
    let class_name = to_wide("HoyuKey_SettingsClass");
    let window_title = to_wide("HoyuKey - Bảng điều khiển");

    unsafe {
        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(settings_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: null_mut(),
            hCursor: null_mut(),
            hbrBackground: 16 as _, // COLOR_BTNFACE + 1
            lpszMenuName: null_mut(),
            lpszClassName: class_name.as_ptr(),
        };

        RegisterClassW(&wnd_class);

        let width = 450;
        let height = 340;
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
        sync_controls_with_config(hwnd);
        unsafe {
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
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
        WM_CREATE => {
            init_controls(hwnd);
            0
        }
        WM_COMMAND => {
            let ctrl_id = (wparam & 0xFFFF) as isize;
            handle_control_click(hwnd, ctrl_id);
            0
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

unsafe fn init_controls(hwnd: HWND) {
    let font = GetStockObject(DEFAULT_GUI_FONT);

    // Group 1: Kiểu gõ & Dấu tiếng Việt
    create_control(hwnd, "BUTTON", "Kiểu gõ & Dấu", BS_GROUPBOX as u32, 15, 10, 405, 130, 0, font);
    create_control(hwnd, "BUTTON", "Telex", BS_AUTORADIOBUTTON as u32, 30, 35, 100, 24, IDC_RADIO_TELEX, font);
    create_control(hwnd, "BUTTON", "VNI", BS_AUTORADIOBUTTON as u32, 150, 35, 100, 24, IDC_RADIO_VNI, font);
    create_control(hwnd, "BUTTON", "Bỏ dấu kiểu mới (hòa, hóa thay vì hoá, hoà)", BS_AUTOCHECKBOX as u32, 30, 68, 350, 24, IDC_CHK_MODERN_TONE, font);
    create_control(hwnd, "BUTTON", "Tự động khôi phục phím với từ không hợp lệ", BS_AUTOCHECKBOX as u32, 30, 98, 350, 24, IDC_CHK_AUTO_RESTORE, font);

    // Group 2: Phím chuyển đổi
    create_control(hwnd, "BUTTON", "Phím chuyển đổi [Việt / Anh]", BS_GROUPBOX as u32, 15, 150, 405, 75, 0, font);
    create_control(hwnd, "BUTTON", "Ctrl + Shift", BS_AUTORADIOBUTTON as u32, 30, 180, 120, 24, IDC_RADIO_CTRL_SHIFT, font);
    create_control(hwnd, "BUTTON", "Alt + Z", BS_AUTORADIOBUTTON as u32, 180, 180, 100, 24, IDC_RADIO_ALT_Z, font);

    // Buttons
    create_control(hwnd, "BUTTON", "Đóng (Ẩn về khay)", BS_PUSHBUTTON as u32, 180, 245, 130, 32, IDC_BTN_CLOSE, font);
    create_control(hwnd, "BUTTON", "Kết thúc", BS_PUSHBUTTON as u32, 325, 245, 95, 32, IDC_BTN_EXIT, font);

    sync_controls_with_config(hwnd);
}

fn sync_controls_with_config(hwnd: HWND) {
    let state = GLOBAL_STATE.lock().unwrap();
    let config = &state.config;

    unsafe {
        check_button(hwnd, IDC_RADIO_TELEX, config.input_method == InputMethod::Telex);
        check_button(hwnd, IDC_RADIO_VNI, config.input_method == InputMethod::Vni);
        check_button(hwnd, IDC_CHK_MODERN_TONE, config.modern_tone);
        check_button(hwnd, IDC_CHK_AUTO_RESTORE, config.auto_restore);
        check_button(hwnd, IDC_RADIO_CTRL_SHIFT, config.switch_key == SwitchKey::CtrlShift);
        check_button(hwnd, IDC_RADIO_ALT_Z, config.switch_key == SwitchKey::AltZ);
    }
}

fn handle_control_click(hwnd: HWND, ctrl_id: isize) {
    let mut state = GLOBAL_STATE.lock().unwrap();

    match ctrl_id {
        IDC_RADIO_TELEX => {
            state.config.input_method = InputMethod::Telex;
        }
        IDC_RADIO_VNI => {
            state.config.input_method = InputMethod::Vni;
        }
        IDC_CHK_MODERN_TONE => {
            let checked = unsafe { is_checked(hwnd, IDC_CHK_MODERN_TONE) };
            state.config.modern_tone = checked;
        }
        IDC_CHK_AUTO_RESTORE => {
            let checked = unsafe { is_checked(hwnd, IDC_CHK_AUTO_RESTORE) };
            state.config.auto_restore = checked;
        }
        IDC_RADIO_CTRL_SHIFT => {
            state.config.switch_key = SwitchKey::CtrlShift;
        }
        IDC_RADIO_ALT_Z => {
            state.config.switch_key = SwitchKey::AltZ;
        }
        IDC_BTN_CLOSE => {
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
        IDC_BTN_EXIT => {
            unsafe {
                PostQuitMessage(0);
            }
        }
        _ => {}
    }
}

unsafe fn create_control(
    parent: HWND,
    class_name: &str,
    text: &str,
    style: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: isize,
    font: HGDIOBJ,
) -> HWND {
    let class_wide = to_wide(class_name);
    let text_wide = to_wide(text);

    let ctrl = CreateWindowExW(
        0,
        class_wide.as_ptr(),
        text_wide.as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        x,
        y,
        w,
        h,
        parent,
        id as _,
        0 as _,
        null_mut(),
    );

    if font != null_mut() {
        SendMessageW(ctrl, WM_SETFONT, font as _, 1);
    }
    ctrl
}

unsafe fn check_button(parent: HWND, id: isize, checked: bool) {
    let ctrl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(parent, id as i32);
    if ctrl != null_mut() {
        let state = if checked { BST_CHECKED } else { BST_UNCHECKED };
        SendMessageW(ctrl, BM_SETCHECK, state as _, 0);
    }
}

unsafe fn is_checked(parent: HWND, id: isize) -> bool {
    let ctrl = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(parent, id as i32);
    if ctrl != null_mut() {
        SendMessageW(ctrl, BM_GETCHECK, 0, 0) == BST_CHECKED as isize
    } else {
        false
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
