mod addon;
mod api;
mod client;
mod config;
// macOS: only the entropy is used (see macos.rs); the rest is kept for its tests
#[cfg_attr(target_os = "macos", allow(dead_code))]
mod dpapi;
mod game;
mod guard;
mod i18n;
mod instance;
#[cfg(target_os = "macos")]
mod macos;
// macOS: no external memory patch (see macos.rs); kept for its tests
#[cfg_attr(target_os = "macos", allow(dead_code))]
mod patcher;
mod secret;
mod system;
mod updater;
#[cfg(windows)]
mod winreg;

use config::{Session, Settings};
use i18n::{t, tf};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
#[cfg(not(target_os = "macos"))]
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

#[cfg(windows)]
pub use system::admin_setup;
#[cfg(windows)]
pub use updater::apply as apply_update;
pub use instance::single_instance;

struct AppState {
    settings: Mutex<Settings>,
    playing: AtomicBool,
    /// Self-update offered by the manifest.
    update: Mutex<Option<updater::Manifest>>,
    /// Result of a self-update, from the command line of the restarted launcher: ("updated"|"error", text).
    update_note: Mutex<Option<(String, String)>>,
    /// Forbidden tool found by the watchdog (guard.rs), None when clean.
    blocked: Mutex<Option<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    settings: Settings,
    email: Option<String>,
    platform: &'static str,
    version: &'static str,
    locales: &'static [&'static str],
    runners: Vec<(String, String)>,
    game_found: bool,
    game_version: Option<String>,
    playing: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GameEvent {
    kind: &'static str,
    message: String,
}

fn emit(app: &AppHandle, kind: &'static str, message: impl Into<String>) {
    let _ = app.emit("game", GameEvent { kind, message: message.into() });
}

fn runners() -> Vec<(String, String)> {
    #[cfg(target_os = "linux")]
    {
        let mut r = system::list_protons();
        r.push((t("wine_system").into(), "wine".into()));
        r
    }
    #[cfg(not(target_os = "linux"))]
    Vec::new()
}

#[tauri::command]
fn get_state(state: State<AppState>) -> Snapshot {
    let settings = state.settings.lock().unwrap().clone();
    Snapshot {
        game_found: settings.game_found(),
        game_version: config::installed_version(std::path::Path::new(&settings.game_dir)),
        email: Session::load().map(|s| s.email),
        platform: if cfg!(windows) { "windows" } else if cfg!(target_os = "macos") { "macos" } else { "linux" },
        version: env!("CARGO_PKG_VERSION"),
        locales: config::LOCALES,
        runners: runners(),
        playing: state.playing.load(Ordering::SeqCst),
        settings,
    }
}

