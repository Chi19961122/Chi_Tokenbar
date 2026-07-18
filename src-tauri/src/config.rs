//! Persisted settings (UX Spec v3 §10). Stored as JSON in the OS config dir.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    /// 供應商多選 (T-916, slimmed in T-917): the unified list of sources to poll
    /// and scan. Any of "claude" | "codex" | "grok"; unknown values (including the
    /// removed "opencode"/"gemini") are dropped on load and an empty vec is
    /// allowed (means: nothing polled/scanned — an honest empty UI).
    ///
    /// Claude/Codex are subscription-quota providers; Grok contributes both a
    /// local context-fill limit (providers/grok.rs) and usage analytics
    /// (analytics::scan_grok), both gated on "grok" membership.
    ///
    /// This is the runtime source of truth (scheduler + analytics gate on it);
    /// `providers` below is kept only for write-back so a one-version downgrade
    /// still reads a sane file.
    #[serde(default = "default_sources")]
    pub sources: Vec<String>,
    /// DEPRECATED (read for migration, written for downgrade-safety only): the
    /// pre-T-916 3-way display filter — "both" | "claude" | "codex". Runtime code
    /// reads `sources`; `save` re-derives this from `sources` before writing.
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
    /// UI theme: "system" (follow the OS `prefers-color-scheme`), "light", or
    /// "dark". Defaults to "system". Resolved entirely in the frontend
    /// (`applyTheme` / `resolveThemeDark`); the backend never reads it.
    #[serde(default = "default_theme")]
    pub theme: String,
    /// 階段 D 戰報 Share: which share-card style the report panel last used —
    /// "statement" | "diagnostics" | "minimal" | "fuel" | "island_card" | "wa".
    /// Defaults to "statement".
    #[serde(default = "default_share_style")]
    pub share_style: String,
    /// 階段 D 戰報 Share: which range the report panel last summarized —
    /// "today" | "week" | "month". Defaults to "week".
    #[serde(default = "default_share_range")]
    pub share_range: String,
    /// T-905 戰報尺寸: which share-card size the report panel last used —
    /// "auto" (1200×675 landscape) | "story" (9:16 portrait). Defaults to "auto".
    #[serde(default = "default_share_size")]
    pub share_size: String,
    /// T-910 更新頻率: how often the quota APIs are polled over the network, in
    /// seconds. One of {30, 60, 180}; serde default 180 (the conservative
    /// cadence). Clamped on read via `refresh_secs_clamped` so a hand-edited
    /// settings.json can never drive a faster-than-offered or absurd cadence —
    /// 30s already runs 6× Claude Code's request rate against a *shared* rate
    /// bucket (docs/FEEDBACK.md F-01), which is why the Anthropic provider pairs
    /// this with 429 exponential backoff.
    #[serde(default = "default_refresh_secs")]
    pub refresh_secs: u32,
}

/// The known sources, in canonical display/order (T-917: OpenCode/Gemini removed
/// as sources). Anything outside this set — including the removed ids — is
/// dropped by `sanitize_sources` on load.
pub const KNOWN_SOURCES: [&str; 3] = ["claude", "codex", "grok"];

/// Fresh-install default: every source on (detect-and-show). Grok is included —
/// T-916/T-917 was requested specifically to add it.
fn default_sources() -> Vec<String> {
    KNOWN_SOURCES.iter().map(|s| s.to_string()).collect()
}

