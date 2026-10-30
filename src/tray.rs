use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows_sys::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, GetDC, ReleaseDC, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, DestroyIcon, DestroyMenu, GetCursorPos,
    PostMessageW, SetForegroundWindow, TrackPopupMenuEx, HICON, HMENU, ICONINFO, MF_CHECKED,
    MF_SEPARATOR, MF_STRING, MF_UNCHECKED, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RIGHTBUTTON,
};

use crate::config::{Config, InputMethod};

pub const WM_APP_TRAY: u32 = 0x8000 + 1; // WM_USER + 1
pub const WM_APP_UPDATE_TRAY: u32 = 0x8000 + 2;
pub const WM_APP_SHOW_SETTINGS: u32 = 0x8000 + 3;

// Command IDs
pub const IDM_TOGGLE_MODE: usize = 1001;
pub const IDM_TELEX: usize = 1002;
pub const IDM_VNI: usize = 1003;
pub const IDM_MODERN_TONE: usize = 1004;
pub const IDM_AUTO_RESTORE: usize = 1005;
pub const IDM_CONTROL_PANEL: usize = 1008;
pub const IDM_EXIT: usize = 1009;

// Nhúng trực tiếp 2 icon V và E 32x32 Premultiplied 32-bit ARGB do người dùng cung cấp
const V_ICON_BYTES_32: &[u8] = include_bytes!("../icon/v_32.bin");
const E_ICON_BYTES_32: &[u8] = include_bytes!("../icon/e_32.bin");

pub struct TrayManager {
    hwnd: HWND,
    icon_v: HICON,
    icon_e: HICON,
}

unsafe impl Send for TrayManager {}
unsafe impl Sync for TrayManager {}

impl TrayManager {
    pub fn new(hwnd: HWND) -> Self {
        let icon_v = load_tray_icon(true);
        let icon_e = load_tray_icon(false);

        Self {
            hwnd,
            icon_v,
            icon_e,
        }
    }

    pub fn add_to_tray(&self, config: &Config) {
        let icon = if config.enabled { self.icon_v } else { self.icon_e };
        let tooltip = if config.enabled {
            to_wide_chars("HoyuKey: Tiếng Việt [ON]")
        } else {
            to_wide_chars("HoyuKey: Tiếng Anh [OFF]")
        };

        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = self.hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = WM_APP_TRAY;
        nid.hIcon = icon;

        let copy_len = tooltip.len().min(nid.szTip.len() - 1);
        nid.szTip[..copy_len].copy_from_slice(&tooltip[..copy_len]);

        unsafe {
            Shell_NotifyIconW(NIM_ADD, &nid);
        }
    }

    pub fn update_tray(&self, config: &Config) {
        let icon = if config.enabled { self.icon_v } else { self.icon_e };
        let tooltip = if config.enabled {
            to_wide_chars("HoyuKey: Tiếng Việt [ON]")
        } else {
            to_wide_chars("HoyuKey: Tiếng Anh [OFF]")
        };

        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = self.hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_ICON | NIF_TIP;
        nid.hIcon = icon;

        let copy_len = tooltip.len().min(nid.szTip.len() - 1);
        nid.szTip[..copy_len].copy_from_slice(&tooltip[..copy_len]);

        unsafe {
            Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    pub fn remove_from_tray(&self) {
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = self.hwnd;
        nid.uID = 1;

        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &nid);
            if self.icon_v != null_mut() {
                DestroyIcon(self.icon_v);
            }
            if self.icon_e != null_mut() {
                DestroyIcon(self.icon_e);
            }
        }
    }

    pub fn show_context_menu(&self, config: &Config) {
        unsafe {
            let menu: HMENU = CreatePopupMenu();
            if menu == null_mut() {
                return;
            }

            // Tiếng Việt / Tiếng Anh
            let vi_flag = if config.enabled { MF_CHECKED } else { MF_UNCHECKED };
            let en_flag = if !config.enabled { MF_CHECKED } else { MF_UNCHECKED };
            append_menu(menu, vi_flag, IDM_TOGGLE_MODE, "[V] Tiếng Việt (Bật)");
            append_menu(menu, en_flag, IDM_TOGGLE_MODE, "[E] Tiếng Anh (Tắt)");

            append_separator(menu);

            // Kiểu gõ
            let telex_flag = if config.input_method == InputMethod::Telex { MF_CHECKED } else { MF_UNCHECKED };
            let vni_flag = if config.input_method == InputMethod::Vni { MF_CHECKED } else { MF_UNCHECKED };
            append_menu(menu, telex_flag, IDM_TELEX, "Kiểu gõ Telex");
            append_menu(menu, vni_flag, IDM_VNI, "Kiểu gõ VNI");

            append_separator(menu);

            // Tùy chọn dấu
            let tone_flag = if config.modern_tone { MF_CHECKED } else { MF_UNCHECKED };
            let restore_flag = if config.auto_restore { MF_CHECKED } else { MF_UNCHECKED };
            append_menu(menu, tone_flag, IDM_MODERN_TONE, "Bỏ dấu kiểu mới (hòa, hóa)");
            append_menu(menu, restore_flag, IDM_AUTO_RESTORE, "Tự động khôi phục phím");

            append_separator(menu);

            // Bảng điều khiển & Thoát
            append_menu(menu, MF_STRING, IDM_CONTROL_PANEL, "Bảng điều khiển HoyuKey...");
            append_separator(menu);
            append_menu(menu, MF_STRING, IDM_EXIT, "Kết thúc (Thoát HoyuKey)");

            let mut pt: POINT = std::mem::zeroed();
            GetCursorPos(&mut pt);

            SetForegroundWindow(self.hwnd);
            TrackPopupMenuEx(
                menu,
                TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_BOTTOMALIGN,
                pt.x,
                pt.y,
                self.hwnd,
                null_mut(),
            );
            DestroyMenu(menu);
            PostMessageW(self.hwnd, 0, 0, 0);
        }
    }
}

unsafe fn append_menu(menu: HMENU, flags: u32, id: usize, text: &str) {
    let wide = to_wide_chars(text);
    AppendMenuW(menu, flags | MF_STRING, id, wide.as_ptr());
}

unsafe fn append_separator(menu: HMENU) {
    AppendMenuW(menu, MF_SEPARATOR, 0, null_mut());
}

fn to_wide_chars(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn load_tray_icon(is_v: bool) -> HICON {
    let bytes = if is_v { V_ICON_BYTES_32 } else { E_ICON_BYTES_32 };
    create_hicon_from_bgra(bytes, 32)
}

pub fn create_hicon_from_bgra(bytes: &[u8], size: i32) -> HICON {
    unsafe {
        let hdc = GetDC(null_mut());
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = size;
        bmi.bmiHeader.biHeight = -size; // top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;

        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let hbm_color = CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if !bits.is_null() {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), bits as *mut u8, bytes.len());
        }

        // Tạo 1-bit monochrome mask (tất cả 0 để cho phép 32-bit alpha hoạt động)
        let hbm_mask = CreateBitmap(size, size, 1, 1, std::ptr::null());

        let mut icon_info: ICONINFO = std::mem::zeroed();
        icon_info.fIcon = 1; // true = ICON
        icon_info.hbmColor = hbm_color;
        icon_info.hbmMask = hbm_mask;

        let hicon = CreateIconIndirect(&icon_info);

        DeleteObject(hbm_color);
        DeleteObject(hbm_mask);
        ReleaseDC(null_mut(), hdc);

        hicon
    }
}
