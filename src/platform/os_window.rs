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
    use macroquad::miniquad::native::apple::frameworks::{class, msg_send, sel, sel_impl, NSSize, ObjcId};

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

    /// Resize the client area itself. miniquad's request_new_screen_size
    /// sets the window frame (setFrame), title bar included, which leaves
    /// the client a title bar short.
    pub fn set_client_size(w: f64, h: f64) -> bool {
        let Some(window) = ns_window() else { return false };
        unsafe {
            let () = msg_send![window, setContentSize: NSSize { width: w, height: h }];
        }
        true
    }

    /// Whether the window is really active: still the key window and not
    /// minimized. miniquad 0.4 reports a window *move* as
    /// `window_minimized_event` (windowDidMove), the same event as a real
    /// focus loss (windowDidResignKey), so callers check this to tell them
    /// apart.
    pub fn window_is_active() -> Option<bool> {
        let window = ns_window()?;
        unsafe {
            let key: bool = msg_send![window, isKeyWindow];
            let minimized: bool = msg_send![window, isMiniaturized];
            Some(key && !minimized)
        }
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

/// Linux: miniquad exposes neither its X11 window nor its Wayland surface.
///
/// X11 (and XWayland): the pause is toggled by a key press, so at that
/// moment our window has the keyboard focus. A separate Xlib connection
/// (libX11 loaded at runtime, as miniquad does) asks for the focused window,
/// climbs to the top-level window whose WM_CLASS is "skifree" (set through
/// `Conf::platform.linux_wm_class`), and sets WM_NAME and _NET_WM_NAME on
/// it. The window is remembered for later calls.
///
/// Native Wayland: the title can only be set on miniquad's private
/// xdg_toplevel, so `set_title` returns false there (the class check fails)
/// and the game draws the notice in the window.
#[cfg(target_os = "linux")]
mod imp {
    use std::cell::RefCell;
    use std::ffi::{c_char, c_int, c_long, c_uchar, c_uint, c_ulong, c_void, CStr, CString};

    type Display = c_void;
    type Window = c_ulong;
    type Atom = c_ulong;

    #[repr(C)]
    struct XClassHint {
        res_name: *mut c_char,
        res_class: *mut c_char,
    }

    /// The WM_CLASS the window is created with (platform::window).
    pub const WM_CLASS: &str = "skifree";

    struct X11 {
        display: *mut Display,
        window: Option<Window>,
        get_input_focus: unsafe extern "C" fn(*mut Display, *mut Window, *mut c_int) -> c_int,
        query_tree: unsafe extern "C" fn(
            *mut Display,
            Window,
            *mut Window,
            *mut Window,
            *mut *mut Window,
            *mut c_uint,
        ) -> c_int,
        get_class_hint: unsafe extern "C" fn(*mut Display, Window, *mut XClassHint) -> c_int,
        store_name: unsafe extern "C" fn(*mut Display, Window, *const c_char) -> c_int,
        intern_atom: unsafe extern "C" fn(*mut Display, *const c_char, c_int) -> Atom,
        change_property:
            unsafe extern "C" fn(*mut Display, Window, Atom, Atom, c_int, c_int, *const c_uchar, c_int) -> c_int,
        get_window_property: unsafe extern "C" fn(
            *mut Display,
            Window,
            Atom,
            c_long,
            c_long,
            c_int,
            Atom,
            *mut Atom,
            *mut c_int,
            *mut c_ulong,
            *mut c_ulong,
            *mut *mut c_uchar,
        ) -> c_int,
        free: unsafe extern "C" fn(*mut c_void) -> c_int,
        flush: unsafe extern "C" fn(*mut Display) -> c_int,
    }

    thread_local! {
        static X: RefCell<Option<X11>> = RefCell::new(unsafe { X11::open() });
    }

    impl X11 {
        unsafe fn open() -> Option<X11> {
            if std::env::var_os("DISPLAY").is_none() {
                return None;
            }
            let lib = libc::dlopen(c"libX11.so.6".as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL);
            if lib.is_null() {
                return None;
            }
            macro_rules! sym {
                ($name:literal) => {{
                    let p = libc::dlsym(lib, concat!($name, "\0").as_ptr() as *const c_char);
                    if p.is_null() {
                        return None;
                    }
                    std::mem::transmute(p)
                }};
            }
            let open_display: unsafe extern "C" fn(*const c_char) -> *mut Display = sym!("XOpenDisplay");
            let display = open_display(std::ptr::null());
            if display.is_null() {
                return None;
            }
            Some(X11 {
                display,
                window: None,
                get_input_focus: sym!("XGetInputFocus"),
                query_tree: sym!("XQueryTree"),
                get_class_hint: sym!("XGetClassHint"),
                store_name: sym!("XStoreName"),
                intern_atom: sym!("XInternAtom"),
                change_property: sym!("XChangeProperty"),
                get_window_property: sym!("XGetWindowProperty"),
                free: sym!("XFree"),
                flush: sym!("XFlush"),
            })
        }

        unsafe fn has_our_class(&self, w: Window) -> bool {
            let mut hint = XClassHint { res_name: std::ptr::null_mut(), res_class: std::ptr::null_mut() };
            if (self.get_class_hint)(self.display, w, &mut hint) == 0 {
                return false;
            }
            let ours = !hint.res_class.is_null() && CStr::from_ptr(hint.res_class).to_bytes() == WM_CLASS.as_bytes();
            for p in [hint.res_name, hint.res_class] {
                if !p.is_null() {
                    (self.free)(p as *mut c_void);
                }
            }
            ours
        }

        /// Our top-level window: the focused window or one of its ancestors
        /// with our WM_CLASS.
        unsafe fn find_window(&mut self) -> Option<Window> {
            if self.window.is_some() {
                return self.window;
            }
            let (mut w, mut revert) = (0, 0);
            (self.get_input_focus)(self.display, &mut w, &mut revert);
            for _ in 0..16 {
                if w <= 1 {
                    return None; // None / PointerRoot
                }
                if self.has_our_class(w) {
                    self.window = Some(w);
                    return self.window;
                }
                let (mut root, mut parent, mut children, mut n) = (0, 0, std::ptr::null_mut(), 0);
                if (self.query_tree)(self.display, w, &mut root, &mut parent, &mut children, &mut n) == 0 {
                    return None;
                }
                if !children.is_null() {
                    (self.free)(children as *mut c_void);
                }
                if parent == 0 || parent == root {
                    return None;
                }
                w = parent;
            }
            None
        }

        unsafe fn atom(&self, name: &CStr) -> Atom {
            (self.intern_atom)(self.display, name.as_ptr(), 0)
        }
    }

    pub fn set_title(title: &str) -> bool {
        X.with(|x| {
            let mut x = x.borrow_mut();
            let Some(x) = x.as_mut() else { return false };
            unsafe {
                let Some(w) = x.find_window() else { return false };
                let Ok(c_title) = CString::new(title) else { return false };
                (x.store_name)(x.display, w, c_title.as_ptr());
                let (net_wm_name, utf8) = (x.atom(c"_NET_WM_NAME"), x.atom(c"UTF8_STRING"));
                (x.change_property)(x.display, w, net_wm_name, utf8, 8, 0, title.as_ptr(), title.len() as c_int);
                (x.flush)(x.display);
            }
            true
        })
    }

    /// False in fullscreen (_NET_WM_STATE_FULLSCREEN), where there is no
    /// title bar.
    pub fn title_visible() -> bool {
        X.with(|x| {
            let mut x = x.borrow_mut();
            let Some(x) = x.as_mut() else { return false };
            unsafe {
                let Some(w) = x.find_window() else { return false };
                let (state, fullscreen) = (x.atom(c"_NET_WM_STATE"), x.atom(c"_NET_WM_STATE_FULLSCREEN"));
                let (mut ty, mut format, mut n, mut after, mut data) = (0, 0, 0, 0, std::ptr::null_mut());
                const XA_ATOM: Atom = 4;
                if (x.get_window_property)(
                    x.display,
                    w,
                    state,
                    0,
                    64,
                    0,
                    XA_ATOM,
                    &mut ty,
                    &mut format,
                    &mut n,
                    &mut after,
                    &mut data,
                ) != 0
                {
                    return true;
                }
                let mut is_fullscreen = false;
                if !data.is_null() {
                    let atoms = std::slice::from_raw_parts(data as *const Atom, n as usize);
                    is_fullscreen = atoms.contains(&fullscreen);
                    (x.free)(data as *mut c_void);
                }
                !is_fullscreen
            }
        })
    }

    pub fn message_box(_caption: &str, _text: &str) -> bool {
        false
    }
}

/// Web: the page title (`document.title`), which a tab shows unless the
/// page is fullscreen. No native message box: the game draws its own.
#[cfg(target_arch = "wasm32")]
mod imp {
    use crate::platform::web;

    pub fn set_title(title: &str) -> bool {
        web::set_title(title);
        true
    }

    pub fn title_visible() -> bool {
        !web::is_fullscreen()
    }

    pub fn message_box(_caption: &str, _text: &str) -> bool {
        false
    }
}

/// Other platforms: no native title or message box.
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux", target_arch = "wasm32")))]
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

/// Whether the window is active (key and not minimized), where the
/// platform can tell; None elsewhere.
pub fn window_is_active() -> Option<bool> {
    #[cfg(target_os = "macos")]
    return imp::window_is_active();
    #[cfg(not(target_os = "macos"))]
    None
}

/// Resize the client area to w x h; false where the platform has no
/// direct way (callers then use request_new_screen_size, which sets the
/// client size there).
pub fn set_client_size(w: f64, h: f64) -> bool {
    #[cfg(target_os = "macos")]
    return imp::set_client_size(w, h);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (w, h);
        false
    }
}
