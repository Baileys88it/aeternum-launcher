//! Settings (game folder, language, Linux runner...), server definition and the saved login session.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Public half of the Ed25519 key built into stock TrinityCore (used by the forever branch too).
pub const STOCK_PUBLIC_KEY: &str = "02596F0D0C061A8B30745988FD72C59E29EC367FB0F341F28E0F08D037BAFC69";
/// Public certificate of the server's private CA (never the key).
pub const CA_PEM: &str = include_str!(concat!(env!("OUT_DIR"), "/forever_root_ca.crt"));

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Server {
    pub name: String,
    /// Host name the client and the certificate use.
    pub portal: String,
    /// Where that host name really is (IP or DNS name of the server). Built into the exe only: never sent to the UI,
    /// never saved in settings.json, never shown.
    #[serde(skip)]
    pub address: String,
    /// Fallback for players on the server's LAN when the router has no NAT loopback (built in, never shown).
    #[serde(skip)]
    pub lan_address: String,
    pub bnet_port: u16,
    pub rest_port: u16,
    pub world_port: u16,
    pub public_key: String,
    pub region_group: u32,
    pub website: String,
}

/// First non-empty build-time string. `AETERNUM_*` is accepted as an alias of `FOREVER_*`.
fn pick_env(primary: Option<&'static str>, fallback: Option<&'static str>) -> Option<&'static str> {
    [primary, fallback].into_iter().flatten().map(str::trim).find(|v| !v.is_empty())
}

fn built_in(value: Option<&'static str>, default: &str) -> String {
    value.map(str::trim).filter(|v| !v.is_empty()).unwrap_or(default).to_string()
}

/// Port from a build-time variable. `0` and anything that is not a port mean "not set".
fn built_port(value: Option<&'static str>) -> u16 {
    value.and_then(|v| v.trim().parse().ok()).filter(|p| *p != 0).unwrap_or(0)
}

/// Operator settings, from optional `FOREVER_*` / `AETERNUM_*` variables at build time.
/// Players cannot change the realm (no Settings > Server section; saved overrides are ignored).
/// Unset host, ports, address, and website stay empty: there is no production realm and no public site.
impl Default for Server {
    fn default() -> Self {
        Self {
            name: built_in(pick_env(option_env!("AETERNUM_SERVER_NAME"), option_env!("FOREVER_SERVER_NAME")), ""),
            portal: built_in(pick_env(option_env!("AETERNUM_PORTAL"), option_env!("FOREVER_PORTAL")), ""),
            address: String::new(), // encrypted in the exe, see secret.rs
            lan_address: String::new(),
            bnet_port: built_port(pick_env(option_env!("AETERNUM_BNET_PORT"), option_env!("FOREVER_BNET_PORT"))),
            rest_port: built_port(pick_env(option_env!("AETERNUM_REST_PORT"), option_env!("FOREVER_REST_PORT"))),
            world_port: built_port(pick_env(option_env!("AETERNUM_WORLD_PORT"), option_env!("FOREVER_WORLD_PORT"))),
            public_key: STOCK_PUBLIC_KEY.into(),
            region_group: 8,
            website: built_in(pick_env(option_env!("AETERNUM_WEBSITE"), option_env!("FOREVER_WEBSITE")), ""),
        }
    }
}

impl Server {
    /// Built-in addresses to try, public first, then the LAN fallback.
    pub fn candidates(&self) -> Vec<String> {
        crate::secret::addresses()
    }

