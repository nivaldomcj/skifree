//! Linux: a shared library loaded at runtime, as miniquad loads its own.
//! The window title uses libX11 (`os_window`), sound uses libasound
//! (`audio`); without either library the game still runs.

use std::ffi::{c_void, CStr};

/// A library opened with `dlopen`. It is never closed, so the symbols taken
/// from it stay valid for the life of the process.
pub struct Library(*mut c_void);

impl Library {
    /// `dlopen(name)`, or None without the library.
    pub fn open(name: &CStr) -> Option<Self> {
        // SAFETY: `name` is NUL-terminated; dlopen returns null on failure.
        let handle = unsafe { libc::dlopen(name.as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL) };
        (!handle.is_null()).then_some(Self(handle))
    }

    /// The function `name` as `F`, or None when the library lacks it.
    ///
    /// # Safety
    ///
    /// `F` must be an `unsafe extern "C" fn` type matching the symbol's C
    /// prototype.
    pub unsafe fn sym<F: Copy>(&self, name: &CStr) -> Option<F> {
        const { assert!(size_of::<F>() == size_of::<*mut c_void>(), "F must be a function pointer") };
        // SAFETY: `self.0` is a live handle and `name` is NUL-terminated.
        let p = unsafe { libc::dlsym(self.0, name.as_ptr()) };
        // SAFETY: non-null, and F is a fn pointer of the symbol's prototype
        // (the caller's contract), the same size as the pointer.
        (!p.is_null()).then(|| unsafe { std::mem::transmute_copy::<*mut c_void, F>(&p) })
    }
}
