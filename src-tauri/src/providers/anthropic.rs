//! Anthropic provider — the fragile path (UX Spec v3 §9, Ai_Assistant/data-sources-findings.md).
//!
//! Reads the OAuth token from `~/.claude/.credentials.json` and calls the
//! undocumented `GET /api/oauth/usage`. Everything is guarded: any failure
//! yields `source_failed` limits so the UI degrades instead of going blank (§7).
//!
//! SAFETY: refreshing the token can rotate the refresh token that Claude Code
//! itself relies on, which could log the user out. So the refresh flow is
//! opt-in (`allow_refresh`, default false). The read-only usage GET never
//! rotates anything and is always safe to attempt.

use crate::model::{Limit, LimitAction, LimitStatus, Provider};
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

/// Why a Claude fetch failed. Has **two readers**, and therefore two outputs:
/// `label()` for developers (precise, via `TOKENBAR_DEBUG` stderr) and
/// `user_hint()` for the user (plain language, shown in the panel).
///
/// SECRET (CLAUDE.md): a stage may carry only the *kind* of error plus an HTTP
/// status code. Never an access/refresh token, email, account id or response
/// body — not even partially masked. Keep it that way when adding variants.
#[derive(Debug, Clone, PartialEq)]
enum FailureStage {
    CredentialsFile,
    CredentialsShape,
    RefreshDisabled,
    RefreshHttp(u16),
    RefreshTransport(&'static str),
    RefreshJson,
    UsageHttp(u16),
    UsageTransport(&'static str),
    UsageJson,
    /// Connected fine and got valid JSON, but nothing parsed out — the API
    /// changed shape. Without this the UI would show "no limits", which looks
    /// like a healthy idle state and hides the breakage.
    UsageShape,
}

impl FailureStage {
    /// Precise technical stage for `TOKENBAR_DEBUG` stderr. Never shown in the UI.
    fn label(&self) -> String {
        match self {
            FailureStage::CredentialsFile => "credentials_file".into(),
            FailureStage::CredentialsShape => "credentials_shape".into(),
            FailureStage::RefreshDisabled => "refresh_disabled".into(),
            FailureStage::RefreshHttp(c) => format!("refresh_http_{}", c),
            FailureStage::RefreshTransport(k) => format!("refresh_transport_{}", k),
            FailureStage::RefreshJson => "refresh_json".into(),
            FailureStage::UsageHttp(c) => format!("usage_http_{}", c),
            FailureStage::UsageTransport(k) => format!("usage_transport_{}", k),
            FailureStage::UsageJson => "usage_json".into(),
            FailureStage::UsageShape => "usage_shape".into(),
        }
    }

    /// Plain-language reason for the user (§7 panel row).
    ///
    /// Rules: no TLS / HTTP / OAuth / token / certificate / transport / proxy
    /// jargon. Every line must either tell the user what to do next, or say
    /// clearly that it retries by itself and needs no action.
    fn user_hint(&self) -> &'static str {
        match self {
            FailureStage::CredentialsFile => "Can't find your Claude login. Sign in to Claude Code first.",
            FailureStage::CredentialsShape => "Can't read your Claude login. Sign in to Claude Code again.",
            FailureStage::RefreshDisabled => "Your Claude login has expired. Sign in again (or enable auto-renew in settings).",
            FailureStage::RefreshHttp(401) | FailureStage::RefreshHttp(403) => {
                "Your Claude login is no longer valid. Sign in to Claude Code again."
            }
            FailureStage::RefreshHttp(_) => "Couldn't refresh Claude access. It will retry automatically.",
            FailureStage::RefreshTransport(_) => "Can't reach Claude. Check your network connection.",
            FailureStage::UsageHttp(401) | FailureStage::UsageHttp(403) => {
                "Claude won't accept this account. Sign in to Claude Code again."
            }
            FailureStage::UsageHttp(429) => "Too many requests. It will retry automatically.",
            FailureStage::UsageHttp(c) if *c >= 500 => "Claude is having temporary issues. It will retry automatically.",
            FailureStage::UsageHttp(_) => "Claude couldn't answer this query. It will retry automatically.",
            // The case the other machine actually hit: certificate interception
            // by AV/corporate network. Pointing at /login here would waste the
            // user's time, so this one names the real suspects instead.
            FailureStage::UsageTransport(_) => "Can't reach Claude. Check your network; a corporate network or antivirus may be blocking the connection.",
            FailureStage::RefreshJson | FailureStage::UsageJson | FailureStage::UsageShape => {
                "Claude's response wasn't recognized; TokenBar may need an update."
            }
        }
    }

    /// Whether re-running the official login flow would actually fix this.
    ///
    /// **Login-class failures only.** Offering "sign in again" for a network
    /// or AV/proxy block sends the user down a dead end — they press it,
    /// nothing improves, and they conclude their account is broken while the
    /// real cause (`user_hint` already names it) goes unread. Transient
    /// server-side states retry on their own, and a schema change needs a new
    /// TokenBar, not a new session; neither gets a button either.
    ///
    /// Invariant, enforced by `relogin_action_matches_what_the_hint_tells_the_user`:
    /// a stage offers `Relogin` exactly when its `user_hint()` tells the user
    /// to log in. Change one and the test makes you change the other.
    fn action(&self) -> Option<LimitAction> {
        match self {
            FailureStage::CredentialsFile
            | FailureStage::CredentialsShape
            | FailureStage::RefreshDisabled
            | FailureStage::RefreshHttp(401 | 403)
            | FailureStage::UsageHttp(401 | 403) => Some(LimitAction::Relogin),
            _ => None,
        }
    }
}

/// Map a ureq transport error to a short, stable label — **debug output only**
/// (`label()`); the user hint deliberately doesn't split this finely.
/// Variants verified against ureq 2.12.1 `src/error.rs`. The catch-all keeps
/// a future ureq upgrade from breaking the build.
fn transport_kind_label(kind: ureq::ErrorKind) -> &'static str {
    use ureq::ErrorKind;
    match kind {
        ErrorKind::Dns => "dns",
        ErrorKind::ConnectionFailed => "connection_failed",
        ErrorKind::ProxyConnect => "proxy_connect",
        ErrorKind::ProxyUnauthorized => "proxy_unauthorized",
        ErrorKind::InvalidProxyUrl => "invalid_proxy_url",
        ErrorKind::BadHeader => "bad_header",
        ErrorKind::BadStatus => "bad_status",
        ErrorKind::Io => "io",
        ErrorKind::InvalidUrl => "invalid_url",
        ErrorKind::UnknownScheme => "unknown_scheme",
        ErrorKind::TooManyRedirects => "too_many_redirects",
        ErrorKind::InsecureRequestHttpsOnly => "insecure_request",
        _ => "other",
    }
}