    /// Portal host, all three ports, and at least one server address were set at build time.
    pub fn login_configured(&self) -> bool {
        !self.portal.is_empty() && self.bnet_port != 0 && self.rest_port != 0 && self.world_port != 0 && !self.candidates().is_empty()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LinuxSettings {
    /// Proton folder (run through umu-run) or "wine".
    pub runner: String,
    pub prefix: String,
    pub gamescope: bool,
    pub gamescope_args: String,
    pub keyboard: String,
}

impl Default for LinuxSettings {
    fn default() -> Self {
        Self {
            runner: String::new(),
            prefix: home().join("Games/forever-wow").to_string_lossy().into_owned(),
            gamescope: false,
            gamescope_args: "-f".into(),
            keyboard: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub server: Server,
    /// The `_forever_` folder (contains Wow.exe; the old `_classic_beta_` folder contains WowB.exe), or the game .app on macOS.
    pub game_dir: String,
    /// textLocale / audioLocale, e.g. frFR.
    pub locale: String,
    /// Launcher language: fr, en, es, pt, or empty for the system language.
    pub language: String,
    pub linux: LinuxSettings,
    /// Battle.net.exe (or its folder); empty = registry / default path.
    pub battlenet_path: String,
    /// Empty the game's Cache folder (WDB / ADB) before every launch. On by default; settings.json files written before
    /// this option existed get the default too.
    pub clear_cache_on_launch: bool,
    /// Install / update the AeternumQuestTag client addon before every launch (default on); off removes it.
    pub quest_tag_addon: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server: Server::default(),
            game_dir: String::new(),
            locale: String::new(),
            language: String::new(),
            linux: LinuxSettings::default(),
            battlenet_path: String::new(),
            clear_cache_on_launch: true,
            quest_tag_addon: true,
        }
    }
}

/// Login kept between launches: the 30-day launcher token of the bnetserver, never the password.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub email: String,
    pub token: String,
    pub expires: u64,
}

pub const LOCALES: &[&str] = &["frFR", "enUS", "enGB", "deDE", "esES", "esMX", "itIT", "ptBR", "ruRU", "koKR", "zhCN", "zhTW"];

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(home).join("aeternum-launcher-app")
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    let tmp = path.with_extension("tmp");
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
        f.write_all(data)?;
    }
    #[cfg(not(unix))]
    std::fs::write(&tmp, data)?;
    std::fs::rename(tmp, path)
}

impl Settings {
    pub fn load() -> Self {
        let mut s: Settings = read_json(&config_dir().join("settings.json")).unwrap_or_default();
        // The realm is whatever was compiled in. A blank portal, port, address, or website stays blank.
        s.server = Server::default();
        if s.game_dir.is_empty() {
            s.game_dir = detect_game_dir().unwrap_or_default();
        } else if let Some(moved) = upgraded_game_dir(Path::new(&s.game_dir)) {
            // client upgraded from the Classic beta (_classic_beta_, WowB.exe) to WoW Classic Forever (_forever_, Wow.exe):
            // switch to the new folder (the server only accepts 70707) and save it, so players do not have to pick it
            s.game_dir = moved;
            let _ = s.save();
        }
        if s.locale.is_empty() {
            s.locale = installed_locale(Path::new(&s.game_dir)).unwrap_or_else(|| "enUS".into());
        }
        #[cfg(target_os = "linux")]
        {
            if s.linux.runner.is_empty() {
                s.linux.runner = crate::system::default_runner();
            }
            if s.linux.keyboard.is_empty() {
                s.linux.keyboard = crate::system::keyboard_layout();
            }
        }
        s
    }

    pub fn save(&self) -> std::io::Result<()> {
        write_private(&config_dir().join("settings.json"), &serde_json::to_vec_pretty(self).unwrap())
    }

    /// Wow.exe / WowB.exe (Windows, Linux/Wine) or the game .app (macOS).
    pub fn exe(&self) -> PathBuf {
        game_binary(Path::new(&self.game_dir))
    }

    pub fn game_found(&self) -> bool {
        !self.game_dir.is_empty() && self.exe().exists()
    }
}

impl Session {
    pub fn load() -> Option<Self> {
        read_json::<Session>(&config_dir().join("session.json")).filter(|s| !s.token.is_empty())
    }

    pub fn save(&self) -> std::io::Result<()> {
        write_private(&config_dir().join("session.json"), &serde_json::to_vec(self).unwrap())
    }

