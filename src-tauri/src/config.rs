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
    /// Keep the island above other windows. Defaults to `true` to match the
    /// `alwaysOnTop` in tauri.conf.json — the window is *created* pinned, so a
    /// `false` here has to be applied over it at startup (`lib::run`), not just
    /// when the user flips it.
    ///
    /// Turning it off changes what "visible" means for the island (it can be
    /// buried), which `lib::toggle_action` accounts for.
    pub always_on_top: bool,
    /// UI language: "system" (follow the OS locale — resolved in the frontend
    /// via `navigator.language`), "en", or "zh-TW". Defaults to "system".
    ///
    /// The backend only consults this for notification copy, and there it takes
    /// a deliberately narrow rule: only an explicit "zh-TW" yields Chinese, and
    /// everything else (including "system") stays English — Rust has no reliable
    /// cross-platform OS-locale read here, so "system" can't be resolved backend
    /// side. See `lib::fire_notifications`.
    #[serde(default = "default_locale")]
    pub locale: String,
    /// Which tab a press on the island opens: "compact" (Limits list) or
    /// "usage" (the Usage analytics tab). Defaults to "compact". Distinct from
    /// `compact` (the panel density switch) — this only picks the entry tab.
    #[serde(default = "default_expand")]
    pub expand_default: String,
    /// Island quota pin per provider: "auto" (worst-ranked, current behaviour),
    /// "5h", "week", or "model:<limit-id>". A pin with no matching data shows
    /// "—" rather than silently falling back to auto. Defaults to "auto".
    #[serde(default = "default_pin")]
    pub island_pin_claude: String,
    #[serde(default = "default_pin")]
    pub island_pin_codex: String,
    /// Island right-side aux readout: "off", "tok_per_min" (today's burn rate,
    /// the current behaviour), or "cost_today" (today's est. cost). Defaults to
    /// "tok_per_min".
    #[serde(default = "default_island_aux")]
    pub island_aux: String,
    /// How reset times render: "relative" (a countdown to reset) or "clock"
    /// (the absolute wall-clock time). Defaults to "relative".
    #[serde(default = "default_reset_display")]
    pub reset_display: String,
    /// 階段 D 戰報 Share: which share-card style the report panel last used —
    /// "statement" | "diagnostics" | "minimal" | "fuel" | "island_card" | "wa".
    /// Defaults to "statement".
    #[serde(default = "default_share_style")]
    pub share_style: String,
    /// 階段 D 戰報 Share: which range the report panel last summarized —
    /// "today" | "week" | "month". Defaults to "week".
    #[serde(default = "default_share_range")]
    pub share_range: String,
    /// 階段 E 多工具:whether OpenCode local usage is scanned into analytics.
    /// Defaults to `true` (detect-and-show); off means it is never scanned and
    /// never appears in byAgent/legend/accounts. Independent of `providers`
    /// (which only narrows the anthropic/codex quota pools).
    #[serde(default = "default_true")]
    pub tool_opencode: bool,
    /// 階段 E 多工具:whether Gemini CLI local usage is scanned. Defaults to
    /// `true` (detect-and-show). See `tool_opencode`.
    #[serde(default = "default_true")]
    pub tool_gemini: bool,
}

fn default_true() -> bool {
    true
}

fn default_locale() -> String {
    "system".into()
}

fn default_expand() -> String {
    "compact".into()
}

fn default_pin() -> String {
    "auto".into()
}

fn default_island_aux() -> String {
    "tok_per_min".into()
}

fn default_reset_display() -> String {
    "relative".into()
}

fn default_share_style() -> String {
    "statement".into()
}

