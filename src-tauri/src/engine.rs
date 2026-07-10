//! Stateful glue: keeps per-limit sample history, derives status/pace/runway,
//! and applies most-dangerous selection to produce a Snapshot for the frontend.

use crate::burnrate::{compute_pace, compute_runway};
use crate::model::{Limit, LimitStatus, Snapshot};
use crate::ranking::WorstTracker;
use std::collections::{HashMap, VecDeque};

/// Warning / lock thresholds on util% (UX Spec v3 §7; configurable later).
const NEAR_PCT: f64 = 75.0;
const LOCKED_PCT: f64 = 100.0;
/// How many samples to retain per limit for slope estimation.
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
            // avoid duplicate samples at the same timestamp
            if hist.back().map_or(true, |(t, _)| *t != now) {
                hist.push_back((now, l.util));
            }
            while hist.len() > HISTORY_CAP {
                hist.pop_front();
            }

            // Only derive status for live limits; preserve provider-set
            // degraded states (SourceFailed/Stale/Idle) and skip their burn-rate.
            if matches!(l.status, LimitStatus::Normal) {
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
        }
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
}
