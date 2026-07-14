//! Burn-rate engine: pace (deficit) and runway projection.
//! Implements UX Spec v3 §4 with its honesty constraints.

use crate::model::{Limit, Pace};
use std::collections::VecDeque;

/// Minimum samples before we dare project a runway (§4.3 constraint 3).
pub const MIN_SAMPLES_FOR_RUNWAY: usize = 3;
/// util%/sec below this is treated as idle → no projection (§4.3 constraint 4).
const IDLE_SLOPE: f64 = 1e-5;
/// deficit above this counts as "in deficit" (small epsilon to avoid flapping at the line).
const DEFICIT_EPS: f64 = 1.0;

/// Pace relative to the even-burn line over a window with known reset + length.
///
/// `f = elapsed / window`; `on_pace = f * 100`; `deficit = util - on_pace`.
pub fn compute_pace(limit: &Limit, now: i64) -> Option<Pace> {
    if limit.window_secs <= 0 || limit.resets_at <= 0 {
        return None;
    }
    let window_start = limit.resets_at - limit.window_secs;
    let elapsed = (now - window_start) as f64;
    if elapsed <= 0.0 {
        return None;
    }
    let f = (elapsed / limit.window_secs as f64).clamp(0.0, 1.0);
    let on_pace = f * 100.0;
    let deficit = limit.util - on_pace;
    Some(Pace {
        deficit,
        in_deficit: deficit > DEFICIT_EPS,
    })
}

/// Project seconds-until-empty from recent util samples.
///
/// Honesty constraints (§4.3): needs >= MIN_SAMPLES_FOR_RUNWAY samples and a
/// positive slope; returns None when idle or samples are insufficient so the UI
/// falls back to showing the reset countdown instead of a fabricated number.
///
/// Contract: the slope is taken from `front`/`back` only, so `history` must
/// already be bounded to a recent time window (see `engine::HISTORY_WINDOW_SECS`).
/// Given a stale `front` this happily reports a badly inflated runway.
pub fn compute_runway(history: &VecDeque<(i64, f64)>, util: f64) -> Option<i64> {
    if history.len() < MIN_SAMPLES_FOR_RUNWAY {
        return None;
    }
    let (t0, u0) = *history.front().unwrap();
    let (t1, u1) = *history.back().unwrap();
    let dt = (t1 - t0) as f64;
    if dt <= 0.0 {
        return None;
    }
    let slope = (u1 - u0) / dt; // util% per second
    if slope <= IDLE_SLOPE {
        return None; // idle / flat / decreasing (rolling window may even refill)
    }
    let remaining = (100.0 - util).max(0.0);
    Some((remaining / slope) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LimitStatus, Provider};

    fn limit(util: f64, resets_at: i64, window_secs: i64) -> Limit {
        Limit {
            id: "t".into(),
            provider: Provider::Codex,
            label: "t".into(),
            util,
            resets_at,
            window_secs,
            status: LimitStatus::Normal,
            absolute: None,
            pace: None,
            runway_secs: None,
        }
    }

    #[test]
    fn pace_in_deficit_when_ahead_of_even_burn() {
        // window 100s, we are 50% through it but have used 90% → clearly in deficit.
        let now = 1000;
        let l = limit(90.0, now + 50, 100);
        let p = compute_pace(&l, now).unwrap();
        assert!(p.in_deficit);
        assert!((p.deficit - 40.0).abs() < 1e-6); // 90 - 50
    }

    #[test]
    fn pace_on_track_when_below_even_burn() {
        let now = 1000;
        let l = limit(30.0, now + 50, 100); // 50% through, used 30%
        let p = compute_pace(&l, now).unwrap();
        assert!(!p.in_deficit);
        assert!(p.deficit < 0.0);
    }

    #[test]
    fn pace_none_without_window_info() {
        let now = 1000;
        assert!(compute_pace(&limit(50.0, 0, 0), now).is_none());
    }

    #[test]
    fn runway_none_with_too_few_samples() {
        let mut h = VecDeque::new();
        h.push_back((0, 10.0));
        h.push_back((10, 20.0));
        assert!(compute_runway(&h, 20.0).is_none());
    }

    #[test]
    fn runway_none_when_idle() {
        let mut h = VecDeque::new();
        h.push_back((0, 40.0));
        h.push_back((10, 40.0));
        h.push_back((20, 40.0));
        assert!(compute_runway(&h, 40.0).is_none());
    }

    #[test]
    fn runway_projects_forward_at_current_slope() {
        // burned 10% over 100s → 0.1%/s; 20% left → ~200s runway.
        let mut h = VecDeque::new();
        h.push_back((0, 70.0));
        h.push_back((50, 75.0));
        h.push_back((100, 80.0));
        let r = compute_runway(&h, 80.0).unwrap();
        assert!((r - 200).abs() <= 1, "runway was {r}");
    }
}