#[tauri::command]
fn save_settings(state: State<AppState>, settings: Settings) -> Result<(), String> {
    let mut settings = settings;
    // the server is not editable from the UI: always the built-in one
    settings.server = Default::default();
    settings.save().map_err(|e| e.to_string())?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

/// Language of the messages sent by the Rust side (the UI resolves "system language" itself).
#[tauri::command]
fn set_language(language: String) {
    i18n::set(&language);
}

/// Language installed with the client in that folder (to preselect it).
#[tauri::command]
fn detect_locale(game_dir: String) -> Option<String> {
    config::installed_locale(std::path::Path::new(&game_dir))
}

#[tauri::command]
async fn login(state: State<'_, AppState>, email: String, password: String) -> Result<String, String> {
    let server = state.settings.lock().unwrap().server.clone();
    let api = api::Api::new(&server).await?;
    let ticket = api.login(email.trim(), &password).await?;
    let token = api.remember(&ticket).await?;
    let check = api.launcher_login(&token.token).await?;
    Session { email: check.email.clone(), token: token.token, expires: token.expires }
        .save()
        .map_err(|e| tf("session_save", &[&e]))?;
    Ok(check.email.to_lowercase())
}

#[tauri::command]
async fn logout(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(s) = Session::load() {
        let server = state.settings.lock().unwrap().server.clone();
        if let Ok(api) = api::Api::new(&server).await {
            api.forget(&s.token).await;
        }
    }
    Session::clear();
    Ok(())
}

#[derive(Serialize)]
struct Status {
    online: bool,
    login: bool,
}

#[tauri::command]
async fn server_status(state: State<'_, AppState>) -> Result<Status, String> {
    let s = state.settings.lock().unwrap().server.clone();
    if !s.login_configured() {
        return Ok(Status { online: false, login: false });
    }
    let Ok(addr) = api::reachable(&s, s.bnet_port).await else {
        return Ok(Status { online: false, login: false });
    };
    // 1.2.4: the reachable() connect above already proves the bnet port is open; a second bare
    // connect to it only added an extra 'SSL Handshake failed' line in Bnet.log per poll.
    let world = api::port_open(&addr.ip().to_string(), s.world_port).await;
    Ok(Status { online: world, login: true })
}

#[tauri::command]
async fn news(state: State<'_, AppState>) -> Result<Vec<api::NewsItem>, String> {
    let website = state.settings.lock().unwrap().server.website.clone();
    if website.is_empty() {
        return Ok(Vec::new());
    }
    api::news(&website).await
}

#[tauri::command]
async fn checks(state: State<'_, AppState>) -> Result<system::Checks, String> {
    let settings = state.settings.lock().unwrap().clone();
    tokio::task::spawn_blocking(move || system::check(&settings)).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn repair(state: State<'_, AppState>) -> Result<system::Checks, String> {
    let settings = state.settings.lock().unwrap().clone();
    tokio::task::spawn_blocking(move || {
        system::repair(&settings)?;
        Ok(system::check(&settings))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Local client check: the game folder is missing or present. No website is asked.
#[tauri::command]
fn client_status(state: State<AppState>) -> Result<client::ClientStatus, String> {
    let mut settings = state.settings.lock().unwrap().clone();
    if !settings.game_found() {
        // e.g. Battle.net has just installed the client: look for it again and remember it
        if let Some(dir) = config::detect_game_dir() {
            settings.game_dir = dir;
            if settings.locale.is_empty() || settings.locale == "enUS" {
                if let Some(l) = config::installed_locale(std::path::Path::new(&settings.game_dir)) {
                    settings.locale = l;
                }
            }
            if settings.game_found() {
                let _ = settings.save();
                *state.settings.lock().unwrap() = settings.clone();
            }
        }
    }
    Ok(client::local_status(&settings))
}

/// "Download client": Battle.net --game=wowf --install. "started", or "missing" (the UI then
/// explains how to install Battle.net and opens its download page).
#[tauri::command]
fn install_client(state: State<AppState>) -> Result<&'static str, String> {
    let settings = state.settings.lock().unwrap().clone();
    match client::start_install(&settings) {
        Ok(()) => Ok("started"),
        Err(e) if e == "battlenet_missing" => Ok("missing"),
        Err(e) => Err(e),
    }
}

#[tauri::command]
fn battlenet_download(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(client::BATTLENET_DOWNLOAD, None::<&str>).map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateInfo {
    current: &'static str,
    available: Option<updater::Manifest>,
    /// ("updated" | "error", text) after a restart by the updater.
    note: Option<(String, String)>,
}

#[tauri::command]
async fn update_check(state: State<'_, AppState>) -> Result<UpdateInfo, String> {
    let note = state.update_note.lock().unwrap().take();
    // mandatory update: always offered while the manifest has a newer version (unreachable manifest: play allowed)
    let available = updater::check().await;
    *state.update.lock().unwrap() = available.clone();
    Ok(UpdateInfo { current: env!("CARGO_PKG_VERSION"), available, note })
}

#[derive(Clone, Serialize)]
struct UpdateProgress {
    stage: &'static str,
    done: u64,
    total: u64,
}

/// Downloads + verifies + stages the new launcher, then hands over to it and closes this one.
#[tauri::command]
async fn update_install(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if state.playing.load(Ordering::SeqCst) {
        return Err(t("already_running").into());
    }
    let manifest = state.update.lock().unwrap().clone().ok_or("no update")?;
    let a = app.clone();
    let staged = updater::download(&manifest, move |stage, done, total| {
        let _ = a.emit("update", UpdateProgress { stage, done, total });
    })
    .await?;
    let _ = app.emit("update", UpdateProgress { stage: "install", done: 0, total: 1 });
    updater::start_apply(&staged)?;
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        a.exit(0);
    });
    Ok(())
}

#[tauri::command]
async fn play(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    // mandatory self-update: no play with an outdated launcher while the manifest offers a newer one
    if state.update.lock().unwrap().is_some() {
        return Err(t("update_required").into());
    }
    if let Some(tool) = state.blocked.lock().unwrap().clone() {
        return Err(tf("tool_blocked", &[&tool]));
    }
    let game_dir = state.settings.lock().unwrap().game_dir.clone();
    if instance::game_running(&game_dir) {
        return Err(t("game_already_running").into());
    }
    if state.playing.swap(true, Ordering::SeqCst) {
        return Err(t("already_running").into());
    }
    let settings = state.settings.lock().unwrap().clone();
    let result = start(&app, settings).await;
    if result.is_err() {
        state.playing.store(false, Ordering::SeqCst);
    }
    result
}

async fn start(app: &AppHandle, settings: Settings) -> Result<(), String> {
    if !settings.game_found() {
        return Err(t("game_not_found").into());
    }
    // No remote client-build allowlist. A realm that cares about the build enforces it at login.
    let session = Session::load().ok_or(t("login_first"))?;
    emit(app, "step", t("step_connecting"));
    let api = api::Api::new(&settings.server).await?;
    let login = match api.launcher_login(&session.token).await {
        Err(e) if e == "expired" => {
            Session::clear();
            return Err(t("session_expired").into());
        }
        other => other?,
    };

    emit(app, "step", t("step_preparing"));
    if settings.clear_cache_on_launch {
        // never fatal: the result goes to the log and the launch goes on
        let s = settings.clone();
        match tokio::task::spawn_blocking(move || game::clear_cache(&s)).await {
            Ok(Ok(r)) => emit(app, "log", format!("cache: {} files removed ({} KB), {} skipped", r.files, r.bytes / 1024, r.skipped)),
            Ok(Err(e)) => emit(app, "log", format!("cache: not cleared ({e})")),
            Err(e) => emit(app, "log", format!("cache: not cleared ({e})")),
        }
    }
    {
        // AeternumQuestTag client addon: installed / updated when the setting is on, removed when off. Never fatal.
        let s = settings.clone();
        let on = settings.quest_tag_addon;
        match tokio::task::spawn_blocking(move || addon::sync(&s, on)).await {
            Ok(Ok(r)) => emit(app, "log", format!("addon {}: {r:?}", addon::ADDON_NAME)),
            Ok(Err(e)) => emit(app, "log", format!("addon {}: not synced ({e})", addon::ADDON_NAME)),
            Err(e) => emit(app, "log", format!("addon {}: not synced ({e})", addon::ADDON_NAME)),
        }
    }
    let s = settings.clone();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        // the game (Wow.exe, or WowB.exe on the old Classic beta) resolves the portal via OS DNS: force hosts/CA to the configured server before launch.
        let checks = system::check(&s);
        if !checks.hosts || (cfg!(windows) && !checks.certificate) {
            let _ = system::repair(&s);
            let again = system::check(&s);
            if !again.hosts {
                return Err(again.hosts_detail);
            }
        }
        game::write_wtf(&s)?;
        #[cfg(target_os = "linux")]
        system::install_ca_in_prefix(&s)?;
        let account = login.game_accounts.first().map(|a| a.name.clone()).unwrap_or_else(|| "WoW1".into());
        let portal = format!("{}:{}", s.server.portal, s.server.bnet_port);
        #[cfg(not(target_os = "macos"))]
        game::write_launch_options(&s, &portal, &login.login_ticket, &account)?;
        #[cfg(target_os = "macos")]
        macos::write_launch_options(&portal, &login.login_ticket, &account)?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;
    launch(app, settings).await
}

/// macOS: re-signed copy of the game with keypatch.dylib, followed through the library's log.
#[cfg(target_os = "macos")]
async fn launch(app: &AppHandle, settings: Settings) -> Result<(), String> {
    let (a, s) = (app.clone(), settings.clone());
    let copy = tokio::task::spawn_blocking(move || macos::prepare_copy(&s, |m| emit(&a, "step", m)))
        .await
        .map_err(|e| e.to_string())??;
    emit(app, "step", t("step_launching"));
    let (child, log) = macos::spawn(&settings, &copy)?;
    let app = app.clone();
    std::thread::spawn(move || {
        emit(&app, "running", t("running"));
        match macos::watch(child, &log, |kind, m| emit(&app, kind, m)) {
            Ok(()) => emit(&app, "closed", t("closed")),
            Err(e) => emit(&app, "error", e),
        }
        app.state::<AppState>().playing.store(false, Ordering::SeqCst);
    });
    Ok(())
}

/// Windows, Linux: the game is started, then the patcher writes the key into its memory.
#[cfg(not(target_os = "macos"))]
async fn launch(app: &AppHandle, settings: Settings) -> Result<(), String> {
    emit(app, "step", t("step_launching"));
    let patcher = patcher::Patcher::new(&settings.server.public_key, settings.server.region_group)?;
    let mut child = game::spawn(&settings)?;
    // launcher closed / killed / crashed = game closed: the game lives in a kill-on-close Job Object
    #[cfg(windows)]
    match guard::assign_to_job(child.id()) {
        Ok(()) => emit(app, "log", "job object: ok (kill on launcher exit)"),
        Err(e) => emit(app, "log", format!("job object: not applied ({e})")),
    }
    let app = app.clone();
    std::thread::spawn(move || {
        // reap the started process (on Linux it is umu-run / gamescope, which may exit early)
        let child_pid = child.id();
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        let first = wait_for_client(child_pid);
        match first {
            Some(mem) => {
                // the client found may be another process than the one started (stub / relaunch): job it too
                #[cfg(windows)]
                if other_pid(&mem) != child_pid {
                    let _ = guard::assign_to_job(other_pid(&mem));
                }
                emit(&app, "running", t("running"));
                minimize(&app);
                patcher.watch(
                    Box::new(mem),
                    |cur| {
                        let other = patcher::find_client()?;
                        let better = !cur.alive() || other.rss() > (current_rss(cur) * 2).max(50 * 1024);
                        let switch = other_pid(&other) != cur.pid() && better;
                        #[cfg(windows)]
                        if switch {
                            let _ = guard::assign_to_job(other_pid(&other));
                        }
                        switch.then(|| Box::new(other) as Box<dyn patcher::Memory>)
                    },
                    |ev| match ev {
                        patcher::Event::Log(m) => emit(&app, "log", m),
                        patcher::Event::Ready => emit(&app, "ready", t("ready")),
                        patcher::Event::Closed => {}
                    },
                );
                emit(&app, "closed", t("closed"));
            }
            None => emit(&app, "error", t("not_started")),
        }
        app.state::<AppState>().playing.store(false, Ordering::SeqCst);
    });
    Ok(())
}

use tauri::Manager;

#[cfg(not(target_os = "macos"))]
fn other_pid(m: &patcher::ProcessMemory) -> u32 {
    use patcher::Memory;
    m.pid()
}

#[cfg(not(target_os = "macos"))]
fn current_rss(m: &dyn patcher::Memory) -> u64 {
    patcher::ProcessMemory::open(m.pid()).map(|p| p.rss()).unwrap_or(0)
}

#[cfg(not(target_os = "macos"))]
fn wait_for_client(child_pid: u32) -> Option<patcher::ProcessMemory> {
    #[cfg(windows)]
    if let Some(m) = patcher::ProcessMemory::open(child_pid) {
        return Some(m);
    }
    let _ = child_pid;
    for _ in 0..180 {
        if let Some(m) = patcher::find_client() {
            return Some(m);
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    None
}

/// The launcher stays open (minimised) as a watchdog while the game runs.
#[cfg(not(target_os = "macos"))]
fn minimize(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.minimize();
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GuardState {
    tool: Option<String>,
    game_running: bool,
}

/// Forever game already running (started by the launcher or not): 'Play' is disabled.
#[tauri::command]
fn game_running(state: State<'_, AppState>) -> GuardState {
    let game_dir = state.settings.lock().unwrap().game_dir.clone();
    GuardState { tool: state.blocked.lock().unwrap().clone(), game_running: instance::game_running(&game_dir) }
}

/// "Close the launcher" button of the mandatory update popup.
#[tauri::command]
fn quit_launcher(app: AppHandle) {
    app.exit(0);
}

/// Watchdog: every 4 s, forbidden tools (processes, windows, modules in the game); list refreshed every 30 min.
fn start_guard(app: AppHandle) {
    tauri::async_runtime::spawn(async { guard::refresh().await });
    std::thread::spawn(move || {
        let mut ticks = 0u32;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(4));
            ticks += 1;
            if ticks % 450 == 0 {
                tauri::async_runtime::spawn(async { guard::refresh().await });
            }
            let state = app.state::<AppState>();
            let game_dir = state.settings.lock().unwrap().game_dir.clone();
            let found = guard::scan(&game_dir);
            let changed = *state.blocked.lock().unwrap() != found;
            *state.blocked.lock().unwrap() = found.clone();
            if let Some(tool) = &found {
                if instance::game_running(&game_dir) {
                    guard::kill_game(&game_dir);
                    emit(&app, "blocked", tf("tool_blocked", &[tool]));
                } else if changed {
                    emit(&app, "blocked", tf("tool_blocked", &[tool]));
                }
            } else if changed {
                emit(&app, "unblocked", "");
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // restarted by the updater: --updated=<version> or --update-error=<message>
    let mut note = None;
    for a in std::env::args().skip(1) {
        if let Some(v) = a.strip_prefix("--updated=") {
            note = Some(("updated".to_string(), v.to_string()));
            let _ = std::fs::remove_dir_all(updater::update_dir());
        } else if let Some(e) = a.strip_prefix("--update-error=") {
            note = Some(("error".to_string(), e.to_string()));
        }
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            settings: Mutex::new(Settings::load()),
            playing: AtomicBool::new(false),
            update: Mutex::new(None),
            update_note: Mutex::new(note),
            blocked: Mutex::new(None),
        })
        .setup(|app| {
            start_guard(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state, save_settings, set_language, detect_locale, login, logout, server_status, news, checks, repair, play,
            client_status, install_client, battlenet_download, update_check, update_install, game_running, quit_launcher
        ])
        .run(tauri::generate_context!())
        .expect("error while running Aeternum launcher");
}
