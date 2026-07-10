//! Anthropic provider — the fragile path (UX Spec v3 §9, docs/data-sources-findings.md).
//!
//! Reads the OAuth token from `~/.claude/.credentials.json` and calls the
//! undocumented `GET /api/oauth/usage`. Everything is guarded: any failure
//! yields `source_failed` limits so the UI degrades instead of going blank (§7).
//!
//! SAFETY: refreshing the token can rotate the refresh token that Claude Code
//! itself relies on, which could log the user out. So the refresh flow is
//! opt-in (`allow_refresh`, default false). The read-only usage GET never
//! rotates anything and is always safe to attempt.

use crate::model::{Limit, LimitStatus, Provider};
use serde_json::Value;
use std::path::PathBuf;

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
/// Claude Code's public OAuth client id (community-known).
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const BETA_HEADER: &str = "oauth-2025-04-20";
const REFRESH_SECS: i64 = 180; // cache window per §9 (~180s)
/// Floor for forced (manual) refreshes so button-spamming can't hammer the API.
const FORCE_MIN_SECS: i64 = 5;

pub struct AnthropicProvider {
    allow_refresh: bool,
    last_fetch: i64,
    cached: Vec<Limit>,
}

impl AnthropicProvider {
    pub fn new(allow_refresh: bool) -> Self {
        Self {
            allow_refresh,
            last_fetch: 0,
            cached: Vec::new(),
        }
    }

    /// Return limits, hitting the network at most every REFRESH_SECS
    /// (FORCE_MIN_SECS when the user asked for a manual refresh).
    pub fn poll(&mut self, now: i64, force: bool) -> Vec<Limit> {
        let min_gap = if force { FORCE_MIN_SECS } else { REFRESH_SECS };
        if now - self.last_fetch < min_gap && !self.cached.is_empty() {
            return self.cached.clone();
        }
        self.last_fetch = now;
        self.cached = self.fetch().unwrap_or_else(degraded_limits);
        self.cached.clone()
    }

    fn fetch(&self) -> Option<Vec<Limit>> {
        let creds = read_creds()?;
        let now_ms = chrono::Utc::now().timestamp_millis();

        let token = if creds.expires_ms > now_ms + 60_000 {
            creds.access
        } else if self.allow_refresh {
            refresh_token(&creds.refresh)?
        } else {
            // Expired and refresh disabled → degrade honestly (no rotation risk).
            return None;
        };

        let usage = get_usage(&token)?;
        Some(parse_usage(&usage))
    }
}

struct Creds {
    access: String,
    refresh: String,
    expires_ms: i64,
}

fn creds_path() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".claude/.credentials.json"))
}

fn read_creds() -> Option<Creds> {
    let raw = std::fs::read_to_string(creds_path()?).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    let o = v.get("claudeAiOauth")?;
    Some(Creds {
        access: o.get("accessToken")?.as_str()?.to_string(),
        refresh: o.get("refreshToken")?.as_str()?.to_string(),
        expires_ms: o.get("expiresAt")?.as_i64().unwrap_or(0),
    })
}

/// Exchange the refresh token. On success, writes the (possibly rotated) tokens
/// back atomically so Claude Code stays in sync. Best-effort/undocumented.
fn refresh_token(refresh: &str) -> Option<String> {
    let resp: Value = ureq::post(TOKEN_URL)
        .set("Content-Type", "application/json")
        .send_json(serde_json::json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh,
            "client_id": CLIENT_ID,
        }))
        .ok()?
        .into_json()
        .ok()?;

    let access = resp.get("access_token")?.as_str()?.to_string();
    let new_refresh = resp
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .unwrap_or(refresh)
        .to_string();
    let expires_in = resp.get("expires_in").and_then(|v| v.as_i64()).unwrap_or(3600);
    let expires_ms = chrono::Utc::now().timestamp_millis() + expires_in * 1000;

    write_back_creds(&access, &new_refresh, expires_ms);
    Some(access)
}

/// Rewrite `.credentials.json` preserving the `claudeAiOauth` shape, atomically.
fn write_back_creds(access: &str, refresh: &str, expires_ms: i64) {
    let Some(path) = creds_path() else { return };
    let Ok(raw) = std::fs::read_to_string(&path) else { return };
    let Ok(mut v) = serde_json::from_str::<Value>(&raw) else { return };
    if let Some(o) = v.get_mut("claudeAiOauth").and_then(|x| x.as_object_mut()) {
        o.insert("accessToken".into(), access.into());
        o.insert("refreshToken".into(), refresh.into());
        o.insert("expiresAt".into(), expires_ms.into());
    }
    let tmp = path.with_extension("json.tmp");
    if serde_json::to_string(&v)
        .ok()
        .and_then(|s| std::fs::write(&tmp, s).ok())
        .is_some()
    {
        let _ = std::fs::rename(&tmp, &path);
    }
}

