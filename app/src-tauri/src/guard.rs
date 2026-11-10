//! Client-side protection (Windows), a light watchdog running for the whole life of the launcher:
//! - every 4 s: running processes (exe name), visible top-level windows (title / class) and, when the Forever game
//!   runs, the modules loaded in Wow.exe (only if readable without elevation), against a data-driven list of
//!   forbidden tools (built-in list, replaced by the list published next to the launcher manifest when reachable);
//! - a tool found before the game: 'Play' is blocked; found while the game runs: the game is terminated;
//! - the game started by the launcher lives in a Job Object with KILL_ON_JOB_CLOSE: when the launcher process ends for
//!   any reason (X, Task Manager, crash), Windows terminates the game too.
//! Limits (launcher side only): a renamed bot exe with an unknown window, tools running in a VM / on another PC,
//! external pixel bots that do not touch the game, kernel-level / elevated tools hidden from a normal user process.

use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Mutex;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tool {
    pub name: String,
    /// exe names, exact match (case-insensitive)
    pub processes: Vec<String>,
    /// window titles, exact match (case-insensitive)
    pub window_titles: Vec<String>,
    /// window title prefixes (case-insensitive), at least 8 characters
    pub window_title_prefixes: Vec<String>,
    /// window class names, exact match (case-insensitive)
    pub window_classes: Vec<String>,
    /// DLL names loaded in the game, exact match (case-insensitive)
    pub modules: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Rules {
    pub version: u32,
    pub tools: Vec<Tool>,
    /// processes whose windows are never matched by title (browsers, chat...: a web page about a tool is not the tool)
    pub ignore_window_processes: Vec<String>,
}

/// Names too generic to be trusted from a remote list (no false positive on ordinary programs).
const GENERIC: [&str; 12] = [
    "bot.exe", "launcher.exe", "updater.exe", "update.exe", "loader.exe", "injector.exe", "relogger.exe", "setup.exe",
    "client.exe", "app.exe", "main.exe", "wow.exe",
];

impl Rules {
    /// Lower-case everything and drop vague entries.
    fn clean(mut self) -> Self {
        for t in &mut self.tools {
            let low = |v: &mut Vec<String>| {
                for s in v.iter_mut() {
                    *s = s.trim().to_lowercase();
                }
            };
            low(&mut t.processes);
            low(&mut t.window_titles);
            low(&mut t.window_title_prefixes);
            low(&mut t.window_classes);
            low(&mut t.modules);
            t.processes.retain(|p| p.ends_with(".exe") && p.len() >= 7 && !GENERIC.contains(&p.as_str()));
            t.window_titles.retain(|p| p.len() >= 5);
            t.window_title_prefixes.retain(|p| p.len() >= 8);
            t.window_classes.retain(|p| p.len() >= 5);
            t.modules.retain(|p| p.ends_with(".dll") && p.len() >= 8);
            t.name = t.name.trim().chars().take(60).collect();
        }
        self.tools.retain(|t| !t.name.is_empty());
        for s in &mut self.ignore_window_processes {
            *s = s.trim().to_lowercase();
        }
        self
    }
}

fn builtin() -> Rules {
    serde_json::from_str::<Rules>(include_str!("guard_default.json")).unwrap_or_default().clean()
}

static RULES: Mutex<Option<Rules>> = Mutex::new(None);

pub fn rules() -> Rules {
    RULES.lock().unwrap().get_or_insert_with(builtin).clone()
}

/// The published list (next to the launcher manifest, or FOREVER_GUARD_URL at build time); the built-in list stays
/// when it is unreachable or invalid. The remote list replaces the built-in one only when it has tools.
pub async fn refresh() {
    let url = option_env!("FOREVER_GUARD_URL")
        .filter(|v| v.trim().starts_with("https://"))
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|| {
            let m = crate::updater::manifest_url();
            match m.rfind('/') {
                Some(i) => format!("{}/antibot.json", &m[..i]),
                None => m,
            }
        });
    if !crate::updater::safe_url(&url) {
        return;
    }
    let Ok(c) = crate::updater::client(8) else { return };
    let Ok(res) = c.get(&url).send().await else { return };
    if !res.status().is_success() {
        return;
    }
    let Ok(body) = res.bytes().await else { return };
    if body.len() > 256 * 1024 {
        return;
    }
    if let Ok(r) = serde_json::from_slice::<Rules>(&body) {
        let r = r.clean();
        if !r.tools.is_empty() {
            *RULES.lock().unwrap() = Some(r);
        }
    }
}