    pub fn clear() {
        let _ = std::fs::remove_file(config_dir().join("session.json"));
    }
}

/// Language the client was installed with, from the Tags column of .build.info next to the game folder.
pub fn installed_locale(game_dir: &Path) -> Option<String> {
    let info = std::fs::read_to_string(game_dir.parent()?.join(".build.info")).ok()?;
    let line = info.lines().find(|l| is_product_line(l))?;
    LOCALES.iter().find(|l| line.contains(&format!("{l} text"))).map(|l| l.to_string())
}

/// Full client version (e.g. 1.60.1.70707), from the Version column of .build.info next to the game folder.
pub fn installed_version(game_dir: &Path) -> Option<String> {
    let info = std::fs::read_to_string(game_dir.parent()?.join(".build.info")).ok()?;
    let mut lines = info.lines();
    let header: Vec<&str> = lines.next()?.split('|').map(|c| c.split('!').next().unwrap_or("")).collect();
    let version = header.iter().position(|c| *c == "Version")?;
    let product = header.iter().position(|c| *c == "Product")?;
    lines
        .map(|l| l.split('|').collect::<Vec<_>>())
        .find(|row| row.get(product).is_some_and(|p| PRODUCTS.contains(p)))
        .and_then(|row| row.get(version).map(|v| v.to_string()))
        .filter(|v| !v.is_empty())
}

/// Battle.net products of the client: WoW Classic Forever (_forever_, Wow.exe) and the old Classic beta (_classic_beta_, WowB.exe).
pub const PRODUCTS: [&str; 2] = ["wowf", "wow_classic_beta"];

/// Client executable names, newest first.
pub const GAME_EXES: [&str; 2] = ["Wow.exe", "WowB.exe"];

fn is_product_line(line: &str) -> bool {
    line.split('|').any(|f| PRODUCTS.contains(&f))
}

#[cfg(not(target_os = "macos"))]
fn game_binary(dir: &Path) -> PathBuf {
    GAME_EXES.iter().map(|e| dir.join(e)).find(|p| p.exists()).unwrap_or_else(|| dir.join(GAME_EXES[0]))
}

/// Re-signed copy of the game app that the launcher starts on macOS (see macos.rs), next to the original.
#[cfg(target_os = "macos")]
pub const MAC_COPY_NAME: &str = "World of Warcraft Forever.app";

/// The first .app in the folder (the Classic beta app name is not fixed), other than the launcher's copy.
#[cfg(target_os = "macos")]
fn game_binary(dir: &Path) -> PathBuf {
    std::fs::read_dir(dir)
        .ok()
        .and_then(|d| {
            let mut apps: Vec<PathBuf> = d
                .flatten()
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    !name.starts_with('.') && name != MAC_COPY_NAME
                })
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "app"))
                .collect();
            apps.sort();
            apps.into_iter().next()
        })
        .unwrap_or_else(|| dir.join("World of Warcraft.app"))
}

/// Client folder names, newest first: WoW Classic Forever, then the old Classic beta.
pub const GAME_DIRS: [&str; 2] = ["_forever_", "_classic_beta_"];

/// The folder to use instead of the saved one, if any:
/// - the saved folder is the old Classic beta (`_classic_beta_`, or it holds WowB.exe) and a `_forever_` folder with
///   Wow.exe sits next to it: always `_forever_`, even if WowB.exe is still there (the server only accepts the
///   WoW Classic Forever build, the old client can never log in);
/// - the saved folder no longer holds the game: a `_forever_` folder next to it with Wow.exe, else any detected install.
fn upgraded_game_dir(saved: &Path) -> Option<String> {
    let forever = saved
        .parent()
        .map(|p| p.join(GAME_DIRS[0]))
        .filter(|p| p.as_path() != saved && p.join(GAME_EXES[0]).exists());
    let is_old_client = saved
        .file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(GAME_DIRS[1]))
        || saved.join(GAME_EXES[1]).exists();
    if is_old_client {
        if let Some(f) = &forever {
            return Some(f.to_string_lossy().into_owned());
        }
    }
    if game_binary(saved).exists() {
        return None;
    }
    forever
        .map(|p| p.to_string_lossy().into_owned())
        .or_else(detect_game_dir)
        .filter(|d| Path::new(d) != saved)
}