fn get_usage(token: &str) -> Option<Value> {
    ureq::get(USAGE_URL)
        .set("Authorization", &format!("Bearer {}", token))
        .set("anthropic-beta", BETA_HEADER)
        .set("User-Agent", "tokenbar")
        .call()
        .ok()?
        .into_json()
        .ok()
}

fn parse_iso(v: Option<&Value>) -> i64 {
    v.and_then(|x| x.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp())
        .unwrap_or(0)
}

fn window(id: &str, label: &str, node: &Value, window_secs: i64) -> Option<Limit> {
    let util = node.get("utilization")?.as_f64()?;
    Some(Limit {
        id: id.into(),
        provider: Provider::Anthropic,
        label: label.into(),
        util,
        resets_at: parse_iso(node.get("resets_at")),
        window_secs,
        status: LimitStatus::Normal,
        absolute: None,
        pace: None,
        runway_secs: None,
    })
}

fn parse_usage(v: &Value) -> Vec<Limit> {
    // Prefer the structured `limits` array (2026-07 API shape): it carries
    // model-scoped weekly windows (e.g. Fable) that the legacy fields never
    // will. Fall back to the legacy top-level fields for older responses.
    let mut out = parse_limits_array(v).unwrap_or_else(|| parse_legacy(v));
    // Extra-usage credit pool, if enabled.
    if let Some(eu) = v.get("extra_usage") {
        if eu.get("is_enabled").and_then(|x| x.as_bool()).unwrap_or(false) {
            let used = eu.get("used_credits").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let cap = eu.get("monthly_limit").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let util = if cap > 0.0 { used / cap * 100.0 } else { 0.0 };
            out.push(Limit {
                id: "cc.extra".into(),
                provider: Provider::Anthropic,
                label: "Claude·額度".into(),
                util,
                resets_at: 0,
                window_secs: 30 * 86400,
                status: LimitStatus::Normal,
                absolute: Some((used as u64, cap as u64)),
                pace: None,
                runway_secs: None,
            });
        }
    }
    out
}