#[cfg(windows)]
pub use win::{assign_to_job, kill_game, scan};

#[cfg(not(windows))]
pub fn scan(_game_dir: &str) -> Option<String> {
    None
}
#[cfg(not(windows))]
pub fn kill_game(_game_dir: &str) {}

#[cfg(windows)]
mod win {
    use super::{rules, HashSet};
    use std::path::PathBuf;
    use std::sync::OnceLock;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, BOOL, HANDLE, HWND, LPARAM};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW, MODULEENTRY32W,
        PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, TerminateProcess, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    };

    struct Job(HANDLE);
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    /// One job for the life of the launcher; its handle is never closed by us: the kernel closes it when this process
    /// ends (normally or not), which kills every process in it.
    fn job() -> Option<&'static Job> {
        static JOB: OnceLock<Option<Job>> = OnceLock::new();
        JOB.get_or_init(|| unsafe {
            let h = CreateJobObjectW(None, None).ok()?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                h,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .ok()?;
            Some(Job(h))
        })
        .as_ref()
    }

    /// Puts the game process in the kill-on-close job. Err(reason) when Windows refuses (reported in the journal).
    pub fn assign_to_job(pid: u32) -> Result<(), String> {
        let job = job().ok_or("CreateJobObject failed")?;
        unsafe {
            let h = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, pid).map_err(|e| e.message())?;
            let r = AssignProcessToJobObject(job.0, h).map_err(|e| e.message());
            let _ = CloseHandle(h);
            r
        }
    }

    struct Proc {
        pid: u32,
        name: String,
    }

    fn processes() -> Vec<Proc> {
        let mut out = Vec::new();
        unsafe {
            let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return out };
            let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
            let mut ok = Process32FirstW(snap, &mut e).is_ok();
            while ok {
                let end = e.szExeFile.iter().position(|c| *c == 0).unwrap_or(e.szExeFile.len());
                out.push(Proc { pid: e.th32ProcessID, name: String::from_utf16_lossy(&e.szExeFile[..end]).to_lowercase() });
                ok = Process32NextW(snap, &mut e).is_ok();
            }
            let _ = CloseHandle(snap);
        }
        out
    }

    fn image_dir(pid: u32) -> Option<PathBuf> {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
            let _ = CloseHandle(h);
            ok.then(|| PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])).parent().map(|p| p.to_path_buf()))?
        }
    }

    /// Forever game processes (same rule as instance::game_running).
    fn game_pids(procs: &[Proc], game_dir: &str) -> Vec<u32> {
        let norm = |s: &str| s.trim().trim_end_matches(['\\', '/']).replace('/', "\\").to_lowercase();
        procs
            .iter()
            .filter(|p| crate::config::GAME_EXES.iter().any(|e| p.name.eq_ignore_ascii_case(e)))
            .filter(|p| match image_dir(p.pid) {
                None => true,
                Some(d) => {
                    let n = d.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
                    n == "_forever_" || n == "_classic_beta_" || (!game_dir.trim().is_empty() && norm(&d.to_string_lossy()) == norm(game_dir))
                }
            })
            .map(|p| p.pid)
            .collect()
    }

    struct Win {
        pid: u32,
        title: String,
        class: String,
    }

    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let out = &mut *(lparam.0 as *mut Vec<Win>);
        if IsWindowVisible(hwnd).as_bool() {
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            let mut t = [0u16; 256];
            let n = GetWindowTextW(hwnd, &mut t).max(0) as usize;
            let mut c = [0u16; 128];
            let m = GetClassNameW(hwnd, &mut c).max(0) as usize;
            if n > 0 || m > 0 {
                out.push(Win {
                    pid,
                    title: String::from_utf16_lossy(&t[..n]).to_lowercase(),
                    class: String::from_utf16_lossy(&c[..m]).to_lowercase(),
                });
            }
        }
        BOOL(1)
    }

    fn windows() -> Vec<Win> {
        let mut out: Vec<Win> = Vec::new();
        unsafe {
            let _ = EnumWindows(Some(each), LPARAM(&mut out as *mut Vec<Win> as isize));
        }
        out
    }

    fn modules(pid: u32) -> Vec<String> {
        let mut out = Vec::new();
        unsafe {
            // fails (access denied) for an elevated / protected game: then only processes and windows are checked
            let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) else { return out };
            let mut e = MODULEENTRY32W { dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32, ..Default::default() };
            let mut ok = Module32FirstW(snap, &mut e).is_ok();
            while ok {
                let end = e.szModule.iter().position(|c| *c == 0).unwrap_or(e.szModule.len());
                out.push(String::from_utf16_lossy(&e.szModule[..end]).to_lowercase());
                ok = Module32NextW(snap, &mut e).is_ok();
            }
            let _ = CloseHandle(snap);
        }
        out
    }

    /// The name of the first forbidden tool found, or None.
    pub fn scan(game_dir: &str) -> Option<String> {
        let rules = rules();
        if rules.tools.is_empty() {
            return None;
        }
        let procs = processes();
        let names: HashSet<&str> = procs.iter().map(|p| p.name.as_str()).collect();
        for t in &rules.tools {
            if t.processes.iter().any(|p| names.contains(p.as_str())) {
                return Some(t.name.clone());
            }
        }
        let by_window = rules.tools.iter().any(|t| {
            !t.window_titles.is_empty() || !t.window_title_prefixes.is_empty() || !t.window_classes.is_empty()
        });
        if by_window {
            let ignored: HashSet<u32> = procs
                .iter()
                .filter(|p| rules.ignore_window_processes.iter().any(|i| *i == p.name))
                .map(|p| p.pid)
                .collect();
            let me = std::process::id();
            for w in windows().iter().filter(|w| w.pid != me && !ignored.contains(&w.pid)) {
                for t in &rules.tools {
                    if t.window_classes.iter().any(|c| *c == w.class)
                        || t.window_titles.iter().any(|x| *x == w.title)
                        || t.window_title_prefixes.iter().any(|x| w.title.starts_with(x.as_str()))
                    {
                        return Some(t.name.clone());
                    }
                }
            }
        }
        if rules.tools.iter().any(|t| !t.modules.is_empty()) {
            for pid in game_pids(&procs, game_dir) {
                let mods: HashSet<String> = modules(pid).into_iter().collect();
                for t in &rules.tools {
                    if t.modules.iter().any(|m| mods.contains(m)) {
                        return Some(t.name.clone());
                    }
                }
            }
        }
        None
    }

    /// Terminates every Forever game process (a forbidden tool was found while playing).
    pub fn kill_game(game_dir: &str) {
        for pid in game_pids(&processes(), game_dir) {
            unsafe {
                if let Ok(h) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                    let _ = TerminateProcess(h, 1);
                    let _ = CloseHandle(h);
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Closing the job handle (what Windows does when the launcher dies) terminates the process in the job.
        #[test]
        fn job_kills_child_on_close() {
            let mut child = std::process::Command::new("ping").args(["-n", "30", "127.0.0.1"]).spawn().unwrap();
            unsafe {
                let h = CreateJobObjectW(None, None).unwrap();
                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                SetInformationJobObject(
                    h,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
                .unwrap();
                let p = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, child.id()).unwrap();
                AssignProcessToJobObject(h, p).unwrap();
                let _ = CloseHandle(p);
                let _ = CloseHandle(h);
            }
            std::thread::sleep(std::time::Duration::from_millis(800));
            assert!(child.try_wait().unwrap().is_some(), "child still alive after the job closed");
        }

        #[test]
        fn assign_real_job() {
            let mut child = std::process::Command::new("ping").args(["-n", "5", "127.0.0.1"]).spawn().unwrap();
            assert_eq!(assign_to_job(child.id()), Ok(()));
            let _ = child.kill();
        }

        #[test]
        fn scan_runs_fast() {
            let t = std::time::Instant::now();
            let _ = scan("");
            assert!(t.elapsed().as_millis() < 1500);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_list_is_valid_and_specific() {
        let r = builtin();
        assert!(r.tools.len() >= 5);
        for t in &r.tools {
            for p in &t.processes {
                assert!(!GENERIC.contains(&p.as_str()), "{p}");
            }
        }
        let vague: Rules = serde_json::from_str(r#"{"tools":[{"name":"x","processes":["bot.exe","a.exe","Foo-Bot.exe"],"windowTitlePrefixes":["bot"]}]}"#).unwrap();
        let v = vague.clean();
        assert_eq!(v.tools[0].processes, vec!["foo-bot.exe".to_string()]);
        assert!(v.tools[0].window_title_prefixes.is_empty());
    }
}
