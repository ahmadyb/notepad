#[cfg(windows)]
use super::ffi;
#[cfg(windows)]
use super::wrapper::DirectEditor;
use std::ffi::c_void;

#[derive(Debug)]
pub enum EmbedError {
    UnsupportedPlatform,
    NativeCallFailed(String),
}

impl std::fmt::Display for EmbedError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPlatform => {
                formatter.write_str("Scintilla embedding unavailable on this backend")
            }
            Self::NativeCallFailed(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for EmbedError {}

#[cfg(windows)]
pub struct ScintillaHost {
    hwnd: *mut c_void,
    direct: DirectEditor,
}

#[cfg(not(windows))]
pub struct ScintillaHost;

#[cfg(windows)]
impl ScintillaHost {
    /// Attach a Scintilla child window to the supplied Win32 parent window.
    ///
    /// # Safety
    /// `parent` must be a valid live Win32 window handle owned by the current UI thread.
    pub unsafe fn attach(
        parent: *mut c_void,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<Self, EmbedError> {
        let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        if instance.is_null() {
            return Err(EmbedError::NativeCallFailed(
                "GetModuleHandleW failed".into(),
            ));
        }
        if !unsafe { Scintilla_RegisterClasses(instance) } {
            return Err(EmbedError::NativeCallFailed(
                "Scintilla_RegisterClasses failed".into(),
            ));
        }
        let class = wide("Scintilla");
        let title = wide("");
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
                x,
                y,
                width,
                height,
                parent,
                std::ptr::null_mut(),
                instance,
                std::ptr::null_mut(),
            )
        };
        if hwnd.is_null() {
            return Err(EmbedError::NativeCallFailed(
                "CreateWindowExW failed".into(),
            ));
        }
        let function = unsafe { SendMessageW(hwnd, ffi::SCI_GETDIRECTFUNCTION, 0, 0) };
        let pointer = unsafe { SendMessageW(hwnd, ffi::SCI_GETDIRECTPOINTER, 0, 0) };
        let direct = unsafe { DirectEditor::from_raw(function, pointer) }.ok_or_else(|| {
            EmbedError::NativeCallFailed("Scintilla direct calls unavailable".into())
        })?;
        Ok(Self { hwnd, direct })
    }

    pub fn direct(&self) -> &DirectEditor {
        &self.direct
    }

    pub fn resize(&self, x: i32, y: i32, width: i32, height: i32) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                x,
                y,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    pub fn focus(&self) {
        unsafe {
            SetFocus(self.hwnd);
        }
    }
}

#[cfg(windows)]
impl Drop for ScintillaHost {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.hwnd);
            Scintilla_ReleaseResources();
        }
    }
}

#[cfg(not(windows))]
impl ScintillaHost {
    pub fn attach(
        _: *mut c_void,
        _: i32,
        _: i32,
        _: i32,
        _: i32,
    ) -> Result<Self, EmbedError> {
        Err(EmbedError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
const WS_CHILD: u32 = 0x4000_0000;
#[cfg(windows)]
const WS_VISIBLE: u32 = 0x1000_0000;
#[cfg(windows)]
const WS_CLIPSIBLINGS: u32 = 0x0400_0000;
#[cfg(windows)]
const WS_CLIPCHILDREN: u32 = 0x0200_0000;
#[cfg(windows)]
const SWP_NOZORDER: u32 = 0x0004;
#[cfg(windows)]
const SWP_NOACTIVATE: u32 = 0x0010;

#[cfg(windows)]
extern "system" {
    fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: *mut c_void,
        menu: *mut c_void,
        instance: *mut c_void,
        parameter: *mut c_void,
    ) -> *mut c_void;
    fn SendMessageW(window: *mut c_void, message: u32, wparam: usize, lparam: isize) -> isize;
    fn SetWindowPos(
        window: *mut c_void,
        insert_after: *mut c_void,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
    fn DestroyWindow(window: *mut c_void) -> i32;
    fn GetModuleHandleW(module_name: *const u16) -> *mut c_void;
    fn SetFocus(window: *mut c_void) -> *mut c_void;
}

#[cfg(windows)]
extern "C" {
    fn Scintilla_RegisterClasses(instance: *mut c_void) -> bool;
    fn Scintilla_ReleaseResources() -> bool;
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
