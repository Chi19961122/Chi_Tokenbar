//! Codex provider — reads the newest `rollout-*.jsonl` session file and pulls
//! the most recent `rate_limits` snapshot. Schema confirmed against a real file
//! (see Ai_Assistant/data-sources-findings.md):
//!
//! ```json
//! "rate_limits": {
//!   "primary":   { "used_percent": 4.0, "window_minutes": 300,   "resets_at": 1782590353 },
//!   "secondary": { "used_percent": 7.0, "window_minutes": 10080, "resets_at": 1782976756 },
//!   "credits": null, "plan_type": "plus"
//! }
//! ```
//! Historically `primary` = 5h window and `secondary` = weekly, but Codex no
//! longer keeps them in fixed slots: a snapshot may carry only the weekly
//! window in `primary` with `secondary` null (observed 2026-07). To stay robust
//! against further schema churn we do NOT rely on the `primary`/`secondary` key
//! names or their order: we scan every field of the `rate_limits` object, keep
//! whatever looks like a window (has `used_percent` + `window_minutes` +
//! `resets_at`), and label each one by its `window_minutes` length. New or
//! renamed windows Codex adds in the future are picked up automatically.
//! `used_percent` is util%.

use crate::model::{Limit, LimitStatus, Provider};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

/// Only the tail holds the freshest snapshot; avoid re-reading the whole (often
/// tens of MB) file each poll.
const TAIL_BYTES: u64 = 512 * 1024;
/// Snapshot older than this (and window still active) is shown as Stale.
const STALE_SECS: i64 = 15 * 60;
/// How many newest session files to try before giving up (some sessions may
/// lack a rate_limits snapshot in their tail).
const MAX_FILES: usize = 5;

#[derive(Deserialize)]
struct Window {
    used_percent: f64,
    window_minutes: i64,
    resets_at: i64,
}

/// Read the current Codex limits, or an empty vec if unavailable (tool not run,
/// no session file yet, etc. — caller renders "tool not running").
pub fn read_limits() -> Vec<Limit> {
    let now = chrono::Utc::now().timestamp();
    for (path, mtime) in newest_rollouts(MAX_FILES) {
        let Ok(text) = read_tail(&path, TAIL_BYTES) else {
            continue;
        };
        let Some(rl) = extract_last_rate_limits(&text) else {
            continue;
        };
        let limits = to_limits(windows_from(&rl), now, now - mtime);
        // A degenerate snapshot (e.g. only `credits`, no windows) yields nothing
        // — keep looking in older files rather than reporting empty.
        if !limits.is_empty() {
            return limits;
        }
    }
    vec![]
}

/// Pull every window-shaped value out of the `rate_limits` object, whatever its
/// key is called. Non-window fields (`credits`, `plan_type`, `null` slots, …)
/// simply fail to deserialize and are skipped.
fn windows_from(rate_limits: &Value) -> Vec<Window> {
    rate_limits
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .filter_map(|v| serde_json::from_value::<Window>(v.clone()).ok())
        .collect()
}

/// Map a window to a stable id + display label by its length. The two windows
/// Codex ships today (≈300 min = 5h, ≈10080 min = weekly) keep canonical ids so
/// analytics/history line up; any other length Codex might introduce is still
/// surfaced with an id/label derived from its duration rather than dropped.
pub(crate) fn classify(window_minutes: i64) -> (String, String) {
    if (250..=360).contains(&window_minutes) {
        ("codex.5h".into(), "Codex·5h".into())
    } else if (9000..=11000).contains(&window_minutes) {
        ("codex.week".into(), "Codex·週".into())
    } else if window_minutes < 24 * 60 {
        let h = ((window_minutes as f64) / 60.0).round() as i64;
        (format!("codex.min{window_minutes}"), format!("Codex·{h}h"))
    } else {
        let d = ((window_minutes as f64) / 1440.0).round() as i64;
        (format!("codex.min{window_minutes}"), format!("Codex·{d}d"))
    }
}

