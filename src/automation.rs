#![allow(non_snake_case, dead_code)]

use std::ffi::c_void;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::System::Com::*;
use windows_sys::Win32::UI::Accessibility::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId, IsHungAppWindow, SendMessageW,
    WM_GETTEXT, WM_GETTEXTLENGTH,
};

#[link(name = "oleaut32")]
extern "system" {
    fn SysFreeString(bstr: *const u16);
    fn SysStringLen(bstr: *const u16) -> u32;
}

#[link(name = "user32")]
extern "system" {
    pub(crate) fn GetGUIThreadInfo(idThread: u32, lpgui: *mut GUITHREADINFO) -> i32;
}

const EM_GETSEL: u32 = 0x00B0;

#[repr(C)]
#[allow(non_snake_case)]
pub(crate) struct GUITHREADINFO {
    pub cbSize: u32,
    pub flags: u32,
    pub hwndActive: HWND,
    pub hwndFocus: HWND,
    pub hwndCapture: HWND,
    pub hwndMenuOwner: HWND,
    pub hwndMoveSize: HWND,
    pub hwndCaret: HWND,
    pub rcCaret: RECT,
}

pub const IID_IUI_AUTOMATION: windows_sys::core::GUID =
    windows_sys::core::GUID::from_u128(0x30cbe57d_d9d0_452a_ab13_7ac07248ee6e);

pub const UIA_TEXT_PATTERN_ID: i32 = 10014;
pub const TEXT_UNIT_WORD: i32 = 2;
pub const TEXT_PATTERN_RANGE_ENDPOINT_START: i32 = 0;
pub const TEXT_PATTERN_RANGE_ENDPOINT_END: i32 = 1;

#[repr(C)]
pub struct IUnknownVtbl {
    pub QueryInterface: unsafe extern "system" fn(
        this: *mut c_void,
        riid: *const windows_sys::core::GUID,
        ppvObject: *mut *mut c_void,
    ) -> windows_sys::core::HRESULT,
    pub AddRef: unsafe extern "system" fn(this: *mut c_void) -> u32,
    pub Release: unsafe extern "system" fn(this: *mut c_void) -> u32,
}

#[repr(C)]
pub struct IUIAutomationVtbl {
    pub parent: IUnknownVtbl,
    pub CompareElements: usize,
    pub CompareRuntimeIds: usize,
    pub GetRootElement: usize,
    pub ElementFromHandle: unsafe extern "system" fn(
        this: *mut c_void,
        hwnd: HWND,
        element: *mut *mut c_void,
    ) -> windows_sys::core::HRESULT,
    pub ElementFromPoint: usize,
    pub GetFocusedElement: unsafe extern "system" fn(
        this: *mut c_void,
        element: *mut *mut c_void,
    ) -> windows_sys::core::HRESULT,
}

#[repr(C)]
pub struct IUIAutomationElementVtbl {
    pub parent: IUnknownVtbl,
    pub SetFocus: usize,
    pub GetRuntimeId: usize,
    pub FindFirst: usize,
    pub FindAll: usize,
    pub FindFirstBuildCache: usize,
    pub FindAllBuildCache: usize,
    pub BuildUpdatedCache: usize,
    pub GetCurrentPropertyValue: usize,
    pub GetCurrentPropertyValueEx: usize,
    pub GetCachedPropertyValue: usize,
    pub GetCachedPropertyValueEx: usize,
    pub GetCurrentPatternAs: usize,
    pub GetCurrentPattern: unsafe extern "system" fn(
        this: *mut c_void,
        patternId: i32,
        patternObject: *mut *mut c_void,
    ) -> windows_sys::core::HRESULT,
}

#[repr(C)]
pub struct IUIAutomationTextPatternVtbl {
    pub parent: IUnknownVtbl,
    pub RangeFromPoint: usize,
    pub RangeFromChild: usize,
    pub GetSelection: unsafe extern "system" fn(
        this: *mut c_void,
        ranges: *mut *mut c_void,
    ) -> windows_sys::core::HRESULT,
    pub GetVisibleRanges: usize,
    pub get_DocumentRange: usize,
    pub get_SupportedTextSelection: usize,
}

