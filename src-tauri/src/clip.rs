// The computer's clipboard, for the phone (mobile.rs): its text read when
// it changed (Windows counts its changes), and text from the phone put in.
// What a password manager marks as not for others (Windows' own
// "ExcludeClipboardContentFromMonitorProcessing", or "CanIncludeInClipboardHistory"
// set to 0, as clipboard history honours) is never read for the phone.

// The clipboard's change count alone (cheap: nothing opened).
#[cfg(windows)]
pub fn seq() -> u32 {
    #[link(name = "user32")]
    extern "system" {
        fn GetClipboardSequenceNumber() -> u32;
    }
    // SAFETY: a read of a number.
    unsafe { GetClipboardSequenceNumber() }
}

#[cfg(not(windows))]
pub fn seq() -> u32 {
    0
}

// The clipboard's change count, its text if it has any (and is not marked
// private), at most so many characters.
#[cfg(windows)]
pub fn read(most: usize) -> (u32, Option<String>) {
    use std::ffi::c_void;
    #[link(name = "user32")]
    extern "system" {
        fn GetClipboardSequenceNumber() -> u32;
        fn OpenClipboard(hwnd: isize) -> i32;
        fn CloseClipboard() -> i32;
        fn GetClipboardData(format: u32) -> *mut c_void;
        fn RegisterClipboardFormatW(name: *const u16) -> u32;
        fn IsClipboardFormatAvailable(format: u32) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalLock(mem: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(mem: *mut c_void) -> i32;
        fn GlobalSize(mem: *mut c_void) -> usize;
    }
    const CF_UNICODETEXT: u32 = 13;
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    // SAFETY: the clipboard opened and closed here; its memory read only
    // while locked, no further than its size.
    unsafe {
        let seq = GetClipboardSequenceNumber();
        if OpenClipboard(0) == 0 {
            return (seq, None);
        }
        let exclude = RegisterClipboardFormatW(wide("ExcludeClipboardContentFromMonitorProcessing").as_ptr());
        let history = RegisterClipboardFormatW(wide("CanIncludeInClipboardHistory").as_ptr());
        let mut private = IsClipboardFormatAvailable(exclude) != 0;
        if !private && IsClipboardFormatAvailable(history) != 0 {
            let h = GetClipboardData(history);
            let p = if h.is_null() { std::ptr::null_mut() } else { GlobalLock(h) };
            if !p.is_null() {
                private = GlobalSize(h) >= 4 && *(p as *const u32) == 0;
                GlobalUnlock(h);
            }
        }
        let mut text = None;
        if !private && IsClipboardFormatAvailable(CF_UNICODETEXT) != 0 {
            let h = GetClipboardData(CF_UNICODETEXT);
            let p = if h.is_null() { std::ptr::null_mut() } else { GlobalLock(h) };
            if !p.is_null() {
                let cap = GlobalSize(h) / 2;
                let units = std::slice::from_raw_parts(p as *const u16, cap);
                let end = units.iter().position(|&u| u == 0).unwrap_or(cap);
                let s = String::from_utf16_lossy(&units[..end]);
                if !s.trim().is_empty() && s.chars().count() <= most {
                    text = Some(s);
                }
                GlobalUnlock(h);
            }
        }
        CloseClipboard();
        (seq, text)
    }
}

// Text put on the clipboard; its change count after, or none.
#[cfg(windows)]
pub fn write(text: &str) -> Option<u32> {
    use std::ffi::c_void;
    #[link(name = "user32")]
    extern "system" {
        fn GetClipboardSequenceNumber() -> u32;
        fn OpenClipboard(hwnd: isize) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(format: u32, mem: *mut c_void) -> *mut c_void;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
        fn GlobalLock(mem: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(mem: *mut c_void) -> i32;
        fn GlobalFree(mem: *mut c_void) -> *mut c_void;
    }
    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 0x0002;
    let units: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    // SAFETY: memory of ours, filled while locked, handed to the clipboard
    // (which owns it once set; freed here only if it was not taken).
    unsafe {
        if OpenClipboard(0) == 0 {
            return None;
        }
        EmptyClipboard();
        let h = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2);
        let mut ok = false;
        if !h.is_null() {
            let p = GlobalLock(h) as *mut u16;
            if !p.is_null() {
                std::ptr::copy_nonoverlapping(units.as_ptr(), p, units.len());
                GlobalUnlock(h);
                ok = !SetClipboardData(CF_UNICODETEXT, h).is_null();
            }
            if !ok {
                GlobalFree(h);
            }
        }
        CloseClipboard();
        ok.then(|| GetClipboardSequenceNumber())
    }
}

#[cfg(not(windows))]
pub fn read(_most: usize) -> (u32, Option<String>) {
    (0, None)
}

#[cfg(not(windows))]
pub fn write(_text: &str) -> Option<u32> {
    None
}
