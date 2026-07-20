//! TokenBar — Tauri entry point: island window, tray, scheduler, providers.

mod analytics;
mod burnrate;
mod config;
mod engine;
mod model;
mod providers;
mod ranking;
mod scan_coord;

use config::Settings;
use engine::Engine;
use model::{Limit, LimitStatus, Snapshot};
use providers::anthropic::AnthropicProvider;
use scan_coord::ScanCoordinator;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindowBuilder,
    WindowEvent,
};
use tauri_plugin_notification::NotificationExt;

const POLL_SECS: u64 = 15;
const NOTIFY_SUPPRESS_SECS: i64 = 1800; // 30 min per limit (§10)
const SHARE_PREVIEW_UPDATED_EVENT: &str = "share-preview-updated";

/// 來源失效通知:恢復前只提醒一次(不像額度警告那樣重複提醒)。
///
/// 刻意**不**沿用 `NOTIFY_SUPPRESS_SECS`:那 30 分鐘是為「額度快用完」設計的,
/// 數字一直在動,重複提醒有意義;「請重新登入」是要使用者動手的事,修好之前
/// 每半小時彈一次只是騷擾。真的修好了會走 `due_source_notices` 的清除路徑,
/// 不必等這個窗口到期。
const SOURCE_FAIL_SUPPRESS_SECS: i64 = 6 * 3600;

/// Suffix marking the source-failure entries in the shared `notified` map, so
/// recovery can clear only its own keys and leave the quota keys (which are
/// bare limit ids) untouched. Limit ids cannot collide with it: they are
/// `cc.*` / `codex.*` and `slug()` strips everything but alphanumerics.
const SOURCE_FAIL_KEY_SUFFIX: &str = ".source_failed";

/// Fixed error codes handed to the panel by `relogin`. Deliberately codes, not
/// prose: an `io::Error` string can echo the resolved path, and the failure
/// copy belongs to the UI layer, which turns any failure into the "run it
/// yourself" fallback.
const ERR_CLAUDE_NOT_FOUND: &str = "claude_not_found";
const ERR_SPAWN_FAILED: &str = "spawn_failed";

/// Launcher filenames to try inside each PATH entry, in preference order.
///
/// Resolving this ourselves is not a nicety — it is what makes the panel's
/// manual fallback reachable. `Command::new("claude")` cannot work: npm ships
/// the launcher as `claude.cmd`, and `CreateProcess` only appends `.exe` when
/// searching PATH, so it reports "not found" on a machine with a perfectly
/// good Claude Code. And running it through a shell instead (`cmd /C start
/// claude …`) hides the failure entirely: `spawn()` then only reports whether
/// *cmd* started, which it always does — verified 2026-07-14, a nonexistent
/// `.cmd` target still yields `Ok`. Either way the error path would be dead
/// code and the user would be left with a button that silently does nothing.
///
/// `.ps1` is excluded: it is not directly executable.
const CLAUDE_NAMES: [&str; 3] = ["claude.exe", "claude.cmd", "claude.bat"];

struct AppData {
    last: Mutex<Option<Snapshot>>,
    settings: Mutex<Settings>,
    /// Wakes the scheduler for an immediate forced poll (manual refresh).
    refresh_tx: Sender<()>,
    /// Stage 1B: TTL cache + coalesce + single full-scan exclusion for analytics.
    scan: ScanCoordinator,
}

/// File-backed share preview: one app-owned PNG under a dedicated temp dir.
/// The data URL is never retained after `replace` — only the absolute path.
///
/// `session` is bumped on every open and clear; updates with a stale session
/// are rejected so a late raster after the user closed Preview cannot orphan a PNG.
struct SharePreviewState {
    path: Mutex<Option<PathBuf>>,
    /// Monotonic session id for the currently open preview window (0 = closed).
    session: Mutex<u64>,
}

impl Default for SharePreviewState {
    fn default() -> Self {
        Self {
            path: Mutex::new(None),
            session: Mutex::new(0),
        }
    }
}

impl SharePreviewState {
    fn preview_dir() -> PathBuf {
        std::env::temp_dir().join("atoll-share-preview")
    }

    /// Open a new preview session; returns the session id the FE must pass to update.
    fn begin_session(&self) -> u64 {
        let mut s = self.session.lock().unwrap_or_else(|p| p.into_inner());
        *s = s.wrapping_add(1).max(1);
        *s
    }

    fn current_session(&self) -> u64 {
        *self.session.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Decode a transient data URL into a new PNG; reject if `session` is stale.
    fn replace_from_data_url(&self, data_url: &str, session: u64) -> Result<PathBuf, String> {
        if session == 0 || session != self.current_session() {
            return Err("preview session stale".into());
        }
        let bytes = decode_data_url_png(data_url)?;
        let dir = Self::preview_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        // Unpredictable name + exclusive create avoids ms collisions.
        let id = format!("{:016x}{:016x}", random_u64(), random_u64());
        let dest = dir.join(format!("preview-{id}.png"));
        let tmp = dir.join(format!("preview-{id}.tmp"));
        // Exclusive create of the final name first (reserves the id).
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp)
                .map_err(|e| e.to_string())?;
            f.write_all(&bytes).map_err(|e| e.to_string())?;
        }
        if let Err(e) = std::fs::rename(&tmp, &dest) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        // Re-check session after IO — user may have closed mid-write.
        if session != self.current_session() {
            let _ = std::fs::remove_file(&dest);
            return Err("preview session stale".into());
        }
        let prev = {
            let mut g = self.path.lock().unwrap_or_else(|p| p.into_inner());
            std::mem::replace(&mut *g, Some(dest.clone()))
        };
        if let Some(old) = prev {
            let _ = std::fs::remove_file(old);
        }
        Ok(dest)
    }

    fn clear(&self) {
        // Invalidate any in-flight update first.
        {
            let mut s = self.session.lock().unwrap_or_else(|p| p.into_inner());
            *s = s.wrapping_add(1);
        }
        let prev = {
            let mut g = self.path.lock().unwrap_or_else(|p| p.into_inner());
            g.take()
        };
        if let Some(p) = prev {
            let _ = std::fs::remove_file(p);
        }
    }

    fn get_path(&self) -> Option<PathBuf> {
        self.path
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Remove PNG and leftover .tmp files older than `max_age`.
    fn cleanup_stale(max_age: Duration) {
        let dir = Self::preview_dir();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        let now = std::time::SystemTime::now();
        for e in entries.flatten() {
            let path = e.path();
            let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("");
            if ext != "png" && ext != "tmp" {
                continue;
            }
            if let Ok(meta) = e.metadata() {
                if let Ok(modified) = meta.modified() {
                    if now.duration_since(modified).unwrap_or_default() > max_age {
                        let _ = std::fs::remove_file(path);
                    }
                }
            }
        }
    }
}

fn random_u64() -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut h = RandomState::new().build_hasher();
    h.write_u64(chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64);
    h.finish()
}

