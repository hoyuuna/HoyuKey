#![windows_subsystem = "windows"]

mod config;
mod engine;
mod hook;
mod tray;
mod ui;

use std::ptr::null_mut;
use std::sync::{LazyLock, Mutex};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, FindWindowW, GetMessageW, MessageBoxW,
    PostMessageW, PostQuitMessage, RegisterClassW, TranslateMessage, MB_OK, MSG, WM_COMMAND,
    WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW,
};

use crate::config::{Config, InputMethod};
use crate::engine::Engine;
use crate::tray::{
    TrayManager, IDM_AUTO_RESTORE, IDM_CONTROL_PANEL, IDM_EXIT, IDM_MODERN_TONE, IDM_TELEX,
    IDM_TOGGLE_MODE, IDM_VNI, WM_APP_SHOW_SETTINGS, WM_APP_TRAY, WM_APP_UPDATE_TRAY,
};
use crate::ui::{create_settings_window, show_settings_window};

pub struct AppState {
    pub config: Config,
    pub engine: Engine,
    pub main_hwnd: isize,
}

pub static GLOBAL_STATE: LazyLock<Mutex<AppState>> = LazyLock::new(|| {
    Mutex::new(AppState {
        config: Config::default(),
        engine: Engine::new(),
        main_hwnd: 0,
    })
});

pub static TRAY_MANAGER: LazyLock<Mutex<Option<TrayManager>>> = LazyLock::new(|| Mutex::new(None));

fn main() {
    let class_name = to_wide("HoyuKey_TrayReceiver");

    // 1. Kiểm tra đơn thể: Nếu HoyuKey đã chạy ngầm, gửi lệnh mở Bảng điều khiển rồi thoát
    let existing_wnd = unsafe { FindWindowW(class_name.as_ptr(), null_mut()) };
    if existing_wnd != null_mut() {
        unsafe {
            PostMessageW(existing_wnd, WM_APP_SHOW_SETTINGS, 0, 0);
        }
        return;
    }

    let hinstance = unsafe { GetModuleHandleW(null_mut()) };

    // 2. Đăng ký Window Class cho cửa sổ nhận sự kiện Tray
    let wnd_class = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(tray_wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: null_mut(),
        hCursor: null_mut(),
        hbrBackground: null_mut(),
        lpszMenuName: null_mut(),
        lpszClassName: class_name.as_ptr(),
    };

    unsafe {
        RegisterClassW(&wnd_class);
    }

    // 3. Tạo Message-Only Window
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            hinstance,
            null_mut(),
        )
    };

    if hwnd == null_mut() {
        return;
    }

    GLOBAL_STATE.lock().unwrap().main_hwnd = hwnd as isize;

    // 4. Khởi tạo Tray Icon và Cửa sổ cài đặt
    let tray = TrayManager::new(hwnd);
    {
        let config = GLOBAL_STATE.lock().unwrap().config.clone();
        tray.add_to_tray(&config);
    }
    *TRAY_MANAGER.lock().unwrap() = Some(tray);

    create_settings_window(hinstance);

    // 5. Cài đặt Low-Level Keyboard Hook
    if !hook::install_hook() {
        unsafe {
            MessageBoxW(
                null_mut(),
                to_wide("Không thể cài đặt Keyboard Hook. Hãy thử chạy với quyền Administrator!").as_ptr(),
                to_wide("HoyuKey - Lỗi").as_ptr(),
                MB_OK,
            );
        }
        return;
    }

    // 6. Vòng lặp tin nhắn Windows (Message Loop)
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    unsafe {
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // 7. Giải phóng tài nguyên khi thoát
    hook::uninstall_hook();
    if let Some(tray) = TRAY_MANAGER.lock().unwrap().take() {
        tray.remove_from_tray();
    }
}

unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_APP_SHOW_SETTINGS => {
            show_settings_window();
            0
        }
        WM_APP_TRAY => {
            let event = lparam as u32;
            match event {
                WM_LBUTTONUP => {
                    hook::toggle_vietnamese_mode();
                }
                WM_LBUTTONDBLCLK => {
                    show_settings_window();
                }
                WM_RBUTTONUP => {
                    let config = GLOBAL_STATE.lock().unwrap().config.clone();
                    if let Some(tray) = TRAY_MANAGER.lock().unwrap().as_ref() {
                        tray.show_context_menu(&config);
                    }
                }
                _ => {}
            }
            0
        }
        WM_APP_UPDATE_TRAY => {
            let config = GLOBAL_STATE.lock().unwrap().config.clone();
            if let Some(tray) = TRAY_MANAGER.lock().unwrap().as_ref() {
                tray.update_tray(&config);
            }
            0
        }
        WM_COMMAND => {
            let cmd_id = (wparam & 0xFFFF) as usize;
            handle_tray_command(cmd_id);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn handle_tray_command(cmd_id: usize) {
    let mut state = GLOBAL_STATE.lock().unwrap();

    match cmd_id {
        IDM_TOGGLE_MODE => {
            state.config.enabled = !state.config.enabled;
            state.engine.reset();
            let config = state.config.clone();
            drop(state);
            if let Some(tray) = TRAY_MANAGER.lock().unwrap().as_ref() {
                tray.update_tray(&config);
            }
        }
        IDM_TELEX => {
            state.config.input_method = InputMethod::Telex;
        }
        IDM_VNI => {
            state.config.input_method = InputMethod::Vni;
        }
        IDM_MODERN_TONE => {
            state.config.modern_tone = !state.config.modern_tone;
        }
        IDM_AUTO_RESTORE => {
            state.config.auto_restore = !state.config.auto_restore;
        }
        IDM_CONTROL_PANEL => {
            drop(state);
            show_settings_window();
        }
        IDM_EXIT => {
            drop(state);
            unsafe {
                PostQuitMessage(0);
            }
        }
        _ => {}
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
