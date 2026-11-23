//! Native window calls miniquad 0.4 doesn't offer: the runtime title and
//! the high-score message box.
//!
//! Title: the original shows its pause notice in the title
//! bar: F3 calls `SetWindowTextA` with string 2, "Ski Paused ... Press F3
//! to continue", and resuming puts back string 1, "SkiFree"
//! (`FUN_00405760`). miniquad 0.4 sets the title only at window creation,
//! so this talks to the platform directly.
//!
//! `set_title` returns whether the title could be set; `title_visible`
//! whether a title bar is on screen (not in fullscreen). Where either is
//! false the game draws the notice in the window instead.
//!
//! Message box: after a course finish the original calls
//! `MessageBoxA(main window, text, "High Scores", MB_OK)` (FUN_00402ec0),
//! a modal box that blocks the game until OK. `message_box` shows the
//! platform's own modal box and returns false where there is none (the
//! game then draws one in the window).

#[cfg(target_os = "macos")]
mod imp {
    use macroquad::miniquad::native::apple::apple_util::str_to_nsstring;
    use macroquad::miniquad::native::apple::frameworks::{class, msg_send, sel, sel_impl, ObjcId};

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

    /// NSAlert, run modally. miniquad's frame timer only runs in the
    /// event-tracking run loop mode, so the game isn't re-entered while the
    /// alert's modal loop runs.
    pub fn message_box(caption: &str, text: &str) -> bool {
        unsafe {
            let alert: ObjcId = msg_send![class!(NSAlert), new];
            if alert.is_null() {
                return false;
            }
            // The app's icon (the SkiFree face, or the .app bundle's).
            let app: ObjcId = msg_send![class!(NSApplication), sharedApplication];
            let icon: ObjcId = msg_send![app, applicationIconImage];
            if !icon.is_null() {
                let () = msg_send![alert, setIcon: icon];
            }
            let () = msg_send![alert, setMessageText: str_to_nsstring(caption)];
            let () = msg_send![alert, setInformativeText: str_to_nsstring(text)];
            let _: ObjcId = msg_send![alert, addButtonWithTitle: str_to_nsstring("OK")];
            let _: isize = msg_send![alert, runModal];
            let () = msg_send![alert, release];
        }
        true
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
        fn MessageBoxW(hwnd: Hwnd, text: *const u16, caption: *const u16, kind: u32) -> i32;
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
        let title = wide(title);
        unsafe { SetWindowTextW(hwnd, title.as_ptr()) != 0 }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// The same call as the original: MessageBox(main window, ..., MB_OK).
    pub fn message_box(caption: &str, text: &str) -> bool {
        let owner = hwnd().unwrap_or(std::ptr::null_mut());
        let (text, caption) = (wide(text), wide(caption));
        unsafe { MessageBoxW(owner, text.as_ptr(), caption.as_ptr(), 0) != 0 }
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

    pub fn message_box(_caption: &str, _text: &str) -> bool {
        false
    }
}

pub use imp::{message_box, set_title, title_visible};
