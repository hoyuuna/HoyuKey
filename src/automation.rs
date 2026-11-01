#![allow(non_snake_case, dead_code)]

use std::ffi::c_void;
use std::ptr::null_mut;
use windows_sys::Win32::System::Com::*;
use windows_sys::Win32::UI::Accessibility::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsHungAppWindow};

#[link(name = "oleaut32")]
extern "system" {
    fn SysFreeString(bstr: *const u16);
    fn SysStringLen(bstr: *const u16) -> u32;
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
    pub ElementFromHandle: usize,
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
    pub GetCachedPatternAs: usize,
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

#[derive(Debug, Clone)]
pub struct CaretWordContext {
    /// Toàn bộ từ đang xét tại vị trí con trỏ (ví dụ: "tày")
    pub full_word: String,
    /// Phần từ nằm phía trước con trỏ (ví dụ: "tày" nếu con trỏ ở cuối từ, hoặc "t" nếu ở giữa)
    pub pre_word: String,
    /// Phần từ nằm phía sau con trỏ (ví dụ: "" nếu con trỏ ở cuối từ, hoặc "ày" nếu ở giữa)
    pub post_word: String,
}

/// Trích xuất từ tại vị trí con trỏ (Caret) thông qua Windows UI Automation
pub fn get_caret_word_context() -> Option<CaretWordContext> {
    unsafe {
        let hwnd_fg = GetForegroundWindow();
        if hwnd_fg.is_null() || IsHungAppWindow(hwnd_fg) != 0 {
            return None;
        }

        let _ = CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED as u32);

        let mut uia_raw: *mut c_void = null_mut();
        let hr = CoCreateInstance(
            &CUIAutomation,
            null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_IUI_AUTOMATION,
            &mut uia_raw as *mut _ as *mut *mut c_void,
        );
        if hr < 0 || uia_raw.is_null() {
            return None;
        }
        let uia = ComPtr::<IUIAutomationVtbl>::new(uia_raw)?;

        let mut elem_raw: *mut c_void = null_mut();
        let hr = (uia.vtbl().GetFocusedElement)(uia.as_ptr(), &mut elem_raw);
        if hr < 0 || elem_raw.is_null() {
            return None;
        }
        let elem = ComPtr::<IUIAutomationElementVtbl>::new(elem_raw)?;

        let mut pat_raw: *mut c_void = null_mut();
        let hr = (elem.vtbl().GetCurrentPattern)(elem.as_ptr(), UIA_TEXT_PATTERN_ID, &mut pat_raw);
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

        // 1. Lấy toàn bộ từ bao quanh con trỏ (Word Range)
        let mut word_range_raw: *mut c_void = null_mut();
        let hr = (caret_range.vtbl().Clone)(caret_range.as_ptr(), &mut word_range_raw);
        if hr < 0 || word_range_raw.is_null() {
            return None;
        }
        let word_range = ComPtr::<IUIAutomationTextRangeVtbl>::new(word_range_raw)?;
        let _ = (word_range.vtbl().ExpandToEnclosingUnit)(word_range.as_ptr(), TEXT_UNIT_WORD);

        let mut bstr_full: *mut u16 = null_mut();
        let hr = (word_range.vtbl().GetText)(word_range.as_ptr(), -1, &mut bstr_full);
        let raw_full_text = if hr >= 0 {
            let s = bstr_to_string(bstr_full);
            SysFreeString(bstr_full);
            s
        } else {
            return None;
        };

        // 2. Lấy đoạn từ đầu từ tới con trỏ (Pre Range)
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

        // 3. Lấy đoạn từ con trỏ tới cuối từ (Post Range)
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

        // Chuẩn hóa từ: lọc bỏ khoảng trắng hoặc ký tự phân cách ở 2 đầu
        let full_clean = raw_full_text.trim();
        if full_clean.is_empty() {
            return None;
        }

        // Tách ra từ thực tế đang xét (bỏ khoảng trắng/dấu câu bám sau nếu có)
        let pre_clean = raw_pre_text.trim_start();
        let post_clean = raw_post_text.trim_end();

        Some(CaretWordContext {
            full_word: full_clean.to_string(),
            pre_word: pre_clean.to_string(),
            post_word: post_clean.to_string(),
        })
    }
}