fn default_share_range() -> String {
    "week".into()
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
            always_on_top: true,
            locale: default_locale(),
            expand_default: default_expand(),
            island_pin_claude: default_pin(),
            island_pin_codex: default_pin(),
            island_aux: default_island_aux(),
            reset_display: default_reset_display(),
            share_style: default_share_style(),
            share_range: default_share_range(),
            tool_opencode: true,
            tool_gemini: true,
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

    // ── always_on_top ────────────────────────────────────────────────
    //
    // The default must stay `true`: tauri.conf.json creates the window with
    // alwaysOnTop, so anything else here would silently change the behaviour
    // every existing user already has.

    #[test]
    fn defaults_to_always_on_top() {
        assert!(Settings::default().always_on_top);
    }

    /// Every settings.json written before this setting existed lacks the key.
    /// Those users must keep the pinned window they have today.
    #[test]
    fn settings_saved_before_this_setting_existed_stay_pinned() {
        assert!(load_from_str(r#"{ "autostart": true }"#).always_on_top);
    }

    // ── locale ───────────────────────────────────────────────────────
    //
    // Default must be "system" so an existing settings.json (written before the
    // field existed) follows the OS language rather than being pinned to a
    // specific one.

    #[test]
    fn defaults_to_system_locale() {
        assert_eq!(Settings::default().locale, "system");
    }

    /// Every settings.json written before this setting existed lacks the key.
    #[test]
    fn missing_locale_deserializes_to_system() {
        let s = load_from_str(r#"{ "autostart": true }"#);
        assert_eq!(s.locale, "system");
    }

    /// An explicit locale choice must survive a round-trip through disk.
    #[test]
    fn explicit_locale_survives_a_save_load_round_trip() {
        let s = Settings {
            locale: "zh-TW".into(),
            ..Settings::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(load_from_str(&json).locale, "zh-TW");
    }

    // ── 階段 B fields (expand_default / island pins / aux / reset_display) ─
    //
    // Every settings.json written before 階段 B lacks these keys; `#[serde(default
    // = ...)]` must fill each one so an old file loads without error and keeps
    // today's behaviour.

    #[test]
    fn defaults_for_stage_b_fields() {
        let d = Settings::default();
        assert_eq!(d.expand_default, "compact");
        assert_eq!(d.island_pin_claude, "auto");
        assert_eq!(d.island_pin_codex, "auto");
        assert_eq!(d.island_aux, "tok_per_min");
        assert_eq!(d.reset_display, "relative");
    }

    #[test]
    fn missing_stage_b_fields_deserialize_to_defaults() {
        let s = load_from_str(r#"{ "autostart": true }"#);
        assert_eq!(s.expand_default, "compact");
        assert_eq!(s.island_pin_claude, "auto");
        assert_eq!(s.island_pin_codex, "auto");
        assert_eq!(s.island_aux, "tok_per_min");
        assert_eq!(s.reset_display, "relative");
    }

    #[test]
    fn explicit_stage_b_fields_survive_a_save_load_round_trip() {
        let s = Settings {
            expand_default: "usage".into(),
            island_pin_claude: "5h".into(),
            island_pin_codex: "model:codex.credits".into(),
            island_aux: "cost_today".into(),
            reset_display: "clock".into(),
            ..Settings::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        let back = load_from_str(&json);
        assert_eq!(back.expand_default, "usage");
        assert_eq!(back.island_pin_claude, "5h");
        assert_eq!(back.island_pin_codex, "model:codex.credits");
        assert_eq!(back.island_aux, "cost_today");
        assert_eq!(back.reset_display, "clock");
    }

    // ── 階段 D fields (share_style / share_range) ──────────────────────
    //
    // Every settings.json written before 階段 D lacks these keys; `#[serde(default
    // = ...)]` must fill each so an old file loads and keeps today's behaviour.

    #[test]
    fn defaults_for_stage_d_fields() {
        let d = Settings::default();
        assert_eq!(d.share_style, "statement");
        assert_eq!(d.share_range, "week");
    }

    #[test]
    fn missing_stage_d_fields_deserialize_to_defaults() {
        let s = load_from_str(r#"{ "autostart": true }"#);
        assert_eq!(s.share_style, "statement");
        assert_eq!(s.share_range, "week");
    }

    #[test]
    fn explicit_stage_d_fields_survive_a_save_load_round_trip() {
        let s = Settings {
            share_style: "fuel".into(),
            share_range: "month".into(),
            ..Settings::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        let back = load_from_str(&json);
        assert_eq!(back.share_style, "fuel");
        assert_eq!(back.share_range, "month");
    }

    // ── 階段 E fields (tool_opencode / tool_gemini) ────────────────────
    //
    // Every settings.json written before 階段 E lacks these keys; `#[serde(default
    // = "default_true")]` must fill each with `true` so an existing install
    // keeps detect-and-show behaviour rather than silently hiding a tool.

    #[test]
    fn defaults_for_stage_e_fields() {
        let d = Settings::default();
        assert!(d.tool_opencode);
        assert!(d.tool_gemini);
    }

    #[test]
    fn missing_stage_e_fields_deserialize_to_true() {
        let s = load_from_str(r#"{ "autostart": true }"#);
        assert!(s.tool_opencode);
        assert!(s.tool_gemini);
    }

    #[test]
    fn explicit_stage_e_fields_survive_a_save_load_round_trip() {
        let s = Settings {
            tool_opencode: false,
            tool_gemini: false,
            ..Settings::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        let back = load_from_str(&json);
        assert!(!back.tool_opencode, "OpenCode 關閉被存檔洗掉了");
        assert!(!back.tool_gemini, "Gemini 關閉被存檔洗掉了");
    }

    /// The whole point of the feature: an explicit opt-out must survive a
    /// round-trip through disk, not be re-defaulted back to pinned.
    #[test]
    fn explicit_opt_out_survives_a_save_load_round_trip() {
        let s = Settings {
            always_on_top: false,
            ..Settings::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(!load_from_str(&json).always_on_top, "取消置頂被存檔洗掉了");
    }
}