/// Parse the structured `limits` array. Each entry:
/// `{ "kind": "session"|"weekly_all"|"weekly_scoped", "group": "session"|"weekly",
///    "percent": 25, "resets_at": "<iso>", "scope": { "model": { "display_name": "Fable" } }? }`
/// Returns None when the array is missing/empty so the caller can fall back.
fn parse_limits_array(v: &Value) -> Option<Vec<Limit>> {
    let arr = v.get("limits")?.as_array()?;
    let mut out = Vec::new();
    for e in arr {
        let Some(util) = e.get("percent").and_then(|x| x.as_f64()) else {
            continue;
        };
        let kind = e.get("kind").and_then(|x| x.as_str()).unwrap_or("");
        let group = e.get("group").and_then(|x| x.as_str()).unwrap_or("");
        let scope_name = e
            .pointer("/scope/model/display_name")
            .and_then(|x| x.as_str());

        let (id, label) = match (kind, scope_name) {
            ("session", _) => ("cc.5h".to_string(), "Claude·5h".to_string()),
            ("weekly_all", _) => ("cc.week".to_string(), "Claude·週".to_string()),
            // Model-scoped weekly windows appear/disappear per plan; keep the
            // historical id for Opus, derive ids for anything else (Fable, …).
            ("weekly_scoped", Some(name)) if name.eq_ignore_ascii_case("opus") => {
                ("cc.opus".to_string(), "Claude·Opus".to_string())
            }
            ("weekly_scoped", Some(name)) => {
                (format!("cc.w.{}", slug(name)), format!("Claude·{}", name))
            }
            _ => {
                if kind.is_empty() {
                    continue;
                }
                (format!("cc.{}", slug(kind)), format!("Claude·{}", kind))
            }
        };
        let window_secs = match group {
            "session" => 5 * 3600,
            "weekly" => 7 * 86400,
            _ => 0,
        };
        out.push(Limit {
            id,
            provider: Provider::Anthropic,
            label,
            util,
            resets_at: parse_iso(e.get("resets_at")),
            window_secs,
            status: LimitStatus::Normal,
            absolute: None,
            pace: None,
            runway_secs: None,
        });
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Legacy top-level fields (pre-2026-07 responses).
fn parse_legacy(v: &Value) -> Vec<Limit> {
    let mut out = Vec::new();
    if let Some(l) = v.get("five_hour").and_then(|n| window("cc.5h", "Claude·5h", n, 5 * 3600)) {
        out.push(l);
    }
    if let Some(l) = v.get("seven_day").and_then(|n| window("cc.week", "Claude·週", n, 7 * 86400)) {
        out.push(l);
    }
    if let Some(l) = v
        .get("seven_day_opus")
        .filter(|n| !n.is_null())
        .and_then(|n| window("cc.opus", "Claude·Opus", n, 7 * 86400))
    {
        out.push(l);
    }
    out
}

/// Lowercase alphanumeric slug for stable limit ids ("Fable" -> "fable").
fn slug(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shape observed live 2026-07-10 (docs/data-sources-findings.md).
    const MODERN: &str = r#"{
        "five_hour": { "utilization": 25.0, "resets_at": "2026-07-10T06:19:59+00:00" },
        "seven_day": { "utilization": 3.0, "resets_at": "2026-07-14T02:59:59+00:00" },
        "seven_day_opus": null,
        "limits": [
            { "kind": "session", "group": "session", "percent": 25, "severity": "normal",
              "resets_at": "2026-07-10T06:19:59+00:00", "scope": null, "is_active": true },
            { "kind": "weekly_all", "group": "weekly", "percent": 3, "severity": "normal",
              "resets_at": "2026-07-14T02:59:59+00:00", "scope": null, "is_active": false },
            { "kind": "weekly_scoped", "group": "weekly", "percent": 5, "severity": "normal",
              "resets_at": "2026-07-14T02:59:59+00:00",
              "scope": { "model": { "id": null, "display_name": "Fable" }, "surface": null },
              "is_active": false }
        ],
        "extra_usage": { "is_enabled": false, "monthly_limit": 2000, "used_credits": 0.0 }
    }"#;

    #[test]
    fn modern_shape_prefers_limits_array_and_finds_fable() {
        let v: Value = serde_json::from_str(MODERN).unwrap();
        let ls = parse_usage(&v);
        let get = |id: &str| ls.iter().find(|l| l.id == id).expect(id);
        assert_eq!(get("cc.5h").util, 25.0);
        assert_eq!(get("cc.week").util, 3.0);
        let fable = get("cc.w.fable");
        assert_eq!(fable.util, 5.0);
        assert_eq!(fable.label, "Claude·Fable");
        assert_eq!(fable.window_secs, 7 * 86400);
        assert!(fable.resets_at > 0);
        // extra_usage disabled → no cc.extra row
        assert!(ls.iter().all(|l| l.id != "cc.extra"));
    }

    #[test]
    fn scoped_opus_keeps_historical_id() {
        let v: Value = serde_json::from_str(
            r#"{ "limits": [ { "kind": "weekly_scoped", "group": "weekly", "percent": 18,
                "resets_at": "2026-07-14T02:59:59+00:00",
                "scope": { "model": { "display_name": "Opus" } } } ] }"#,
        )
        .unwrap();
        let ls = parse_usage(&v);
        assert_eq!(ls.len(), 1);
        assert_eq!(ls[0].id, "cc.opus");
        assert_eq!(ls[0].util, 18.0);
    }

    #[test]
    fn legacy_shape_still_parses() {
        let v: Value = serde_json::from_str(
            r#"{ "five_hour": { "utilization": 30.0, "resets_at": "2026-07-10T06:19:59+00:00" },
                 "seven_day": { "utilization": 41.0, "resets_at": "2026-07-14T02:59:59+00:00" },
                 "seven_day_opus": { "utilization": 18.0, "resets_at": "2026-07-14T02:59:59+00:00" } }"#,
        )
        .unwrap();
        let ls = parse_usage(&v);
        assert_eq!(ls.len(), 3);
        assert_eq!(ls[0].id, "cc.5h");
        assert_eq!(ls[2].id, "cc.opus");
    }

    #[test]
    fn unknown_kinds_are_skipped_not_fatal() {
        let v: Value = serde_json::from_str(
            r#"{ "limits": [
                { "kind": "session", "group": "session", "percent": 10, "resets_at": "2026-07-10T06:19:59+00:00" },
                { "kind": "weekly_scoped", "group": "weekly", "resets_at": "2026-07-14T02:59:59+00:00" },
                { "group": "weekly", "percent": 7 }
            ] }"#,
        )
        .unwrap();
        let ls = parse_usage(&v);
        // entry without percent skipped; entry without kind skipped
        assert_eq!(ls.len(), 1);
        assert_eq!(ls[0].id, "cc.5h");
    }
}

/// Placeholder limits when the source is unavailable (§7 SourceFailed).
fn degraded_limits() -> Vec<Limit> {
    ["cc.5h", "cc.week"]
        .iter()
        .zip(["Claude·5h", "Claude·週"])
        .map(|(id, label)| Limit {
            id: (*id).into(),
            provider: Provider::Anthropic,
            label: label.into(),
            util: 0.0,
            resets_at: 0,
            window_secs: 5 * 3600,
            status: LimitStatus::SourceFailed,
            absolute: None,
            pace: None,
            runway_secs: None,
        })
        .collect()
}
