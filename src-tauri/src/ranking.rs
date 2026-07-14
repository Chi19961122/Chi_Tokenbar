//! "Most dangerous one" selection with hysteresis (UX Spec v3 §3).
//! Ranking uses util% (not runway, which is a noisy estimate — §13 constraint 9).

use crate::model::Limit;

/// The shown limit must be beaten by this many percentage points before we switch.
pub const HYSTERESIS_PCT: f64 = 5.0;
/// Minimum seconds the current pick stays shown before another may replace it.
pub const MIN_DWELL_SECS: i64 = 45;

/// Tracks which limit is currently on the island and since when, so the single
/// island slot doesn't ping-pong between two near-equal limits.
#[derive(Default)]
pub struct WorstTracker {
    current: Option<String>,
    since: i64,
}

impl WorstTracker {
    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// Pick the most-dangerous limit id, applying the ±HYSTERESIS_PCT margin and
    /// MIN_DWELL_SECS dwell. Returns the id that should be shown.
    pub fn select(&mut self, limits: &[Limit], now: i64) -> Option<String> {
        let top = limits
            .iter()
            .max_by(|a, b| a.util.partial_cmp(&b.util).unwrap_or(std::cmp::Ordering::Equal))?;

        match &self.current {
            Some(cur) => {
                match limits.iter().find(|l| &l.id == cur) {
                    // current pick vanished (e.g. tool stopped) → jump to top.
                    None => self.set(top, now),
                    Some(cur_limit) => {
                        let dwell = now - self.since;
                        let beats_by = top.util - cur_limit.util;
                        if top.id != cur_limit.id
                            && beats_by >= HYSTERESIS_PCT
                            && dwell >= MIN_DWELL_SECS
                        {
                            self.set(top, now);
                        }
                    }
                }
            }
            None => self.set(top, now),
        }
        self.current.clone()
    }

    fn set(&mut self, l: &Limit, now: i64) {
        self.current = Some(l.id.clone());
        self.since = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LimitStatus, Provider};

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
    fn picks_highest_util_initially() {
        let mut t = WorstTracker::default();
        let ls = vec![limit("a", 40.0), limit("b", 88.0), limit("c", 12.0)];
        assert_eq!(t.select(&ls, 0).as_deref(), Some("b"));
    }

    #[test]
    fn does_not_switch_for_small_lead() {
        let mut t = WorstTracker::default();
        t.select(&vec![limit("a", 87.0), limit("b", 85.0)], 0);
        // b creeps to 89 (only +2 over a) well after dwell — should stay on a.
        let out = t.select(&vec![limit("a", 87.0), limit("b", 89.0)], 100);
        assert_eq!(out.as_deref(), Some("a"));
    }

    #[test]
    fn switches_when_lead_exceeds_margin_after_dwell() {
        let mut t = WorstTracker::default();
        t.select(&vec![limit("a", 80.0), limit("b", 78.0)], 0);
        let out = t.select(&vec![limit("a", 80.0), limit("b", 90.0)], 100);
        assert_eq!(out.as_deref(), Some("b"));
    }

    #[test]
    fn respects_min_dwell_even_with_big_lead() {
        let mut t = WorstTracker::default();
        t.select(&vec![limit("a", 80.0), limit("b", 78.0)], 0);
        // big lead but only 5s elapsed (< MIN_DWELL_SECS) → keep a.
        let out = t.select(&vec![limit("a", 80.0), limit("b", 99.0)], 5);
        assert_eq!(out.as_deref(), Some("a"));
    }

    #[test]
    fn jumps_when_current_disappears() {
        let mut t = WorstTracker::default();
        t.select(&vec![limit("a", 80.0)], 0);
        let out = t.select(&vec![limit("b", 30.0)], 5);
        assert_eq!(out.as_deref(), Some("b"));
    }
}
