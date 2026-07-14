//! Persisted settings (UX Spec v3 §10). Stored as JSON in the OS config dir.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// Opt-in: allow refreshing the Claude OAuth token. Off by default because
    /// a rotated refresh token could disrupt Claude Code's own session.
    pub allow_token_refresh: bool,
    /// Launch TokenBar at login.
    pub autostart: bool,
    /// Notify thresholds on util%.
    pub warn_pct: f64,
    pub crit_pct: f64,
    /// Expanded panel starts in compact mode (limits only, no analytics).
    pub compact: bool,
    /// Global display filter: "both" (show every provider), "claude", "codex".
    /// Applied once in the scheduler, so it drives the island, panel, tray,
    /// notifications, ranking and analytics alike. Any unknown value degrades
    /// to "show everything" — see `lib::apply_provider_filter`.
    pub providers: String,
    /// DEPRECATED, read-only: the pre-`providers` island-only layout setting.
    /// Kept so an existing settings.json can be migrated in `load_from_str`;
    /// never written back (skip_serializing) and never read at runtime.
    #[serde(skip_serializing)]
    pub island_mode: String,
    /// Codex quota source: "local" (rollout snapshot), "live" (account API),
    /// or "auto" (live first, then local fallback).
    pub codex_usage_source: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            allow_token_refresh: false,
            autostart: false,
            warn_pct: 75.0,
            crit_pct: 90.0,
            compact: false,
            providers: "both".into(),
            island_mode: "both".into(),
            codex_usage_source: "local".into(),
        }
    }
}

fn path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("TokenBar").join("settings.json"))
}

pub fn load() -> Settings {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| load_from_str(&s))
        .unwrap_or_default()
}

/// Parse settings JSON, migrating legacy fields. Split out of `load()` so the
/// migration is testable without touching the real config dir.
///
/// Note `#[serde(default)]` is a *container* attribute: it only fills in
/// **missing** fields, it never inspects a value. So a legacy `island_mode`
/// has to be moved across here explicitly, and any surviving junk value is
/// dealt with downstream by `lib::apply_provider_filter`'s catch-all.
pub fn load_from_str(raw: &str) -> Settings {
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(raw) else {
        return Settings::default();
    };
    // `island_mode` used to scope the island only; it is now the global
    // `providers` filter. Carry an existing preference over rather than
    // silently dropping the user back to the default. An explicit
    // `providers` always wins.
    if let Some(obj) = v.as_object_mut() {
        if !obj.contains_key("providers") {
            if let Some(legacy) = obj.get("island_mode").and_then(|m| m.as_str()) {
                let legacy = serde_json::Value::String(legacy.to_string());
                obj.insert("providers".into(), legacy);
            }
        }
    }
    serde_json::from_value(v).unwrap_or_default()
}

pub fn save(s: &Settings) {
    if let Some(p) = path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(s) {
            let _ = std::fs::write(p, json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_local_codex_usage() {
        assert_eq!(Settings::default().codex_usage_source, "local");
    }

    #[test]
    fn missing_source_deserializes_to_local() {
        let s: Settings = serde_json::from_str(r#"{ "autostart": true }"#).unwrap();
        assert_eq!(s.codex_usage_source, "local");
    }

    #[test]
    fn migrates_island_mode_to_providers() {
        let s = load_from_str(r#"{ "island_mode": "claude" }"#);
        assert_eq!(s.providers, "claude");
    }

    #[test]
    fn explicit_providers_wins_over_legacy_island_mode() {
        let s = load_from_str(r#"{ "island_mode": "codex", "providers": "claude" }"#);
        assert_eq!(s.providers, "claude");
    }

    #[test]
    fn missing_both_defaults_to_all() {
        assert_eq!(load_from_str("{}").providers, "both");
    }

    /// Garbage on disk must never lose the user's other settings silently —
    /// and must never yield a filter that hides everything.
    #[test]
    fn unparseable_settings_fall_back_to_defaults() {
        assert_eq!(load_from_str("not json").providers, "both");
    }
}