/// Convert the discovered windows into limits, shortest window first and one per
/// id, honestly accounting for snapshot age:
/// - window already reset (`resets_at` passed) → the old util no longer applies;
///   the true current value is 0 (Idle when the file is old, i.e. tool not running).
/// - window still active but snapshot old → last-known util, marked Stale.
fn to_limits(mut windows: Vec<Window>, now: i64, age_secs: i64) -> Vec<Limit> {
    windows.sort_by_key(|w| w.window_minutes);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for w in windows {
        let (id, label) = classify(w.window_minutes);
        if seen.insert(id.clone()) {
            out.push(mk_limit(&id, &label, w, now, age_secs));
        }
    }
    out
}

fn mk_limit(id: &str, label: &str, w: Window, now: i64, age_secs: i64) -> Limit {
    let expired = w.resets_at <= now;
    let stale = age_secs > STALE_SECS;
    let (util, resets_at, status) = if expired {
        // The window rolled over since this snapshot; usage restarted at 0.
        (0.0, 0, if stale { LimitStatus::Idle } else { LimitStatus::Normal })
    } else if stale {
        (w.used_percent, w.resets_at, LimitStatus::Stale)
    } else {
        (w.used_percent, w.resets_at, LimitStatus::Normal)
    };
    Limit {
        id: id.into(),
        provider: Provider::Codex,
        label: label.into(),
        util,
        resets_at,
        window_secs: w.window_minutes * 60,
        // pace/runway are filled by the engine (Normal only).
        status,
        absolute: None,
        pace: None,
        runway_secs: None,
        hint: None,
        action: None,
    }
}

/// Newest N rollout files by mtime, with their mtimes (epoch secs).
fn newest_rollouts(n: usize) -> Vec<(PathBuf, i64)> {
    let Some(home) = dirs::home_dir() else {
        return vec![];
    };
    let pattern = home
        .join(".codex/sessions/**/rollout-*.jsonl")
        .to_string_lossy()
        .replace('\\', "/");
    let Ok(paths) = glob::glob(&pattern) else {
        return vec![];
    };
    let mut files: Vec<(PathBuf, i64)> = paths
        .filter_map(Result::ok)
        .filter_map(|p| {
            let m = fs::metadata(&p)
                .ok()?
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?;
            Some((p, m.as_secs() as i64))
        })
        .collect();
    files.sort_by_key(|(_, m)| std::cmp::Reverse(*m));
    files.truncate(n);
    files
}

