//! Self-update of the launcher.
//!
//! 1. At start the UI asks `update_check`: HTTPS GET of the manifest {version, url, sha256} (AETERNUM_UPDATE_URL at build
//!    time; empty by default, which disables the updater). Only https URLs, never an IP, no secret.
//! 2. The player confirms in the popup; `update_download` downloads the zip (progress events), checks its SHA-256,
//!    extracts the launcher exe into <config>/update/ and starts it with `--apply-update <target> <pid>` (no console:
//!    the exe is a GUI-subsystem program, started detached).
//! 3. The old launcher closes; the new exe waits for it, backs up the old exe (`.bak`), copies itself over it and
//!    starts it with `--updated=<version>`. On any error it restores the backup and starts it with
//!    `--update-error=<message>`, which the launcher shows in its own dialog. If a step fails before the swap,
//!    the current version simply keeps running.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Self-update manifest URL from AETERNUM_UPDATE_URL at build time (https only); empty disables the updater.
pub fn manifest_url() -> String {
    // Aeternum launcher: self-update disabled (no publish channel). Empty URL => check() returns None.
    option_env!("AETERNUM_UPDATE_URL")
        .filter(|v| v.trim().starts_with("https://"))
        .unwrap_or("")
        .trim()
        .to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub url: String,
    pub sha256: String,
}

fn parts(v: &str) -> Vec<u64> {
    v.trim().trim_start_matches('v').split(['.', '-', '+']).take(3).map(|p| p.parse().unwrap_or(0)).collect()
}

/// a > b on major.minor.patch.
pub fn newer(a: &str, b: &str) -> bool {
    let (mut x, mut y) = (parts(a), parts(b));
    x.resize(3, 0);
    y.resize(3, 0);
    x > y
}

/// True for an https URL whose host is a name (no IP literal).
pub(crate) fn safe_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(['/', ':', '?', '#']).next().unwrap_or("");
    !host.is_empty() && host.contains('.') && !host.chars().all(|c| c.is_ascii_digit() || c == '.') && !host.starts_with('[')
}

pub(crate) fn client(timeout: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout))
        .connect_timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())
}

/// The manifest when it offers a newer launcher; None when up to date or unreachable (the launcher goes on).
pub async fn check() -> Option<Manifest> {
    let url = manifest_url();
    if !safe_url(&url) {
        return None;
    }
    let res = client(8).ok()?.get(&url).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    let m: Manifest = res.json().await.ok()?;
    let sha_ok = m.sha256.len() == 64 && m.sha256.chars().all(|c| c.is_ascii_hexdigit());
    (sha_ok && safe_url(&m.url) && newer(&m.version, env!("CARGO_PKG_VERSION"))).then_some(m)
}

pub fn update_dir() -> PathBuf {
    crate::config::config_dir().join("update")
}

/// Downloads, verifies and extracts; returns the staged exe. `progress(stage, done, total)`.
pub async fn download(m: &Manifest, mut progress: impl FnMut(&'static str, u64, u64)) -> Result<PathBuf, String> {
    progress("download", 0, 0);
    let mut res = client(600)?.get(&m.url).send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("HTTP {}", res.status().as_u16()));
    }
    let total = res.content_length().unwrap_or(0);
    let mut data: Vec<u8> = Vec::with_capacity(total as usize);
    let mut last = 0u64;
    while let Some(chunk) = res.chunk().await.map_err(|e| e.to_string())? {
        data.extend_from_slice(&chunk);
        if data.len() as u64 - last > 256 * 1024 {
            last = data.len() as u64;
            progress("download", last, total);
        }
        if data.len() > 200 * 1024 * 1024 {
            return Err("download too large".into());
        }
    }
    progress("download", data.len() as u64, total.max(data.len() as u64));
    progress("verify", 0, 1);
    let got: String = Sha256::digest(&data).iter().map(|b| format!("{b:02x}")).collect();
    if !got.eq_ignore_ascii_case(m.sha256.trim()) {
        return Err("SHA-256 mismatch".into());
    }
    progress("verify", 1, 1);
    progress("extract", 0, 1);
    let exe = extract_launcher(&data)?;
    let dir = update_dir();
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let staged = dir.join(format!("aeternum-launcher-{}.exe", m.version.replace(|c: char| !c.is_ascii_alphanumeric() && c != '.', "_")));
    std::fs::write(&staged, exe).map_err(|e| e.to_string())?;
    progress("extract", 1, 1);
    Ok(staged)
}

/// The launcher exe inside the zip (the first .exe that is not an installer).
fn extract_launcher(zip: &[u8]) -> Result<Vec<u8>, String> {
    let entries = zip_entries(zip)?;
    let pick = entries
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(".exe") && !e.name.to_ascii_lowercase().contains("setup"))
        .next()
        .ok_or("no launcher exe in the zip")?;
    let raw = zip.get(pick.data..pick.data + pick.csize).ok_or("bad zip")?;
    let out = match pick.method {
        0 => raw.to_vec(),
        8 => {
            let mut v = Vec::with_capacity(pick.usize);
            flate2::read::DeflateDecoder::new(raw).read_to_end(&mut v).map_err(|e| e.to_string())?;
            v
        }
        m => return Err(format!("zip method {m}")),
    };
    if out.len() != pick.usize || !out.starts_with(b"MZ") {
        return Err("bad exe in the zip".into());
    }
    Ok(out)
}

pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) method: u16,
    pub(crate) csize: usize,
    pub(crate) usize: usize,
    pub(crate) data: usize,
}

fn u16_at(b: &[u8], o: usize) -> Result<u16, String> {
    b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| "bad zip".into())
}
fn u32_at(b: &[u8], o: usize) -> Result<u32, String> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(|| "bad zip".into())
}

/// Central directory of a (non-zip64) archive.
pub(crate) fn zip_entries(b: &[u8]) -> Result<Vec<Entry>, String> {
    let start = b.len().saturating_sub(22 + 65535);
    let eocd = (start..b.len().saturating_sub(21)).rev().find(|&i| b[i..i + 4] == [0x50, 0x4b, 0x05, 0x06]).ok_or("not a zip")?;
    let count = u16_at(b, eocd + 10)? as usize;
    let mut p = u32_at(b, eocd + 16)? as usize;
    let mut out = Vec::new();
    for _ in 0..count {
        if u32_at(b, p)? != 0x0201_4b50 {
            return Err("bad zip directory".into());
        }
        let method = u16_at(b, p + 10)?;
        let csize = u32_at(b, p + 20)? as usize;
        let usize_ = u32_at(b, p + 24)? as usize;
        let (n, x, c) = (u16_at(b, p + 28)? as usize, u16_at(b, p + 30)? as usize, u16_at(b, p + 32)? as usize);
        let local = u32_at(b, p + 42)? as usize;
        let name = String::from_utf8_lossy(b.get(p + 46..p + 46 + n).ok_or("bad zip")?).into_owned();
        if u32_at(b, local)? != 0x0403_4b50 {
            return Err("bad zip entry".into());
        }
        let data = local + 30 + u16_at(b, local + 26)? as usize + u16_at(b, local + 28)? as usize;
        out.push(Entry { name, method, csize, usize: usize_, data });
        p += 46 + n + x + c;
    }
    Ok(out)
}

/// Starts the staged exe in "apply" mode (detached, no window of its own until it restarts the launcher).
#[cfg(windows)]
pub fn start_apply(staged: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    let target = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new(staged)
        .arg("--apply-update")
        .arg(&target)
        .arg(std::process::id().to_string())
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn start_apply(_staged: &Path) -> Result<(), String> {
    Err("self-update is only available on Windows".into())
}

/// `--apply-update <target> <pid>`: runs in the NEW exe, without UI. Returns the process exit code.
#[cfg(windows)]
pub fn apply(target: &str, pid: &str) -> i32 {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    let target = PathBuf::from(target);
    let backup = target.with_extension("exe.bak");
    let version = env!("CARGO_PKG_VERSION");
    wait_exit(pid.parse().unwrap_or(0));
    let start = |args: &[String]| {
        std::process::Command::new(&target)
            .args(args)
            .current_dir(target.parent().unwrap_or(Path::new(".")))
            .creation_flags(DETACHED_PROCESS)
            .spawn()
            .is_ok()
    };
    let result = (|| -> Result<(), String> {
        let me = std::env::current_exe().map_err(|e| e.to_string())?;
        retry(|| std::fs::copy(&target, &backup).map(|_| ())).map_err(|e| format!("backup: {e}"))?;
        retry(|| std::fs::copy(&me, &target).map(|_| ())).map_err(|e| format!("copy: {e}"))?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            if start(&[format!("--updated={version}")]) {
                return 0;
            }
            let _ = std::fs::copy(&backup, &target);
            if start(&["--update-error=the new version did not start".to_string()]) {
                return 1;
            }
            message_box("Forever Launcher", "L'aggiornamento non ÃƒÂ¨ riuscito e il launcher non si ÃƒÂ¨ riavviato. Riaprilo a mano.");
            2
        }
        Err(e) => {
            if backup.exists() {
                let _ = retry(|| std::fs::copy(&backup, &target).map(|_| ()));
            }
            if !start(&[format!("--update-error={e}")]) {
                message_box("Forever Launcher", &format!("Aggiornamento non riuscito: {e}. Riapri il launcher."));
            }
            1
        }
    }
}

#[cfg(windows)]
fn retry(mut f: impl FnMut() -> std::io::Result<()>) -> std::io::Result<()> {
    let mut last = None;
    for _ in 0..40 {
        match f() {
            Ok(()) => return Ok(()),
            Err(e) => last = Some(e),
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err(last.unwrap())
}

#[cfg(windows)]
fn wait_exit(pid: u32) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};
    if pid == 0 {
        std::thread::sleep(Duration::from_secs(2));
        return;
    }
    unsafe {
        if let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            let _ = WaitForSingleObject(h, 30_000);
            let _ = CloseHandle(h);
        }
    }
    std::thread::sleep(Duration::from_millis(300));
}

#[cfg(windows)]
pub fn message_box(title: &str, text: &str) {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    unsafe {
        MessageBoxW(None, &HSTRING::from(text), &HSTRING::from(title), MB_OK | MB_ICONERROR);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_and_urls() {
        assert!(newer("1.1.0", "1.0.9"));
        assert!(!newer("1.0.0", "1.0.0"));
        assert!(newer("v2.0", "1.9.9"));
        assert!(safe_url("https://downloads.example.com/launcher/latest.json"));
        assert!(!safe_url("http://downloads.example.com/x"));
        assert!(!safe_url("https://192.0.2.1/x"));
    }
}