fn decode_data_url_png(data_url: &str) -> Result<Vec<u8>, String> {
    const PREFIX: &str = "data:image/png;base64,";
    let b64 = data_url
        .strip_prefix(PREFIX)
        .ok_or_else(|| "expected data:image/png;base64, …".to_string())?;
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SharePreviewPayload {
    /// Absolute path to the backend-owned PNG (frontend uses convertFileSrc).
    file_path: Option<String>,
    /// Legacy: always null when file-backed (kept so older FE still deserializes).
    data_url: Option<String>,
    locale: String,
}

/// The full source list — the safe fallback whenever the live settings can't be
/// read (a lock failure must never blank the app to an empty selection).
fn all_sources() -> Vec<String> {
    config::KNOWN_SOURCES.iter().map(|s| s.to_string()).collect()
}

#[tauri::command]
fn get_snapshot(data: State<'_, AppData>) -> Option<Snapshot> {
    data.last.lock().ok().and_then(|g| g.clone())
}

#[tauri::command]
fn refresh_now(data: State<'_, AppData>) {
    let _ = data.refresh_tx.send(());
}

#[tauri::command]
async fn get_analytics(
    data: State<'_, AppData>,
    range: String,
    force: Option<bool>,
) -> Result<analytics::Analytics, String> {
    // Read the live in-memory setting, not config::load(): set_settings updates
    // this immediately, so switching the selection reflects on the next fetch
    // instead of waiting for a disk round-trip. Fall back to all sources on a
    // lock failure so an error never blanks the page.
    let sources = data
        .settings
        .lock()
        .ok()
        .map(|s| s.sources.clone())
        .unwrap_or_else(all_sources);
    let scan = data.scan.clone();
    // Stage 1B: coordinator owns the fingerprint cache (T-perf-001), same-key
    // coalesce, and global exclusion. The scan re-parses every session log in
    // range (hundreds of MB on a heavy machine) — must stay off the UI /
    // async-runtime threads. `force` (⟳) always bypasses the cache read and
    // refreshes the stored fingerprint; omitted by any caller that doesn't
    // send it, so this stays backward compatible.
    let force = force.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || scan.get(range, sources, force))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_settings(data: State<'_, AppData>) -> Settings {
    data.settings.lock().map(|g| g.clone()).unwrap_or_default()
}

#[tauri::command]
fn set_settings(app: AppHandle, data: State<'_, AppData>, settings: Settings) {
    config::save(&settings);
    apply_autostart(&app, settings.autostart);
    apply_always_on_top(&app, settings.always_on_top);
    // Cache keys already include sources. Only wipe when the selection actually
    // changes — theme/locale/share style must not force a full log rescan.
    let sources_changed = data
        .settings
        .lock()
        .ok()
        .map(|g| !scan_coord::sources_equal(&g.sources, &settings.sources))
        .unwrap_or(true);
    if sources_changed {
        data.scan.invalidate_all();
    }
    if let Ok(mut g) = data.settings.lock() {
        *g = settings;
    }
}

// ── re-login (§7 source failure remedy) ──────────────────────────────

/// Find the `claude` launcher on PATH, mirroring how a shell resolves a bare
/// `claude`: each PATH entry in order, each known extension within it.
///
/// `exists` is injected so the resolution policy is testable without touching
/// the filesystem.
fn find_claude_in(path_var: &str, exists: &dyn Fn(&Path) -> bool) -> Option<PathBuf> {
    for dir in path_var.split(';') {
        // An empty PATH entry means "the current directory" on Windows, and a
        // relative entry is CWD-relative. Both are skipped: TokenBar's working
        // directory is whatever launched it (Explorer, autostart, the repo
        // under `tauri dev`), so honouring them would let a `claude.cmd`
        // dropped there be the thing this button starts.
        let dir = dir.trim();
        if dir.is_empty() || !Path::new(dir).is_absolute() {
            continue;
        }
        for name in CLAUDE_NAMES {
            let candidate = Path::new(dir).join(name);
            if exists(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

/// `None` is a **supported outcome**, not an edge case: TokenBar is started
/// from Explorer/autostart and inherits a different PATH than the user's
/// terminal, and a Claude Code that lives in WSL has no Windows launcher at
/// all. The panel shows the command to run by hand in that case.
fn claude_path() -> Option<PathBuf> {
    find_claude_in(&std::env::var("PATH").ok()?, &|p| p.is_file())
}

/// Start the vendor's login flow in its own console window.
///
/// `CREATE_NEW_CONSOLE`: `claude auth login` is interactive — it opens a
/// browser and waits. TokenBar is a GUI app with no console, so without this
/// the flow would run invisibly with nothing for the user to watch or cancel.
///
/// Passing the resolved launcher as the *program* (rather than as an argument
/// to `cmd /C`) is deliberate: std applies cmd-safe escaping for `.cmd`/`.bat`
/// programs itself, whereas building a `cmd /C <path> …` line would re-parse
/// that path as shell syntax — a PATH directory containing `&` would then
/// split into a second command. Verified 2026-07-14 that a `.cmd` under a
/// directory named `A&B` runs correctly with its arguments intact this way.
fn launch_login(claude: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

    std::process::Command::new(claude)
        // Compile-time constants only. Never interpolate anything here — no
        // settings value, no API string, and specifically not `--email`, which
        // would make the arguments dynamic *and* publish a personal identifier
        // on a command line every process on the machine can read.
        .args(["auth", "login", "--claudeai"])
        .creation_flags(CREATE_NEW_CONSOLE)
        // stdout/stderr are left alone on purpose: the login flow prints URLs
        // and codes, and capturing them would pull secrets into TokenBar.
        .spawn()
        .map(|_| ())
}

/// Hand the user off to the official login flow.
///
/// TokenBar deliberately does **not** implement OAuth, touch `CLIENT_ID`, or
/// rewrite `.credentials.json` here: doing so rotates the refresh token that
/// the user's running Claude Code session depends on and can log them out
/// (see providers/anthropic.rs:7-10). The vendor ships a front door; this uses
/// it and owns none of the credential handling.
///
/// This starts a *new* login flow — it cannot inject `/login` into an
/// already-running Claude Code session. TokenBar recovers on the next poll
/// (≤180s) or when the user hits ⟳.
#[tauri::command]
fn relogin() -> Result<(), String> {
    let claude = claude_path().ok_or_else(|| ERR_CLAUDE_NOT_FOUND.to_string())?;
    launch_login(&claude).map_err(|_| ERR_SPAWN_FAILED.to_string())
}

// ── 階段 D 戰報 Share: local PNG export ───────────────────────────────

/// Reduce a caller-supplied filename to a bare basename that can only ever land
/// directly inside the downloads dir — never traverse out of it. Strips any path
/// separators (`/`, `\`) and rejects `..` segments by taking only the last
/// component and refusing dotted-only names. Pure so it is unit-testable without
/// touching the filesystem.
fn sanitize_share_filename(raw: &str) -> Option<String> {
    // Take the final component after any separator, then reject empties and the
    // parent/current markers so nothing can point outside the target dir.
    let base = raw.rsplit(['/', '\\']).next().unwrap_or("").trim();
    if base.is_empty() || base == "." || base == ".." {
        return None;
    }
    // A remaining `..` (e.g. embedded) or NUL would be a path-trick smell.
    if base.contains("..") || base.contains('\0') {
        return None;
    }
    // Windows reserved device names (CON, NUL, COM1…) address devices, not files,
    // even with an extension ("con.png"); an ADS colon or trailing dot/space also
    // changes the write target. Reject them all — app-generated names never hit this.
    if base.contains(':') || base.ends_with('.') || base.ends_with(' ') {
        return None;
    }
    let stem = base.split('.').next().unwrap_or("").to_ascii_uppercase();
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
        "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED.contains(&stem.as_str()) {
        return None;
    }
    Some(base.to_string())
}

/// Write the exported share-card PNG bytes to the user's Downloads folder and
/// return the full path. Filename is sanitized to a bare basename first so it
/// can never escape the downloads dir.
#[tauri::command]
fn save_share_png(bytes: Vec<u8>, filename: String) -> Result<String, String> {
    let name = sanitize_share_filename(&filename).ok_or_else(|| "invalid filename".to_string())?;
    let dir = dirs::download_dir().ok_or_else(|| "no downloads directory".to_string())?;
    let path = dir.join(&name);
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
fn get_share_preview(
    preview: State<'_, SharePreviewState>,
    data: State<'_, AppData>,
) -> SharePreviewPayload {
    let file_path = preview
        .get_path()
        .map(|p| p.to_string_lossy().into_owned());
    let locale = data
        .settings
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .locale
        .clone();
    SharePreviewPayload {
        file_path,
        data_url: None,
        locale,
    }
}

#[tauri::command]
fn update_share_preview(
    app: AppHandle,
    preview: State<'_, SharePreviewState>,
    data_url: String,
    session: u64,
) -> Result<(), String> {
    // Transient data URL is decoded and written to an app-owned temp PNG;
    // the base64 string is not retained. Stale session (window closed) → reject.
    preview.replace_from_data_url(&data_url, session)?;
    app.emit_to("share-preview", SHARE_PREVIEW_UPDATED_EVENT, ())
        .map_err(|error| error.to_string())
}


#[tauri::command]
fn close_share_preview(
    app: AppHandle,
    preview: State<'_, SharePreviewState>,
) -> Result<(), String> {
    // Clear state first so a destroy path that races still leaves no base64 PNG
    // retained in the host (Preview may also close via Esc / system chrome).
    preview.clear();
    if let Some(window) = app.get_webview_window("share-preview") {
        window.destroy().map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Stage 1A: any close path for the Preview window (command, Esc → window.close,
/// system destroy) must drop the in-memory data URL. Registered once on the app.
fn clear_share_preview_if_label(label: &str, app: &AppHandle) {
    if label != "share-preview" {
        return;
    }
    if let Some(state) = app.try_state::<SharePreviewState>() {
        state.clear();
    }
}

/// Clear any previous export and recreate the dedicated preview WebView so it
/// can appear while the next PNG is still rendering.
/// Returns a session id that `update_share_preview` must pass; updates after
/// close/destroy are rejected so late rasters cannot orphan PNGs.
#[tauri::command]
async fn open_share_preview(
    app: AppHandle,
    preview: State<'_, SharePreviewState>,
) -> Result<u64, String> {
    preview.clear();
    let session = preview.begin_session();

    if let Some(existing) = app.get_webview_window("share-preview") {
        existing.destroy().map_err(|error| error.to_string())?;
    }

    let current_monitor = if let Some(main) = app.get_webview_window("main") {
        main.current_monitor().map_err(|error| error.to_string())?
    } else {
        None
    };
    let monitor = match current_monitor {
        Some(monitor) => monitor,
        None => app
            .primary_monitor()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "no monitor available".to_string())?,
    };
    let work = monitor.work_area();
    let width = ((work.size.width as f64) * 0.9).round() as u32;
    let height = ((work.size.height as f64) * 0.9).round() as u32;
    let scale = monitor.scale_factor();
    let logical_width = width as f64 / scale;
    let logical_height = height as f64 / scale;
    let x = work.position.x + ((work.size.width - width) / 2) as i32;
    let y = work.position.y + ((work.size.height - height) / 2) as i32;

    let window = WebviewWindowBuilder::new(
        &app,
        "share-preview",
        WebviewUrl::App("index.html#share-preview".into()),
    )
    .title("Atoll Share Preview")
    .additional_browser_args(
        "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,IsolateOrigins,site-per-process --disable-gpu --disable-gpu-compositing --renderer-process-limit=1",
    )
    .inner_size(logical_width, logical_height)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(true)
    .visible(false)
    .build()
    .map_err(|error| error.to_string())?;
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    Ok(session)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings = config::load();
    let (refresh_tx, refresh_rx) = mpsc::channel::<()>();

    tauri::Builder::default()
        // Single-instance guard MUST be the first plugin: if TokenBar is already
        // running, a second launch just wakes the existing one (shows + focuses
        // the island) and exits, instead of stacking another tray icon.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(AppData {
            last: Mutex::new(None),
            settings: Mutex::new(settings.clone()),
            refresh_tx,
            scan: ScanCoordinator::new(),
        })
        .manage(SharePreviewState::default())
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_analytics,
            get_settings,
            set_settings,
            refresh_now,
            relogin,
            save_share_png,
            get_share_preview,
            open_share_preview,
            update_share_preview,
            close_share_preview
        ])
        // Preview may close without going through `close_share_preview` (Esc /
        // getCurrentWindow().close()). Destroy is the universal path — clear
        // SharePreviewState so base64 PNG never sticks in the host.
        .on_window_event(|window, event| {
            if matches!(event, WindowEvent::Destroyed) {
                clear_share_preview_if_label(window.label(), window.app_handle());
            }
        })
        .setup(move |app| {
            // Drop stale share-preview PNGs from previous runs (>24h).
            SharePreviewState::cleanup_stale(Duration::from_secs(24 * 3600));
            build_tray(app.handle())?;
            position_island(app.handle());
            apply_autostart(app.handle(), settings.autostart);
            apply_always_on_top(app.handle(), settings.always_on_top);
            spawn_scheduler(app.handle().clone(), refresh_rx);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running TokenBar");
}

// ── tray ─────────────────────────────────────────────────────────────

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Show / Hide", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Atoll", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &quit])?;

    TrayIconBuilder::with_id("atoll")
        // The app logo (icon-source.png → bundle icons in tauri.conf.json),
        // set once and never updated: the tray shows *who* this is, the tooltip
        // shows how much is left. `update_tray` must not set an icon.
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Atoll — starting…")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            "toggle" => toggle_main(app),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

/// What the tray's "Show / Hide" should do to the island.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ToggleAction {
    Show,
    Hide,
}

/// The tray toggle's decision, split out from the window calls so it can be
/// tested (a `WebviewWindow` cannot be built under `cargo test`).
///
/// "Visible" is not "the user can see it". While `always_on_top` was hardcoded
/// on, the two were interchangeable and `is_visible()` alone was enough. With
/// the pin now optional, a visible window can sit buried under whatever the
/// user is reading — and `skipTaskbar: true` means the tray is the *only* way
/// back, so treating that as "hide it" would make the menu item do the exact
/// opposite of what was clicked, twice in a row.
///
/// Focus is the discriminator: hide only what is both visible and frontmost.
pub fn toggle_action(visible: bool, focused: bool) -> ToggleAction {
    if visible && focused {
        ToggleAction::Hide
    } else {
        ToggleAction::Show
    }
}

fn toggle_main(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    // Fail toward Show: if either query errors, the safe move is to surface a
    // window the user just asked for, never to hide one they cannot recover.
    let visible = win.is_visible().unwrap_or(false);
    let focused = win.is_focused().unwrap_or(false);
    match toggle_action(visible, focused) {
        ToggleAction::Hide => {
            let _ = win.hide();
        }
        ToggleAction::Show => {
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}

/// Default docking: bottom-right of the work area (above the taskbar, near the tray).
fn position_island(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    if let (Ok(Some(monitor)), Ok(size)) = (win.current_monitor(), win.outer_size()) {
        let wa = monitor.work_area();
        let margin = (8.0 * monitor.scale_factor()) as i32;
        let x = wa.position.x + wa.size.width as i32 - size.width as i32 - margin;
        let y = wa.position.y + wa.size.height as i32 - size.height as i32 - margin;
        let _ = win.set_position(PhysicalPosition::new(x.max(0), y.max(0)));
    }
}

fn apply_autostart(app: &AppHandle, enable: bool) {
    use tauri_plugin_autostart::ManagerExt;
    let mgr = app.autolaunch();
    let _ = if enable { mgr.enable() } else { mgr.disable() };
}

/// Pin/unpin the island at runtime.
///
/// Must also run at startup, not only on change: tauri.conf.json creates the
/// window with `alwaysOnTop: true` unconditionally, so a stored `false` only
/// takes effect if we override it here — otherwise the setting would appear to
/// reset itself on every launch.
fn apply_always_on_top(app: &AppHandle, enable: bool) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.set_always_on_top(enable);
    }
}

// ── scheduler ────────────────────────────────────────────────────────

fn spawn_scheduler(app: AppHandle, refresh_rx: Receiver<()>) {
    std::thread::spawn(move || {
        let mut engine = Engine::new();
        let mut anthropic = AnthropicProvider::new();
        let mut codex_live = providers::codex_live::CodexLiveProvider::new();
        let mut notified: HashMap<String, i64> = HashMap::new();
        let debug = std::env::var("TOKENBAR_DEBUG").is_ok();
        let mut first = true;
        let mut force = false;

        loop {
            let now = chrono::Utc::now().timestamp();

            // Read live settings once per round so every toggle applies without restart.
            let (codex_source, allow_refresh, sources, refresh_secs) = app
                .try_state::<AppData>()
                .and_then(|data| {
                    data.settings.lock().ok().map(|s| {
                        (
                            s.codex_usage_source.clone(),
                            s.allow_token_refresh,
                            s.sources.clone(),
                            // Clamped on read to an offered cadence {30,60,180}.
                            s.refresh_secs_clamped(),
                        )
                    })
                })
                .unwrap_or_else(|| ("local".into(), false, all_sources(), 180));
            let refresh_secs = refresh_secs as i64;

            // Skip the polls an unselected provider does not need. T-916: gate on
            // explicit `sources` membership — a quota provider absent from the
            // list is not polled (and an empty list polls nothing, an honest
            // empty UI).
            let want_codex = sources.iter().any(|s| s == "codex");
            let want_claude = sources.iter().any(|s| s == "claude");

            let live = if want_codex && matches!(codex_source.as_str(), "live" | "auto") {
                codex_live.poll(now, force, refresh_secs)
            } else {
                None
            };
            let local = if want_codex && matches!(codex_source.as_str(), "local" | "auto") {
                providers::codex::read_limits()
            } else {
                Vec::new()
            };
            // Guarded: with source="live", choose_limits falls back to
            // degraded_limits() when `live` is None — which would fabricate
            // two "SourceFailed" Codex rows out of a poll we skipped on
            // purpose. apply_provider_filter would drop them anyway; not
            // building them keeps the intent obvious.
            let mut limits = if want_codex {
                providers::codex_live::choose_limits(&codex_source, live, local)
            } else {
                Vec::new()
            };
            if want_claude {
                limits.extend(anthropic.poll(now, force, allow_refresh, refresh_secs));
            }
            // Grok deliberately produces NO limits (T-918 使用者定案): its only
            // local reading is per-session context fill, which confused more
            // than it informed — Grok stays usage-only (analytics). The
            // frontend keeps its Provider::Grok rendering path dormant so a
            // real 5h/week card can light up if xAI ever exposes a quota API.
            // The single filter node for the whole app (§ see apply_provider_filter).
            // Skipping polls above is an optimisation; this is the correctness
            // guarantee — it still runs so a future third provider cannot leak
            // through by being absent from the skip logic.
            let limits = apply_provider_filter(&sources, limits);
            let mut snapshot = engine.ingest(limits, now);

            // Countdown to the next real data fetch (header "Refresh in Ns").
            // The scheduler wakes every POLL_SECS but the providers only hit the
            // network every REFRESH_SECS, so anchor on their cache expiry — the
            // soonest among the providers actually polled this round. A manual
            // refresh re-fetches and pushes these out, restarting the countdown.
            // Pure local-Codex (no network provider) falls back to the poll tick,
            // its real re-read cadence.
            let mut next_fetch: Vec<i64> = Vec::new();
            if want_claude {
                next_fetch.push(anthropic.next_fetch_at());
            }
            if want_codex && matches!(codex_source.as_str(), "live" | "auto") {
                next_fetch.push(codex_live.next_fetch_at());
            }
            snapshot.next_fetch_in = next_fetch
                .iter()
                .min()
                .map(|&t| (t - now).max(0))
                .unwrap_or(POLL_SECS as i64);

            if debug {
                for l in &snapshot.limits {
                    eprintln!(
                        "[tb] {:?} {} util={:.0} status={:?} runway={:?}",
                        l.provider, l.id, l.util, l.status, l.runway_secs
                    );
                }
                if first {
                    // Debug log only: use the live source selection.
                    let a = analytics::compute_with("today", &sources);
                    eprintln!(
                        "[tb] analytics today: total_tokens={} by_agent={:?} sessions={} tok/min={}",
                        a.total_tokens, a.by_agent, a.sessions_this_week, a.tok_per_min
                    );
                    first = false;
                }
            }

            if let Some(data) = app.try_state::<AppData>() {
                if let Ok(mut g) = data.last.lock() {
                    *g = Some(snapshot.clone());
                }
            }
            let _ = app.emit("snapshot", &snapshot);
            update_tray(&app, &snapshot);
            fire_notifications(&app, &snapshot, &mut notified, now);

            // Sleep until the next tick, or wake early on a manual refresh.
            force = match refresh_rx.recv_timeout(Duration::from_secs(POLL_SECS)) {
                Ok(()) => {
                    // collapse queued clicks into one forced round
                    while refresh_rx.try_recv().is_ok() {}
                    true
                }
                Err(RecvTimeoutError::Timeout) => false,
                Err(RecvTimeoutError::Disconnected) => {
                    std::thread::sleep(Duration::from_secs(POLL_SECS));
                    false
                }
            };
        }
    });
}

/// Filter limits to the selected `sources`. Anthropic limits show iff "claude"
/// is selected, Codex iff "codex". Grok currently produces no limits (T-918:
/// usage-only again), but its filter arm stays so a future quota card is
/// already gated correctly the day a real xAI quota source exists.
///
/// Membership is explicit: an unselected provider is dropped, and an empty
/// selection yields an empty island (an honest empty UI is the intended result
/// of deselecting everything). The island itself renders only the two quota
/// providers, so a Grok limit surviving this filter reaches the panel/digest
/// but never the island pill (island.ts filters by provider).
///
/// This is the single filter node for the whole app: it runs in the scheduler
/// between merging the providers' limits and `engine.ingest()`, so the island,
/// panel, tray tooltip, notifications and ranking all inherit it. Do not add a
/// second filter at any consumer.
pub fn apply_provider_filter(sources: &[String], limits: Vec<Limit>) -> Vec<Limit> {
    let want = |id: &str| sources.iter().any(|s| s == id);
    limits
        .into_iter()
        .filter(|l| match l.provider {
            model::Provider::Anthropic => want("claude"),
            model::Provider::Codex => want("codex"),
            model::Provider::Grok => want("grok"),
        })
        .collect()
}

/// Rich hover text: every limit (§5 — the one tray surface not limited to the
/// worst one). Split from `update_tray` so it can be tested: since the icon
/// became the static app logo, this string is the *only* place the tray still
/// carries the quota numbers, which makes it worth pinning down.
fn tray_tooltip(snap: &Snapshot) -> String {
    if snap.limits.is_empty() {
        return "Atoll — no data".to_string();
    }
    let mut lines = vec!["Atoll".to_string()];
    for l in &snap.limits {
        let val = match l.status {
            // A failed source's util is a 0.0 placeholder, not a reading —
            // "0% used" here would say "plenty left" when we mean "unknown".
            LimitStatus::SourceFailed => "n/a".to_string(),
            LimitStatus::Locked => "LOCKED".to_string(),
            _ => format!("{:.0}% used", l.util),
        };
        lines.push(format!("{}  {}", l.label, val));
    }
    lines.join("\n")
}

/// The tray icon is the app logo, set once in `build_tray` and never touched
/// again: it is deliberately **static**, so this only refreshes the tooltip.
///
/// It used to be a fuel capsule redrawn every round from the worst limit's
/// colour. That traded a recognisable app identity in the notification area for
/// an at-a-glance quota read; the user asked for the logo knowing it costs them
/// that glance — the numbers live one hover away in `tray_tooltip`, and the
/// island still carries the colour-coded capsule. Do not reintroduce a
/// state-dependent icon here as a "compromise".
fn update_tray(app: &AppHandle, snap: &Snapshot) {
    let Some(tray) = app.tray_by_id("atoll") else {
        return;
    };
    let _ = tray.set_tooltip(Some(&tray_tooltip(snap)));
}

/// Which source-failure notices a snapshot warrants — **at most one per
/// provider**, carrying that provider's plain-language reason.
///
/// One per provider because `cc.5h` and `cc.week` always fail together (they
/// come from a single request), and firing per limit would pop two identical
/// toasts. The body is Task 2's `hint` verbatim: the copy is written once, in
/// `FailureStage::user_hint`, and the panel and the notification read the same
/// string so they can never drift apart.
fn source_failed_notices(snap: &Snapshot) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for l in &snap.limits {
        if l.status != LimitStatus::SourceFailed {
            continue;
        }
        let key = format!("{:?}{}", l.provider, SOURCE_FAIL_KEY_SUFFIX);
        if out.iter().any(|(k, _)| *k == key) {
            continue;
        }
        // Codex's live degradation carries no hint (providers/codex_live.rs).
        // Say *something* honest rather than drop the notice: a silent failure
        // is the exact bug this whole path exists to fix.
        let body = l
            .hint
            .clone()
            .unwrap_or_else(|| format!("{} usage unavailable", l.label));
        out.push((key, body));
    }
    out
}

/// Apply de-duplication, suppression and recovery, returning only the notices
/// that should actually be shown this round.
///
/// Separate from `fire_notifications` because that one needs an `AppHandle`
/// and cannot be tested; all three behaviours live here so they can be.
fn due_source_notices(
    snap: &Snapshot,
    notified: &mut HashMap<String, i64>,
    now: i64,
) -> Vec<String> {
    let pending = source_failed_notices(snap);
    let failing: std::collections::HashSet<&str> =
        pending.iter().map(|(k, _)| k.as_str()).collect();

    // Forget providers that are no longer failing, so "broke → fixed → broke
    // again" notifies again instead of being swallowed by the 6h window.
    // Scoped to our own keys: the quota warnings share this map and clearing
    // theirs would make them repeat every poll.
    notified.retain(|k, _| !k.ends_with(SOURCE_FAIL_KEY_SUFFIX) || failing.contains(k.as_str()));

    let mut out = Vec::new();
    for (key, body) in pending {
        let due = match notified.get(&key) {
            Some(&last) => now - last >= SOURCE_FAIL_SUPPRESS_SECS,
            None => true,
        };
        if !due {
            continue;
        }
        notified.insert(key, now);
        out.push(body);
    }
    out
}

fn fire_notifications(
    app: &AppHandle,
    snap: &Snapshot,
    notified: &mut HashMap<String, i64>,
    now: i64,
) {
    // `zh` follows the same narrow rule documented on `Settings::locale`: only an
    // explicit "zh-TW" gives Chinese copy. "system" can't be resolved backend
    // side (no reliable cross-platform OS-locale read here), so it stays English.
    let (warn, crit, zh) = app
        .try_state::<AppData>()
        .and_then(|d| {
            d.settings
                .lock()
                .ok()
                .map(|s| (s.warn_pct, s.crit_pct, s.locale == "zh-TW"))
        })
        .unwrap_or((75.0, 90.0, false));

    // Source failures first: they mean the numbers below are missing entirely,
    // which is more urgent than any of them being high. Until this existed the
    // user could only find out by opening the panel — a SourceFailed limit's
    // util is a 0.0 placeholder, so the quota loop below never fires for one.
    for body in due_source_notices(snap, notified, now) {
        let _ = app.notification().builder().title("Atoll").body(body).show();
    }

    for l in &snap.limits {
        // A SourceFailed row's util is a placeholder, not "0% used" — it is
        // not a quota signal. It has its own notice above, and without this
        // guard a hand-edited crit_pct of 0 would make every failed row also
        // announce itself as a critical quota alert.
        if matches!(l.status, LimitStatus::SourceFailed) {
            continue;
        }
        let level = if matches!(l.status, LimitStatus::Locked) || l.util >= crit {
            Some("critical")
        } else if l.util >= warn {
            Some("warning")
        } else {
            None
        };
        let Some(level) = level else { continue };

        if now - notified.get(&l.id).copied().unwrap_or(0) < NOTIFY_SUPPRESS_SECS {
            continue;
        }
        notified.insert(l.id.clone(), now);

        let tip = match (l.provider, zh) {
            (model::Provider::Codex, false) => "Switch to a mini model to stretch your quota.",
            (model::Provider::Codex, true) => "切換到 mini 模型以延長額度。",
            (model::Provider::Anthropic, false) => "Try /compact or switch to Sonnet.",
            (model::Provider::Anthropic, true) => "試試 /compact 或切換到 Sonnet。",
            // Grok is a context window, not a subscription quota: it empties when
            // a new session starts, so the remedy is a fresh session, not a model
            // swap or /compact.
            (model::Provider::Grok, false) => "Start a new session to reset the context window.",
            (model::Provider::Grok, true) => "開新對話以清空 context 視窗。",
        };
        let body = if matches!(l.status, LimitStatus::Locked) {
            if zh {
                format!("{} 已鎖定。{}", l.label, tip)
            } else {
                format!("{} is locked. {}", l.label, tip)
            }
        } else if zh {
            let level_zh = if level == "critical" { "嚴重" } else { "警告" };
            format!("{} 已達 {:.0}%({})。{}", l.label, l.util, level_zh, tip)
        } else {
            format!("{} at {:.0}% ({}). {}", l.label, l.util, level, tip)
        };
        let _ = app.notification().builder().title("Atoll").body(body).show();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::Provider;

    fn limit(id: &str, provider: Provider) -> Limit {
        Limit {
            id: id.into(),
            provider,
            label: id.into(),
            util: 50.0,
            resets_at: 0,
            window_secs: 0,
            status: LimitStatus::Normal,
            absolute: None,
            pace: None,
            runway_secs: None,
            hint: None,
            action: None,
        }
    }

    fn both_providers() -> Vec<Limit> {
        vec![
            limit("codex.5h", Provider::Codex),
            limit("cc.5h", Provider::Anthropic),
        ]
    }

    /// The three providers together, including Grok's context-fill limit (T-917).
    fn all_three() -> Vec<Limit> {
        vec![
            limit("codex.5h", Provider::Codex),
            limit("cc.5h", Provider::Anthropic),
            limit("grok.ctx", Provider::Grok),
        ]
    }

    // ── 階段 D share PNG filename sanitization ─────────────────────────

    /// Minimal 1×1 PNG (valid base64) for lifecycle tests.
    fn tiny_png_data_url() -> String {
        // 1x1 transparent PNG
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==".into()
    }

    #[test]
    fn share_preview_state_keeps_only_the_latest_file() {
        let state = SharePreviewState::default();
        let s = state.begin_session();
        let p1 = state
            .replace_from_data_url(&tiny_png_data_url(), s)
            .unwrap();
        assert!(p1.is_file());
        let p2 = state
            .replace_from_data_url(&tiny_png_data_url(), s)
            .unwrap();
        assert!(p2.is_file());
        assert_ne!(p1, p2);
        assert!(!p1.exists());
        assert_eq!(state.get_path().as_deref(), Some(p2.as_path()));
        state.clear();
        assert_eq!(state.get_path(), None);
        assert!(!p2.exists());
    }

    #[test]
    fn share_preview_rejects_stale_session_after_clear() {
        let state = SharePreviewState::default();
        let s = state.begin_session();
        state.clear(); // bumps session
        assert!(state
            .replace_from_data_url(&tiny_png_data_url(), s)
            .is_err());
        assert_eq!(state.get_path(), None);
    }

    #[test]
    fn share_preview_rejects_non_png_data_url() {
        let state = SharePreviewState::default();
        let s = state.begin_session();
        assert!(state
            .replace_from_data_url("data:text/plain;base64,YQ==", s)
            .is_err());
    }

    #[test]
    fn sanitize_keeps_a_plain_filename() {
        assert_eq!(
            sanitize_share_filename("tokenbar-week-20260717.png").as_deref(),
            Some("tokenbar-week-20260717.png"),
        );
    }

    #[test]
    fn sanitize_rejects_windows_reserved_device_names() {
        // "con.png" 這類名字在 Windows 指向裝置而非檔案;ADS 冒號與結尾點/空白
        // 也會改寫入目標。全部拒絕(app 自產檔名永不觸發)。
        for bad in [
            "con.png", "CON", "nul.png", "aux.PNG", "com1.png", "lpt9.png", "card.png:ads",
            "card.png.",
        ] {
            assert_eq!(sanitize_share_filename(bad), None, "should reject {bad}");
        }
        // 結尾空白被既有 trim 消毒成安全名(而非拒絕);保留名前綴的正常檔名放行。
        assert_eq!(sanitize_share_filename("card.png ").as_deref(), Some("card.png"));
        assert_eq!(
            sanitize_share_filename("console.png").as_deref(),
            Some("console.png"),
        );
    }

    #[test]
    fn sanitize_strips_path_separators() {
        // Only the last component survives, so nothing can escape the dir.
        assert_eq!(
            sanitize_share_filename("../../etc/passwd").as_deref(),
            Some("passwd"),
        );
        assert_eq!(
            sanitize_share_filename(r"C:\Windows\evil.png").as_deref(),
            Some("evil.png"),
        );
        assert_eq!(
            sanitize_share_filename("sub/dir/card.png").as_deref(),
            Some("card.png"),
        );
    }

    #[test]
    fn sanitize_rejects_traversal_and_empty() {
        assert_eq!(sanitize_share_filename(".."), None);
        assert_eq!(sanitize_share_filename("."), None);
        assert_eq!(sanitize_share_filename(""), None);
        assert_eq!(sanitize_share_filename("   "), None);
        // an embedded `..` (double-dotted name) is refused outright
        assert_eq!(sanitize_share_filename("a..b.png"), None);
    }

    fn srcs(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn both_quota_sources_keep_everything() {
        assert_eq!(
            apply_provider_filter(&srcs(&["claude", "codex"]), both_providers()).len(),
            2
        );
        // Grok riding along doesn't change the quota limits shown when there is
        // no Grok limit present.
        assert_eq!(
            apply_provider_filter(&srcs(&["claude", "codex", "grok"]), both_providers()).len(),
            2
        );
    }

    /// T-917: Grok's context-fill limit passes iff "grok" is selected, and is
    /// dropped otherwise — exactly like the two quota providers.
    #[test]
    fn grok_limit_follows_its_source() {
        // All three selected → all three limits pass.
        assert_eq!(
            apply_provider_filter(&srcs(&["claude", "codex", "grok"]), all_three()).len(),
            3
        );
        // Grok deselected → its context limit is dropped, quota pair stays.
        let no_grok = apply_provider_filter(&srcs(&["claude", "codex"]), all_three());
        assert_eq!(no_grok.len(), 2);
        assert!(no_grok.iter().all(|l| l.provider != Provider::Grok));
        // Only grok selected → only the context limit survives.
        let only_grok = apply_provider_filter(&srcs(&["grok"]), all_three());
        assert_eq!(only_grok.len(), 1);
        assert_eq!(only_grok[0].provider, Provider::Grok);
    }

    #[test]
    fn claude_source_drops_codex() {
        let out = apply_provider_filter(&srcs(&["claude"]), both_providers());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].provider, Provider::Anthropic);
    }

    #[test]
    fn codex_source_drops_claude() {
        let out = apply_provider_filter(&srcs(&["codex"]), both_providers());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].provider, Provider::Codex);
    }

    /// Empty selection blanks every limit; a Grok-only selection keeps no quota
    /// limit (the quota pair is deselected) — both are honest empty islands.
    #[test]
    fn no_quota_source_yields_empty_quota_limits() {
        assert!(apply_provider_filter(&[], both_providers()).is_empty());
        // "grok" selected but the snapshot only has the quota pair → nothing (the
        // island still renders empty; Grok has no island presence anyway).
        assert!(apply_provider_filter(&srcs(&["grok"]), both_providers()).is_empty());
    }

    /// A source selected for a provider that simply has no data yet is
    /// legitimately empty — the filter must not invent rows to compensate.
    #[test]
    fn single_source_on_absent_provider_is_empty() {
        let only_codex = vec![limit("codex.5h", Provider::Codex)];
        assert!(apply_provider_filter(&srcs(&["claude"]), only_codex).is_empty());
    }

    // ── source-failure notifications ─────────────────────────────────
    //
    // These feed real Snapshots through the real functions and assert what
    // the user would actually receive. They deliberately do not assert
    // "predicate X returns Y" — a notice that never reaches the user is the
    // bug being fixed here, and only end-to-end-shaped assertions catch it.

    fn failed(id: &str, provider: Provider, hint: &str) -> Limit {
        Limit {
            status: LimitStatus::SourceFailed,
            util: 0.0, // 佔位值,不是「用了 0%」—— 正是舊邏輯永遠不發通知的原因
            hint: Some(hint.into()),
            ..limit(id, provider)
        }
    }

    fn snapshot_with(limits: Vec<Limit>) -> Snapshot {
        Snapshot {
            worst_id: limits.first().map(|l| l.id.clone()),
            limits,
            updated_at: 0,
            next_fetch_in: 0,
        }
    }

    const T0: i64 = 1_800_000_000; // a realistic epoch, not 0

    #[test]
    fn two_failed_limits_of_one_provider_produce_one_notice() {
        let snap = snapshot_with(vec![
            failed("cc.5h", Provider::Anthropic, "Claude 登入已失效，請重新登入 Claude Code"),
            failed("cc.week", Provider::Anthropic, "Claude 登入已失效，請重新登入 Claude Code"),
        ]);
        assert_eq!(source_failed_notices(&snap).len(), 1);
    }

    #[test]
    fn notice_body_is_the_user_hint() {
        let snap = snapshot_with(vec![failed("cc.5h", Provider::Anthropic, "連不上 Claude。請檢查網路")]);
        assert_eq!(source_failed_notices(&snap)[0].1, "連不上 Claude。請檢查網路");
    }

    #[test]
    fn healthy_snapshot_produces_no_notice() {
        assert!(source_failed_notices(&snapshot_with(vec![limit("cc.5h", Provider::Anthropic)])).is_empty());
    }

    /// 兩個 provider 同時失效 = 兩則,各自帶自己的原因。
    #[test]
    fn each_failed_provider_gets_its_own_notice() {
        let snap = snapshot_with(vec![
            failed("cc.5h", Provider::Anthropic, "Claude 登入已失效"),
            failed("cc.week", Provider::Anthropic, "Claude 登入已失效"),
            failed("codex.5h", Provider::Codex, "Codex 讀不到"),
        ]);
        let n = source_failed_notices(&snap);
        assert_eq!(n.len(), 2);
        assert_ne!(n[0].0, n[1].0, "去重 key 必須分得開兩個 provider");
    }

    /// Codex 的 live 降級沒有 hint(codex_live::degraded_limits) —— 仍要通知,
    /// 不能因為缺文案就整則消失。
    #[test]
    fn failure_without_a_hint_still_notifies() {
        let mut l = limit("codex.5h", Provider::Codex);
        l.status = LimitStatus::SourceFailed;
        l.hint = None;
        let n = source_failed_notices(&snapshot_with(vec![l]));
        assert_eq!(n.len(), 1);
        assert!(!n[0].1.is_empty(), "通知內文不得空白");
    }

    #[test]
    fn first_failure_notifies() {
        let mut notified = HashMap::new();
        let snap = snapshot_with(vec![failed("cc.5h", Provider::Anthropic, "請重新登入")]);
        assert_eq!(due_source_notices(&snap, &mut notified, T0), vec!["請重新登入"]);
    }

    /// 「請重新登入」是要人動手的事,不能每半小時彈一次。既有的 30 分鐘額度抑制
    /// 若被誤用在這裡,這個測試會抓到。
    #[test]
    fn repeat_failure_stays_quiet_far_beyond_the_quota_suppression_window() {
        let mut notified = HashMap::new();
        let snap = snapshot_with(vec![failed("cc.5h", Provider::Anthropic, "請重新登入")]);
        assert_eq!(due_source_notices(&snap, &mut notified, T0).len(), 1);
        // 每一輪都會來(15s),半小時後也還在壞 —— 全程都必須安靜
        for t in [T0 + 15, T0 + NOTIFY_SUPPRESS_SECS + 1, T0 + 5 * 3600] {
            assert!(
                due_source_notices(&snap, &mut notified, t).is_empty(),
                "第 {}s 又彈了一次 —— 這是騷擾",
                t - T0
            );
        }
    }

    /// 六小時後仍未修好 → 再提醒一次(不是永遠閉嘴)。
    #[test]
    fn notice_repeats_once_the_suppression_window_expires() {
        let mut notified = HashMap::new();
        let snap = snapshot_with(vec![failed("cc.5h", Provider::Anthropic, "請重新登入")]);
        assert_eq!(due_source_notices(&snap, &mut notified, T0).len(), 1);
        assert_eq!(
            due_source_notices(&snap, &mut notified, T0 + SOURCE_FAIL_SUPPRESS_SECS).len(),
            1
        );
    }

    /// 壞掉 → 修好 → 又壞掉:必須再通知一次,不能被 6 小時抑制吃掉。
    #[test]
    fn recovery_clears_the_dedupe_so_a_later_failure_notifies_again() {
        let mut notified = HashMap::new();
        let broken = snapshot_with(vec![failed("cc.5h", Provider::Anthropic, "請重新登入")]);
        let healthy = snapshot_with(vec![limit("cc.5h", Provider::Anthropic)]);

        assert_eq!(due_source_notices(&broken, &mut notified, T0).len(), 1);
        assert!(due_source_notices(&healthy, &mut notified, T0 + 60).is_empty());
        assert_eq!(
            due_source_notices(&broken, &mut notified, T0 + 120).len(),
            1,
            "修好之後又壞掉必須再通知"
        );
    }

    /// 恢復時只能清掉自己的 key —— 不得順手洗掉額度警告的去重狀態,
    /// 否則額度通知會退化成每輪 15s 彈一次。
    #[test]
    fn clearing_source_keys_leaves_quota_keys_alone() {
        let mut notified = HashMap::new();
        notified.insert("cc.5h".to_string(), T0); // 額度警告的 key(§10)
        let healthy = snapshot_with(vec![limit("cc.5h", Provider::Anthropic)]);
        due_source_notices(&healthy, &mut notified, T0 + 60);
        assert_eq!(notified.get("cc.5h"), Some(&T0));
    }

    // ── tray tooltip (§5.1) ──────────────────────────────────────────
    //
    // The icon is now the static app logo, so the tooltip is the *only* place
    // the tray still carries numbers. It is also the only tray surface that
    // shows every limit rather than just the worst. These assert the string the
    // user actually hovers, not that some formatter was called.

    #[test]
    fn tooltip_lists_every_limit_not_just_the_worst() {
        let tip = tray_tooltip(&snapshot_with(both_providers()));
        assert!(tip.contains("codex.5h"), "少了 Codex 那條:{tip}");
        assert!(tip.contains("cc.5h"), "少了 Claude 那條:{tip}");
    }

    #[test]
    fn tooltip_shows_used_percentage() {
        let tip = tray_tooltip(&snapshot_with(vec![limit("cc.5h", Provider::Anthropic)]));
        assert!(tip.contains("50% used"), "額度數字沒進 tooltip:{tip}");
    }

    /// A SourceFailed row's util is a 0.0 placeholder — showing it as "0% used"
    /// would read as "plenty left" when the truth is "we don't know".
    #[test]
    fn tooltip_never_reports_a_failed_source_as_zero_percent_used() {
        let tip = tray_tooltip(&snapshot_with(vec![failed(
            "cc.5h",
            Provider::Anthropic,
            "請重新登入",
        )]));
        assert!(tip.contains("n/a"), "失效來源要標 n/a:{tip}");
        assert!(!tip.contains("0% used"), "把佔位值當成用量報出去了:{tip}");
    }

    #[test]
    fn tooltip_calls_a_locked_limit_locked() {
        let mut l = limit("codex.5h", Provider::Codex);
        l.status = LimitStatus::Locked;
        l.util = 100.0;
        assert!(tray_tooltip(&snapshot_with(vec![l])).contains("LOCKED"));
    }

    #[test]
    fn tooltip_without_any_limits_still_says_something() {
        assert_eq!(tray_tooltip(&snapshot_with(vec![])), "Atoll — no data");
    }

    // ── claude launcher resolution ───────────────────────────────────

    /// `exists` is injected so these assert resolution *policy*, not the
    /// machine this happens to run on.
    fn find(path_var: &str, present: &[&str]) -> Option<PathBuf> {
        let owned: Vec<String> = present.iter().map(|s| s.to_string()).collect();
        find_claude_in(path_var, &|p: &Path| {
            owned.iter().any(|f| Path::new(f) == p)
        })
    }

    #[test]
    fn finds_the_npm_shim_on_path() {
        assert_eq!(
            find(r"C:\nodejs", &[r"C:\nodejs\claude.cmd"]),
            Some(PathBuf::from(r"C:\nodejs\claude.cmd"))
        );
    }

    #[test]
    fn finds_a_native_exe_on_path() {
        assert_eq!(
            find(r"C:\a;C:\b", &[r"C:\b\claude.exe"]),
            Some(PathBuf::from(r"C:\b\claude.exe"))
        );
    }

    #[test]
    fn earlier_path_entries_win() {
        assert_eq!(
            find(r"C:\a;C:\b", &[r"C:\a\claude.cmd", r"C:\b\claude.exe"]),
            Some(PathBuf::from(r"C:\a\claude.cmd"))
        );
    }

    /// The supported outcome, not an edge case: TokenBar autostarts from
    /// Explorer with a different PATH than the user's terminal, and a
    /// WSL-only Claude Code install has no Windows launcher at all. `None`
    /// is what drives the "run it yourself" fallback in the panel.
    #[test]
    fn absent_claude_resolves_to_none() {
        assert_eq!(find(r"C:\a;C:\b", &[r"C:\a\node.exe"]), None);
    }

    /// An empty PATH entry means "the current directory" on Windows. Honouring
    /// it would let a `claude.cmd` dropped in TokenBar's working directory be
    /// launched instead of the real one, so empty entries must be skipped.
    #[test]
    fn empty_path_entries_never_resolve_to_the_working_directory() {
        assert_eq!(find(r";C:\a;;", &["claude.cmd", r".\claude.cmd"]), None);
    }

    /// Only a full match — no partial/prefix launcher may be picked up.
    #[test]
    fn similarly_named_programs_are_not_mistaken_for_claude() {
        assert_eq!(find(r"C:\a", &[r"C:\a\claude-code.exe", r"C:\a\myclaude.cmd"]), None);
    }

    // ── tray Show/Hide decision (§5.1) ───────────────────────────────
    //
    // This is the exact function `toggle_main` dispatches on — the window
    // calls around it cannot be driven under test, the decision can. Once the
    // window can be un-pinned, "visible" stops meaning "the user can see it",
    // which is what these pin down.

    #[test]
    fn tray_toggle_hides_the_window_the_user_is_looking_at() {
        assert_eq!(toggle_action(true, true), ToggleAction::Hide);
    }

    #[test]
    fn tray_toggle_shows_a_hidden_window() {
        assert_eq!(toggle_action(false, false), ToggleAction::Show);
    }

    /// The regression this feature would otherwise introduce: with alwaysOnTop
    /// off, a window buried under other windows is still `is_visible() == true`.
    /// Hiding it there is the opposite of what the user asked for — and with
    /// `skipTaskbar: true` there is no taskbar button to get it back, so they
    /// must click the tray twice to undo TokenBar's own mistake.
    #[test]
    fn tray_toggle_raises_a_visible_window_that_is_buried_instead_of_hiding_it() {
        assert_eq!(toggle_action(true, false), ToggleAction::Show);
    }

    /// A hidden window cannot hold focus; if the platform ever claims it does,
    /// showing it is still the safe answer — never hide something invisible.
    #[test]
    fn a_hidden_window_is_never_hidden_again() {
        assert_eq!(toggle_action(false, true), ToggleAction::Show);
    }
}