#[repr(C)]
pub struct IUIAutomationTextRangeArrayVtbl {
    pub parent: IUnknownVtbl,
    pub get_Length: unsafe extern "system" fn(
        this: *mut c_void,
        length: *mut i32,
    ) -> windows_sys::core::HRESULT,
    pub GetElement: unsafe extern "system" fn(
        this: *mut c_void,
        index: i32,
        element: *mut *mut c_void,
    ) -> windows_sys::core::HRESULT,
}

#[repr(C)]
pub struct IUIAutomationTextRangeVtbl {
    pub parent: IUnknownVtbl,
    pub Clone: unsafe extern "system" fn(
        this: *mut c_void,
        cloned: *mut *mut c_void,
    ) -> windows_sys::core::HRESULT,
    pub Compare: usize,
    pub CompareEndpoints: usize,
    pub ExpandToEnclosingUnit: unsafe extern "system" fn(
        this: *mut c_void,
        unit: i32,
    ) -> windows_sys::core::HRESULT,
    pub FindAttribute: usize,
    pub FindText: usize,
    pub GetAttributeValue: usize,
    pub GetBoundingRectangles: usize,
    pub GetEnclosingElement: usize,
    pub GetText: unsafe extern "system" fn(
        this: *mut c_void,
        maxLength: i32,
        text: *mut *mut u16,
    ) -> windows_sys::core::HRESULT,
    pub Move: usize,
    pub MoveEndpointByUnit: usize,
    pub MoveEndpointByRange: unsafe extern "system" fn(
        this: *mut c_void,
        srcEndPoint: i32,
        targetRange: *mut c_void,
        targetEndPoint: i32,
    ) -> windows_sys::core::HRESULT,
    pub Select: unsafe extern "system" fn(this: *mut c_void) -> windows_sys::core::HRESULT,
    pub AddToSelection: usize,
    pub RemoveFromSelection: usize,
    pub ScrollIntoView: usize,
    pub GetChildren: usize,
}

struct ComPtr<T> {
    ptr: *mut c_void,
    _marker: std::marker::PhantomData<T>,
}

impl<T> ComPtr<T> {
    fn new(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self {
                ptr,
                _marker: std::marker::PhantomData,
            })
        }
    }

    fn as_ptr(&self) -> *mut c_void {
        self.ptr
    }

    fn vtbl(&self) -> &T {
        unsafe { &**(self.ptr as *mut *mut T) }
    }
}

impl<T> Drop for ComPtr<T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                let unk = &**(self.ptr as *mut *mut IUnknownVtbl);
                (unk.Release)(self.ptr);
            }
        }
    }
}