/// Keep only known source ids, first occurrence wins (dedup, order-preserving).
/// An empty result is valid and means "show nothing" — never re-expanded to a
/// default here (that would silently re-enable a source the user turned off).
fn sanitize_sources(sources: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in sources {
        if KNOWN_SOURCES.contains(&s.as_str()) && !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// Derive `sources` from the legacy fields when a stored file predates the
/// multi-select (no `sources` key).
///
/// Migration matrix (T-917 — OpenCode/Gemini are no longer sources, so the old
/// `tool_opencode`/`tool_gemini` flags are ignored):
///   · providers "claude" → ["claude"]  (only the Claude half of the quota pair)
///   · providers "codex"  → ["codex"]
///   · anything else/"both"/unknown → ["claude","codex"]
///   · then append "grok". Grok would normally default OFF on migration (the
///     user never had it), but this ticket exists *because* this user asked for
///     Grok, so migrating it ON is the request, not a surprise. Noted in report.
fn derive_sources_from_legacy(obj: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    let providers = obj.get("providers").and_then(|p| p.as_str()).unwrap_or("both");
    let mut out: Vec<String> = match providers {
        "claude" => vec!["claude".into()],
        "codex" => vec!["codex".into()],
        _ => vec!["claude".into(), "codex".into()],
    };
    out.push("grok".into());
    out
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

fn default_theme() -> String {
    "system".into()
}

fn default_share_style() -> String {
    "statement".into()
}

fn default_share_range() -> String {
    "week".into()
}

fn default_share_size() -> String {
    "auto".into()
}

fn default_refresh_secs() -> u32 {
    180
}

/// The three offered refresh cadences (seconds). The settings UI only ever
/// sends one of these; the clamp below is a defensive read for a hand-edited
/// settings.json.
pub const REFRESH_CHOICES: [u32; 3] = [30, 60, 180];

/// Snap any stored `refresh_secs` to the nearest offered cadence. A too-fast
/// value (e.g. a hand-edit to 5) floors to 30; a too-slow one (3600) caps at
/// 180. Pure and standalone so the clamp is unit-testable.
pub fn clamp_refresh_secs(secs: u32) -> u32 {
    *REFRESH_CHOICES
        .iter()
        .min_by_key(|&&choice| choice.abs_diff(secs))
        .expect("REFRESH_CHOICES is non-empty")
}

impl Settings {
    /// `refresh_secs` snapped to an offered cadence — the value the scheduler
    /// must gate on. Read every round so a change applies without a restart.
    pub fn refresh_secs_clamped(&self) -> u32 {
        clamp_refresh_secs(self.refresh_secs)
    }

    /// Whether a given source id is currently selected. The one runtime gate for
    /// polling/scanning a source (scheduler + analytics + limit filter).
    pub fn has_source(&self, id: &str) -> bool {
        self.sources.iter().any(|s| s == id)
    }

    /// Re-derive the deprecated `providers` field from `sources` so a written
    /// file stays readable by a one-version downgrade. Called from `save`.
    ///
    /// `providers`: both quota providers selected → "both"; exactly one → that
    /// one; neither → "both" (the least-broken value for an old build, which has
    /// no way to represent "no quota provider"; noted — a rare edge).
    fn sync_legacy_from_sources(&mut self) {
        let claude = self.has_source("claude");
        let codex = self.has_source("codex");
        self.providers = match (claude, codex) {
            (true, false) => "claude",
            (false, true) => "codex",
            _ => "both",
        }
        .into();
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            allow_token_refresh: false,
            autostart: false,
            warn_pct: 75.0,
            crit_pct: 90.0,
            compact: false,
            sources: default_sources(),
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
            theme: default_theme(),
            share_style: default_share_style(),
            share_range: default_share_range(),
            share_size: default_share_size(),
            refresh_secs: default_refresh_secs(),
        }
    }
}

fn path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("Atoll").join("settings.json"))
}

/// The pre-rename settings location (`%APPDATA%\TokenBar\settings.json`). Kept
/// only so `load()` can perform the one-time TokenBar→Atoll migration; nothing
/// ever writes here.
fn legacy_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("TokenBar").join("settings.json"))
}

/// One-time TokenBar→Atoll settings migration decision. Pure so the three
/// states are unit-testable without touching disk:
///   (a) old exists, new absent → true  (copy once)
///   (b) new exists              → false (never overwrite a migrated/newer file)
///   (c) neither exists          → false (fall through to defaults)
fn should_migrate_legacy(new_exists: bool, old_exists: bool) -> bool {
    !new_exists && old_exists
}

/// Copy the legacy TokenBar settings into the new Atoll location exactly once,
/// creating the Atoll dir first. A no-op unless `should_migrate_legacy` holds,
/// so it is safe to call on every startup. Best-effort: any IO error leaves the
/// new path absent and `load` falls through to defaults, same as a fresh user.
fn migrate_legacy(new_path: &Path, old_path: &Path) {
    if !should_migrate_legacy(new_path.exists(), old_path.exists()) {
        return;
    }
    if let Some(dir) = new_path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::copy(old_path, new_path);
}

