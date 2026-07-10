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
fn get_analytics(range: String) -> analytics::Analytics {
    analytics::compute(&range)
}

#[tauri::command]
fn get_settings(data: State<'_, AppData>) -> Settings {
    data.settings.lock().map(|g| g.clone()).unwrap_or_default()
}

#[tauri::command]
fn set_settings(app: AppHandle, data: State<'_, AppData>, settings: Settings) {
    config::save(&settings);
    apply_autostart(&app, settings.autostart);
    if let Ok(mut g) = data.settings.lock() {
        *g = settings;
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings = config::load();
    let allow_refresh = settings.allow_token_refresh;
    let (refresh_tx, refresh_rx) = mpsc::channel::<()>();

    tauri::Builder::default()
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
            refresh_now
        ])
        .setup(move |app| {
            build_tray(app.handle())?;
            position_island(app.handle());
            apply_autostart(app.handle(), settings.autostart);
            spawn_scheduler(app.handle().clone(), allow_refresh, refresh_rx);
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

fn toggle_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        match win.is_visible() {
            Ok(true) => {
                let _ = win.hide();
            }
            _ => {
                let _ = win.show();
                let _ = win.set_focus();
            }
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

fn spawn_scheduler(app: AppHandle, allow_refresh: bool, refresh_rx: Receiver<()>) {
    std::thread::spawn(move || {
        let mut engine = Engine::new();
        let mut anthropic = AnthropicProvider::new(allow_refresh);
        let mut codex_live = providers::codex_live::CodexLiveProvider::new();
        let mut notified: HashMap<String, i64> = HashMap::new();
        let debug = std::env::var("TOKENBAR_DEBUG").is_ok();
        let mut first = true;
        let mut force = false;

        loop {
            let now = chrono::Utc::now().timestamp();

            let codex_source = app
                .try_state::<AppData>()
                .and_then(|data| data.settings.lock().ok().map(|s| s.codex_usage_source.clone()))
                .unwrap_or_else(|| "local".into());
            let live = if matches!(codex_source.as_str(), "live" | "auto") {
                codex_live.poll(now, force)
            } else {
                None
            };
            let local = if matches!(codex_source.as_str(), "local" | "auto") {
                providers::codex::read_limits()
            } else {
                Vec::new()
            };
            let mut limits = providers::codex_live::choose_limits(&codex_source, live, local);
            limits.extend(anthropic.poll(now, force));
            let snapshot = engine.ingest(limits, now);

            if debug {
                for l in &snapshot.limits {
                    eprintln!(
                        "[tb] {:?} {} util={:.0} status={:?} runway={:?}",
                        l.provider, l.id, l.util, l.status, l.runway_secs
                    );
                }
                if first {
                    let a = analytics::compute("today");
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

    for l in &snap.limits {
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