unsafe fn bstr_to_string(bstr: *const u16) -> String {
    if bstr.is_null() {
        return String::new();
    }
    let len = SysStringLen(bstr) as usize;
    let slice = std::slice::from_raw_parts(bstr, len);
    String::from_utf16_lossy(slice)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaretWordContext {
    /// Toàn bộ từ đang xét tại vị trí con trỏ (ví dụ: "tày")
    pub full_word: String,
    /// Phần từ nằm phía trước con trỏ (ví dụ: "tày" nếu con trỏ ở cuối từ, hoặc "t" nếu ở giữa)
    pub pre_word: String,
    /// Phần từ nằm phía sau con trỏ (ví dụ: "" nếu con trỏ ở cuối từ, hoặc "ày" nếu ở giữa)
    pub post_word: String,
}

/// Trích xuất từ tại vị trí con trỏ từ 2 chuỗi: chuỗi trước con trỏ và chuỗi sau con trỏ.
/// QUY TẮC CỐT LÕI:
/// 1. Vị trí con trỏ BẮT BUỘC phải tiếp giáp trực tiếp với chữ cái đứng trước nó.
/// 2. Nếu trước con trỏ là khoảng trắng, đầu dòng, ký tự đặc biệt -> KHÔNG PHẢI đang sửa từ -> trả về None!
/// 3. Từ tiếng Việt trích xuất chỉ bao gồm các ký tự chữ cái và độ dài từ 1..=10 ký tự.
pub fn extract_context_from_pre_post(pre_str: &str, post_str: &str) -> Option<CaretWordContext> {
    let last_pre = pre_str.chars().last()?;
    if !last_pre.is_alphabetic() {
        return None;
    }

    let pre_word: String = pre_str
        .chars()
        .rev()
        .take_while(|c| c.is_alphabetic())
        .collect::<Vec<char>>()
        .into_iter()
        .rev()
        .collect();

    if pre_word.is_empty() {
        return None;
    }

    let post_word: String = post_str
        .chars()
        .take_while(|c| c.is_alphabetic())
        .collect();

    let full_word = format!("{}{}", pre_word, post_word);
    let len = full_word.chars().count();
    if len == 0 || len > 10 {
        return None;
    }

    Some(CaretWordContext {
        full_word,
        pre_word,
        post_word,
    })
}

/// Kiểm tra xem biến đổi có xảy ra hoàn toàn ở phía sau con trỏ (vị trí con trỏ phải đứng sau ký tự được sửa).
/// Nghĩa là phần đuôi post_word phía sau con trỏ không hề bị thay đổi.
pub fn is_modification_behind_caret(post_word: &str, new_word: &str) -> bool {
    let post_len = post_word.chars().count();
    let new_chars: Vec<char> = new_word.chars().collect();
    let post_chars: Vec<char> = post_word.chars().collect();
    if new_chars.len() < post_len {
        return false;
    }
    new_chars[new_chars.len() - post_len..] == post_chars[..]
}

/// Trích xuất từ và vị trí con trỏ từ một chuỗi văn bản và chỉ số vị trí con trỏ (character offset)
pub fn extract_word_from_text_at_caret(text: &str, caret_char_offset: usize) -> Option<CaretWordContext> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    if n == 0 || caret_char_offset == 0 {
        return None;
    }
    let pos = caret_char_offset.min(n);
    let pre_str: String = chars[..pos].iter().collect();
    let post_str: String = chars[pos..].iter().collect();
    extract_context_from_pre_post(&pre_str, &post_str)
}

/// Cách 1: Đọc từ qua Win32 Edit / RichEdit control (Notepad, WordPad, hộp thoại Windows...)
fn get_win32_edit_caret_word(fg: HWND) -> Option<CaretWordContext> {
    unsafe {
        let tid = GetWindowThreadProcessId(fg, null_mut());
        let mut gui: GUITHREADINFO = std::mem::zeroed();
        gui.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        if GetGUIThreadInfo(tid, &mut gui) == 0 {
            return None;
        }

        let target_hwnd = if !gui.hwndFocus.is_null() {
            gui.hwndFocus
        } else if !gui.hwndCaret.is_null() {
            gui.hwndCaret
        } else {
            return None;
        };

        // BẮT BUỘC: Kiểm tra target_hwnd có thực sự là Edit / RichEdit control hay không!
        // Tuyệt đối không gửi WM_GETTEXT tới các cửa sổ khác vì nó sẽ trả về Window Title (tiêu đề cửa sổ/tab)!
        let mut class_buf = [0u16; 64];
        let class_len = GetClassNameW(target_hwnd, class_buf.as_mut_ptr(), 64);
        if class_len == 0 {
            return None;
        }
        let class_name = String::from_utf16_lossy(&class_buf[..class_len as usize]).to_ascii_lowercase();
        let is_edit = class_name == "edit"
            || class_name.starts_with("richedit")
            || class_name.starts_with("msftedit")
            || class_name.contains("richedit");
        if !is_edit {
            return None;
        }

        let mut start: u32 = 0;
        let mut end: u32 = 0;
        SendMessageW(
            target_hwnd,
            EM_GETSEL,
            &mut start as *mut _ as usize,
            &mut end as *mut _ as isize,
        );

        // Nếu con trỏ ở vị trí 0 (đầu văn bản) hoặc đang bôi đen vùng chọn (start != end): không can thiệp
        if start != end || start == 0 {
            return None;
        }

        let len = SendMessageW(target_hwnd, WM_GETTEXTLENGTH, 0, 0) as usize;
        if len == 0 || len > 65536 || (start as usize) > len {
            return None;
        }

        let mut buf = vec![0u16; len + 1];
        let copied = SendMessageW(
            target_hwnd,
            WM_GETTEXT,
            (len + 1) as usize,
            buf.as_mut_ptr() as isize,
        ) as usize;
        if copied == 0 {
            return None;
        }

        let start_idx = (start as usize).min(copied);
        let pre_str = String::from_utf16_lossy(&buf[..start_idx]);
        let post_str = String::from_utf16_lossy(&buf[start_idx..copied]);
        extract_context_from_pre_post(&pre_str, &post_str)
    }
}

