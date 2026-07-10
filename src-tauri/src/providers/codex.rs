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
//! `primary` = 5h window, `secondary` = weekly. `used_percent` is util%.

use crate::model::{Limit, LimitStatus, Provider};
use serde::Deserialize;
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

#[derive(Deserialize)]
struct RateLimits {
    primary: Option<Window>,
    secondary: Option<Window>,
}

/// Read the current Codex limits, or an empty vec if unavailable (tool not run,
/// no session file yet, etc. — caller renders "tool not running").
pub fn read_limits() -> Vec<Limit> {
    let now = chrono::Utc::now().timestamp();
    for (path, mtime) in newest_rollouts(MAX_FILES) {
        let Ok(text) = read_tail(&path, TAIL_BYTES) else {
            continue;
        };
        if let Some(rl) = extract_last_rate_limits(&text) {
            return to_limits(rl, now, now - mtime);
        }
    }
    vec![]
}

/// Convert a snapshot into limits, honestly accounting for its age:
/// - window already reset (`resets_at` passed) → the old util no longer applies;
///   the true current value is 0 (Idle when the file is old, i.e. tool not running).
/// - window still active but snapshot old → last-known util, marked Stale.
fn to_limits(rl: RateLimits, now: i64, age_secs: i64) -> Vec<Limit> {
    let mut out = Vec::new();
    if let Some(w) = rl.primary {
        out.push(mk_limit("codex.5h", "Codex·5h", w, now, age_secs));
    }
    if let Some(w) = rl.secondary {
        out.push(mk_limit("codex.week", "Codex·週", w, now, age_secs));
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

/// Find the last `"rate_limits":{...}` object via brace matching and parse it.
/// Values inside are numbers/null/short strings with no braces, so naive
/// brace-depth scanning is safe here.
fn extract_last_rate_limits(text: &str) -> Option<RateLimits> {
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
    serde_json::from_str::<RateLimits>(obj).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"foo":1,"rate_limits":{"limit_id":"codex","limit_name":null,"primary":{"used_percent":4.0,"window_minutes":300,"resets_at":1782590353},"secondary":{"used_percent":7.0,"window_minutes":10080,"resets_at":1782976756},"credits":null,"individual_limit":null,"plan_type":"plus","rate_limit_reached_type":null}}"#;

    #[test]
    fn parses_real_rate_limits_shape() {
        let rl = extract_last_rate_limits(SAMPLE).expect("should parse");
        let p = rl.primary.unwrap();
        assert_eq!(p.used_percent, 4.0);
        assert_eq!(p.window_minutes, 300);
        let s = rl.secondary.unwrap();
        assert_eq!(s.window_minutes, 10080);
    }

    #[test]
    fn takes_the_last_occurrence() {
        let two = format!(
            "{}\n{}",
            SAMPLE.replace("4.0", "1.0"),
            SAMPLE.replace("4.0", "42.0")
        );
        let rl = extract_last_rate_limits(&two).unwrap();
        assert_eq!(rl.primary.unwrap().used_percent, 42.0);
    }

    #[test]
    fn none_when_absent() {
        assert!(extract_last_rate_limits(r#"{"no":"limits here"}"#).is_none());
    }

    fn rl(resets_at: i64) -> RateLimits {
        RateLimits {
            primary: Some(Window { used_percent: 12.0, window_minutes: 300, resets_at }),
            secondary: None,
        }
    }

    #[test]
    fn expired_window_reads_as_zero() {
        // snapshot says 12% but the window reset an hour ago → truth is 0%.
        let now = 1_000_000;
        let ls = to_limits(rl(now - 3600), now, /*age*/ 6 * 86400);
        assert_eq!(ls[0].util, 0.0);
        assert_eq!(ls[0].resets_at, 0);
        assert_eq!(ls[0].status, LimitStatus::Idle);
    }

    #[test]
    fn active_window_with_old_file_is_stale() {
        let now = 1_000_000;
        let ls = to_limits(rl(now + 3600), now, /*age*/ 30 * 60);
        assert_eq!(ls[0].util, 12.0);
        assert_eq!(ls[0].status, LimitStatus::Stale);
    }

    #[test]
    fn active_window_with_fresh_file_is_normal() {
        let now = 1_000_000;
        let ls = to_limits(rl(now + 3600), now, /*age*/ 60);
        assert_eq!(ls[0].util, 12.0);
        assert_eq!(ls[0].status, LimitStatus::Normal);
    }
}