pub fn detect_game_dir() -> Option<String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    // Battle.net installs: uninstall entries + Battle.net default install folder
    #[cfg(windows)]
    for dir in battlenet_install_roots() {
        for name in GAME_DIRS {
            candidates.push(dir.join(name));
        }
    }
    // next to the launcher itself (e.g. <games folder>\forever-launcher.exe + _forever_\Wow.exe)
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        for name in GAME_DIRS {
            candidates.push(dir.join(name));
        }
        // the launcher may also have been put inside the client folder
        candidates.push(dir.clone());
    }
    #[cfg(windows)]
    {
        let mut roots: Vec<String> = vec!["C:\\Program Files (x86)".into(), "C:\\Program Files".into()];
        for drive in ['D', 'E'] {
            roots.push(format!("{drive}:\\"));
            roots.push(format!("{drive}:\\{}", "Games"));
        }
        for root in &roots {
            candidates.push(Path::new(root).join("World of Warcraft").join("_forever_"));
            candidates.push(Path::new(root).join("World of Warcraft").join("_classic_beta_"));
        }
    }
    #[cfg(target_os = "macos")]
    for root in ["/Applications/World of Warcraft", &home().join("Applications/World of Warcraft").to_string_lossy()] {
        candidates.push(Path::new(root).join("_forever_"));
        candidates.push(Path::new(root).join("_classic_beta_"));
    }
    #[cfg(target_os = "linux")]
    {
        // a Lutris entry that already points at Wow.exe / WowB.exe
        if let Ok(dir) = std::fs::read_dir(home().join(".local/share/lutris/games")) {
            for entry in dir.flatten() {
                if let Ok(text) = std::fs::read_to_string(entry.path()) {
                    for line in text.lines() {
                        if let Some(exe) = line.trim().strip_prefix("exe:") {
                            let exe = exe.trim().trim_matches(|c| c == '\'' || c == '"');
                            let lower = exe.to_ascii_lowercase();
                            if lower.ends_with("/wow.exe") || lower.ends_with("/wowb.exe") {
                                if let Some(parent) = Path::new(exe).parent() {
                                    candidates.push(parent.to_path_buf());
                                }
                            }
                        }
                    }
                }
            }
        }
        for p in ["Games/battlenet/drive_c/Program Files (x86)/World of Warcraft/_forever_",
                  "Games/World of Warcraft/_forever_",
                  "Games/battlenet/drive_c/Program Files (x86)/World of Warcraft/_classic_beta_",
                  "Games/World of Warcraft/_classic_beta_"] {
            candidates.push(home().join(p));
        }
    }
    candidates.into_iter().find(|p| game_binary(p).exists()).map(|p| p.to_string_lossy().into_owned())
}

