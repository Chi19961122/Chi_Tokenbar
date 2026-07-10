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
    /// Island layout: "both" (providers side-by-side), "claude", "codex".
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
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
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
}