static UIA_INSTANCE: std::sync::Mutex<Option<usize>> = std::sync::Mutex::new(None);

fn get_cached_uia() -> Option<*mut c_void> {
    let mut lock = UIA_INSTANCE.lock().ok()?;
    if let Some(ptr_val) = *lock {
        return Some(ptr_val as *mut c_void);
    }
    unsafe {
        let _ = CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED as u32);
        let mut uia_raw: *mut c_void = null_mut();
        let hr = CoCreateInstance(
            &CUIAutomation,
            null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_IUI_AUTOMATION,
            &mut uia_raw as *mut _ as *mut *mut c_void,
        );
        if hr >= 0 && !uia_raw.is_null() {
            *lock = Some(uia_raw as usize);
            Some(uia_raw)
        } else {
            None
        }
    }
}

/// Cách 2: Đọc từ qua Windows UI Automation (Word, Chrome, Edge, VS Code, UWP, WPF...)
fn get_uia_caret_word() -> Option<CaretWordContext> {
    unsafe {
        let hwnd_fg = GetForegroundWindow();
        if hwnd_fg.is_null() {
            return None;
        }

        let thread_id = GetWindowThreadProcessId(hwnd_fg, null_mut());
        let mut gui: GUITHREADINFO = std::mem::zeroed();
        gui.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        let hwnd_target = if GetGUIThreadInfo(thread_id, &mut gui) != 0 && !gui.hwndFocus.is_null() {
            gui.hwndFocus
        } else {
            hwnd_fg
        };

        // Đánh thức cây Accessibility của Chromium (Chrome, Edge...) nếu đang ở chế độ lười (lazy-init)
        const WM_GETOBJECT: u32 = 0x003D;
        const OBJID_CLIENT: isize = -4;
        const SMTO_ABORTIFHUNG: u32 = 0x0002;
        let mut result: usize = 0;
        windows_sys::Win32::UI::WindowsAndMessaging::SendMessageTimeoutW(
            hwnd_target,
            WM_GETOBJECT,
            0,
            OBJID_CLIENT,
            SMTO_ABORTIFHUNG,
            20,
            &mut result,
        );
        if hwnd_target != hwnd_fg {
            let _ = windows_sys::Win32::UI::WindowsAndMessaging::SendMessageTimeoutW(
                hwnd_fg,
                WM_GETOBJECT,
                0,
                OBJID_CLIENT,
                SMTO_ABORTIFHUNG,
                20,
                &mut result,
            );
        }

        let uia_raw = get_cached_uia()?;
        let uia = &**(uia_raw as *mut *mut IUIAutomationVtbl);

        let mut elem_raw: *mut c_void = null_mut();
        let mut hr = (uia.GetFocusedElement)(uia_raw, &mut elem_raw);
        if (hr < 0 || elem_raw.is_null()) && !hwnd_target.is_null() {
            hr = (uia.ElementFromHandle)(uia_raw, hwnd_target, &mut elem_raw);
        }
        if (hr < 0 || elem_raw.is_null()) && !hwnd_fg.is_null() {
            hr = (uia.ElementFromHandle)(uia_raw, hwnd_fg, &mut elem_raw);
        }
        if hr < 0 || elem_raw.is_null() {
            return None;
        }
        let elem = ComPtr::<IUIAutomationElementVtbl>::new(elem_raw)?;

        let mut pat_raw: *mut c_void = null_mut();
        let mut hr = (elem.vtbl().GetCurrentPattern)(elem.as_ptr(), UIA_TEXT_PATTERN_ID, &mut pat_raw);
        if hr < 0 || pat_raw.is_null() {
            // Dự phòng TextPattern2 (ID 10024) cho các ứng dụng hiện đại (Chromium, Edge...)
            hr = (elem.vtbl().GetCurrentPattern)(elem.as_ptr(), 10024, &mut pat_raw);
        }
        if pat_raw.is_null() && !hwnd_target.is_null() {
            let mut wnd_elem_raw: *mut c_void = null_mut();
            if (uia.ElementFromHandle)(uia_raw, hwnd_target, &mut wnd_elem_raw) >= 0 && !wnd_elem_raw.is_null() {
                if let Some(we) = ComPtr::<IUIAutomationElementVtbl>::new(wnd_elem_raw) {
                    let _ = (we.vtbl().GetCurrentPattern)(we.as_ptr(), UIA_TEXT_PATTERN_ID, &mut pat_raw);
                    if pat_raw.is_null() {
                        let _ = (we.vtbl().GetCurrentPattern)(we.as_ptr(), 10024, &mut pat_raw);
                    }
                }
            }
        }
        if pat_raw.is_null() && !hwnd_fg.is_null() {
            let mut wnd_elem_raw: *mut c_void = null_mut();
            if (uia.ElementFromHandle)(uia_raw, hwnd_fg, &mut wnd_elem_raw) >= 0 && !wnd_elem_raw.is_null() {
                if let Some(we) = ComPtr::<IUIAutomationElementVtbl>::new(wnd_elem_raw) {
                    let _ = (we.vtbl().GetCurrentPattern)(we.as_ptr(), UIA_TEXT_PATTERN_ID, &mut pat_raw);
                    if pat_raw.is_null() {
                        let _ = (we.vtbl().GetCurrentPattern)(we.as_ptr(), 10024, &mut pat_raw);
                    }
                }
            }
        }
        if hr < 0 || pat_raw.is_null() {
            return None;
        }
        let text_pat = ComPtr::<IUIAutomationTextPatternVtbl>::new(pat_raw)?;

        let mut ranges_raw: *mut c_void = null_mut();
        let hr = (text_pat.vtbl().GetSelection)(text_pat.as_ptr(), &mut ranges_raw);
        if hr < 0 || ranges_raw.is_null() {
            return None;
        }
        let ranges = ComPtr::<IUIAutomationTextRangeArrayVtbl>::new(ranges_raw)?;

        let mut count: i32 = 0;
        let hr = (ranges.vtbl().get_Length)(ranges.as_ptr(), &mut count);
        if hr < 0 || count <= 0 {
            return None;
        }

        let mut caret_range_raw: *mut c_void = null_mut();
        let hr = (ranges.vtbl().GetElement)(ranges.as_ptr(), 0, &mut caret_range_raw);
        if hr < 0 || caret_range_raw.is_null() {
            return None;
        }
        let caret_range = ComPtr::<IUIAutomationTextRangeVtbl>::new(caret_range_raw)?;

        // Lấy toàn bộ từ bao quanh con trỏ
        let mut word_range_raw: *mut c_void = null_mut();
        let hr = (caret_range.vtbl().Clone)(caret_range.as_ptr(), &mut word_range_raw);
        if hr < 0 || word_range_raw.is_null() {
            return None;
        }
        let word_range = ComPtr::<IUIAutomationTextRangeVtbl>::new(word_range_raw)?;
        let _ = (word_range.vtbl().ExpandToEnclosingUnit)(word_range.as_ptr(), TEXT_UNIT_WORD);

        // Lấy đoạn từ đầu từ tới con trỏ
        let mut pre_range_raw: *mut c_void = null_mut();
        let hr = (caret_range.vtbl().Clone)(caret_range.as_ptr(), &mut pre_range_raw);
        let raw_pre_text = if hr >= 0 && !pre_range_raw.is_null() {
            let pre_range = ComPtr::<IUIAutomationTextRangeVtbl>::new(pre_range_raw)?;
            let _ = (pre_range.vtbl().MoveEndpointByRange)(
                pre_range.as_ptr(),
                TEXT_PATTERN_RANGE_ENDPOINT_START,
                word_range.as_ptr(),
                TEXT_PATTERN_RANGE_ENDPOINT_START,
            );
            let mut bstr_pre: *mut u16 = null_mut();
            let hr_pre = (pre_range.vtbl().GetText)(pre_range.as_ptr(), -1, &mut bstr_pre);
            if hr_pre >= 0 {
                let s = bstr_to_string(bstr_pre);
                SysFreeString(bstr_pre);
                s
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Lấy đoạn từ con trỏ tới cuối từ
        let mut post_range_raw: *mut c_void = null_mut();
        let hr = (word_range.vtbl().Clone)(word_range.as_ptr(), &mut post_range_raw);
        let raw_post_text = if hr >= 0 && !post_range_raw.is_null() {
            let post_range = ComPtr::<IUIAutomationTextRangeVtbl>::new(post_range_raw)?;
            let _ = (post_range.vtbl().MoveEndpointByRange)(
                post_range.as_ptr(),
                TEXT_PATTERN_RANGE_ENDPOINT_START,
                caret_range.as_ptr(),
                TEXT_PATTERN_RANGE_ENDPOINT_END,
            );
            let mut bstr_post: *mut u16 = null_mut();
            let hr_post = (post_range.vtbl().GetText)(post_range.as_ptr(), -1, &mut bstr_post);
            if hr_post >= 0 {
                let s = bstr_to_string(bstr_post);
                SysFreeString(bstr_post);
                s
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        extract_context_from_pre_post(&raw_pre_text, &raw_post_text)
    }
}

/// Trích xuất từ tại vị trí con trỏ (Caret):
/// Kết hợp Win32 Edit API (Notepad, hộp thoại) và Windows UI Automation (Word, Chrome, Edge...)
pub fn get_caret_word_context() -> Option<CaretWordContext> {
    unsafe {
        let hwnd_fg = GetForegroundWindow();
        if hwnd_fg.is_null() || IsHungAppWindow(hwnd_fg) != 0 {
            return None;
        }

        // 1. Ưu tiên Win32 Edit API siêu nhanh và hỗ trợ 100% Notepad, WordPad, form desktop
        if let Some(ctx) = get_win32_edit_caret_word(hwnd_fg) {
            return Some(ctx);
        }

        // 2. Dự phòng sang UI Automation cho các ứng dụng hiện đại (Chrome, Word, VS Code...)
        get_uia_caret_word()
    }
}

static CARET_CACHE: std::sync::Mutex<Option<CaretWordContext>> = std::sync::Mutex::new(None);
static WORKER_TRIGGER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Yêu cầu background thread quét lại vị trí con trỏ (gọi bất đồng bộ, 0ms latency)
pub fn request_caret_refresh() {
    WORKER_TRIGGER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Lấy từ ngữ cảnh con trỏ đã được background thread nạp sẵn từ trước (0ms latency)
pub fn get_cached_caret_word() -> Option<CaretWordContext> {
    CARET_CACHE.lock().ok()?.clone()
}

/// Xóa cache khi người dùng bắt đầu gõ hoặc click chuột
pub fn clear_cached_caret_word() {
    if let Ok(mut lock) = CARET_CACHE.lock() {
        *lock = None;
    }
}

/// Khởi chạy background worker thread nạp sẵn từ tại con trỏ mà không làm lag hook phím
pub fn init_background_caret_loader() {
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }

    std::thread::Builder::new()
        .name("hoyukey-caret-loader".to_string())
        .spawn(|| {
            unsafe {
                let _ = CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED as u32);
            }
            let mut last_processed = 0;
            loop {
                std::thread::sleep(std::time::Duration::from_millis(35));
                let current_trigger = WORKER_TRIGGER.load(std::sync::atomic::Ordering::Relaxed);
                if current_trigger != 0 && current_trigger != last_processed {
                    last_processed = current_trigger;
                    let ctx = get_caret_word_context();
                    if WORKER_TRIGGER.load(std::sync::atomic::Ordering::Relaxed) == current_trigger {
                        if let Ok(mut lock) = CARET_CACHE.lock() {
                            *lock = ctx;
                        }
                    }
                }
            }
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_word_from_text() {
        let text = "đây là gì đó";
        // Con trỏ đứng ngay sau từ "đây" (char index 3)
        let ctx = extract_word_from_text_at_caret(text, 3).unwrap();
        assert_eq!(ctx.full_word, "đây");
        assert_eq!(ctx.pre_word, "đây");
        assert_eq!(ctx.post_word, "");

        // Con trỏ đứng ở giữa: "đ|ây" (char index 1)
        let ctx2 = extract_word_from_text_at_caret(text, 1).unwrap();
        assert_eq!(ctx2.full_word, "đây");
        assert_eq!(ctx2.pre_word, "đ");
        assert_eq!(ctx2.post_word, "ây");

        // Con trỏ đứng ở từ "đó" (char index 12)
        let ctx3 = extract_word_from_text_at_caret(text, 12).unwrap();
        assert_eq!(ctx3.full_word, "đó");
        assert_eq!(ctx3.pre_word, "đó");
        assert_eq!(ctx3.post_word, "");
    }

    #[test]
    fn test_is_modification_behind_caret() {
        // 1. Con trỏ đứng ở cuối từ: "tày|" + 's' -> "táy" (post_word = "")
        assert!(is_modification_behind_caret("", "táy"));

        // 2. Con trỏ đứng sau chữ 'à': "tà|y" + 's' -> "táy" (post_word = "y")
        assert!(is_modification_behind_caret("y", "táy"));

        // 3. Con trỏ đứng trước chữ 'à': "t|ày" + 's' -> "táy" (post_word = "ày")
        // Biến đổi xảy ra ở phía trước con trỏ (ở chữ à) -> BỊ TỪ CHỐI!
        assert!(!is_modification_behind_caret("ày", "táy"));

        // 4. "đ|ây" + 's' -> "đấy" (post_word = "ây") -> BỊ TỪ CHỐI!
        assert!(!is_modification_behind_caret("ây", "đấy"));

        // 5. "đâ|y" + 's' -> "đấy" (post_word = "y") -> CHẤP NHẬN!
        assert!(is_modification_behind_caret("y", "đấy"));

        // 6. "đây|" + 's' -> "đấy" (post_word = "") -> CHẤP NHẬN!
        assert!(is_modification_behind_caret("", "đấy"));
    }

    #[test]
    fn test_caret_at_start_or_before_word_returns_none() {
        // Con trỏ ở đầu văn bản (|Tao hoặc |đây) -> TUYỆT ĐỐI KHÔNG BIẾN ĐỔI!
        assert_eq!(extract_word_from_text_at_caret("Tao", 0), None);
        assert_eq!(extract_word_from_text_at_caret("đây là gì đó", 0), None);
        assert_eq!(extract_word_from_text_at_caret("", 0), None);
    }

    #[test]
    fn test_caret_after_space_or_punct_returns_none() {
        // Con trỏ đứng sau dấu cách ("đây |") -> KHÔNG BIẾN ĐỔI!
        assert_eq!(extract_word_from_text_at_caret("đây là ", 4), None);
        assert_eq!(extract_word_from_text_at_caret("đây là ", 7), None);
        assert_eq!(extract_word_from_text_at_caret("xong. ", 6), None);
    }

    #[test]
    fn test_extract_context_from_pre_post() {
        // Trường hợp con trỏ sau từ "tày"
        let ctx = extract_context_from_pre_post("Hôm nay tày", "").unwrap();
        assert_eq!(ctx.full_word, "tày");
        assert_eq!(ctx.pre_word, "tày");
        assert_eq!(ctx.post_word, "");

        // Trường hợp con trỏ ở giữa "t|ày" -> trích xuất pre_word="t", post_word="ày"
        let ctx2 = extract_context_from_pre_post("t", "ày là gì").unwrap();
        assert_eq!(ctx2.full_word, "tày");
        assert_eq!(ctx2.pre_word, "t");
        assert_eq!(ctx2.post_word, "ày");

        // Trường hợp con trỏ sau khoảng trắng
        assert_eq!(extract_context_from_pre_post("Tao ", ""), None);
        assert_eq!(extract_context_from_pre_post("", "Tao"), None);
    }
}
