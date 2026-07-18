//! Grok provider (T-917) — a **context-fill** limit read from the newest Grok
//! session's `signals.json`. Local file read only, NO network.
//!
//! Schema confirmed against a real file (2026-07-18):
//! ```json
//! { "contextTokensUsed": 68468, "contextWindowTokens": 500000,
//!   "primaryModelId": "grok-4.5", ... }
//! ```
//! `util = contextTokensUsed / contextWindowTokens × 100`, clamped 0..=100.
//!
//! This is NOT a subscription quota: a context window has no reset schedule
//! (`resets_at = 0`, `window_secs = 0`) — it empties when the user starts a new
//! session. The panel shows a per-session note instead of a reset countdown.
//!
//! Freshness mirrors the spirit of the Codex local source (providers/codex.rs)
//! but with the closer of its two semantics: because we KEEP the last-known
//! fill value when the file is old (rather than zeroing it the way Codex zeroes
//! an expired window), an old file is **Stale**, never Idle. A missing file / no
//! sessions yields an `InsufficientData` placeholder — an honest "tracking Grok,
//! no reading yet", NEVER a fake 0%.
//!
//! `read_limits` always returns exactly one limit so a selected Grok always
//! shows a card on the limits page (使用者決策 2026-07-18:勾選的供應商要出現在
//! 限額頁), even before there is any data.

use crate::model::{Limit, LimitStatus, Provider};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

/// Stable limit id + display label (label is a fallback; the panel localizes the
/// name via LIMIT_NAME_KEYS["grok.ctx"]).
pub const LIMIT_ID: &str = "grok.ctx";
const LIMIT_LABEL: &str = "Grok·Context";
/// A signals.json older than this (context still holds its last value) is Stale.
const STALE_SECS: i64 = 15 * 60;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Signals {
    context_tokens_used: Option<u64>,
    context_window_tokens: Option<u64>,
    #[allow(dead_code)]
    primary_model_id: Option<String>,
}

/// The Grok context-fill limit. Always one element:
///   fresh reading      → Normal (engine derives Near/Locked by threshold)
///   old file (>15min)  → Stale, last-known fill kept
///   no data / no window → InsufficientData placeholder (util 0.0, shown as "—")
pub fn read_limits() -> Vec<Limit> {
    let now = chrono::Utc::now().timestamp();
    vec![match newest_signals() {
        Some((sig, mtime)) => from_signals(&sig, now - mtime).unwrap_or_else(insufficient),
        None => insufficient(),
    }]
}

/// Build the limit from a parsed signals file, honouring snapshot age. `None`
/// when the file lacks a usable window (so the caller falls back to the
/// insufficient-data placeholder rather than dividing by zero / faking a 0%).
fn from_signals(sig: &Signals, age_secs: i64) -> Option<Limit> {
    let used = sig.context_tokens_used?;
    let window = sig.context_window_tokens.filter(|&w| w > 0)?;
    let util = ((used as f64 / window as f64) * 100.0).clamp(0.0, 100.0);
    let status = if age_secs > STALE_SECS {
        LimitStatus::Stale
    } else {
        // Normal: the engine derives Near/Locked from the util threshold and
        // records a burn sample. A full context window is exactly what the
        // warning colours are for (T-917 brief).
        LimitStatus::Normal
    };
    Some(Limit {
        id: LIMIT_ID.into(),
        provider: Provider::Grok,
        label: LIMIT_LABEL.into(),
        util,
        resets_at: 0,   // context has no reset schedule
        window_secs: 0, // …and no window length, so pace/runway stay honest
        status,
        absolute: Some((used, window)),
        pace: None,
        runway_secs: None,
        hint: None,
        action: None,
    })
}

/// The placeholder shown when there is no readable Grok data. Util is a 0.0
/// placeholder that the UI renders as "—" (InsufficientData), never "0% used".
fn insufficient() -> Limit {
    Limit {
        id: LIMIT_ID.into(),
        provider: Provider::Grok,
        label: LIMIT_LABEL.into(),
        util: 0.0,
        resets_at: 0,
        window_secs: 0,
        status: LimitStatus::InsufficientData,
        absolute: None,
        pace: None,
        runway_secs: None,
        hint: None,
        action: None,
    }
}

/// Parse the newest `~/.grok/sessions/**/signals.json` (by mtime), returning it
/// with the file's mtime (epoch secs). `None` when no session file exists or
/// none parses.
fn newest_signals() -> Option<(Signals, i64)> {
    let home = dirs::home_dir()?;
    let pattern = home
        .join(".grok/sessions/**/signals.json")
        .to_string_lossy()
        .replace('\\', "/");
    let mut files: Vec<(PathBuf, i64)> = glob::glob(&pattern)
        .ok()?
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
    // Try newest-first; a truncated/half-written newest file falls back to the
    // next one rather than reporting no data.
    for (path, mtime) in files {
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(sig) = serde_json::from_str::<Signals>(&text) {
                return Some((sig, mtime));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig(used: Option<u64>, window: Option<u64>) -> Signals {
        Signals {
            context_tokens_used: used,
            context_window_tokens: window,
            primary_model_id: Some("grok-4.5".into()),
        }
    }

    #[test]
    fn parses_the_real_signals_shape() {
        // The exact fields from a real signals.json (2026-07-18).
        let s: Signals = serde_json::from_str(
            r#"{ "turnCount": 9, "contextWindowUsage": 13,
                 "contextTokensUsed": 68468, "contextWindowTokens": 500000,
                 "primaryModelId": "grok-4.5" }"#,
        )
        .unwrap();
        assert_eq!(s.context_tokens_used, Some(68468));
        assert_eq!(s.context_window_tokens, Some(500_000));
    }

    #[test]
    fn util_is_used_over_window_and_fresh_is_normal() {
        // 68468 / 500000 = 13.6936%.
        let l = from_signals(&sig(Some(68_468), Some(500_000)), /*age*/ 60).unwrap();
        assert_eq!(l.id, "grok.ctx");
        assert_eq!(l.provider, Provider::Grok);
        assert!((l.util - 13.6936).abs() < 1e-4);
        assert_eq!(l.status, LimitStatus::Normal);
        assert_eq!(l.resets_at, 0);
        assert_eq!(l.absolute, Some((68_468, 500_000)));
    }

    #[test]
    fn util_clamps_to_one_hundred_when_over_full() {
        // A window that reports used > window must never exceed 100%.
        let l = from_signals(&sig(Some(600_000), Some(500_000)), 0).unwrap();
        assert_eq!(l.util, 100.0);
    }

    #[test]
    fn old_file_keeps_value_but_marks_stale() {
        // >15min old: the fill is the last-known value, flagged Stale (kept, not
        // zeroed — a context window has no rollover to reset it to 0).
        let l = from_signals(&sig(Some(250_000), Some(500_000)), /*age*/ 20 * 60).unwrap();
        assert_eq!(l.util, 50.0);
        assert_eq!(l.status, LimitStatus::Stale);
    }

    #[test]
    fn missing_window_is_insufficient_not_a_fake_zero() {
        // No window (or a zero window) can't yield a percentage → None, so the
        // caller shows the InsufficientData placeholder, never a fake 0%.
        assert!(from_signals(&sig(Some(100), None), 0).is_none());
        assert!(from_signals(&sig(Some(100), Some(0)), 0).is_none());
        assert!(from_signals(&sig(None, Some(500_000)), 0).is_none());

        let l = insufficient();
        assert_eq!(l.status, LimitStatus::InsufficientData);
        assert_eq!(l.util, 0.0);
        assert_eq!(l.absolute, None);
    }
}