/// Classify a ureq error. **Only the status code is read from `Error::Status`;
/// the response body is never touched** (it can contain account data).
fn classify(err: ureq::Error, http: fn(u16) -> FailureStage, transport: fn(&'static str) -> FailureStage) -> FailureStage {
    match err {
        ureq::Error::Status(code, _) => http(code),
        ureq::Error::Transport(t) => transport(transport_kind_label(t.kind())),
    }
}

pub struct AnthropicProvider {
    last_fetch: i64,
    cached: Vec<Limit>,
}

impl AnthropicProvider {
    pub fn new() -> Self {
        Self {
            last_fetch: 0,
            cached: Vec::new(),
        }
    }

    /// Return limits, hitting the network at most every REFRESH_SECS
    /// (FORCE_MIN_SECS when the user asked for a manual refresh).
    /// `allow_refresh` is read from live settings each round so the toggle
    /// takes effect without restarting the app.
    pub fn poll(&mut self, now: i64, force: bool, allow_refresh: bool) -> Vec<Limit> {
        let min_gap = if force { FORCE_MIN_SECS } else { REFRESH_SECS };
        if now - self.last_fetch < min_gap && !self.cached.is_empty() {
            return self.cached.clone();
        }
        self.last_fetch = now;
        self.cached = self.fetch(allow_refresh);
        self.cached.clone()
    }

    /// Never fails: a failure becomes degraded limits carrying a plain-language
    /// hint, so the UI always has something honest to show (§7). The precise
    /// stage goes to stderr only, and only under `TOKENBAR_DEBUG`.
    ///
    /// Single degradation path on purpose — if `poll` also produced degraded
    /// limits we'd get hint-less `SourceFailed` rows that the UI can't explain.
    fn fetch(&self, allow_refresh: bool) -> Vec<Limit> {
        match self.fetch_inner(allow_refresh) {
            Ok(limits) => limits,
            Err(stage) => {
                if std::env::var("TOKENBAR_DEBUG").is_ok() {
                    eprintln!("[tb] anthropic fetch failed: {}", stage.label());
                }
                degraded_limits(&stage)
            }
        }
    }

    fn fetch_inner(&self, allow_refresh: bool) -> Result<Vec<Limit>, FailureStage> {
        let creds = read_creds()?;
        let now_ms = chrono::Utc::now().timestamp_millis();

        let token = if creds.expires_ms > now_ms + 60_000 {
            creds.access
        } else if allow_refresh {
            refresh_token(&creds.refresh)?
        } else {
            // Expired and refresh disabled → degrade honestly (no rotation risk).
            return Err(FailureStage::RefreshDisabled);
        };

        usage_to_limits(&get_usage(&token)?)
    }
}

/// Turn a usage response into limits, treating "parsed to nothing" as a
/// failure rather than as "no limits" (see `FailureStage::UsageShape`).
fn usage_to_limits(v: &Value) -> Result<Vec<Limit>, FailureStage> {
    let limits = parse_usage(v);
    if limits.is_empty() {
        Err(FailureStage::UsageShape)
    } else {
        Ok(limits)
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

/// Read the credentials file. Only the file access lives here so that the
/// parsing (the part worth testing) stays pure — see `parse_creds`.
fn read_creds() -> Result<Creds, FailureStage> {
    let path = creds_path().ok_or(FailureStage::CredentialsFile)?;
    let raw = std::fs::read_to_string(path).map_err(|_| FailureStage::CredentialsFile)?;
    parse_creds(&raw)
}

/// Pure credentials parse: valid JSON → has `claudeAiOauth` → non-empty
/// `accessToken` → non-empty `refreshToken` → `expiresAt`.
///
/// SECRET: on failure return only the stage. Never echo `raw` or any field of
/// it into an error, a log line, or a panic message.
fn parse_creds(raw: &str) -> Result<Creds, FailureStage> {
    let v: Value = serde_json::from_str(raw).map_err(|_| FailureStage::CredentialsShape)?;
    let o = v.get("claudeAiOauth").ok_or(FailureStage::CredentialsShape)?;
    let field = |k: &str| -> Result<String, FailureStage> {
        match o.get(k).and_then(|x| x.as_str()) {
            Some(s) if !s.is_empty() => Ok(s.to_string()),
            _ => Err(FailureStage::CredentialsShape),
        }
    };
    Ok(Creds {
        access: field("accessToken")?,
        refresh: field("refreshToken")?,
        expires_ms: o.get("expiresAt").and_then(|x| x.as_i64()).unwrap_or(0),
    })
}

/// Exchange the refresh token. On success, writes the (possibly rotated) tokens
/// back atomically so Claude Code stays in sync. Best-effort/undocumented.
fn refresh_token(refresh: &str) -> Result<String, FailureStage> {
    let resp: Value = ureq::post(TOKEN_URL)
        .set("Content-Type", "application/json")
        .send_json(serde_json::json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh,
            "client_id": CLIENT_ID,
        }))
        .map_err(|e| classify(e, FailureStage::RefreshHttp, FailureStage::RefreshTransport))?
        .into_json()
        .map_err(|_| FailureStage::RefreshJson)?;

    let access = resp
        .get("access_token")
        .and_then(|x| x.as_str())
        .ok_or(FailureStage::RefreshJson)?
        .to_string();
    let new_refresh = resp
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .unwrap_or(refresh)
        .to_string();
    let expires_in = resp.get("expires_in").and_then(|v| v.as_i64()).unwrap_or(3600);
    let expires_ms = chrono::Utc::now().timestamp_millis() + expires_in * 1000;

    write_back_creds(&access, &new_refresh, expires_ms);
    Ok(access)
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

/// Read-only usage GET. `.call()` returns `Err(Error::Status(code, _))` for
/// 4xx/5xx — we take **only** `code`; the body may carry account data and is
/// never read. (The old `.ok()?` collapsed 403/429/network into one `None`,
/// which is exactly why failures were indistinguishable.)
fn get_usage(token: &str) -> Result<Value, FailureStage> {
    ureq::get(USAGE_URL)
        .set("Authorization", &format!("Bearer {}", token))
        .set("anthropic-beta", BETA_HEADER)
        .set("User-Agent", "tokenbar")
        .call()
        .map_err(|e| classify(e, FailureStage::UsageHttp, FailureStage::UsageTransport))?
        .into_json()
        .map_err(|_| FailureStage::UsageJson)
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
        hint: None,
        action: None,
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
                label: "Claude·Credits".into(),
                util,
                resets_at: 0,
                window_secs: 30 * 86400,
                status: LimitStatus::Normal,
                absolute: Some((used as u64, cap as u64)),
                pace: None,
                runway_secs: None,
                hint: None,
                action: None,
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
            ("weekly_all", _) => ("cc.week".to_string(), "Claude·Weekly".to_string()),
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
            hint: None,
            action: None,
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
    if let Some(l) = v.get("seven_day").and_then(|n| window("cc.week", "Claude·Weekly", n, 7 * 86400)) {
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

    /// Shape observed live 2026-07-10 (Ai_Assistant/data-sources-findings.md).
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

    /// `.err()` rather than `.unwrap_err()` on purpose: `unwrap_err` would
    /// require `Creds: Debug`, and a Debug impl on a struct holding tokens is
    /// exactly the leak CLAUDE.md forbids. Keep `Creds` unprintable.
    fn creds_err(raw: &str) -> Option<FailureStage> {
        parse_creds(raw).err()
    }

    #[test]
    fn malformed_json_is_shape_failure() {
        assert_eq!(creds_err("not json"), Some(FailureStage::CredentialsShape));
    }

    #[test]
    fn missing_oauth_block_is_shape_failure() {
        assert_eq!(
            creds_err(r#"{ "other": 1 }"#),
            Some(FailureStage::CredentialsShape)
        );
    }

    #[test]
    fn empty_access_token_is_shape_failure() {
        let raw = r#"{ "claudeAiOauth": { "accessToken": "", "refreshToken": "r", "expiresAt": 1 } }"#;
        assert_eq!(creds_err(raw), Some(FailureStage::CredentialsShape));
    }

    #[test]
    fn empty_refresh_token_is_shape_failure() {
        let raw = r#"{ "claudeAiOauth": { "accessToken": "a", "refreshToken": "", "expiresAt": 1 } }"#;
        assert_eq!(creds_err(raw), Some(FailureStage::CredentialsShape));
    }

    #[test]
    fn well_formed_creds_parse() {
        let raw = r#"{ "claudeAiOauth": { "accessToken": "a", "refreshToken": "r", "expiresAt": 42 } }"#;
        let c = parse_creds(raw).expect("should parse");
        assert_eq!(c.access, "a");
        assert_eq!(c.refresh, "r");
        assert_eq!(c.expires_ms, 42);
    }

    #[test]
    fn debug_labels_include_status_code() {
        assert_eq!(FailureStage::UsageHttp(403).label(), "usage_http_403");
        assert_eq!(FailureStage::RefreshHttp(401).label(), "refresh_http_401");
        assert_eq!(
            FailureStage::UsageTransport("connection_failed").label(),
            "usage_transport_connection_failed"
        );
    }

    /// Transport labels must map real ureq kinds, and never panic on new ones.
    #[test]
    fn transport_labels_map_known_kinds() {
        use ureq::ErrorKind;
        assert_eq!(transport_kind_label(ErrorKind::Dns), "dns");
        assert_eq!(
            transport_kind_label(ErrorKind::ConnectionFailed),
            "connection_failed"
        );
        assert_eq!(
            transport_kind_label(ErrorKind::ProxyUnauthorized),
            "proxy_unauthorized"
        );
        // Not a transport kind we classify — must fall through, not panic.
        assert_eq!(transport_kind_label(ErrorKind::HTTP), "other");
    }

    /// 白話提示不得洩漏術語,也不得空白 (§7:降級不得空白)。
    #[test]
    fn user_hints_are_plain_language_and_never_empty() {
        let stages = [
            FailureStage::CredentialsFile,
            FailureStage::CredentialsShape,
            FailureStage::RefreshDisabled,
            FailureStage::RefreshHttp(401),
            FailureStage::RefreshHttp(500),
            FailureStage::RefreshTransport("dns"),
            FailureStage::RefreshJson,
            FailureStage::UsageHttp(403),
            FailureStage::UsageHttp(429),
            FailureStage::UsageHttp(503),
            FailureStage::UsageTransport("connection_failed"),
            FailureStage::UsageJson,
            FailureStage::UsageShape,
        ];
        for s in stages {
            let h = s.user_hint();
            assert!(!h.is_empty(), "{:?} 沒有提示文案", s);
            // "TokenBar" is the product name, not jargon — drop it before the
            // scan so the case-insensitive "token" check stays meaningful.
            let scan = h.replace("TokenBar", "").to_lowercase();
            for jargon in [
                "tls", "http", "oauth", "token", "transport", "json", "proxy", "certificate",
                "socket", "timeout", "server error",
            ] {
                assert!(!scan.contains(jargon), "{:?} 的提示含術語 {}", s, jargon);
            }
        }
    }

    /// 同一階段的兩種輸出必須真的不同:label 給開發者、hint 給使用者。
    #[test]
    fn label_and_hint_are_distinct_outputs() {
        let s = FailureStage::UsageHttp(429);
        assert_eq!(s.label(), "usage_http_429");
        assert!(s.user_hint().to_lowercase().contains("retry"));
    }

    /// 認證類與連線類的提示必須不同 —— 這正是這個 Task 的重點。
    #[test]
    fn auth_failure_and_network_failure_give_different_advice() {
        let auth = FailureStage::UsageHttp(403).user_hint();
        let net = FailureStage::UsageTransport("connection_failed").user_hint();
        assert_ne!(auth, net);
        assert!(auth.to_lowercase().contains("sign in"), "認證失敗應該叫使用者重新登入");
        assert!(net.to_lowercase().contains("network"), "連線失敗應該叫使用者查網路");
        assert!(!net.to_lowercase().contains("sign in"), "連線失敗不該誤導使用者去重新登入");
    }

    /// 只有「重新登入真的會修好」的階段才可以帶 relogin。
    ///
    /// 這是這顆按鈕的整個重點:連不上 Claude(防毒/公司網路)時給重新登入按鈕,
    /// 使用者按了沒用,還會以為是自己帳號有問題。
    #[test]
    fn only_login_failures_offer_relogin() {
        for s in [
            FailureStage::CredentialsFile,
            FailureStage::CredentialsShape,
            FailureStage::RefreshDisabled,
            FailureStage::RefreshHttp(401),
            FailureStage::RefreshHttp(403),
            FailureStage::UsageHttp(401),
            FailureStage::UsageHttp(403),
        ] {
            assert_eq!(s.action(), Some(LimitAction::Relogin), "{:?} 應可重新登入", s);
        }
        for s in [
            // 網路/防毒擋住 —— 重新登入完全幫不上忙
            FailureStage::UsageTransport("connection_failed"),
            FailureStage::RefreshTransport("dns"),
            // 會自動再試的暫時狀況
            FailureStage::UsageHttp(429),
            FailureStage::UsageHttp(503),
            FailureStage::RefreshHttp(500),
            // schema 變了 —— 要更新 TokenBar,不是重新登入
            FailureStage::RefreshJson,
            FailureStage::UsageJson,
            FailureStage::UsageShape,
        ] {
            assert_eq!(s.action(), None, "{:?} 不該給重新登入按鈕", s);
        }
    }

    /// 提示叫使用者去登入,就必須給得出按鈕;反之亦然 —— 兩者不可走針。
    ///
    /// 比對的是**祈使句**(「請重新登入」/「請先登入」),不是「登入」二字:
    /// `RefreshHttp(500)` 的「Claude 登入更新失敗,稍後會自動再試」有「登入」
    /// 卻是在說「不用動作」—— 它不該有按鈕。這個區別正是這條不變式的重點。
    #[test]
    fn relogin_action_matches_what_the_hint_tells_the_user() {
        for s in [
            FailureStage::CredentialsFile,
            FailureStage::CredentialsShape,
            FailureStage::RefreshDisabled,
            FailureStage::RefreshHttp(401),
            FailureStage::RefreshHttp(403),
            FailureStage::RefreshHttp(500),
            FailureStage::RefreshTransport("dns"),
            FailureStage::RefreshJson,
            FailureStage::UsageHttp(401),
            FailureStage::UsageHttp(403),
            FailureStage::UsageHttp(429),
            FailureStage::UsageHttp(503),
            FailureStage::UsageTransport("connection_failed"),
            FailureStage::UsageJson,
            FailureStage::UsageShape,
        ] {
            let h = s.user_hint();
            let asks_user_to_log_in = h.to_lowercase().contains("sign in");
            assert_eq!(
                s.action().is_some(),
                asks_user_to_log_in,
                "{:?}: 提示與按鈕不一致 ({:?})",
                s,
                s.user_hint()
            );
        }
    }

    /// 降級資料本身要帶 action —— UI 讀的是 limit,不是 FailureStage。
    #[test]
    fn degraded_limits_carry_the_action_for_login_failures() {
        let ls = degraded_limits(&FailureStage::UsageHttp(403));
        assert!(!ls.is_empty());
        assert!(ls.iter().all(|l| l.action == Some(LimitAction::Relogin)));
    }

    /// 連線失敗的降級資料**不得**帶 action。
    #[test]
    fn degraded_limits_omit_the_action_for_network_failures() {
        let ls = degraded_limits(&FailureStage::UsageTransport("connection_failed"));
        assert!(!ls.is_empty());
        assert!(ls.iter().all(|l| l.action.is_none()));
    }

    /// 正常路徑的 limit 不該帶 action(否則按鈕會出現在健康的列上)。
    #[test]
    fn healthy_limits_have_no_action() {
        let v: Value = serde_json::from_str(MODERN).unwrap();
        assert!(parse_usage(&v).iter().all(|l| l.action.is_none()));
    }

    /// 降級的 limit 必須帶著提示,否則 UI 沒東西可顯示。
    #[test]
    fn degraded_limits_carry_the_hint() {
        let stage = FailureStage::UsageTransport("connection_failed");
        let ls = degraded_limits(&stage);
        assert_eq!(ls.len(), 2);
        assert!(ls.iter().all(|l| l.status == LimitStatus::SourceFailed));
        assert!(ls
            .iter()
            .all(|l| l.hint.as_deref() == Some(stage.user_hint())));
    }

    /// 不同原因要產生不同的降級文案(否則等於沒做)。
    #[test]
    fn degraded_limits_reflect_the_actual_stage() {
        let a = degraded_limits(&FailureStage::CredentialsFile);
        let b = degraded_limits(&FailureStage::UsageTransport("dns"));
        assert_ne!(a[0].hint, b[0].hint);
    }

    /// 正常路徑的 limit 不該帶提示。
    #[test]
    fn healthy_limits_have_no_hint() {
        let v: Value = serde_json::from_str(MODERN).unwrap();
        assert!(parse_usage(&v).iter().all(|l| l.hint.is_none()));
    }

    /// 連線成功但解析出空陣列 = 官方改了 schema,必須當成失效而非「沒有額度」。
    #[test]
    fn empty_parse_is_usage_shape_failure() {
        let v: Value = serde_json::from_str(r#"{ "limits": [] }"#).unwrap();
        assert!(parse_usage(&v).is_empty());
        assert_eq!(
            usage_to_limits(&v).unwrap_err(),
            FailureStage::UsageShape
        );
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
///
/// NOTE: `util: 0.0` is a placeholder, **not** an estimate — there is no local
/// estimation anywhere in this codebase. The UI must therefore never label
/// these rows "估算"; it shows `hint` instead. See the panel's source_failed
/// branch.
fn degraded_limits(stage: &FailureStage) -> Vec<Limit> {
    ["cc.5h", "cc.week"]
        .iter()
        .zip(["Claude·5h", "Claude·Weekly"])
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
            hint: Some(stage.user_hint().to_string()),
            action: stage.action(),
        })
        .collect()
}
