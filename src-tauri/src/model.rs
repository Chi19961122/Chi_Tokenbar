//! Domain types shared between backend and frontend.
//! Data model uses the client × provider two-axis idea from UX Spec v3 §8;
//! for M0/M1 we only populate the Codex provider.

use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Anthropic,
    Codex,
}

/// Seven-state machine from UX Spec v3 §7.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LimitStatus {
    Normal,
    Near,
    Locked,
    Stale,
    InsufficientData,
    SourceFailed,
    Idle,
}

/// A remedy the **backend** decided the user can act on for a failed limit.
///
/// A closed enum, not a free string, on purpose: the panel turns this into a
/// button that launches an external process, so the value must never be
/// derivable from an API response or from matching on hint text. Adding a
/// variant here is a deliberate decision about what a button may start.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LimitAction {
    /// A login-class failure that `claude auth login` can actually fix.
    Relogin,
}

/// Pace vs an even-burn line over the window (UX Spec v3 §4.1).
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Pace {
    /// util% minus the on-pace util% (positive = burning too fast).
    pub deficit: f64,
    pub in_deficit: bool,
}

/// A single rate limit for one provider window.
#[derive(Clone, Debug, Serialize)]
pub struct Limit {
    pub id: String,
    pub provider: Provider,
    pub label: String,
    /// utilization percentage 0..=100 (canonical metric — ranking/color use this).
    pub util: f64,
    /// epoch seconds when the window resets; 0 if unknown.
    pub resets_at: i64,
    /// window length in seconds; 0 if unknown.
    pub window_secs: i64,
    pub status: LimitStatus,
    /// (used, cap) absolute tokens when available; None for %-only sources.
    pub absolute: Option<(u64, u64)>,
    pub pace: Option<Pace>,
    /// projected seconds until empty ("~empty in X"); None when not projectable (§4.3).
    pub runway_secs: Option<i64>,
    /// 來源失效時給使用者看的白話提示 (§7);正常狀態為 None。
    /// SECRET: 只放固定文案,絕不放 token / email / account id / response body。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// 只有「重新登入真的修得好」的失效才帶 (§7);其餘一律 None。
    ///
    /// 連線被防毒/公司網路擋住時給「重新登入」按鈕是誤導 —— 使用者按了沒用,
    /// 還會以為是自己帳號有問題。決定權在後端的 `FailureStage::action()`,
    /// 前端不得改用比對 `hint` 文字來猜。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<LimitAction>,
}

impl Limit {
    /// `% left` framing for display (UX Spec v3 §2.1) while `util` stays canonical.
    pub fn pct_left(&self) -> f64 {
        (100.0 - self.util).clamp(0.0, 100.0)
    }
}

/// Full snapshot emitted to the frontend on every refresh.
#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub limits: Vec<Limit>,
    /// id of the single most-dangerous limit shown on the island (§3).
    pub worst_id: Option<String>,
    pub updated_at: i64,
    /// Seconds until the next backend data fetch, as of `updated_at`. Drives the
    /// header "Refresh in Ns" countdown. Set by the scheduler (engine leaves 0).
    pub next_fetch_in: i64,
}
