//! Installed-client lookup and Battle.net hand-off.
//!
//! - installed build: `.build.info` next to the game folder (Battle.net rewrites it when it patches), else the
//!   FileVersion string inside Wow.exe;
//! - the launcher does not ask a website whether that build is too old or too recent;
//! - Battle.net: `Battle.net.exe --game=wowf --install` installs or updates WoW Classic Forever (works without a
//!   subscription). If Battle.net is already open, it hands the arguments to the running instance, so it is never
//!   closed by the launcher.

use crate::config::Settings;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Battle.net product code of WoW Classic Forever.
pub const PRODUCT: &str = "wowf";
/// Official Battle.net download page (opened when Battle.net is not installed).
pub const BATTLENET_DOWNLOAD: &str = "https://download.battle.net";

/// Build number of a version string: "1.60.1.70707" -> 70707 (last dotted number).
pub fn build_of(version: &str) -> Option<u32> {
    version.trim().rsplit('.').next()?.trim().parse().ok().filter(|b| *b > 0)
}

/// Installed client version: .build.info first, then the Wow.exe FileVersion resource string.
pub fn installed_version(settings: &Settings) -> Option<String> {
    crate::config::installed_version(Path::new(&settings.game_dir)).or_else(|| exe_file_version(&settings.exe()))
}

/// FileVersion from the version resource (UTF-16 "FileVersion" key followed by its value).
pub fn exe_file_version(exe: &Path) -> Option<String> {
    let data = std::fs::read(exe).ok()?;
    let key: Vec<u8> = "FileVersion".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let mut from = 0;
    while let Some(pos) = memchr::memmem::find(&data[from..], &key) {
        let mut i = from + pos + key.len();
        // key terminator + padding to 32 bits
        while i + 1 < data.len() && data[i] == 0 && data[i + 1] == 0 {
            i += 2;
        }
        let mut s = String::new();
        while i + 1 < data.len() && s.len() < 40 {
            let u = u16::from_le_bytes([data[i], data[i + 1]]);
            if u == 0 {
                break;
            }
            s.push(char::from_u32(u as u32).unwrap_or('?'));
            i += 2;
        }
        let v = s.replace(", ", ".").replace(',', ".");
        if build_of(&v).is_some() && v.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return Some(v);
        }
        from += pos + key.len();
    }
    None
}

/// What the launcher shows for the local install. A website is not consulted.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientStatus {
    /// "missing" | "ok"
    pub state: &'static str,
    pub installed_version: Option<String>,
    pub installed_build: Option<u32>,
    pub battlenet_found: bool,
    pub battlenet_running: bool,
    /// Command shown to the player when Battle.net cannot be started.
    pub command: String,
}

/// Local install only: the game folder is either missing or present. No remote build list.
pub fn local_status(settings: &Settings) -> ClientStatus {
    let found = settings.game_found();
    let installed_version = if found { installed_version(settings) } else { None };
    let installed_build = installed_version.as_deref().and_then(build_of);
    ClientStatus {
        state: if found { "ok" } else { "missing" },
        installed_version,
        installed_build,
        battlenet_found: battlenet_exe(settings).is_some(),
        battlenet_running: battlenet_running(),
        command: command_line(settings),
    }
}

pub fn command_line(settings: &Settings) -> String {
    let exe = battlenet_exe(settings).unwrap_or_else(|| default_battlenet().unwrap_or_else(|| PathBuf::from("Battle.net.exe")));
    format!("\"{}\" --game={PRODUCT} --install", exe.display())
}

#[cfg(windows)]
fn default_battlenet() -> Option<PathBuf> {
    let pf = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)"));
    Some(pf.join("Battle.net").join("Battle.net.exe"))
}

#[cfg(not(windows))]
fn default_battlenet() -> Option<PathBuf> {
    None
}

/// Battle.net.exe: the path set in the settings, else the registry (uninstall entry, battlenet:// handler),
/// else the default folder.
pub fn battlenet_exe(settings: &Settings) -> Option<PathBuf> {
    let custom = settings.battlenet_path.trim();
    if !custom.is_empty() {
        let p = PathBuf::from(custom);
        let p = if p.is_dir() { p.join("Battle.net.exe") } else { p };
        if p.exists() {
            return Some(p);
        }
    }
    #[cfg(windows)]
    {
        use crate::winreg::read_string;
        use windows::Win32::System::Registry::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        let uninstall = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Battle.net";
        for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            if let Some(dir) = read_string(root, uninstall, "InstallLocation") {
                let p = Path::new(dir.trim_matches('"')).join("Battle.net.exe");
                if p.exists() {
                    return Some(p);
                }
            }
        }
        if let Some(cmd) = read_string(HKEY_CLASSES_ROOT, r"battlenet\shell\open\command", "") {
            // "C:\...\Battle.net Launcher.exe" "%1"
            if let Some(first) = cmd.split('"').find(|s| s.to_ascii_lowercase().ends_with(".exe")) {
                if let Some(dir) = Path::new(first).parent() {
                    let p = dir.join("Battle.net.exe");
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
        }
    }
    default_battlenet().filter(|p| p.exists())
}

#[cfg(windows)]
pub fn battlenet_running() -> bool {
    crate::winreg::process_running("Battle.net.exe")
}

#[cfg(not(windows))]
pub fn battlenet_running() -> bool {
    false
}

/// Starts `Battle.net.exe --game=wowf --install` without any console window. A running Battle.net receives the
/// request (single instance), so it is not closed first.
#[cfg(windows)]
pub fn start_install(settings: &Settings) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    let exe = battlenet_exe(settings).ok_or_else(|| "battlenet_missing".to_string())?;
    std::process::Command::new(&exe)
        .arg(format!("--game={PRODUCT}"))
        .arg("--install")
        .current_dir(exe.parent().unwrap_or(Path::new(".")))
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn start_install(_settings: &Settings) -> Result<(), String> {
    Err("battlenet_missing".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn website_does_not_change_the_local_client_status() {
        let mut settings = Settings::default();
        settings.server.website = "https://play.example.com".into();
        settings.game_dir.clear();
        let status = local_status(&settings);
        assert_eq!(status.state, "missing");
        assert!(status.installed_version.is_none());
        assert!(status.installed_build.is_none());
    }
}
