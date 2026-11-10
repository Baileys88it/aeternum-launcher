//! Anti-multibox (launcher side):
//! - one launcher per PC: a mutex with a FIXED name (does not depend on the exe path or file name, so a copied or
//!   renamed exe finds the same mutex). `Global\` first (all sessions of the PC), `Local\` only when Global cannot be
//!   created at all. The second instance says so, brings the first window to the front and exits.
//! - self-update: the restarted launcher (--updated= / --update-error=) waits for the old process to release the mutex.
//! - the Forever game (Wow.exe / WowB.exe in `_forever_` / `_classic_beta_` or in the configured folder) already running:
//!   'Play' stays off.
//! Not covered (by design, launcher side only): starting Wow.exe directly, other PCs / VMs / other OS users with
//! their own sandbox; the server side (one session per account, IP/hardware limits) has the final say.

#[cfg(windows)]
mod imp {
    use std::path::{Path, PathBuf};
    use windows::core::{HSTRING, PWSTR};
    use windows::Win32::Foundation::{
        CloseHandle, GetLastError, BOOL, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, HANDLE, HWND, LPARAM,
    };
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::{
        CreateMutexW, OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, MessageBoxW,
        SetForegroundWindow, ShowWindow, MB_ICONINFORMATION, MB_OK, MB_SETFOREGROUND, MB_TOPMOST, SW_RESTORE,
    };

    /// Fixed for every build and copy of the launcher: never change it (old and new versions must see each other).
    const MUTEX_NAME: &str = "aeternumLauncher-9c4e2b71-6a1d-4f83-9e05-2b8d7c14a6f0";

    fn pid_file() -> PathBuf {
        std::env::temp_dir().join("aeternumLauncher.pid")
    }

    enum Open {
        /// We own the mutex (kept until the process exits).
        Owned,
        /// Another launcher holds it; Some(handle) when we can wait on it.
        Busy(Option<HANDLE>),
    }

    fn open(prefix: &str) -> Result<Open, windows::core::Error> {
        let name = HSTRING::from(format!("{prefix}\\{MUTEX_NAME}"));
        unsafe {
            let h = CreateMutexW(None, true, &name)?;
            if GetLastError() == ERROR_ALREADY_EXISTS {
                Ok(Open::Busy(Some(h)))
            } else {
                Ok(Open::Owned)
            }
        }
    }

    /// True when this process may run the launcher UI. `wait_ms`: how long to wait for the previous instance
    /// (self-update restart); 0 for a normal start.
    pub fn acquire(wait_ms: u32) -> bool {
        let state = match open("Global") {
            Ok(s) => s,
            // the mutex exists in another session with a DACL we cannot open: another launcher runs on this PC
            Err(e) if e.code() == ERROR_ACCESS_DENIED.to_hresult() => Open::Busy(None),
            // Global namespace not allowed here: Local (this session) only
            Err(_) => match open("Local") {
                Ok(s) => s,
                Err(_) => return true, // cannot create any mutex: do not lock the player out
            },
        };
        let owned = match state {
            Open::Owned => true,
            Open::Busy(Some(h)) if wait_ms > 0 => {
                // the old launcher exits after starting the update: its mutex becomes ours (WAIT_OBJECT_0 or ABANDONED)
                let r = unsafe { WaitForSingleObject(h, wait_ms) }.0;
                r == 0 || r == 0x80
            }
            Open::Busy(_) => false,
        };
        if owned {
            let _ = std::fs::write(pid_file(), std::process::id().to_string());
        }
        owned
    }

    struct Find {
        pid: u32,
        hwnd: Option<HWND>,
    }

    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let find = &mut *(lparam.0 as *mut Find);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == find.pid && IsWindowVisible(hwnd).as_bool() && GetWindowTextLengthW(hwnd) > 0 {
            find.hwnd = Some(hwnd);
            return BOOL(0);
        }
        BOOL(1)
    }

    /// Brings the window of the running launcher to the front (same session only).
    pub fn focus_other() {
        let Some(pid) = std::fs::read_to_string(pid_file()).ok().and_then(|s| s.trim().parse::<u32>().ok()) else { return };
        let mut find = Find { pid, hwnd: None };
        unsafe {
            let _ = EnumWindows(Some(each), LPARAM(&mut find as *mut Find as isize));
            if let Some(h) = find.hwnd {
                if IsIconic(h).as_bool() {
                    let _ = ShowWindow(h, SW_RESTORE);
                }
                let _ = SetForegroundWindow(h);
            }
        }
    }

    pub fn already_open_message() {
        unsafe {
            MessageBoxW(
                None,
                &HSTRING::from("Il launcher e' gia' aperto.\n\nThe launcher is already open."),
                &HSTRING::from("Aeternum launcher"),
                MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND | MB_TOPMOST,
            );
        }
    }

    fn image_path(pid: u32) -> Option<PathBuf> {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
            let _ = CloseHandle(h);
            ok.then(|| PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])))
        }
    }

    fn same_dir(a: &Path, b: &str) -> bool {
        let norm = |s: &str| s.trim().trim_end_matches(['\\', '/']).replace('/', "\\").to_lowercase();
        !b.trim().is_empty() && norm(&a.to_string_lossy()) == norm(b)
    }

    /// A Forever game client is running: Wow.exe / WowB.exe whose folder is `_forever_` / `_classic_beta_` or the
    /// configured game folder. A game process whose path cannot be read (e.g. elevated) counts as running.
    pub fn game_running(game_dir: &str) -> bool {
        let mut found = false;
        unsafe {
            let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return false };
            let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
            let mut ok = Process32FirstW(snap, &mut entry).is_ok();
            while ok && !found {
                let end = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
                if crate::config::GAME_EXES.iter().any(|e| name.eq_ignore_ascii_case(e)) {
                    found = match image_path(entry.th32ProcessID) {
                        None => true,
                        Some(p) => p.parent().is_some_and(|dir| {
                            let n = dir.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
                            n == "_forever_" || n == "_classic_beta_" || same_dir(dir, game_dir)
                        }),
                    };
                }
                ok = Process32NextW(snap, &mut entry).is_ok();
            }
            let _ = CloseHandle(snap);
        }
        found
    }
}

/// main(): false = another launcher is open (message shown, its window brought to the front): exit.
#[cfg(windows)]
pub fn single_instance(restarted_by_update: bool) -> bool {
    if imp::acquire(if restarted_by_update { 30_000 } else { 0 }) {
        return true;
    }
    imp::already_open_message();
    imp::focus_other();
    false
}

#[cfg(not(windows))]
pub fn single_instance(_restarted_by_update: bool) -> bool {
    true
}

#[cfg(windows)]
pub fn game_running(game_dir: &str) -> bool {
    imp::game_running(game_dir)
}

#[cfg(all(not(windows), not(target_os = "macos")))]
pub fn game_running(_game_dir: &str) -> bool {
    crate::patcher::find_client().is_some()
}

#[cfg(target_os = "macos")]
pub fn game_running(_game_dir: &str) -> bool {
    false
}