pub fn load() -> Settings {
    // Migrate the user's pre-rename settings before the first read of the new
    // path (see `migrate_legacy`): if the Atoll file is absent but a TokenBar
    // file exists, copy it over once. Runs every startup but only acts once.
    if let (Some(new_p), Some(old_p)) = (path(), legacy_path()) {
        migrate_legacy(&new_p, &old_p);
    }
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
        // T-916: a file with no `sources` predates the multi-select — derive it
        // from the legacy providers/tool_* fields. An explicit `sources` (even
        // an empty array) always wins and is only sanitized below.
        if !obj.contains_key("sources") {
            let derived = derive_sources_from_legacy(obj);
            obj.insert("sources".into(), serde_json::json!(derived));
        }
    }
    let mut s: Settings = serde_json::from_value(v).unwrap_or_default();
    // Drop any unknown / duplicate source ids that reached us (hand-edited file,
    // or a future id read by an older build).
    s.sources = sanitize_sources(s.sources);
    s
}

pub fn save(s: &Settings) {
    // Write `sources` as the source of truth, but keep the deprecated legacy
    // fields in sync so a one-version downgrade still reads a coherent file.
    let mut s = s.clone();
    s.sources = sanitize_sources(s.sources);
    s.sync_legacy_from_sources();
    if let Some(p) = path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(&s) {
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

    // ── theme (T-901 dual light/dark) ─────────────────────────────────
    //
    // Default must be "system" so an existing settings.json (written before the
    // field existed) follows the OS `prefers-color-scheme` rather than being
    // pinned to a specific theme.

    #[test]
    fn defaults_to_system_theme() {
        assert_eq!(Settings::default().theme, "system");
    }

    #[test]
    fn missing_theme_deserializes_to_system() {
        let s = load_from_str(r#"{ "autostart": true }"#);
        assert_eq!(s.theme, "system");
    }

    #[test]
    fn explicit_theme_survives_a_save_load_round_trip() {
        let s = Settings {
            theme: "dark".into(),
            ..Settings::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(load_from_str(&json).theme, "dark");
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
        // T-905: the new size field defaults to the original landscape.
        assert_eq!(d.share_size, "auto");
    }

    #[test]
    fn missing_stage_d_fields_deserialize_to_defaults() {
        let s = load_from_str(r#"{ "autostart": true }"#);
        assert_eq!(s.share_style, "statement");
        assert_eq!(s.share_range, "week");
        // T-905: a pre-905 file lacks share_size → serde default "auto".
        assert_eq!(s.share_size, "auto");
    }

    #[test]
    fn explicit_stage_d_fields_survive_a_save_load_round_trip() {
        let s = Settings {
            share_style: "fuel".into(),
            share_range: "month".into(),
            share_size: "story".into(),
            ..Settings::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        let back = load_from_str(&json);
        assert_eq!(back.share_style, "fuel");
        assert_eq!(back.share_range, "month");
        assert_eq!(back.share_size, "story");
    }

    // ── 供應商多選 + migration (T-916, slimmed T-917) ──────────────────

    #[test]
    fn fresh_default_has_three_sources_including_grok() {
        assert_eq!(Settings::default().sources, vec!["claude", "codex", "grok"]);
    }

    /// Migration matrix (T-917): a legacy file (no `sources`) derives the list
    /// from the old `providers` filter and always migrates grok ON. The old
    /// `tool_opencode`/`tool_gemini` flags are ignored — those sources are gone.
    #[test]
    fn migrates_legacy_providers_to_sources() {
        // providers "both" (the pre-multi-select default) → the quota pair + grok.
        let s = load_from_str(r#"{ "providers": "both" }"#);
        assert_eq!(s.sources, vec!["claude", "codex", "grok"]);

        // providers "claude" keeps only the Claude half of the quota pair (+ grok).
        let s = load_from_str(r#"{ "providers": "claude" }"#);
        assert_eq!(s.sources, vec!["claude", "grok"]);

        // providers "codex" keeps only Codex of the quota pair (+ grok).
        let s = load_from_str(r#"{ "providers": "codex" }"#);
        assert_eq!(s.sources, vec!["codex", "grok"]);

        // The removed tool flags no longer add sources — they are simply ignored.
        let s = load_from_str(
            r#"{ "providers": "both", "tool_opencode": true, "tool_gemini": true }"#,
        );
        assert_eq!(s.sources, vec!["claude", "codex", "grok"]);
    }

    /// An empty settings file (a brand-new install writing {} first) migrates to
    /// the quota pair + grok — providers defaults to "both", grok on.
    #[test]
    fn empty_file_migrates_to_all_sources() {
        assert_eq!(load_from_str("{}").sources, vec!["claude", "codex", "grok"]);
    }

    /// An explicit `sources` array always wins over the legacy fields and is only
    /// sanitized: unknown ids — including the removed "opencode"/"gemini" — are
    /// dropped, duplicates collapsed, order preserved.
    #[test]
    fn explicit_sources_wins_and_is_sanitized() {
        let s = load_from_str(
            r#"{ "providers": "both", "sources": ["grok", "codex", "bogus", "grok", "gemini"] }"#,
        );
        assert_eq!(s.sources, vec!["grok", "codex"]);
    }

    /// A T-916 file still listing opencode/gemini loads with those dropped, so an
    /// upgrade slims the selection to the surviving sources automatically.
    #[test]
    fn removed_sources_are_dropped_on_load() {
        let s = load_from_str(
            r#"{ "sources": ["claude", "codex", "opencode", "gemini", "grok"] }"#,
        );
        assert_eq!(s.sources, vec!["claude", "codex", "grok"]);
    }

    /// An explicit empty `sources` is honoured (nothing shown) — never re-filled
    /// from the legacy fields.
    #[test]
    fn explicit_empty_sources_stays_empty() {
        let s = load_from_str(r#"{ "providers": "both", "sources": [] }"#);
        assert!(s.sources.is_empty());
    }

    /// `save` re-derives the deprecated `providers` field from `sources` so a
    /// downgrade reads a coherent file. Exercised via `sync_legacy_from_sources`.
    #[test]
    fn legacy_providers_tracks_sources_for_downgrade_safety() {
        let mut claude_only = Settings {
            sources: vec!["claude".into(), "grok".into()],
            ..Settings::default()
        };
        claude_only.sync_legacy_from_sources();
        assert_eq!(claude_only.providers, "claude");

        let mut both = Settings {
            sources: vec!["claude".into(), "codex".into()],
            ..Settings::default()
        };
        both.sync_legacy_from_sources();
        assert_eq!(both.providers, "both");

        // Neither quota provider → "both" (an old build can't express "neither").
        let mut none = Settings {
            sources: vec!["grok".into()],
            ..Settings::default()
        };
        none.sync_legacy_from_sources();
        assert_eq!(none.providers, "both");
    }

    /// An explicit sources selection survives a save/load round-trip.
    #[test]
    fn explicit_sources_survive_a_save_load_round_trip() {
        let s = Settings {
            sources: vec!["codex".into(), "grok".into()],
            ..Settings::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(load_from_str(&json).sources, vec!["codex", "grok"]);
    }

    // ── T-910 refresh_secs ─────────────────────────────────────────────
    //
    // Default must be the conservative 180 so an existing settings.json
    // (written before the field existed) keeps today's cadence. The clamp is a
    // defensive read: only {30, 60, 180} may ever reach the scheduler.

    #[test]
    fn defaults_to_conservative_refresh_secs() {
        assert_eq!(Settings::default().refresh_secs, 180);
        assert_eq!(Settings::default().refresh_secs_clamped(), 180);
    }

    #[test]
    fn missing_refresh_secs_deserializes_to_180() {
        let s = load_from_str(r#"{ "autostart": true }"#);
        assert_eq!(s.refresh_secs, 180);
    }

    #[test]
    fn clamp_keeps_offered_cadences() {
        assert_eq!(clamp_refresh_secs(30), 30);
        assert_eq!(clamp_refresh_secs(60), 60);
        assert_eq!(clamp_refresh_secs(180), 180);
    }

    #[test]
    fn clamp_snaps_out_of_range_values() {
        // Too fast → floored to the 30s minimum.
        assert_eq!(clamp_refresh_secs(0), 30);
        assert_eq!(clamp_refresh_secs(5), 30);
        // Too slow / absurd → capped at the 180s maximum.
        assert_eq!(clamp_refresh_secs(3600), 180);
        assert_eq!(clamp_refresh_secs(u32::MAX), 180);
        // Between-bucket values snap to the nearest offered cadence.
        assert_eq!(clamp_refresh_secs(40), 30);
        assert_eq!(clamp_refresh_secs(90), 60);
        assert_eq!(clamp_refresh_secs(150), 180);
    }

    #[test]
    fn explicit_refresh_secs_survives_a_save_load_round_trip() {
        let s = Settings {
            refresh_secs: 30,
            ..Settings::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        let back = load_from_str(&json);
        assert_eq!(back.refresh_secs, 30);
        assert_eq!(back.refresh_secs_clamped(), 30);
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

    // ── TokenBar → Atoll settings-dir migration (T-920) ─────────────────
    //
    // The settings dir moved from %APPDATA%\TokenBar to %APPDATA%\Atoll. A
    // one-time copy in `load()` must keep an existing user's settings. Three
    // states, tested pure (the copy decision) and on disk (the copy itself).

    #[test]
    fn migrate_decision_covers_three_states() {
        // (a) old exists, new absent → copy.
        assert!(should_migrate_legacy(false, true));
        // (b) new exists → never overwrite (regardless of old).
        assert!(!should_migrate_legacy(true, true));
        assert!(!should_migrate_legacy(true, false));
        // (c) neither → defaults, no copy.
        assert!(!should_migrate_legacy(false, false));
    }

    /// A unique scratch dir under the OS temp dir, so parallel test threads and
    /// repeat runs never collide. Cleaned up at the end of each test.
    fn unique_tmp_dir(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("atoll-mig-{tag}-{nanos}-{n}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// (a) old exists, new absent → the old file's contents are copied to new.
    #[test]
    fn migrate_copies_old_to_new_when_new_absent() {
        let base = unique_tmp_dir("copy");
        let old_p = base.join("TokenBar").join("settings.json");
        let new_p = base.join("Atoll").join("settings.json");
        std::fs::create_dir_all(old_p.parent().unwrap()).unwrap();
        std::fs::write(&old_p, r#"{"warn_pct": 42.0}"#).unwrap();

        migrate_legacy(&new_p, &old_p);

        assert!(new_p.exists(), "新檔應被建立");
        let migrated = load_from_str(&std::fs::read_to_string(&new_p).unwrap());
        assert_eq!(migrated.warn_pct, 42.0, "舊值應被遷移");
        std::fs::remove_dir_all(&base).ok();
    }

    /// (b) new exists → migration must NOT overwrite it with the old file.
    #[test]
    fn migrate_never_overwrites_existing_new() {
        let base = unique_tmp_dir("keep");
        let old_p = base.join("TokenBar").join("settings.json");
        let new_p = base.join("Atoll").join("settings.json");
        std::fs::create_dir_all(old_p.parent().unwrap()).unwrap();
        std::fs::create_dir_all(new_p.parent().unwrap()).unwrap();
        std::fs::write(&old_p, r#"{"warn_pct": 42.0}"#).unwrap();
        std::fs::write(&new_p, r#"{"warn_pct": 88.0}"#).unwrap();

        migrate_legacy(&new_p, &old_p);

        let kept = load_from_str(&std::fs::read_to_string(&new_p).unwrap());
        assert_eq!(kept.warn_pct, 88.0, "既有新檔不可被舊檔覆蓋");
        std::fs::remove_dir_all(&base).ok();
    }

    /// (c) neither exists → no file is created; caller falls through to defaults.
    #[test]
    fn migrate_creates_nothing_when_neither_exists() {
        let base = unique_tmp_dir("none");
        let old_p = base.join("TokenBar").join("settings.json");
        let new_p = base.join("Atoll").join("settings.json");

        migrate_legacy(&new_p, &old_p);

        assert!(!new_p.exists(), "皆無時不應建立任何檔");
        std::fs::remove_dir_all(&base).ok();
    }
}
