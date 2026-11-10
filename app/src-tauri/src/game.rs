//! Game preparation and start: WTF config, Launch Options\WoW (auto-login), Wow.exe / WowB.exe -launcherlogin.
//! macOS starts a copy of the game app instead: see macos.rs.

use crate::config::Settings;
use crate::i18n::tf;
use std::path::Path;
#[cfg(not(target_os = "macos"))]
use std::process::{Child, Command};

pub const WTF_NAME: &str = "AeternumLauncher.wtf";
/// The client's cache folder inside the game folder: WDB (creature, quest, item... caches) and ADB (hotfix cache).
pub const CACHE_DIR: &str = "Cache";
#[cfg(not(target_os = "macos"))]
const LAUNCH_OPTIONS: &str = r"Software\Blizzard Entertainment\Battle.net\Launch Options\WoW";

/// WTF/AeternumLauncher.wtf: a copy of Config.wtf with the portal and the language set.
pub fn write_wtf(settings: &Settings) -> Result<(), String> {
    let dir = Path::new(&settings.game_dir).join("WTF");
    std::fs::create_dir_all(&dir).map_err(|e| tf("wtf_folder", &[&e]))?;
    let path = dir.join(WTF_NAME);
    let base = dir.join("Config.wtf");
    let text = std::fs::read_to_string(&path).or_else(|_| std::fs::read_to_string(&base)).unwrap_or_default();
    let wanted = [
        ("portal", settings.server.portal.clone()),
        ("textLocale", settings.locale.clone()),
        ("audioLocale", settings.locale.clone()),
    ];
    let mut lines: Vec<String> = text
        .lines()
        .filter(|l| !wanted.iter().any(|(k, _)| l.starts_with(&format!("SET {k} "))))
        .map(str::to_string)
        .collect();
    for (k, v) in wanted {
        lines.push(format!("SET {k} \"{v}\""));
    }
    std::fs::write(&path, lines.join("\r\n") + "\r\n").map_err(|e| format!("{} : {e}", path.display()))
}

/// Stores the login ticket where the client reads it with -launcherlogin.
#[cfg(windows)]
pub fn write_launch_options(_settings: &Settings, portal: &str, ticket: &str, game_account: &str) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::*;
    let blob = crate::dpapi::protect(ticket.as_bytes())?;
    unsafe {
        let mut key = HKEY::default();
        RegCreateKeyExW(HKEY_CURRENT_USER, &HSTRING::from(LAUNCH_OPTIONS), 0, None, REG_OPTION_NON_VOLATILE, KEY_WRITE, None, &mut key, None)
            .ok()
            .map_err(|e| tf("registry", &[&e]))?;
        let set_str = |name: &str, value: &str| {
            let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
            RegSetValueExW(key, &HSTRING::from(name), 0, REG_SZ, Some(bytes)).ok()
        };
        let r = set_str("CONNECTION_STRING", portal)
            .and_then(|_| set_str("GAME_ACCOUNT", game_account))
            .and_then(|_| RegSetValueExW(key, &HSTRING::from("WEB_TOKEN"), 0, REG_BINARY, Some(&blob)).ok());
        let _ = RegCloseKey(key);
        r.map_err(|e| tf("registry", &[&e]))
    }
}

/// Linux: the same values in the prefix registry, through `reg import` (the Wine DPAPI blob is built here).
#[cfg(target_os = "linux")]
pub fn write_launch_options(settings: &Settings, portal: &str, ticket: &str, game_account: &str) -> Result<(), String> {
    use crate::dpapi::{wine, WEB_TOKEN_ENTROPY};
    let blob = wine::protect(ticket.as_bytes(), wine::PROTON_USER, &WEB_TOKEN_ENTROPY);
    let hexdata: Vec<String> = blob.iter().map(|b| format!("{b:02x}")).collect();
    let content = format!(
        "Windows Registry Editor Version 5.00\r\n\r\n[HKEY_CURRENT_USER\\{LAUNCH_OPTIONS}]\r\n\"WEB_TOKEN\"=hex:{}\r\n\"GAME_ACCOUNT\"=\"{game_account}\"\r\n\"CONNECTION_STRING\"=\"{portal}\"\r\n",
        hexdata.join(",")
    );
    // regedit files are UTF-16 LE with a BOM; Wine sees the Linux file system as Z:
    let mut bytes = vec![0xFF, 0xFE];
    for u in content.encode_utf16() {
        bytes.extend(u.to_le_bytes());
    }
    let dir = crate::config::config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join("launch-options.reg");
    std::fs::write(&file, bytes).map_err(|e| e.to_string())?;
    let wine_path = format!("Z:{}", file.to_string_lossy().replace('/', "\\"));
    let res = crate::system::run_in_prefix(settings, &["reg", "import", &wine_path]);
    let _ = std::fs::remove_file(&file);
    res
}

