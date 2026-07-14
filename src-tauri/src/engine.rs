//! Stateful glue: keeps per-limit sample history, derives status/pace/runway,
//! and applies most-dangerous selection to produce a Snapshot for the frontend.

use crate::burnrate::{compute_pace, compute_runway};
use crate::model::{Limit, LimitStatus, Snapshot};
use crate::ranking::WorstTracker;
use std::collections::{HashMap, VecDeque};

/// Warning / lock thresholds on util% (UX Spec v3 §7; configurable later).
const NEAR_PCT: f64 = 75.0;
const LOCKED_PCT: f64 = 100.0;
/// How long a sample stays relevant for slope estimation.
///
/// This is the real intent behind the retention rule: runway is projected from
/// *recent* burn. `HISTORY_CAP` alone only approximates it, and the approximation
/// holds solely while sampling is regular — after a gap (laptop sleep, network
/// drop, dead source) a pre-gap sample survives as `history.front()`, stretching
/// `dt` and diluting the slope into a wildly optimistic runway. Bounding by time
/// makes the window explicit and gap-proof.
const HISTORY_WINDOW_SECS: i64 = 900;
/// Upper bound on retained samples per limit (memory guard). 60 x the 15s
/// POLL_SECS in lib.rs is exactly `HISTORY_WINDOW_SECS`, which is where that
/// window length comes from: at the nominal cadence this cap binds first and
/// slope estimation is untouched by the time bound. Real polls are spaced
/// slightly wider than 15s (the poll's own cost lands before the sleep), so the
/// time bound may trim one extra sample -- a sub-1% tightening, and the correct
/// direction. Only a genuine gap makes the two rules diverge meaningfully.
const HISTORY_CAP: usize = 60;

pub struct Engine {
    history: HashMap<String, VecDeque<(i64, f64)>>,
    tracker: WorstTracker,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            history: HashMap::new(),
            tracker: WorstTracker::default(),
        }
    }

    /// Fold a fresh set of limits into state and return the snapshot to emit.
    pub fn ingest(&mut self, mut limits: Vec<Limit>, now: i64) -> Snapshot {
        for l in &mut limits {
            let hist = self.history.entry(l.id.clone()).or_default();

            // Only live limits carry a real util reading. Degraded states ship a
            // `util: 0.0` placeholder, so recording them would poison every later
            // slope; skip both the sample and the projection.
            let live = matches!(l.status, LimitStatus::Normal);
            if live && hist.back().map_or(true, |(t, _)| *t != now) {
                // avoid duplicate samples at the same timestamp
                hist.push_back((now, l.util));
            }

            // Drop samples that fell out of the window. Applied unconditionally so
            // the invariant "history only holds samples from the last
            // HISTORY_WINDOW_SECS" also holds across a degraded stretch.
            let cutoff = now - HISTORY_WINDOW_SECS;
            while hist.front().map_or(false, |(t, _)| *t < cutoff) {
                hist.pop_front();
            }
            while hist.len() > HISTORY_CAP {
                hist.pop_front();
            }

            // Only derive status for live limits; preserve provider-set
            // degraded states (SourceFailed/Stale/Idle) and skip their burn-rate.
            if live {
                l.status = status_of(l.util);
                l.pace = compute_pace(l, now);
                l.runway_secs = compute_runway(hist, l.util);
            }
        }

        let worst_id = self.tracker.select(&limits, now);
        Snapshot {
            limits,
            worst_id,
            updated_at: now,
        }
    }
}

