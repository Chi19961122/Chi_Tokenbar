//! TokenBar — Tauri entry point: island window, tray, scheduler, providers.

mod analytics;
mod burnrate;
mod config;
mod engine;
mod model;
mod providers;
mod ranking;

use config::Settings;
use engine::Engine;
use model::{Limit, LimitStatus, Snapshot};
use providers::anthropic::AnthropicProvider;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, State,
};
use tauri_plugin_notification::NotificationExt;

const POLL_SECS: u64 = 15;
const NOTIFY_SUPPRESS_SECS: i64 = 1800; // 30 min per limit (§10)

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
fn get_analytics(data: State<'_, AppData>, range: String) -> analytics::Analytics {
    // Read the live in-memory setting, not config::load(): set_settings updates
    // this immediately, so switching the filter reflects on the next fetch
    // instead of waiting for a disk round-trip.
    let filter = data
        .settings
        .lock()
        .ok()
        .map(|s| s.providers.clone())
        .unwrap_or_else(|| "both".into());
    analytics::compute_with(&range, &filter)
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
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_analytics,
            get_settings,
            set_settings,
            refresh_now,
            relogin
        ])
        .setup(move |app| {
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
    let quit = MenuItem::with_id(app, "quit", "Quit TokenBar", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &quit])?;

    TrayIconBuilder::with_id("tokenbar")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("TokenBar — starting…")
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

// ── tray fuel-capsule icon (§5.1) ────────────────────────────────────

fn status_rgb(status: LimitStatus, util: f64) -> (u8, u8, u8) {
    match status {
        LimitStatus::Locked => (248, 113, 113),
        LimitStatus::Near => (251, 191, 36),
        LimitStatus::Normal if util >= 75.0 => (251, 191, 36),
        LimitStatus::Normal => (52, 211, 153),
        _ => (138, 146, 157),
    }
}

/// Render a small horizontal capsule filled to `pct_left`% (fuel remaining) in `rgb`.
fn capsule_icon(pct_left: f64, rgb: (u8, u8, u8)) -> Image<'static> {
    const W: i32 = 32;
    const H: i32 = 32;
    let mut buf = vec![0u8; (W * H * 4) as usize];

    let (x0, x1, y0, y1) = (3.0f64, 29.0f64, 11.0f64, 21.0f64);
    let r = (y1 - y0) / 2.0;
    let cy = (y0 + y1) / 2.0;
    let fill_x = x0 + (x1 - x0) * (pct_left.clamp(0.0, 100.0) / 100.0);

    for y in 0..H {
        for x in 0..W {
            let fx = x as f64 + 0.5;
            let fy = y as f64 + 0.5;
            let inside = if fx >= x0 + r && fx <= x1 - r {
                fy >= y0 && fy <= y1
            } else if fx < x0 + r {
                let (dx, dy) = (fx - (x0 + r), fy - cy);
                dx * dx + dy * dy <= r * r
            } else {
                let (dx, dy) = (fx - (x1 - r), fy - cy);
                dx * dx + dy * dy <= r * r
            };
            if !inside {
                continue;
            }
            let idx = ((y * W + x) * 4) as usize;
            let (cr, cg, cb) = if fx <= fill_x { rgb } else { (60, 66, 74) };
            buf[idx] = cr;
            buf[idx + 1] = cg;
            buf[idx + 2] = cb;
            buf[idx + 3] = 255;
        }
    }
    Image::new_owned(buf, W as u32, H as u32)
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
            let (codex_source, allow_refresh, providers_filter) = app
                .try_state::<AppData>()
                .and_then(|data| {
                    data.settings.lock().ok().map(|s| {
                        (
                            s.codex_usage_source.clone(),
                            s.allow_token_refresh,
                            s.providers.clone(),
                        )
                    })
                })
                .unwrap_or_else(|| ("local".into(), false, "both".into()));

            // Skip the polls a hidden provider does not need. Only an exact
            // "claude"/"codex" narrows anything: an unknown value must keep
            // polling both, to stay consistent with apply_provider_filter's
            // "unknown shows everything".
            let want_codex = providers_filter != "claude";
            let want_claude = providers_filter != "codex";

            let live = if want_codex && matches!(codex_source.as_str(), "live" | "auto") {
                codex_live.poll(now, force)
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
                limits.extend(anthropic.poll(now, force, allow_refresh));
            }
            // The single filter node for the whole app (§ see apply_provider_filter).
            // Skipping polls above is an optimisation; this is the correctness
            // guarantee — it still runs so a future third provider cannot leak
            // through by being absent from the skip logic.
            let limits = apply_provider_filter(&providers_filter, limits);
            let snapshot = engine.ingest(limits, now);

            if debug {
                for l in &snapshot.limits {
                    eprintln!(
                        "[tb] {:?} {} util={:.0} status={:?} runway={:?}",
                        l.provider, l.id, l.util, l.status, l.runway_secs
                    );
                }
                if first {
                    let a = analytics::compute_with("today", &providers_filter);
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

/// Apply the "display platform" setting (`Settings::providers`). Any unknown
/// value shows everything — this must never return empty for a value it does
/// not recognise.
///
/// This is the single filter node for the whole app: it runs in the scheduler
/// between merging the providers' limits and `engine.ingest()`, so the island,
/// panel, tray tooltip, notifications and ranking all inherit it. Do not add a
/// second filter at any consumer.
pub fn apply_provider_filter(filter: &str, limits: Vec<Limit>) -> Vec<Limit> {
    match filter {
        "claude" => limits
            .into_iter()
            .filter(|l| l.provider == model::Provider::Anthropic)
            .collect(),
        "codex" => limits
            .into_iter()
            .filter(|l| l.provider == model::Provider::Codex)
            .collect(),
        // "both" plus every unknown / legacy / typo'd value. `serde(default)`
        // only fills in *missing* fields — it never validated this string, so a
        // stale "worst" can still reach us. Blanking the whole app would be far
        // worse than showing an extra provider, hence the catch-all.
        _ => limits,
    }
}

fn worst<'a>(snap: &'a Snapshot) -> Option<&'a Limit> {
    snap.worst_id
        .as_ref()
        .and_then(|id| snap.limits.iter().find(|l| &l.id == id))
}

fn update_tray(app: &AppHandle, snap: &Snapshot) {
    let Some(tray) = app.tray_by_id("tokenbar") else {
        return;
    };
    // Icon = worst limit's fuel capsule, filled to what remains (§5.1).
    if let Some(l) = worst(snap) {
        let _ = tray.set_icon(Some(capsule_icon(100.0 - l.util, status_rgb(l.status, l.util))));
    }
    // Rich hover: list every limit (§5 — the one place not limited to one).
    let tip = if snap.limits.is_empty() {
        "TokenBar — no data".to_string()
    } else {
        let mut lines = vec!["TokenBar".to_string()];
        for l in &snap.limits {
            let val = match l.status {
                LimitStatus::SourceFailed => "估算".to_string(),
                LimitStatus::Locked => "LOCKED".to_string(),
                _ => format!("{:.0}% used", l.util),
            };
            lines.push(format!("{}  {}", l.label, val));
        }
        lines.join("\n")
    };
    let _ = tray.set_tooltip(Some(&tip));
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
            .unwrap_or_else(|| format!("{} 目前無法取得用量", l.label));
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
    let (warn, crit) = app
        .try_state::<AppData>()
        .and_then(|d| d.settings.lock().ok().map(|s| (s.warn_pct, s.crit_pct)))
        .unwrap_or((75.0, 90.0));

    // Source failures first: they mean the numbers below are missing entirely,
    // which is more urgent than any of them being high. Until this existed the
    // user could only find out by opening the panel — a SourceFailed limit's
    // util is a 0.0 placeholder, so the quota loop below never fires for one.
    for body in due_source_notices(snap, notified, now) {
        let _ = app.notification().builder().title("TokenBar").body(body).show();
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

        let tip = match l.provider {
            model::Provider::Codex => "可切 mini 模型延長額度",
            model::Provider::Anthropic => "可 /compact 或改用 Sonnet",
        };
        let body = if matches!(l.status, LimitStatus::Locked) {
            format!("{} 已鎖定。{}", l.label, tip)
        } else {
            format!("{} 已用 {:.0}%（{}）。{}", l.label, l.util, level, tip)
        };
        let _ = app.notification().builder().title("TokenBar").body(body).show();
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

    /// Unknown values (the legacy "worst", or a hand-edited typo in
    /// settings.json) must show everything. Never empty — an empty filter
    /// result blanks out the entire app.
    #[test]
    fn unknown_filter_value_shows_everything() {
        assert_eq!(apply_provider_filter("worst", both_providers()).len(), 2);
        assert_eq!(apply_provider_filter("", both_providers()).len(), 2);
        // wrong case must not silently become a single-provider filter either
        assert_eq!(apply_provider_filter("CLAUDE", both_providers()).len(), 2);
    }

    #[test]
    fn claude_filter_drops_codex() {
        let out = apply_provider_filter("claude", both_providers());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].provider, Provider::Anthropic);
    }

    #[test]
    fn codex_filter_drops_claude() {
        let out = apply_provider_filter("codex", both_providers());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].provider, Provider::Codex);
    }

    #[test]
    fn both_keeps_everything() {
        assert_eq!(apply_provider_filter("both", both_providers()).len(), 2);
    }

    /// A filter for a provider that simply has no data yet is legitimately
    /// empty — the filter must not invent rows to compensate.
    #[test]
    fn single_provider_filter_on_absent_provider_is_empty() {
        let only_codex = vec![limit("codex.5h", Provider::Codex)];
        assert!(apply_provider_filter("claude", only_codex).is_empty());
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