/// World of Warcraft folders known to Battle.net on this PC: "World of Warcraft" uninstall entries, the Battle.net
/// default install path (Battle.net.config) and the paths stored in the Battle.net agent database.
#[cfg(windows)]
fn battlenet_install_roots() -> Vec<PathBuf> {
    use crate::winreg::{read_string, subkeys};
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    let mut out: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf| {
        if !out.contains(&p) {
            out.push(p);
        }
    };
    for base in [r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall", r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"] {
        for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            for key in subkeys(root, base) {
                let sub = format!("{base}\\{key}");
                let name = read_string(root, &sub, "DisplayName").unwrap_or_default();
                if name.contains("World of Warcraft") {
                    if let Some(loc) = read_string(root, &sub, "InstallLocation") {
                        let p = PathBuf::from(loc.trim_matches('"'));
                        // InstallLocation may be the WoW root or a product subfolder
                        let is_sub = p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('_'));
                        push(if is_sub { p.parent().map(Path::to_path_buf).unwrap_or(p) } else { p });
                    }
                }
            }
        }
    }
    // %APPDATA%\Battle.net\Battle.net.config: "DefaultInstallPath": "<drive>:/Games"
    if let Some(appdata) = std::env::var_os("APPDATA") {
        if let Ok(text) = std::fs::read_to_string(Path::new(&appdata).join("Battle.net").join("Battle.net.config")) {
            if let Some(i) = text.find("\"DefaultInstallPath\"") {
                if let Some(v) = text[i + 20..].split('"').nth(1) {
                    push(Path::new(&v.replace('/', "\\")).join("World of Warcraft"));
                }
            }
        }
    }
    // %ProgramData%\Battle.net\Agent\product.db (protobuf): install paths appear as plain strings
    let pd = std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\ProgramData"));
    if let Ok(db) = std::fs::read(pd.join("Battle.net").join("Agent").join("product.db")) {
        let text = String::from_utf8_lossy(&db);
        for part in text.split(|c: char| c.is_control()) {
            let part = part.trim();
            if part.len() > 3 && part.as_bytes()[1] == b':' && part.contains("World of Warcraft") {
                let end = part.find("World of Warcraft").unwrap() + "World of Warcraft".len();
                push(PathBuf::from(part[..end].replace('/', "\\")));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_cache_on_launch_defaults_on() {
        assert!(Settings::default().clear_cache_on_launch);
        let old: Settings = serde_json::from_str(r#"{"gameDir":"","locale":"enUS"}"#).unwrap();
        assert!(old.clear_cache_on_launch, "settings.json without the key keeps the default");
        let off: Settings = serde_json::from_str(r#"{"clearCacheOnLaunch":false}"#).unwrap();
        assert!(!off.clear_cache_on_launch);
    }

    #[test]
    fn quest_tag_addon_defaults_on() {
        assert!(Settings::default().quest_tag_addon);
        let old: Settings = serde_json::from_str(r#"{"gameDir":"","clearCacheOnLaunch":false}"#).unwrap();
        assert!(old.quest_tag_addon, "settings.json without the key keeps the default");
        let off: Settings = serde_json::from_str(r#"{"questTagAddon":false}"#).unwrap();
        assert!(!off.quest_tag_addon);
    }

    fn env_set(primary: Option<&str>, fallback: Option<&str>) -> bool {
        [primary, fallback].into_iter().flatten().any(|v| !v.trim().is_empty())
    }

    #[test]
    fn unset_build_env_leaves_login_and_website_empty() {
        let s = Server::default();
        if !env_set(option_env!("AETERNUM_PORTAL"), option_env!("FOREVER_PORTAL")) {
            assert!(s.portal.is_empty(), "portal must default empty");
        }
        if !env_set(option_env!("AETERNUM_BNET_PORT"), option_env!("FOREVER_BNET_PORT")) {
            assert_eq!(s.bnet_port, 0);
        }
        if !env_set(option_env!("AETERNUM_REST_PORT"), option_env!("FOREVER_REST_PORT")) {
            assert_eq!(s.rest_port, 0);
        }
        if !env_set(option_env!("AETERNUM_WORLD_PORT"), option_env!("FOREVER_WORLD_PORT")) {
            assert_eq!(s.world_port, 0);
        }
        let website_set = env_set(option_env!("AETERNUM_WEBSITE"), option_env!("FOREVER_WEBSITE"));
        if !website_set {
            assert!(s.website.is_empty(), "website must default empty");
        }
        assert!(!s.login_configured() || env_set(option_env!("AETERNUM_PORTAL"), option_env!("FOREVER_PORTAL")));
    }

    #[test]
    fn missing_port_is_not_configured() {
        let s = Server {
            portal: "portal.example.invalid".into(),
            bnet_port: 0,
            rest_port: 1,
            world_port: 1,
            ..Server::default()
        };
        assert!(!s.login_configured());
    }
}
