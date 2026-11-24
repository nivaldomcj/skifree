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
//! `MessageBoxA(main window, text, "High Scores", MB_OK)` (`FUN_00402ec0`),
//! a modal box that blocks the game until OK. `message_box` shows the
//! platform's own modal box and returns false where there is none (the
//! game then draws one in the window).

#[cfg(target_os = "macos")]
mod imp {
    use macroquad::miniquad::native::apple::apple_util::str_to_nsstring;
    use macroquad::miniquad::native::apple::frameworks::{class, msg_send, sel, sel_impl, NSSize, ObjcId};

    /// `NSWindowStyleMaskFullScreen`.
    const FULL_SCREEN: u64 = 1 << 14;

    fn ns_window() -> Option<ObjcId> {
        // SAFETY: miniquad's view is an NSView (null before the window
        // exists, checked); `window` is an NSView method.
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
        // SAFETY: `window` is the live NSWindow from `ns_window`.
        unsafe {
            let () = msg_send![window, setTitle: str_to_nsstring(title)];
        }
        true
    }

    /// Resize the client area itself. miniquad's `request_new_screen_size`
    /// sets the window frame (setFrame), title bar included, which leaves
    /// the client a title bar short.
    pub fn set_client_size(w: f64, h: f64) -> bool {
        let Some(window) = ns_window() else { return false };
        // SAFETY: `window` is the live NSWindow from `ns_window`.
        unsafe {
            let () = msg_send![window, setContentSize: NSSize { width: w, height: h }];
        }
        true
    }

