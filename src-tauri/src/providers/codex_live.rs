//! Live Codex quota reader. It uses the current Codex login only when the
//! user selects the live or auto source; credentials stay in memory and are
//! never refreshed, persisted, or logged.

use super::codex::classify;
use crate::model::{Limit, LimitStatus, Provider};
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;

const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const REFRESH_SECS: i64 = 180; // default cadence; the live value now comes from
                               // settings.refresh_secs (T-910)
const FORCE_MIN_SECS: i64 = 5;

pub struct CodexLiveProvider {
    last_fetch: i64,
    cached: Option<Vec<Limit>>,
    /// The interval (secs) used at the last poll, so `next_fetch_at`'s countdown
    /// matches the cadence actually in force this round (T-910).
    interval: i64,
}

impl CodexLiveProvider {
    pub fn new() -> Self {
        Self {
            last_fetch: 0,
            cached: None,
            interval: REFRESH_SECS,
        }
    }

    /// Return a cached live response for `refresh_secs` seconds, or five seconds
    /// after a user-forced refresh. Failures are cached too, avoiding retry
    /// storms. `refresh_secs` is read from live settings each round (T-910).
    ///
    /// NOTE: unlike the Anthropic provider this has *no* 429 exponential
    /// backoff. That is deliberate: it hits a different host/rate bucket
    /// (chatgpt.com, not the shared Claude Code OAuth bucket of F-01), and its
    /// `fetch()` collapses every error into `None` via `.ok()?`, so it cannot
    /// distinguish a 429 without restructuring its error handling — which would
    /// be more than the mechanical interval change T-910 asks for here.
    pub fn poll(&mut self, now: i64, force: bool, refresh_secs: i64) -> Option<Vec<Limit>> {
        self.interval = refresh_secs;
        let min_gap = if force { FORCE_MIN_SECS } else { refresh_secs };
        if now - self.last_fetch < min_gap {
            return self.cached.clone();
        }
        self.last_fetch = now;
        self.cached = self.fetch();
        self.cached.clone()
    }

    /// Epoch secs of the next scheduled network fetch (cache expiry). Drives the
    /// header refresh countdown; the scheduler polls sooner but returns cached
    /// data until this point.
    pub fn next_fetch_at(&self) -> i64 {
        self.last_fetch + self.interval
    }

    fn fetch(&self) -> Option<Vec<Limit>> {
        let creds = read_creds()?;
        let usage = ureq::get(USAGE_URL)
            .set("Authorization", &format!("Bearer {}", creds.access_token))
            .set("ChatGPT-Account-Id", &creds.account_id)
            .set("User-Agent", "atoll")
            .call()
            .ok()?
            .into_json::<Value>()
            .ok()?;
        parse_usage(&usage)
    }
}

struct Creds {
    access_token: String,
    account_id: String,
}

fn creds_path() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".codex/auth.json"))
}

fn read_creds() -> Option<Creds> {
    let raw = std::fs::read_to_string(creds_path()?).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    let tokens = value.get("tokens")?;
    Some(Creds {
        access_token: tokens.get("access_token")?.as_str()?.to_owned(),
        account_id: tokens.get("account_id")?.as_str()?.to_owned(),
    })
}

/// Turn one window node into a Limit, labeling it by length via the shared
/// `classify` (`limit_window_seconds` → minutes). Returns None for anything that
/// isn't a window object (scalars, `null` slots, credits, …).
fn parse_window(node: &Value) -> Option<Limit> {
    let window_secs = node.get("limit_window_seconds")?.as_i64()?;
    let (id, label) = classify(window_secs / 60);
    Some(Limit {
        id,
        provider: Provider::Codex,
        label,
        util: node.get("used_percent")?.as_f64()?,
        resets_at: node.get("reset_at")?.as_i64()?,
        window_secs,
        status: LimitStatus::Normal,
        absolute: None,
        pace: None,
        runway_secs: None,
        hint: None,
        action: None,
    })
}

/// Discover every window in the `rate_limit` object regardless of its key name
/// (`primary_window`, `secondary_window`, or anything Codex adds later), shortest
/// first and one per id. `None` when no usable window is present (e.g. a
/// credits-only response) so `auto` can fall back to the local snapshot.
pub fn parse_usage(v: &Value) -> Option<Vec<Limit>> {
    let rate_limit = v.get("rate_limit")?.as_object()?;
    let mut out: Vec<Limit> = rate_limit.values().filter_map(parse_window).collect();
    out.sort_by_key(|l| l.window_secs);
    let mut seen = HashSet::new();
    out.retain(|l| seen.insert(l.id.clone()));
    (!out.is_empty()).then_some(out)
}

pub fn choose_limits(source: &str, live: Option<Vec<Limit>>, local: Vec<Limit>) -> Vec<Limit> {
    match source {
        "local" => local,
        "auto" => live.unwrap_or(local),
        _ => live.unwrap_or_else(degraded_limits),
    }
}

fn degraded_limits() -> Vec<Limit> {
    [("codex.5h", "Codex·5h", 5 * 3600), ("codex.week", "Codex·Weekly", 7 * 86400)]
        .iter()
        .map(|(id, label, window_secs)| Limit {
            id: (*id).into(),
            provider: Provider::Codex,
            label: (*label).into(),
            util: 0.0,
            resets_at: 0,
            window_secs: *window_secs,
            status: LimitStatus::SourceFailed,
            absolute: None,
            pace: None,
            runway_secs: None,
            hint: None,
            action: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_live_usage_windows() {
        let usage = json!({
            "rate_limit": {
                "primary_window": { "used_percent": 15, "limit_window_seconds": 18000, "reset_at": 1783697640i64 },
                "secondary_window": { "used_percent": 3, "limit_window_seconds": 604800, "reset_at": 1784252456i64 }
            }
        });
        let limits = parse_usage(&usage).expect("valid response");
        assert_eq!(limits[0].id, "codex.5h");
        assert_eq!(limits[0].util, 15.0);
        assert_eq!(limits[0].window_secs, 18_000);
        assert_eq!(limits[1].id, "codex.week");
        assert_eq!(limits[1].util, 3.0);
    }

    #[test]
    fn rejects_missing_primary_window() {
        assert!(parse_usage(&json!({ "rate_limit": {} })).is_none());
    }

    #[test]
    fn weekly_only_live_response_is_labeled_weekly() {
        // Observed 2026-07: primary_window carries the weekly window (604800s),
        // secondary_window is null. Must surface as the weekly limit, not "5h".
        let usage = json!({
            "rate_limit": {
                "primary_window": { "used_percent": 3, "limit_window_seconds": 604800, "reset_at": 1784549867i64 },
                "secondary_window": null
            }
        });
        let limits = parse_usage(&usage).expect("weekly window is usable data");
        assert_eq!(limits.len(), 1);
        assert_eq!(limits[0].id, "codex.week");
        assert_eq!(limits[0].util, 3.0);
    }

    fn limit(id: &str, util: f64) -> Limit {
        Limit {
            id: id.into(),
            provider: Provider::Codex,
            label: id.into(),
            util,
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

    #[test]
    fn auto_keeps_local_limits_when_live_result_is_missing() {
        let local = vec![limit("codex.5h", 42.0), limit("codex.week", 5.0)];
        assert_eq!(choose_limits("auto", None, local.clone())[0].util, 42.0);
        assert_eq!(choose_limits("live", None, local)[0].status, LimitStatus::SourceFailed);
    }
}