fn status_of(util: f64) -> LimitStatus {
    if util >= LOCKED_PCT {
        LimitStatus::Locked
    } else if util >= NEAR_PCT {
        LimitStatus::Near
    } else {
        LimitStatus::Normal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Provider;

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
        }
    }

    fn limit_with_status(id: &str, util: f64, status: LimitStatus) -> Limit {
        Limit {
            status,
            ..limit(id, util)
        }
    }

    /// Poll cadence in lib.rs; history spans are expressed in these steps.
    const STEP: i64 = 15;

    fn runway_of(snap: &Snapshot, id: &str) -> Option<i64> {
        snap.limits.iter().find(|l| l.id == id).unwrap().runway_secs
    }

    #[test]
    fn assigns_status_by_threshold() {
        let mut e = Engine::new();
        let snap = e.ingest(vec![limit("a", 20.0), limit("b", 80.0), limit("c", 100.0)], 0);
        let get = |id: &str| snap.limits.iter().find(|l| l.id == id).unwrap().status;
        assert_eq!(get("a"), LimitStatus::Normal);
        assert_eq!(get("b"), LimitStatus::Near);
        assert_eq!(get("c"), LimitStatus::Locked);
    }

    #[test]
    fn builds_runway_after_enough_samples() {
        let mut e = Engine::new();
        e.ingest(vec![limit("a", 70.0)], 0);
        e.ingest(vec![limit("a", 75.0)], 50);
        let snap = e.ingest(vec![limit("a", 80.0)], 100);
        let a = snap.limits.iter().find(|l| l.id == "a").unwrap();
        assert!(a.runway_secs.is_some());
    }

    /// Regression: a sampling gap (laptop sleep, network drop, source outage)
    /// used to leave a pre-gap sample as `history.front()`, stretching `dt` and
    /// diluting the slope until runway was reported ~25x optimistic.
    ///
    /// Repro: 15 min of idle-at-40% samples, a 2h gap, then 5 min burning
    /// 40% -> 50%. True runway is 50% remaining / (10%/300s) = 1500s.
    #[test]
    fn runway_not_inflated_by_stale_samples_after_gap() {
        let mut e = Engine::new();

        // 15 min of regular samples, flat at 40% (fills history to HISTORY_CAP).
        for i in 0..60 {
            e.ingest(vec![limit("a", 40.0)], i * STEP);
        }

        // Laptop sleeps for 2 hours: no samples at all.
        let wake = 59 * STEP + 7200;

        // 5 minutes of real burn: 40% -> 50% over 300s.
        let mut last = None;
        for i in 0..=20 {
            let util = 40.0 + 10.0 * (i as f64 / 20.0);
            last = Some(e.ingest(vec![limit("a", util)], wake + i * STEP));
        }

        // Deterministic: the 900s window keeps exactly the 21 post-gap samples,
        // so the slope is the true 10%/300s. Pre-fix this was Some(40350) (11.2h,
        // 27x optimistic). Asserted exactly rather than as a range so that a
        // regression to None -- also "honest", but a silent loss of the
        // projection -- has to be an explicit decision, not an accident.
        assert_eq!(runway_of(&last.unwrap(), "a"), Some(1500));
    }

    /// Guard: with regular sampling and no gap, the fix must not change the
    /// numbers at all. Values here were captured from the pre-fix engine.
    #[test]
    fn regular_sampling_runway_is_unchanged() {
        let mut e = Engine::new();

        // 100 samples at the 15s poll cadence, util ramping 40.0 -> 49.9.
        let mut last = None;
        for i in 0..100 {
            let util = 40.0 + i as f64 * 0.1;
            last = Some(e.ingest(vec![limit("a", util)], i * STEP));
        }

        // Retained window is the last 60 samples: t=600..1485, util 44.0..49.9.
        // slope = 5.9/885 %/s; remaining = 50.1 -> 7515s.
        let r = runway_of(&last.unwrap(), "a").expect("regular sampling must project");
        assert!((r - 7515).abs() <= 1, "runway was {r}, expected ~7515");
    }

    /// Garbage in, garbage out: degraded sources ship placeholder `util: 0.0`,
    /// which must never be recorded as a real sample.
    #[test]
    fn degraded_limits_are_not_recorded_in_history() {
        for status in [
            LimitStatus::SourceFailed,
            LimitStatus::Stale,
            LimitStatus::Idle,
        ] {
            let mut e = Engine::new();
            e.ingest(vec![limit_with_status("a", 0.0, status)], 0);
            e.ingest(vec![limit_with_status("a", 0.0, status)], STEP);
            e.ingest(vec![limit_with_status("a", 0.0, status)], 2 * STEP);
            assert!(
                e.history.get("a").map_or(true, |h| h.is_empty()),
                "{status:?} placeholder samples leaked into history"
            );
        }
    }

    /// A degraded stretch must not poison the slope once the source recovers.
    #[test]
    fn placeholder_samples_do_not_poison_recovery_slope() {
        let mut e = Engine::new();
        // Source is down from the start, reporting the util: 0.0 placeholder.
        for i in 0..=10 {
            e.ingest(
                vec![limit_with_status("a", 0.0, LimitStatus::SourceFailed)],
                i * STEP,
            );
        }
        // Recovery: 80% and genuinely idle. Recorded placeholders would make
        // this look like a 0% -> 80% burn and project a fake ~52s runway.
        let mut last = None;
        for i in 11..=14 {
            last = Some(e.ingest(vec![limit("a", 80.0)], i * STEP));
        }
        assert_eq!(
            runway_of(&last.unwrap(), "a"),
            None,
            "idle-through-outage must not project a runway"
        );
    }

    /// After a gap the surviving sample count drops below the projection
    /// threshold; §4.3 requires falling back to None (UI shows the reset
    /// countdown) rather than fabricating a number.
    #[test]
    fn insufficient_samples_after_gap_yield_none() {
        let mut e = Engine::new();
        for i in 0..60 {
            e.ingest(vec![limit("a", 40.0)], i * STEP);
        }
        let wake = 59 * STEP + 7200;

        // Post-gap samples climb, so a surviving pre-gap `front` would yield a
        // positive slope and a number. Pre-fix both of these projected (~88000s).
        assert_eq!(
            runway_of(&e.ingest(vec![limit("a", 45.0)], wake), "a"),
            None,
            "one post-gap sample must not project"
        );
        assert_eq!(
            runway_of(&e.ingest(vec![limit("a", 50.0)], wake + STEP), "a"),
            None,
            "two post-gap samples must not project"
        );
    }
}