    /// The smallest client area the user can drag the window to (like the
    /// original's `WM_GETMINMAXINFO`), in points: client pixels with
    /// `high_dpi` off.
    pub fn set_min_client_size(w: f64, h: f64) -> bool {
        let Some(window) = ns_window() else { return false };
        // SAFETY: `window` is the live NSWindow from `ns_window`.
        unsafe {
            let () = msg_send![window, setContentMinSize: NSSize { width: w, height: h }];
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
        // SAFETY: `window` is the live NSWindow from `ns_window`.
        unsafe {
            let key: bool = msg_send![window, isKeyWindow];
            let minimized: bool = msg_send![window, isMiniaturized];
            Some(key && !minimized)
        }
    }

    pub fn title_visible() -> bool {
        let Some(window) = ns_window() else { return false };
        // SAFETY: `window` is the live NSWindow from `ns_window`.
        let mask: u64 = unsafe { msg_send![window, styleMask] };
        mask & FULL_SCREEN == 0
    }

    /// `NSAlert`, run modally. miniquad's frame timer only runs in the
    /// event-tracking run loop mode, so the game isn't re-entered while the
    /// alert's modal loop runs.
    pub fn message_box(caption: &str, text: &str) -> bool {
        // SAFETY: each message goes to the class or object that defines it,
        // with nulls checked; the alert is released once, after runModal.
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
        fn GetClassNameW(hwnd: Hwnd, name: *mut u16, len: i32) -> i32;
        fn GetGUIThreadInfo(thread: u32, info: *mut GuiThreadInfo) -> i32;
        fn SetWindowTextW(hwnd: Hwnd, text: *const u16) -> i32;
        fn GetWindowLongW(hwnd: Hwnd, index: i32) -> i32;
        fn MessageBoxW(hwnd: Hwnd, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThreadId() -> u32;
    }

    /// miniquad's game window class. The same thread also owns miniquad's
    /// hidden top-level helper window (class `MINIQUADMSGWND`), so the first
    /// top-level window is not necessarily the game's.
    const GAME_CLASS: &str = "MINIQUADAPP";

    /// `EnumThreadWindows` callback: stores the top-level window of class
    /// `GAME_CLASS` in `*out` (the `Hwnd` `hwnd()` passes) and stops.
    extern "system" fn game_window(hwnd: Hwnd, out: isize) -> i32 {
        let mut class = [0u16; 32];
        // SAFETY: `hwnd` comes from EnumThreadWindows, and GetClassNameW
        // writes at most `class.len()` units, NUL included.
        let len = unsafe { GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32) };
        let ours = String::from_utf16_lossy(&class[..len.max(0) as usize]) == GAME_CLASS;
        // SAFETY: as above.
        if ours && unsafe { GetParent(hwnd) }.is_null() {
            // SAFETY: `out` is the address of `hwnd()`'s local, which
            // outlives the synchronous EnumThreadWindows call.
            unsafe { *(out as *mut Hwnd) = hwnd };
            return 0;
        }
        1
    }

    /// The game's window: this (the main) thread's top-level game window.
    fn hwnd() -> Option<Hwnd> {
        let mut hwnd: Hwnd = std::ptr::null_mut();
        // SAFETY: the callback writes only through the pointer to `hwnd`,
        // and only during this call.
        unsafe {
            EnumThreadWindows(GetCurrentThreadId(), game_window, &raw mut hwnd as isize);
        }
        (!hwnd.is_null()).then_some(hwnd)
    }

    pub fn set_title(title: &str) -> bool {
        let Some(hwnd) = hwnd() else { return false };
        let title = wide(title);
        // SAFETY: `title` is NUL-terminated UTF-16 that outlives the call.
        unsafe { SetWindowTextW(hwnd, title.as_ptr()) != 0 }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// `GUITHREADINFO`.
    #[repr(C)]
    struct GuiThreadInfo {
        cb_size: u32,
        flags: u32,
        hwnds: [Hwnd; 6],
        rc_caret: [i32; 4],
    }
    const GUI_INMOVESIZE: u32 = 0x2;
    const GUI_INMENUMODE: u32 = 0x4;

    /// Whether this thread is in a modal move/size or menu loop. miniquad
    /// then draws frames from a `WM_TIMER`, and a `MessageBoxW` opened in
    /// such a frame would dispatch that timer and re-enter the frame loop
    /// while it is still running.
    fn in_modal_loop() -> bool {
        let mut info = GuiThreadInfo {
            cb_size: std::mem::size_of::<GuiThreadInfo>() as u32,
            flags: 0,
            hwnds: [std::ptr::null_mut(); 6],
            rc_caret: [0; 4],
        };
        // SAFETY: `info` is a GUITHREADINFO with cbSize set, live for the
        // call; the thread is ours.
        let ok = unsafe { GetGUIThreadInfo(GetCurrentThreadId(), &raw mut info) } != 0;
        !ok || info.flags & (GUI_INMOVESIZE | GUI_INMENUMODE) != 0
    }

    /// The same call as the original: MessageBox(main window, ..., `MB_OK`).
    /// Returns false inside a move/size or menu loop (see `in_modal_loop`):
    /// the game then draws its own box, which doesn't block.
    pub fn message_box(caption: &str, text: &str) -> bool {
        if in_modal_loop() {
            return false;
        }
        let owner = hwnd().unwrap_or(std::ptr::null_mut());
        let (text, caption) = (wide(text), wide(caption));
        // SAFETY: both strings are NUL-terminated UTF-16 that outlive the
        // call; a null owner is allowed.
        unsafe { MessageBoxW(owner, text.as_ptr(), caption.as_ptr(), 0) != 0 }
    }

    /// miniquad's fullscreen drops the caption (title bar) style.
    pub fn title_visible() -> bool {
        let Some(hwnd) = hwnd() else { return false };
        // SAFETY: reads a style word of our own window.
        let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
        style & WS_CAPTION == WS_CAPTION
    }
}

/// Linux: miniquad exposes neither its X11 window nor its Wayland surface.
///
/// X11 (and XWayland): the pause is toggled by a key press, so at that
/// moment our window has the keyboard focus. A separate Xlib connection
/// (libX11 loaded at runtime, as miniquad does) asks for the focused window,
/// climbs to the top-level window whose `WM_CLASS` is "skifree" (set through
/// `Conf::platform.linux_wm_class`), and sets `WM_NAME` and `_NET_WM_NAME` on
/// it. The window is remembered for later calls.
///
/// Native Wayland: the title can only be set on miniquad's private
/// `xdg_toplevel`, so `set_title` returns false there (the class check fails)
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

    /// The `WM_CLASS` the window is created with (`platform::window`).
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

    // One Xlib connection per thread (Xlib is not thread-safe); only the
    // main thread uses it.
    thread_local! {
        static X: RefCell<Option<X11>> = RefCell::new(X11::open());
    }

    impl X11 {
        /// A connection to the X server, or None without one or without libX11.
        #[expect(
            clippy::missing_transmute_annotations,
            reason = "each sym! lands in a field or binding with an explicit fn-pointer type"
        )]
        fn open() -> Option<X11> {
            // No X server: Wayland only, or headless.
            std::env::var_os("DISPLAY")?;
            // SAFETY: dlopen and dlsym take NUL-terminated names. Each symbol
            // is checked for null, then transmuted to its binding's or
            // field's type, its prototype in Xlib.h. The library and the
            // display are never closed, so both stay valid; XOpenDisplay's
            // null name means $DISPLAY.
            unsafe {
                let lib = libc::dlopen(c"libX11.so.6".as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL);
                if lib.is_null() {
                    return None;
                }
                macro_rules! sym {
                    ($name:literal) => {{
                        let p = libc::dlsym(lib, concat!($name, "\0").as_ptr().cast::<c_char>());
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
        }

        // The methods below call Xlib on `self.display`, the connection
        // `open` made, through the prototypes `open` checked; Xlib allocates
        // what it returns, and they free it with XFree.

        fn has_our_class(&self, w: Window) -> bool {
            // SAFETY: see above; the hint's strings are null or Xlib's.
            unsafe {
                let mut hint = XClassHint { res_name: std::ptr::null_mut(), res_class: std::ptr::null_mut() };
                if (self.get_class_hint)(self.display, w, &raw mut hint) == 0 {
                    return false;
                }
                let ours =
                    !hint.res_class.is_null() && CStr::from_ptr(hint.res_class).to_bytes() == WM_CLASS.as_bytes();
                for p in [hint.res_name, hint.res_class] {
                    if !p.is_null() {
                        (self.free)(p.cast::<c_void>());
                    }
                }
                ours
            }
        }

        /// Our top-level window: the focused window or one of its ancestors
        /// with our `WM_CLASS`.
        fn find_window(&mut self) -> Option<Window> {
            if self.window.is_some() {
                return self.window;
            }
            // SAFETY: see above; the out-pointers are live locals.
            unsafe {
                let (mut w, mut revert) = (0, 0);
                (self.get_input_focus)(self.display, &raw mut w, &raw mut revert);
                for _ in 0..16 {
                    if w <= 1 {
                        return None; // None / PointerRoot
                    }
                    if self.has_our_class(w) {
                        self.window = Some(w);
                        return self.window;
                    }
                    let (mut root, mut parent, mut children, mut n) = (0, 0, std::ptr::null_mut(), 0);
                    if (self.query_tree)(self.display, w, &raw mut root, &raw mut parent, &raw mut children, &raw mut n)
                        == 0
                    {
                        return None;
                    }
                    if !children.is_null() {
                        (self.free)(children.cast::<c_void>());
                    }
                    if parent == 0 || parent == root {
                        return None;
                    }
                    w = parent;
                }
                None
            }
        }

        fn atom(&self, name: &CStr) -> Atom {
            // SAFETY: see above; `name` is NUL-terminated.
            unsafe { (self.intern_atom)(self.display, name.as_ptr(), 0) }
        }
    }

    pub fn set_title(title: &str) -> bool {
        X.with(|x| {
            let mut x = x.borrow_mut();
            let Some(x) = x.as_mut() else { return false };
            let Some(w) = x.find_window() else { return false };
            let Ok(c_title) = CString::new(title) else { return false };
            let (net_wm_name, utf8) = (x.atom(c"_NET_WM_NAME"), x.atom(c"UTF8_STRING"));
            // SAFETY: Xlib calls on the open display (see `X11`); `c_title`
            // is NUL-terminated and `title` is `title.len()` bytes of UTF-8,
            // both alive for the calls.
            unsafe {
                (x.store_name)(x.display, w, c_title.as_ptr());
                (x.change_property)(x.display, w, net_wm_name, utf8, 8, 0, title.as_ptr(), title.len() as c_int);
                (x.flush)(x.display);
            }
            true
        })
    }

    /// False in fullscreen (`_NET_WM_STATE_FULLSCREEN`), where there is no
    /// title bar.
    pub fn title_visible() -> bool {
        const XA_ATOM: Atom = 4;
        X.with(|x| {
            let mut x = x.borrow_mut();
            let Some(x) = x.as_mut() else { return false };
            let Some(w) = x.find_window() else { return false };
            let (state, fullscreen) = (x.atom(c"_NET_WM_STATE"), x.atom(c"_NET_WM_STATE_FULLSCREEN"));
            let (mut ty, mut format, mut n, mut after, mut data) = (0, 0, 0, 0, std::ptr::null_mut());
            // SAFETY: Xlib calls on the open display (see `X11`), with live
            // out-pointers. An ATOM property comes back as `n` C longs (Xlib
            // stores format-32 data as long), malloc-aligned, freed once.
            unsafe {
                if (x.get_window_property)(
                    x.display,
                    w,
                    state,
                    0,
                    64,
                    0,
                    XA_ATOM,
                    &raw mut ty,
                    &raw mut format,
                    &raw mut n,
                    &raw mut after,
                    &raw mut data,
                ) != 0
                {
                    return true;
                }
                let mut is_fullscreen = false;
                // Only an ATOM list in format 32 holds `n` longs: another
                // client can set the property with format 8 or 16, which
                // Xlib returns as `n` bytes or shorts.
                if !data.is_null() && ty == XA_ATOM && format == 32 {
                    #[expect(clippy::cast_ptr_alignment, reason = "Xlib's buffer is malloc'd, aligned for long")]
                    let atoms = std::slice::from_raw_parts(data as *const Atom, n as usize);
                    is_fullscreen = atoms.contains(&fullscreen);
                }
                if !data.is_null() {
                    (x.free)(data.cast::<c_void>());
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

/// Stop the user's resize at a w x h client area; false where the platform
/// has no direct way (the game then bounces the window back to it).
pub fn set_min_client_size(w: f64, h: f64) -> bool {
    #[cfg(target_os = "macos")]
    return imp::set_min_client_size(w, h);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (w, h);
        false
    }
}

/// Resize the client area to w x h; false where the platform has no
/// direct way (callers then use `request_new_screen_size`, which sets the
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
