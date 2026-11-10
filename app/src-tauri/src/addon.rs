//! Client addons embedded in the launcher and synced into `<game>/Interface/AddOns/<Name>` before every launch.
//!
//! - **AeternumQuestTag** (repo folder `addon/AeternumQuestTag`, setting `questTagAddon`, default on): yellow "QUEST"
//!   above the nameplate of creatures the player's active quests still need.
//!
//! Rules:
//! - The installed copy carries `.launcher-version` (SHA-256 of the embedded content): files are written only when it
//!   differs or a file is missing or changed size.
//! - Only that one folder is written or removed; other addons are never touched. In WTF/Account/**/AddOns.txt only the
//!   addon's own line is set to `enabled` (added when the file has none) while its setting is on.
//! - Links and junctions are never followed; the game folder must hold the game binary.
//! - Setting off: the folder is removed only when it carries the launcher's version file. SavedVariables (in WTF) are
//!   never touched.
//! - A folder with that name that the launcher did not write (no version file): adopted when the setting is on (the
//!   name is ours), left alone when it is off.

use crate::config::Settings;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const ADDON_NAME: &str = "AeternumQuestTag";
pub const VERSION_FILE: &str = ".launcher-version";

/// (file name, content): AeternumQuestTag exactly as committed in `addon/AeternumQuestTag`.
pub const FILES: &[(&str, &[u8])] = &[
    ("AeternumQuestTag.toc", include_bytes!("../../../addon/AeternumQuestTag/AeternumQuestTag.toc")),
    ("Data.lua", include_bytes!("../../../addon/AeternumQuestTag/Data.lua")),
    ("AeternumQuestTag.lua", include_bytes!("../../../addon/AeternumQuestTag/AeternumQuestTag.lua")),
];

/// What a sync did.
#[derive(Debug, PartialEq, Eq)]
pub enum AddonSync {
    Installed { version: String, enabled_lines: usize },
    UpToDate { version: String, enabled_lines: usize },
    Removed,
    NotInstalled,
    /// A folder with that name the launcher did not install: left as it is.
    UserInstall,
}

/// SHA-256 (hex, 16 chars) of the embedded AeternumQuestTag files.
#[allow(dead_code)]
pub fn version() -> String {
    version_of(&owned(FILES))
}

fn owned(files: &[(&str, &[u8])]) -> Vec<(String, Vec<u8>)> {
    files.iter().map(|(n, d)| (n.to_string(), d.to_vec())).collect()
}

fn version_of(files: &[(String, Vec<u8>)]) -> String {
    let mut h = Sha256::new();
    for (name, data) in files {
        h.update(name.as_bytes());
        h.update([0u8]);
        h.update((data.len() as u64).to_le_bytes());
        h.update(data);
    }
    h.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect()
}

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

/// A real folder (not a link) or nothing yet; Err when the path is a link or a file.
fn real_dir_or_absent(p: &Path) -> Result<bool, String> {
    match std::fs::symlink_metadata(p) {
        Ok(m) if is_link(&m) => Err(format!("{} is a link", p.display())),
        Ok(m) if m.is_dir() => Ok(true),
        Ok(_) => Err(format!("{} is not a folder", p.display())),
        Err(_) => Ok(false),
    }
}

/// `<game>/Interface/AddOns`, or Err when the game folder is not set / does not hold the game.
pub fn addons_dir(settings: &Settings) -> Result<PathBuf, String> {
    if settings.game_dir.trim().is_empty() || !settings.game_found() {
        return Err("game folder not set or without the game".into());
    }
    let base = Path::new(&settings.game_dir).canonicalize().map_err(|e| e.to_string())?;
    Ok(base.join("Interface").join("AddOns"))
}

/// AeternumQuestTag: install / update (on) or remove (off).
pub fn sync(settings: &Settings, enabled: bool) -> Result<AddonSync, String> {
    sync_addon(settings, ADDON_NAME, enabled, &owned(FILES))
}