#[cfg(windows)]
pub fn spawn(settings: &Settings) -> Result<Child, String> {
    Command::new(settings.exe())
        .args(["-launcherlogin", "-config", WTF_NAME])
        .current_dir(&settings.game_dir)
        .spawn()
        .map_err(|e| tf("launch_failed", &[&settings.exe().file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), &e]))
}

#[cfg(target_os = "linux")]
pub fn spawn(settings: &Settings) -> Result<Child, String> {
    let (runner, mut env) = crate::system::runner(settings)?;
    let mut cmd: Vec<String> = runner;
    cmd.push(settings.exe().to_string_lossy().into_owned());
    cmd.extend(["-launcherlogin", "-config", WTF_NAME].map(String::from));
    let l = &settings.linux;
    if l.gamescope && crate::system::which("gamescope") {
        // gamescope starts its own Xwayland with a US keyboard unless told otherwise
        let (layout, variant) = l.keyboard.split_once('+').unwrap_or((l.keyboard.as_str(), ""));
        if !layout.is_empty() {
            env.push(("XKB_DEFAULT_LAYOUT".into(), layout.into()));
        }
        if !variant.is_empty() {
            env.push(("XKB_DEFAULT_VARIANT".into(), variant.into()));
        }
        let mut wrapped = vec!["gamescope".to_string()];
        wrapped.extend(l.gamescope_args.split_whitespace().map(String::from));
        wrapped.push("--".into());
        wrapped.extend(cmd);
        cmd = wrapped;
    }
    crate::system::clean_env(&mut Command::new(&cmd[0]))
        .args(&cmd[1..])
        .envs(env)
        .current_dir(&settings.game_dir)
        .spawn()
        .map_err(|e| tf("launch_failed", &[&cmd[0], &e]))
}

/// What clear_cache removed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CacheClear {
    pub files: u64,
    pub bytes: u64,
    /// Locked or unreadable files, links: left in place.
    pub skipped: u64,
}

/// A symbolic link or (Windows) any reparse point such as a junction: never followed, never deleted.
fn is_link(meta: &std::fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

/// The Cache folder to empty, or None when it cannot be resolved safely: the game folder must be set and hold the game
/// (Wow.exe / WowB.exe, or the game app on macOS), and Cache must be a real folder (not a link) directly inside it.
pub fn cache_dir(settings: &Settings) -> Option<std::path::PathBuf> {
    if settings.game_dir.trim().is_empty() || !settings.game_found() {
        return None;
    }
    let base = Path::new(&settings.game_dir).canonicalize().ok()?;
    let cache = base.join(CACHE_DIR);
    let meta = std::fs::symlink_metadata(&cache).ok()?;
    if is_link(&meta) || !meta.is_dir() {
        return None;
    }
    let real = cache.canonicalize().ok()?;
    (real.parent() == Some(base.as_path()) && real.file_name() == Some(std::ffi::OsStr::new(CACHE_DIR))).then_some(real)
}

/// Empties the client's Cache folder before a launch; the folder itself stays and the client fills it again. Only that
/// folder is touched (never WTF, Interface or anything else), links are not followed, locked files are skipped.
/// An error is reported to the caller, who logs it and goes on with the launch.
pub fn clear_cache(settings: &Settings) -> Result<CacheClear, String> {
    let cache = cache_dir(settings).ok_or_else(|| format!("no {CACHE_DIR} folder to clear in the game folder"))?;
    let mut report = CacheClear::default();
    clear_dir(&cache, &mut report);
    Ok(report)
}

fn clear_dir(dir: &Path, report: &mut CacheClear) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        report.skipped += 1;
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            report.skipped += 1;
            continue;
        };
        if is_link(&meta) {
            report.skipped += 1;
        } else if meta.is_dir() {
            clear_dir(&path, report);
            // only succeeds when it is empty now
            let _ = std::fs::remove_dir(&path);
        } else if std::fs::remove_file(&path).is_ok() {
            report.files += 1;
            report.bytes += meta.len();
        } else {
            report.skipped += 1;
        }
    }
}

