//! Runtime window title. The original shows its pause notice in the title
//! bar: F3 calls `SetWindowTextA` with string 2, "Ski Paused ... Press F3
//! to continue", and resuming puts back string 1, "SkiFree"
//! (`FUN_00405760`). miniquad 0.4 sets the title only at window creation,
//! so this talks to the platform directly.
//!
//! `set_title` returns whether the title could be set; `title_visible`
//! whether a title bar is on screen (not in fullscreen). Where either is
//! false the game draws the notice in the window instead.

#[cfg(target_os = "macos")]
mod imp {
    use macroquad::miniquad::native::apple::apple_util::str_to_nsstring;
    use macroquad::miniquad::native::apple::frameworks::{msg_send, sel, sel_impl, ObjcId};

    /// NSWindowStyleMaskFullScreen.
    const FULL_SCREEN: u64 = 1 << 14;

    fn ns_window() -> Option<ObjcId> {
        unsafe {
            let view = macroquad::miniquad::window::apple_view();
            if view.is_null() {
                return None;
            }
            let window: ObjcId = msg_send![view, window];
            (!window.is_null()).then_some(window)
        }
    }

    pub fn set_title(title: &str) -> bool {
        let Some(window) = ns_window() else { return false };
        unsafe {
            let () = msg_send![window, setTitle: str_to_nsstring(title)];
        }
        true
    }

    pub fn title_visible() -> bool {
        let Some(window) = ns_window() else { return false };
        let mask: u64 = unsafe { msg_send![window, styleMask] };
        mask & FULL_SCREEN == 0
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use std::ffi::c_void;
    type Hwnd = *mut c_void;

    const GWL_STYLE: i32 = -16;
    const WS_CAPTION: u32 = 0x00C0_0000;

    #[link(name = "user32")]
    extern "system" {
        fn EnumThreadWindows(thread: u32, cb: extern "system" fn(Hwnd, isize) -> i32, param: isize) -> i32;
        fn GetParent(hwnd: Hwnd) -> Hwnd;
        fn SetWindowTextW(hwnd: Hwnd, text: *const u16) -> i32;
        fn GetWindowLongW(hwnd: Hwnd, index: i32) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThreadId() -> u32;
    }

    extern "system" fn first_top_level(hwnd: Hwnd, out: isize) -> i32 {
        unsafe {
            if GetParent(hwnd).is_null() {
                *(out as *mut Hwnd) = hwnd;
                return 0;
            }
        }
        1
    }

    /// The game's window: the top-level window of this (the main) thread.
    fn hwnd() -> Option<Hwnd> {
        let mut hwnd: Hwnd = std::ptr::null_mut();
        unsafe {
            EnumThreadWindows(GetCurrentThreadId(), first_top_level, &mut hwnd as *mut Hwnd as isize);
        }
        (!hwnd.is_null()).then_some(hwnd)
    }

    pub fn set_title(title: &str) -> bool {
        let Some(hwnd) = hwnd() else { return false };
        let wide: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
        unsafe { SetWindowTextW(hwnd, wide.as_ptr()) != 0 }
    }

    /// miniquad's fullscreen drops the caption (title bar) style.
    pub fn title_visible() -> bool {
        let Some(hwnd) = hwnd() else { return false };
        let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
        style & WS_CAPTION == WS_CAPTION
    }
}

/// Linux (X11/Wayland) and others: miniquad doesn't expose the window, so
/// the notice is drawn in the window. See TODO.md (Linux test, upstream
/// `set_window_title`).
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    pub fn set_title(_title: &str) -> bool {
        false
    }

    pub fn title_visible() -> bool {
        false
    }
}

pub use imp::{set_title, title_visible};