fn read_tail(path: &PathBuf, max: u64) -> std::io::Result<String> {
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    let start = len.saturating_sub(max);
    f.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Find the last `"rate_limits":{...}` object via brace matching and parse it
/// into a generic JSON object. Values inside are numbers/null/short strings with
/// no braces, so naive brace-depth scanning is safe here.
fn extract_last_rate_limits(text: &str) -> Option<Value> {
    let key = "\"rate_limits\":";
    let key_at = text.rfind(key)?;
    let rest = &text[key_at + key.len()..];
    let brace_off = rest.find('{')?;
    let obj_start = key_at + key.len() + brace_off;

    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut obj_end = None;
    for i in obj_start..bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    obj_end = Some(i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let obj = &text[obj_start..obj_end?];
    serde_json::from_str::<Value>(obj).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"foo":1,"rate_limits":{"limit_id":"codex","limit_name":null,"primary":{"used_percent":4.0,"window_minutes":300,"resets_at":1782590353},"secondary":{"used_percent":7.0,"window_minutes":10080,"resets_at":1782976756},"credits":null,"individual_limit":null,"plan_type":"plus","rate_limit_reached_type":null}}"#;

    #[test]
    fn parses_real_rate_limits_shape() {
        let rl = extract_last_rate_limits(SAMPLE).expect("should parse");
        let mut ws = windows_from(&rl);
        ws.sort_by_key(|w| w.window_minutes);
        assert_eq!(ws.len(), 2);
        assert_eq!(ws[0].window_minutes, 300);
        assert_eq!(ws[0].used_percent, 4.0);
        assert_eq!(ws[1].window_minutes, 10080);
    }

    #[test]
    fn takes_the_last_occurrence() {
        let two = format!(
            "{}\n{}",
            SAMPLE.replace("4.0", "1.0"),
            SAMPLE.replace("4.0", "42.0")
        );
        let rl = extract_last_rate_limits(&two).unwrap();
        let five_h = windows_from(&rl)
            .into_iter()
            .find(|w| w.window_minutes == 300)
            .unwrap();
        assert_eq!(five_h.used_percent, 42.0);
    }

    #[test]
    fn none_when_absent() {
        assert!(extract_last_rate_limits(r#"{"no":"limits here"}"#).is_none());
    }

    /// Windows carrying only a 5h (300 min) slot — mirrors the old fixture where
    /// `primary` was the 5h window.
    fn ws_5h(resets_at: i64) -> Vec<Window> {
        vec![Window { used_percent: 12.0, window_minutes: 300, resets_at }]
    }

    #[test]
    fn expired_window_reads_as_zero() {
        // snapshot says 12% but the window reset an hour ago → truth is 0%.
        let now = 1_000_000;
        let ls = to_limits(ws_5h(now - 3600), now, /*age*/ 6 * 86400);
        assert_eq!(ls[0].util, 0.0);
        assert_eq!(ls[0].resets_at, 0);
        assert_eq!(ls[0].status, LimitStatus::Idle);
    }

    #[test]
    fn active_window_with_old_file_is_stale() {
        let now = 1_000_000;
        let ls = to_limits(ws_5h(now + 3600), now, /*age*/ 30 * 60);
        assert_eq!(ls[0].util, 12.0);
        assert_eq!(ls[0].status, LimitStatus::Stale);
    }

    #[test]
    fn active_window_with_fresh_file_is_normal() {
        let now = 1_000_000;
        let ls = to_limits(ws_5h(now + 3600), now, /*age*/ 60);
        assert_eq!(ls[0].util, 12.0);
        assert_eq!(ls[0].status, LimitStatus::Normal);
    }

    #[test]
    fn weekly_only_snapshot_is_not_mislabeled_as_5h() {
        // Codex 2026-07 shape: the weekly window arrives in `primary` with
        // `secondary` null. It must show as the weekly limit, not as "5h".
        let now = 1_000_000;
        let ls = to_limits(
            vec![Window { used_percent: 3.0, window_minutes: 10080, resets_at: now + 3600 }],
            now,
            /*age*/ 60,
        );
        assert_eq!(ls.len(), 1);
        assert_eq!(ls[0].id, "codex.week");
        assert_eq!(ls[0].util, 3.0);
    }

    #[test]
    fn classifies_by_length_regardless_of_order() {
        // Order in the source object is irrelevant; each window is routed by its
        // length and the shorter (5h) window is listed first.
        let now = 1_000_000;
        let ls = to_limits(
            vec![
                Window { used_percent: 3.0, window_minutes: 10080, resets_at: now + 3600 },
                Window { used_percent: 40.0, window_minutes: 300, resets_at: now + 3600 },
            ],
            now,
            /*age*/ 60,
        );
        assert_eq!(ls[0].id, "codex.5h");
        assert_eq!(ls[0].util, 40.0);
        assert_eq!(ls[1].id, "codex.week");
    }

    #[test]
    fn discovers_windows_under_any_key_name() {
        // Future-proofing: even if Codex renames the slots, any window-shaped
        // field is still discovered and labeled by its length.
        let rl: Value = serde_json::from_str(
            r#"{"main_window":{"used_percent":5.0,"window_minutes":300,"resets_at":9999999999},
                "credits":null,"plan_type":"plus"}"#,
        )
        .unwrap();
        let ws = windows_from(&rl);
        assert_eq!(ws.len(), 1);
        assert_eq!(ws[0].window_minutes, 300);
    }

    #[test]
    fn unknown_window_length_is_surfaced_not_dropped() {
        // A window Codex might add in the future (e.g. hourly) still shows up
        // with a duration-derived id/label instead of vanishing.
        let (id, label) = classify(60);
        assert_eq!(id, "codex.min60");
        assert_eq!(label, "Codex·1h");
        let (id, label) = classify(43200); // 30 days
        assert_eq!(id, "codex.min43200");
        assert_eq!(label, "Codex·30d");
    }
}
