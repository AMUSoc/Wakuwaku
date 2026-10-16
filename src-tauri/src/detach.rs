// Her process out of the job of whoever starts her, as if opened from
// Explorer. A session's hook (Claude Code's, Codex's, Antigravity's) runs in
// its agent's job, and what a desktop app starts may stay in the app's job,
// which ends everything in it as the app quits: started from Claude's
// desktop app, she and her taskbar's guard went with it as it updated, and
// Windows' taskbar was left put away on every display until she was started
// again (the user, 2026-10-13). Breaking away is up to the job, and that one
// would not let her. Explorer as the parent instead: what it opens goes in
// its job, as anything opened from the desktop does.
//
// spawn: detached, inheriting no handle (a hook's process holds the ones its
// agent left inheritable, its output pipes among them, and whoever reads
// those to the end would wait on her for good), Explorer the parent while it
// is up; as before (from here) when it is not.
//
// should_leave: in a job and not started by Explorer (a hook of an older
// her, a terminal, an app's own shell): she starts herself again through
// spawn, once (FLAG), and this copy goes. Not run from the source tree.

pub const FLAG: &str = "--wakuwaku-out-of-job";

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    #[repr(C)]
    struct StartupInfo {
        cb: u32,
        reserved: *mut u16,
        desktop: *mut u16,
        title: *mut u16,
        x: u32,
        y: u32,
        x_size: u32,
        y_size: u32,
        x_chars: u32,
        y_chars: u32,
        fill: u32,
        flags: u32,
        show: u16,
        reserved2_len: u16,
        reserved2: *mut u8,
        stdin: isize,
        stdout: isize,
        stderr: isize,
    }

    #[repr(C)]
    struct StartupInfoEx {
        info: StartupInfo,
        attributes: *mut c_void,
    }

    #[repr(C)]
    struct ProcessInfo {
        process: isize,
        thread: isize,
        process_id: u32,
        thread_id: u32,
    }

    #[repr(C)]
    struct ProcessEntry {
        size: u32,
        usage: u32,
        process_id: u32,
        heap_id: usize,
        module_id: u32,
        threads: u32,
        parent_id: u32,
        priority: i32,
        flags: u32,
        exe: [u16; 260],
    }

    #[link(name = "kernel32")]
    extern "system" {
        #[allow(clippy::too_many_arguments)]
        fn CreateProcessW(
            app: *const u16,
            command_line: *mut u16,
            process_attrs: *const u8,
            thread_attrs: *const u8,
            inherit_handles: i32,
            flags: u32,
            env: *const u8,
            dir: *const u16,
            startup: *const StartupInfo,
            info: *mut ProcessInfo,
        ) -> i32;
        fn CloseHandle(handle: isize) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn GetCurrentProcess() -> isize;
        fn GetCurrentProcessId() -> u32;
        fn IsProcessInJob(process: isize, job: isize, result: *mut i32) -> i32;
        fn InitializeProcThreadAttributeList(list: *mut c_void, count: u32, flags: u32, size: *mut usize) -> i32;
        fn UpdateProcThreadAttribute(list: *mut c_void, flags: u32, attribute: usize, value: *const c_void, size: usize, previous: *mut c_void, returned: *mut usize) -> i32;
        fn DeleteProcThreadAttributeList(list: *mut c_void);
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> isize;
        fn Process32FirstW(snapshot: isize, entry: *mut ProcessEntry) -> i32;
        fn Process32NextW(snapshot: isize, entry: *mut ProcessEntry) -> i32;
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetShellWindow() -> *mut c_void;
        fn GetWindowThreadProcessId(hwnd: *mut c_void, pid: *mut u32) -> u32;
    }

    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;
    const PROCESS_CREATE_PROCESS: u32 = 0x0080;
    const PROC_THREAD_ATTRIBUTE_PARENT_PROCESS: usize = 0x0002_0000;
    const TH32CS_SNAPPROCESS: u32 = 0x2;
    const INVALID_HANDLE: isize = -1;

    fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
        s.encode_wide().chain(Some(0)).collect()
    }

    // Explorer's process: the one with the shell's (desktop's) window.
    fn explorer() -> Option<u32> {
        let mut pid = 0;
        // SAFETY: plain queries; a missing window gives none.
        let shell = unsafe { GetShellWindow() };
        if shell.is_null() || unsafe { GetWindowThreadProcessId(shell, &mut pid) } == 0 || pid == 0 {
            return None;
        }
        Some(pid)
    }

    // A command line as CreateProcess takes it: each part quoted (no part of
    // hers holds a quote: paths, flags, numbers).
    fn command_line(exe: &Path, args: &[&str]) -> Vec<u16> {
        let mut line = format!("\"{}\"", exe.display());
        for a in args {
            line.push_str(&format!(" \"{a}\""));
        }
        wide(std::ffi::OsStr::new(&line))
    }

    // Started: its process id.
    pub fn spawn_pid(exe: &Path, args: &[&str]) -> Option<u32> {
        let app = wide(exe.as_os_str());
        let mut line = command_line(exe, args);
        // Her own folder, not the starter's (a hook's: the project's): she would hold on to it.
        let dir = exe.parent().map(|d| wide(d.as_os_str()));
        let dir = dir.as_ref().map_or(std::ptr::null(), |d| d.as_ptr());
        // SAFETY: plain Win32 structs, zero is a valid start for each.
        let mut startup: StartupInfoEx = unsafe { std::mem::zeroed() };
        startup.info.cb = std::mem::size_of::<StartupInfoEx>() as u32;
        let mut info: ProcessInfo = unsafe { std::mem::zeroed() };
        let flags = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
        let null = std::ptr::null();

        // Explorer as the parent: a list of one attribute, its process.
        let parent = explorer().map(|pid| unsafe { OpenProcess(PROCESS_CREATE_PROCESS, 0, pid) }).filter(|h| *h != 0);
        let mut list: Vec<u8> = Vec::new();
        let mut made = false;
        if let Some(parent) = parent {
            let mut size = 0usize;
            // SAFETY: asked for its size first (the call fails, saying it), then
            // made in a buffer of that size; the parent's handle outlives the call.
            unsafe { InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size) };
            list.resize(size.max(1), 0);
            let ready = unsafe { InitializeProcThreadAttributeList(list.as_mut_ptr().cast(), 1, 0, &mut size) } != 0
                && unsafe { UpdateProcThreadAttribute(list.as_mut_ptr().cast(), 0, PROC_THREAD_ATTRIBUTE_PARENT_PROCESS, (&parent as *const isize).cast(), std::mem::size_of::<isize>(), std::ptr::null_mut(), std::ptr::null_mut()) } != 0;
            if ready {
                startup.attributes = list.as_mut_ptr().cast();
                // SAFETY: every pointer is valid for the call or null where Win32 allows it.
                made = unsafe { CreateProcessW(app.as_ptr(), line.as_mut_ptr(), null, null, 0, flags | EXTENDED_STARTUPINFO_PRESENT, null, dir, &startup.info, &mut info) } != 0;
                // SAFETY: the list was made above and is not used after.
                unsafe { DeleteProcThreadAttributeList(list.as_mut_ptr().cast()) };
            }
            // SAFETY: our own handle.
            unsafe { CloseHandle(parent) };
        }
        // No Explorer, or it would not have it: from here, as before.
        if !made {
            startup.attributes = std::ptr::null_mut();
            startup.info.cb = std::mem::size_of::<StartupInfo>() as u32;
            let mut line = command_line(exe, args);
            // SAFETY: as above.
            made = unsafe { CreateProcessW(app.as_ptr(), line.as_mut_ptr(), null, null, 0, flags, null, dir, &startup.info, &mut info) } != 0;
        }
        if made {
            // SAFETY: the handles CreateProcess gave us.
            unsafe {
                CloseHandle(info.thread);
                CloseHandle(info.process);
            }
        }
        made.then_some(info.process_id)
    }

    // The process that started this one (by a snapshot of them all).
    fn parent_id() -> Option<u32> {
        // SAFETY: a snapshot we close; entries written into our own struct.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE || snap == 0 {
                return None;
            }
            let me = GetCurrentProcessId();
            let mut entry: ProcessEntry = std::mem::zeroed();
            entry.size = std::mem::size_of::<ProcessEntry>() as u32;
            let mut found = None;
            let mut ok = Process32FirstW(snap, &mut entry) != 0;
            while ok {
                if entry.process_id == me {
                    found = Some(entry.parent_id);
                    break;
                }
                ok = Process32NextW(snap, &mut entry) != 0;
            }
            CloseHandle(snap);
            found
        }
    }

    pub fn should_leave() -> bool {
        let mut in_job = 0;
        // SAFETY: a plain query about this process.
        if unsafe { IsProcessInJob(GetCurrentProcess(), 0, &mut in_job) } == 0 || in_job == 0 {
            return false;
        }
        match (explorer(), parent_id()) {
            (Some(explorer), Some(parent)) => parent != explorer,
            _ => false,
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::Path;

    pub fn spawn_pid(exe: &Path, args: &[&str]) -> Option<u32> {
        std::process::Command::new(exe)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()
            .map(|c| c.id())
    }

    pub fn should_leave() -> bool {
        false
    }
}

pub use imp::{should_leave, spawn_pid};

pub fn spawn(exe: &std::path::Path, args: &[&str]) -> bool {
    spawn_pid(exe, args).is_some()
}
