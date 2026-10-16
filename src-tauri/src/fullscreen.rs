// Whether another app is full screen, so she can step aside. Windows only;
// elsewhere it never reports one.
//
// Full screen: the foreground window covers its whole monitor and has no title
// bar. A maximised window has one (and stops at the taskbar), so it is not.
// And only on a display she is on (on: their rects; none given, any): a
// video full screen on the second display is none of hers (the user,
// 2026-10-13). Looked at every 1.5 s (main.rs).
//
// A League of Legends game on (lol.rs) is not full screen to her: her pets
// stay out over it (the game in its borderless mode: README).

use std::sync::atomic::{AtomicBool, Ordering};

static GAME: AtomicBool = AtomicBool::new(false);

pub fn set_game(on: bool) {
    GAME.store(on, Ordering::SeqCst);
}

fn is_game(program: &str) -> bool {
    GAME.load(Ordering::SeqCst) && program == "league of legends.exe"
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    type Hwnd = *mut c_void;

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> Hwnd;
        fn IsWindowVisible(hwnd: Hwnd) -> i32;
        fn GetClassNameW(hwnd: Hwnd, name: *mut u16, max: i32) -> i32;
        fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
        fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
        fn MonitorFromWindow(hwnd: Hwnd, flags: u32) -> *mut c_void;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
        fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn CloseHandle(handle: isize) -> i32;
        fn QueryFullProcessImageNameW(process: isize, flags: u32, name: *mut u16, size: *mut u32) -> i32;
    }

    const GWL_EXSTYLE: i32 = -20;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    // The desktop, the taskbars, and task view and Alt+Tab (they cover the screen).
    const SKIP_CLASSES: [&str; 5] = ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd", "XamlExplorerHostIslandWindow"];
    // Windows' own, which cover the screen a moment as they open: search
    // (SearchHost's CoreWindow, some 60 ms: the taskbar flashed, #5), Start,
    // the quick settings and notifications.
    const SKIP_PROGRAMS: [&str; 3] = ["searchhost.exe", "startmenuexperiencehost.exe", "shellexperiencehost.exe"];

    // The file name of the program a window is of, lower case.
    fn program_of(hwnd: Hwnd) -> String {
        let mut pid = 0u32;
        // SAFETY: our own out-parameter, buffer and size; the handle closed.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process == 0 {
                return String::new();
            }
            let mut path = [0u16; 512];
            let mut size = 512u32;
            let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut size) != 0;
            CloseHandle(process);
            if !ok {
                return String::new();
            }
            let path = String::from_utf16_lossy(&path[..size as usize]);
            path.rsplit('\\').next().unwrap_or(&path).to_lowercase()
        }
    }
    const GWL_STYLE: i32 = -16;
    const WS_CAPTION: isize = 0x00c0_0000;
    const MONITOR_DEFAULTTONEAREST: u32 = 2;

    // None: the foreground window is one of ours (keep what was decided).
    pub fn check(own: &[isize], on: &[(i32, i32, i32, i32)]) -> Option<bool> {
        covering(own).map(|c| c.is_some_and(|mon| on.is_empty() || on.contains(&mon)))
    }

    // The display an app full screen covers (x, y, w, h): Some(None) when
    // none does; None, the foreground window one of ours (keep what was
    // decided).
    pub fn covering(own: &[isize]) -> Option<Option<(i32, i32, i32, i32)>> {
        // SAFETY: plain user32 calls; the buffers and structs are ours and sized.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return Some(None);
            }
            if own.contains(&(hwnd as isize)) {
                return None;
            }
            if IsWindowVisible(hwnd) == 0 {
                return Some(None);
            }
            let mut name = [0u16; 256];
            let n = GetClassNameW(hwnd, name.as_mut_ptr(), 256).max(0) as usize;
            if SKIP_CLASSES.contains(&String::from_utf16_lossy(&name[..n]).as_str()) {
                return Some(None);
            }
            if GetWindowLongPtrW(hwnd, GWL_STYLE) & WS_CAPTION != 0 {
                return Some(None);
            }
            let mut rect = Rect::default();
            if GetWindowRect(hwnd, &mut rect) == 0 {
                return Some(None);
            }
            let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
            if GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info) == 0 {
                return Some(None);
            }
            let m = &info.monitor;
            let mon = (m.left, m.top, m.right - m.left, m.bottom - m.top);
            let covers = rect.left <= m.left && rect.top <= m.top && rect.right >= m.right && rect.bottom >= m.bottom;
            let program = program_of(hwnd);
            Some((covers && !SKIP_PROGRAMS.contains(&program.as_str()) && !super::is_game(&program)).then_some(mon))
        }
    }

    // The window in front, for the log: its class, its program's file and
    // its extended style. To tell a screenshot tool's layer over the screen
    // a moment (the taskbar hid and came back, a flash, with every shot,
    // 2026-10-10) from a game full screen, before a rule tells them apart.
    pub fn front() -> String {
        // SAFETY: plain calls; the buffers are ours and sized; the handle closed.
        unsafe {
            let hwnd = GetForegroundWindow();
            let mut name = [0u16; 256];
            let n = GetClassNameW(hwnd, name.as_mut_ptr(), 256).max(0) as usize;
            let class = String::from_utf16_lossy(&name[..n]);
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            let mut file = String::new();
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process != 0 {
                let mut path = [0u16; 512];
                let mut size = 512u32;
                if QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut size) != 0 {
                    let path = String::from_utf16_lossy(&path[..size as usize]);
                    file = path.rsplit('\\').next().unwrap_or(&path).to_string();
                }
                CloseHandle(process);
            }
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            // And the display it covers.
            let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
            GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info);
            let m = &info.monitor;
            // Its class says which program it is: the person's (crash.rs private).
            format!("{} ({file}) exstyle {ex:#x} on ({}, {}, {}, {})", crate::crash::private(&class), m.left, m.top, m.right - m.left, m.bottom - m.top)
        }
    }

    pub const AVAILABLE: bool = true;
}

#[cfg(not(windows))]
mod imp {
    pub fn check(_own: &[isize], _on: &[(i32, i32, i32, i32)]) -> Option<bool> {
        Some(false)
    }

    pub fn covering(_own: &[isize]) -> Option<Option<(i32, i32, i32, i32)>> {
        Some(None)
    }

    pub fn front() -> String {
        String::new()
    }

    pub const AVAILABLE: bool = false;
}

pub use imp::{check, covering, front, AVAILABLE};
