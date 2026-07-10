//! Live Codex quota reader. It uses the current Codex login only when the
//! user selects the live or auto source; credentials stay in memory and are
//! never refreshed, persisted, or logged.

use crate::model::{Limit, LimitStatus, Provider};
use serde_json::Value;
use std::path::PathBuf;

const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const REFRESH_SECS: i64 = 180;
const FORCE_MIN_SECS: i64 = 5;

pub struct CodexLiveProvider {
    last_fetch: i64,
    cached: Option<Vec<Limit>>,
}

impl CodexLiveProvider {
    pub fn new() -> Self {
        Self {
            last_fetch: 0,
            cached: None,
        }
    }

    /// Return a cached live response for 180 seconds, or five seconds after a
    /// user-forced refresh. Failures are cached too, avoiding retry storms.
    pub fn poll(&mut self, now: i64, force: bool) -> Option<Vec<Limit>> {
        let min_gap = if force { FORCE_MIN_SECS } else { REFRESH_SECS };
        if now - self.last_fetch < min_gap {
            return self.cached.clone();
        }
        self.last_fetch = now;
        self.cached = self.fetch();
        self.cached.clone()
    }

    fn fetch(&self) -> Option<Vec<Limit>> {
        let creds = read_creds()?;
        let usage = ureq::get(USAGE_URL)
            .set("Authorization", &format!("Bearer {}", creds.access_token))
            .set("ChatGPT-Account-Id", &creds.account_id)
            .set("User-Agent", "tokenbar")
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

fn parse_window(id: &str, label: &str, node: &Value) -> Option<Limit> {
    Some(Limit {
        id: id.into(),
        provider: Provider::Codex,
        label: label.into(),
        util: node.get("used_percent")?.as_f64()?,
        resets_at: node.get("reset_at")?.as_i64()?,
        window_secs: node.get("limit_window_seconds")?.as_i64()?,
        status: LimitStatus::Normal,
        absolute: None,
        pace: None,
        runway_secs: None,
    })
}

pub fn parse_usage(v: &Value) -> Option<Vec<Limit>> {
    let limits = v.get("rate_limit")?;
    Some(vec![
        parse_window("codex.5h", "Codex·5h", limits.get("primary_window")?)?,
        parse_window("codex.week", "Codex·週", limits.get("secondary_window")?)?,
    ])
}

pub fn choose_limits(source: &str, live: Option<Vec<Limit>>, local: Vec<Limit>) -> Vec<Limit> {
    match source {
        "local" => local,
        "auto" => live.unwrap_or(local),
        _ => live.unwrap_or_else(degraded_limits),
    }
}

fn degraded_limits() -> Vec<Limit> {
    [("codex.5h", "Codex·5h", 5 * 3600), ("codex.week", "Codex·週", 7 * 86400)]
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
        }
    }

    #[test]
    fn auto_keeps_local_limits_when_live_result_is_missing() {
        let local = vec![limit("codex.5h", 42.0), limit("codex.week", 5.0)];
        assert_eq!(choose_limits("auto", None, local.clone())[0].util, 42.0);
        assert_eq!(choose_limits("live", None, local)[0].status, LimitStatus::SourceFailed);
    }
}