fn sync_addon(settings: &Settings, name: &str, enabled: bool, files: &[(String, Vec<u8>)]) -> Result<AddonSync, String> {
    let addons = addons_dir(settings)?;
    let interface = addons.parent().unwrap().to_path_buf();
    let target = addons.join(name);
    let ours = || target.join(VERSION_FILE).is_file();
    if !enabled {
        if real_dir_or_absent(&interface)? && real_dir_or_absent(&addons)? && real_dir_or_absent(&target)? {
            if !ours() {
                return Ok(AddonSync::UserInstall);
            }
            std::fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
            return Ok(AddonSync::Removed);
        }
        return Ok(AddonSync::NotInstalled);
    }
    let version = version_of(files);
    for dir in [&interface, &addons] {
        if !real_dir_or_absent(dir)? {
            std::fs::create_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
    }
    if !real_dir_or_absent(&target)? {
        std::fs::create_dir(&target).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    let enabled_lines = enable_in_addons_txt(Path::new(&settings.game_dir), name);
    let current = std::fs::read_to_string(target.join(VERSION_FILE)).unwrap_or_default();
    let complete = files.iter().all(|(rel, data)| {
        std::fs::symlink_metadata(target.join(rel)).map(|m| m.is_file() && m.len() == data.len() as u64).unwrap_or(false)
    });
    if current.trim() == version && complete {
        return Ok(AddonSync::UpToDate { version, enabled_lines });
    }
    // version file removed first, written last: an interrupted update is redone next time
    let _ = std::fs::remove_file(target.join(VERSION_FILE));
    for (rel, data) in files {
        let path = target.join(rel);
        // every folder between the addon folder and the file must be a real folder
        let mut dir = target.clone();
        let parts: Vec<&str> = rel.split('/').collect();
        for p in &parts[..parts.len() - 1] {
            dir = dir.join(p);
            if !real_dir_or_absent(&dir)? {
                std::fs::create_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
        }
        if let Ok(m) = std::fs::symlink_metadata(&path) {
            if is_link(&m) || m.is_dir() {
                return Err(format!("{} is not a plain file", path.display()));
            }
        }
        let tmp = path.with_file_name(format!("{}.tmp", parts[parts.len() - 1]));
        std::fs::write(&tmp, data).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    remove_stale(&target, &target, files);
    std::fs::write(target.join(VERSION_FILE), &version).map_err(|e| e.to_string())?;
    Ok(AddonSync::Installed { version, enabled_lines })
}

/// Plain files of an older version that are no longer shipped (inside the addon folder only, links skipped).
fn remove_stale(root: &Path, dir: &Path, files: &[(String, Vec<u8>)]) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if is_link(&meta) {
            continue;
        }
        let rel = path.strip_prefix(root).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
        if meta.is_dir() {
            remove_stale(root, &path, files);
            let _ = std::fs::remove_dir(&path); // only when empty
        } else if rel != VERSION_FILE && !files.iter().any(|(n, _)| *n == rel) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// In every WTF/Account/**/AddOns.txt: the `<name>:` line set to `enabled` (appended when missing). Other lines are
/// left as they are. Returns how many files changed.
fn enable_in_addons_txt(game: &Path, name: &str) -> usize {
    let mut changed = 0;
    let mut stack = vec![(game.join("WTF").join("Account"), 0u32)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let path = e.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
            if is_link(&meta) {
                continue;
            }
            if meta.is_dir() {
                if depth < 4 {
                    stack.push((path, depth + 1));
                }
            } else if e.file_name().to_string_lossy().eq_ignore_ascii_case("AddOns.txt") {
                let Ok(text) = std::fs::read_to_string(&path) else { continue };
                let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
                let mut found = false;
                let mut touched = false;
                let mut out: Vec<String> = text
                    .lines()
                    .map(|l| {
                        let (n, state) = l.split_once(':').unwrap_or((l, ""));
                        if n.trim() == name {
                            found = true;
                            if !state.trim().eq_ignore_ascii_case("enabled") {
                                touched = true;
                                return format!("{name}: enabled");
                            }
                        }
                        l.to_string()
                    })
                    .collect();
                if !found {
                    out.push(format!("{name}: enabled"));
                    touched = true;
                }
                if touched && std::fs::write(&path, out.join(nl) + nl).is_ok() {
                    changed += 1;
                }
            }
        }
    }
    changed
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::*;
    use std::fs;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("launcher-addon-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, data: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }

    fn game(root: &Path) -> (PathBuf, Settings) {
        let game = root.join("_forever_");
        write(&game.join("Wow.exe"), b"exe");
        let s = Settings { game_dir: game.to_string_lossy().into_owned(), ..Settings::default() };
        (game, s)
    }

    #[test]
    fn embedded_addon_is_complete() {
        let toc = std::str::from_utf8(FILES[0].1).unwrap();
        assert!(toc.contains("## Interface: 16001"));
        assert!(toc.contains("## SavedVariables: AeternumQuestTagDB"));
        for (name, _) in &FILES[1..] {
            assert!(toc.lines().any(|l| l.trim() == *name), "{name} listed in the toc");
        }
        assert!(std::str::from_utf8(FILES[1].1).unwrap().contains("Q["), "generated data present");
        assert_eq!(version().len(), 16);
    }

    #[test]
    fn installs_updates_and_keeps_other_addons() {
        let root = scratch("install");
        let (game, s) = game(&root);
        write(&game.join("Interface/AddOns/Other/Other.toc"), b"## Title: Other");
        let txt = game.join("WTF/Account/1#1/70/Char/AddOns.txt");
        write(&txt, b"Other: disabled\r\nAeternumQuestTag: disabled\r\n");

        let r = sync(&s, true).unwrap();
        assert_eq!(r, AddonSync::Installed { version: version(), enabled_lines: 1 });
        let dir = game.join("Interface/AddOns").join(ADDON_NAME);
        for (name, data) in FILES {
            assert_eq!(fs::read(dir.join(name)).unwrap(), *data);
        }
        assert_eq!(fs::read_to_string(dir.join(VERSION_FILE)).unwrap(), version());
        assert_eq!(fs::read_to_string(&txt).unwrap(), "Other: disabled\r\nAeternumQuestTag: enabled\r\n");
        assert!(game.join("Interface/AddOns/Other/Other.toc").exists());

        assert_eq!(sync(&s, true).unwrap(), AddonSync::UpToDate { version: version(), enabled_lines: 0 });

        // an older install (other version, stale file) is replaced
        fs::write(dir.join(VERSION_FILE), "old").unwrap();
        fs::write(dir.join("Old.lua"), "x").unwrap();
        fs::write(dir.join("Data.lua"), "stale").unwrap();
        assert!(matches!(sync(&s, true).unwrap(), AddonSync::Installed { .. }));
        assert!(!dir.join("Old.lua").exists());
        assert_eq!(fs::read(dir.join("Data.lua")).unwrap(), FILES[1].1);

        // a damaged install with the right version file is repaired
        fs::remove_file(dir.join("AeternumQuestTag.lua")).unwrap();
        assert!(matches!(sync(&s, true).unwrap(), AddonSync::Installed { .. }));
        assert!(dir.join("AeternumQuestTag.lua").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn off_removes_only_our_install() {
        let root = scratch("off");
        let (game, s) = game(&root);
        assert_eq!(sync(&s, false).unwrap(), AddonSync::NotInstalled);
        sync(&s, true).unwrap();
        write(&game.join("Interface/AddOns/Other/Other.toc"), b"## Title: Other");
        assert_eq!(sync(&s, false).unwrap(), AddonSync::Removed);
        assert!(!game.join("Interface/AddOns").join(ADDON_NAME).exists());
        assert!(game.join("Interface/AddOns/Other/Other.toc").exists());
        // a folder with that name the launcher did not write is left alone
        write(&game.join("Interface/AddOns").join(ADDON_NAME).join("mine.lua"), b"x");
        assert_eq!(sync(&s, false).unwrap(), AddonSync::UserInstall);
        assert!(game.join("Interface/AddOns").join(ADDON_NAME).join("mine.lua").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn refuses_without_game() {
        assert!(sync(&Settings::default(), true).is_err());
        let root = scratch("nogame");
        let s = Settings { game_dir: root.to_string_lossy().into_owned(), ..Settings::default() };
        assert!(sync(&s, true).is_err());
        assert!(!root.join("Interface").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_links() {
        let root = scratch("links");
        let (game, s) = game(&root);
        fs::create_dir_all(root.join("elsewhere")).unwrap();
        fs::create_dir_all(game.join("Interface")).unwrap();
        std::os::unix::fs::symlink(root.join("elsewhere"), game.join("Interface/AddOns")).unwrap();
        assert!(sync(&s, true).is_err());
        assert_eq!(fs::read_dir(root.join("elsewhere")).unwrap().count(), 0);
        let _ = fs::remove_dir_all(&root);
    }
}