#[cfg(all(test, not(target_os = "macos")))]
mod cache_tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("aeternum-launcher-cache-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, data: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }

    fn settings_for(game: &Path) -> Settings {
        Settings { game_dir: game.to_string_lossy().into_owned(), ..Settings::default() }
    }

    #[test]
    fn clears_only_the_cache_folder() {
        let root = scratch("only");
        let game = root.join("_forever_");
        write(&game.join("Wow.exe"), b"exe");
        write(&game.join("Cache/WDB/enUS/creaturecache.wdb"), b"0123456789");
        write(&game.join("Cache/ADB/enUS/DBCache.bin"), b"01234");
        write(&game.join("Cache/DamageMeter.bin"), b"x");
        write(&game.join("WTF/Config.wtf"), b"SET portal \"x\"");
        write(&game.join("Interface/AddOns/A/A.toc"), b"## Title: A");
        write(&root.join("Cache/outside.bin"), b"outside");

        let report = clear_cache(&settings_for(&game)).unwrap();
        assert_eq!(report, CacheClear { files: 3, bytes: 16, skipped: 0 });
        assert!(game.join("Cache").is_dir(), "the Cache folder itself stays");
        assert_eq!(fs::read_dir(game.join("Cache")).unwrap().count(), 0);
        assert!(game.join("Wow.exe").exists());
        assert!(game.join("WTF/Config.wtf").exists());
        assert!(game.join("Interface/AddOns/A/A.toc").exists());
        assert!(root.join("Cache/outside.bin").exists(), "nothing outside the game folder");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn refuses_unresolved_or_foreign_folders() {
        assert!(clear_cache(&Settings::default()).is_err(), "empty game folder");

        let root = scratch("foreign");
        write(&root.join("Cache/keep.bin"), b"keep");
        assert!(clear_cache(&settings_for(&root)).is_err(), "folder without the game binary");
        assert!(root.join("Cache/keep.bin").exists());

        let game = root.join("_forever_");
        write(&game.join("Wow.exe"), b"exe");
        assert!(clear_cache(&settings_for(&game)).is_err(), "no Cache folder yet");
        write(&game.join("Cache"), b"a file, not a folder");
        assert!(clear_cache(&settings_for(&game)).is_err(), "Cache is not a folder");
        assert!(game.join("Cache").is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_links() {
        let root = scratch("links");
        let game = root.join("_forever_");
        write(&game.join("Wow.exe"), b"exe");
        write(&root.join("elsewhere/keep.bin"), b"keep");
        fs::create_dir_all(game.join("Cache")).unwrap();
        std::os::unix::fs::symlink(root.join("elsewhere"), game.join("Cache/link")).unwrap();
        let report = clear_cache(&settings_for(&game)).unwrap();
        assert_eq!(report.skipped, 1);
        assert!(root.join("elsewhere/keep.bin").exists());

        fs::remove_dir_all(game.join("Cache")).unwrap();
        std::os::unix::fs::symlink(root.join("elsewhere"), game.join("Cache")).unwrap();
        assert!(clear_cache(&settings_for(&game)).is_err(), "Cache itself is a link");
        assert!(root.join("elsewhere/keep.bin").exists());
        let _ = fs::remove_dir_all(&root);
    }
}
