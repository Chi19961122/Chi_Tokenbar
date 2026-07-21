//! Burn-rate engine: pace (deficit) and runway projection.
//! Implements UX Spec v3 §4 with its honesty constraints.
//!
//! T-feat-007 adds a *historical* projection path on top of the linear one:
//! `split_cycles` carves the landed sample series into complete quota cycles,
//! `historical_pace` reads the median historical curve at the current window
//! progress, and `project_runway` is the single switch the engine calls — it
//! stays byte-for-byte on the linear path until ≥2 complete cycles exist, and
//! falls back to linear whenever the historical curve can't produce a number
//! (§8「不可倒退」).

use crate::model::{Limit, Pace, PaceBasis};
use std::collections::VecDeque;

/// Minimum samples before we dare project a runway (§4.3 constraint 3).
pub const MIN_SAMPLES_FOR_RUNWAY: usize = 3;
/// util%/sec below this is treated as idle → no projection (§4.3 constraint 4).
const IDLE_SLOPE: f64 = 1e-5;
/// deficit above this counts as "in deficit" (small epsilon to avoid flapping at the line).
const DEFICIT_EPS: f64 = 1.0;

/// util% at/above which a limit is spent (mirrors engine::LOCKED_PCT). Kept local
/// so burnrate stays a leaf with no engine dependency.
const LOCKED_UTIL: f64 = 100.0;
/// Fixed bucket count a complete cycle is normalized to (§B-6). 48 is a
/// reasonable resolution for a 5h window sampled every few minutes and still
/// cheap to median across cycles every round.
pub const CYCLE_BUCKETS: usize = 48;
/// Cycles required before the historical path activates (§C-8). Below this the
/// engine's projection is the untouched linear one.
pub const HIST_MIN_CYCLES: usize = 2;
/// A segment counts as a *complete* cycle only if its first→last sample span
/// covers this fraction of the window (§B-5): a sparse runt at the tail is not
/// a cycle we can learn a curve from.
const CYCLE_COMPLETE_FRAC: f64 = 0.8;
/// util rise (from >CYCLE_HIGH to <CYCLE_LOW) that marks a reset even when
/// `resets_at` did not advance (e.g. a local snapshot source), §B-5.
const CYCLE_HIGH: f64 = 20.0;
const CYCLE_LOW: f64 = 5.0;

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
        // Defaults; the engine overwrites these when the historical path is live.
        pace_basis: PaceBasis::Linear,
        run_out_probability: None,
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

// ── Historical pace (T-feat-007 §B/§C) ───────────────────────────────

/// One complete quota cycle normalized to a `window-progress 0..1 → util%`
/// curve (§B-6). `hit_locked` records whether this cycle ever touched 100 —
/// the raw material for `run_out_probability`.
#[derive(Clone, Debug)]
pub struct Cycle {
    pub curve: [f64; CYCLE_BUCKETS],
    pub hit_locked: bool,
}

impl Cycle {
    /// util at window-progress `t` (0..1), linearly interpolated between buckets.
    pub fn at(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        let x = t * (CYCLE_BUCKETS - 1) as f64;
        let i = x.floor() as usize;
        if i >= CYCLE_BUCKETS - 1 {
            return self.curve[CYCLE_BUCKETS - 1];
        }
        let frac = x - i as f64;
        self.curve[i] * (1.0 - frac) + self.curve[i + 1] * frac
    }
}

/// Output of `historical_pace`. `expected_util` is the median historical curve
/// at the current progress; `runway_secs` is None when the median curve never
/// reaches 100 in the remaining window (caller then falls back to linear).
#[derive(Clone, Copy, Debug)]
pub struct HistPace {
    pub expected_util: f64,
    pub runway_secs: Option<i64>,
    pub run_out_probability: f64,
}

/// Median of a slice (average of the two middle values for an even count). 0.0
/// for an empty slice — callers guard cycle count before it matters.
fn median(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

/// Median across all cycles of their util at window-progress `p`.
fn median_curve_at(cycles: &[Cycle], p: f64) -> f64 {
    let mut vals: Vec<f64> = cycles.iter().map(|c| c.at(p)).collect();
    median(&mut vals)
}

/// util at progress `p` over a set of ascending `(progress, util)` points, with
/// flat extrapolation past either end.
fn interp_at(points: &[(f64, f64)], p: f64) -> f64 {
    if points.is_empty() {
        return 0.0;
    }
    if p <= points[0].0 {
        return points[0].1;
    }
    let last = points.len() - 1;
    if p >= points[last].0 {
        return points[last].1;
    }
    for w in points.windows(2) {
        let (p0, u0) = w[0];
        let (p1, u1) = w[1];
        if p >= p0 && p <= p1 {
            if (p1 - p0).abs() < 1e-12 {
                return u1;
            }
            let f = (p - p0) / (p1 - p0);
            return u0 * (1.0 - f) + u1 * f;
        }
    }
    points[last].1
}

/// Resample ascending `(progress, util)` points onto the fixed bucket grid.
fn interp_curve(points: &[(f64, f64)]) -> [f64; CYCLE_BUCKETS] {
    let mut curve = [0.0; CYCLE_BUCKETS];
    for (b, slot) in curve.iter_mut().enumerate() {
        let p = b as f64 / (CYCLE_BUCKETS - 1) as f64;
        *slot = interp_at(points, p);
    }
    curve
}

/// Turn one contiguous `[(ts, util)]` segment into a normalized cycle iff it is
/// *complete* (spans ≥ 80% of the window). The cycle's own first sample is
/// treated as window start; progress = (ts - start) / window, clamped.
fn finalize(seg: &[(i64, f64)], window_secs: i64) -> Option<Cycle> {
    if seg.len() < 2 {
        return None;
    }
    let span = seg[seg.len() - 1].0 - seg[0].0;
    if (span as f64) < CYCLE_COMPLETE_FRAC * window_secs as f64 {
        return None;
    }
    let start = seg[0].0;
    let mut points: Vec<(f64, f64)> = Vec::with_capacity(seg.len());
    let mut hit_locked = false;
    for &(ts, u) in seg {
        let p = ((ts - start) as f64 / window_secs as f64).clamp(0.0, 1.0);
        points.push((p, u));
        if u >= LOCKED_UTIL {
            hit_locked = true;
        }
    }
    Some(Cycle {
        curve: interp_curve(&points),
        hit_locked,
    })
}

/// Carve the landed sample series into complete cycles (§B-5/6).
///
/// A boundary sits between two consecutive samples when either the applicable
/// `resets_at` advances (new window issued) or util falls from a high point
/// (>20%) back to a low one (<5%). `resets` is the `(ts, resets_at)` change-point
/// list, assumed ascending in ts. Only complete segments become cycles; a sparse
/// tail runt is dropped.
pub fn split_cycles(samples: &[(i64, f64)], resets: &[(i64, i64)], window_secs: i64) -> Vec<Cycle> {
    if window_secs <= 0 || samples.len() < 2 {
        return Vec::new();
    }
    // resets_at in effect at instant `ts`: the last change-point at or before it.
    let applicable = |ts: i64| -> i64 {
        let mut r = 0;
        for &(t, val) in resets {
            if t <= ts {
                r = val;
            } else {
                break;
            }
        }
        r
    };
    let mut out = Vec::new();
    let mut start = 0usize;
    for i in 1..samples.len() {
        let (pts, pu) = samples[i - 1];
        let (cts, cu) = samples[i];
        let reset_adv = applicable(cts) > applicable(pts);
        let dropped = pu > CYCLE_HIGH && cu < CYCLE_LOW;
        if reset_adv || dropped {
            if let Some(c) = finalize(&samples[start..i], window_secs) {
                out.push(c);
            }
            start = i;
        }
    }
    if let Some(c) = finalize(&samples[start..], window_secs) {
        out.push(c);
    }
    out
}

/// Median historical curve read-out + runway + run-out probability at the
/// current window progress `t`, extrapolating from the *current* util (§C-7).
///
/// Runway walks the median curve forward from `t`: the increment it predicts
/// (median(t+Δ) - median(t)) is added to the current util, and we return the Δ
/// (mapped to seconds) at which that reaches 100. If the curve never gets there
/// within the remaining window, runway is None so the caller can fall back to
/// the linear projection rather than fabricate or blank the number.
pub fn historical_pace(cycles: &[Cycle], t: f64, util: f64, window_secs: i64) -> HistPace {
    let t = t.clamp(0.0, 1.0);
    let expected_util = median_curve_at(cycles, t);
    let hit = cycles.iter().filter(|c| c.hit_locked).count();
    let run_out_probability = if cycles.is_empty() {
        0.0
    } else {
        hit as f64 / cycles.len() as f64
    };
    let runway_secs = historical_runway(cycles, t, util, window_secs);
    HistPace {
        expected_util,
        runway_secs,
        run_out_probability,
    }
}

fn historical_runway(cycles: &[Cycle], t: f64, util: f64, window_secs: i64) -> Option<i64> {
    if cycles.is_empty() || window_secs <= 0 {
        return None;
    }
    if util >= LOCKED_UTIL {
        return Some(0);
    }
    let base = median_curve_at(cycles, t);
    // Sample the remaining progress finely; the curve is piecewise-linear so a
    // fixed step is exact enough at second granularity.
    const STEPS: usize = 240;
    let remaining = 1.0 - t;
    for i in 1..=STEPS {
        let dp = remaining * (i as f64 / STEPS as f64);
        let inc = median_curve_at(cycles, (t + dp).min(1.0)) - base;
        if util + inc >= LOCKED_UTIL {
            return Some((dp * window_secs as f64) as i64);
        }
    }
    None
}

/// The single projection switch the engine calls (§C-8). Byte-for-byte the
/// linear `compute_runway` until ≥2 complete cycles exist AND we can place the
/// current instant on the window (`t`); historical otherwise, with a linear
/// fallback whenever the historical curve yields no number (§8 不可倒退).
///
/// Returns `(runway_secs, basis, run_out_probability)`. `run_out_probability`
/// is reported whenever the historical path is eligible, even when runway falls
/// back to linear, so callers that want the probability still have it.
pub fn project_runway(
    cycles: &[Cycle],
    history: &VecDeque<(i64, f64)>,
    util: f64,
    t: Option<f64>,
    window_secs: i64,
) -> (Option<i64>, PaceBasis, Option<f64>) {
    let t = match t {
        Some(t) if cycles.len() >= HIST_MIN_CYCLES && window_secs > 0 => t,
        // Below threshold / unplaceable → the untouched linear path.
        _ => return (compute_runway(history, util), PaceBasis::Linear, None),
    };
    let hp = historical_pace(cycles, t, util, window_secs);
    match hp.runway_secs {
        Some(r) => (Some(r), PaceBasis::Historical, Some(hp.run_out_probability)),
        None => (
            compute_runway(history, util),
            PaceBasis::Linear,
            Some(hp.run_out_probability),
        ),
    }
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
            hint: None,
            action: None,
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

    // ── Historical pace (T-feat-007) ─────────────────────────────────

    /// A flat cycle pinned at `util` for the whole window; `hit_locked` explicit.
    fn flat_cycle(util: f64, hit_locked: bool) -> Cycle {
        Cycle {
            curve: [util; CYCLE_BUCKETS],
            hit_locked,
        }
    }

    /// A cycle ramping linearly from `lo` (progress 0) to `hi` (progress 1).
    fn ramp_cycle(lo: f64, hi: f64) -> Cycle {
        let mut curve = [0.0; CYCLE_BUCKETS];
        for (b, slot) in curve.iter_mut().enumerate() {
            let p = b as f64 / (CYCLE_BUCKETS - 1) as f64;
            *slot = lo + (hi - lo) * p;
        }
        Cycle {
            curve,
            hit_locked: hi >= 100.0,
        }
    }

    /// §D 週期切分:3 段樣本(第三段是稀疏殘段)→ 恰 2 個完整週期。
    #[test]
    fn split_cycles_drops_the_sparse_tail_runt() {
        let w = 1000;
        let mut samples: Vec<(i64, f64)> = Vec::new();
        let mut resets: Vec<(i64, i64)> = Vec::new();
        // Cycle 1: ts 0..=1000 at reset R=5000, util 0→100 (hits locked).
        resets.push((0, 5000));
        for k in 0..=10 {
            samples.push((k * 100, k as f64 * 10.0));
        }
        // Cycle 2: reset advances to 6000 at ts 1100; util restarts 0→90.
        resets.push((1100, 6000));
        for k in 0..=9 {
            samples.push((1100 + k * 100, k as f64 * 10.0));
        }
        // Runt: reset advances again at ts 2100 but only 3 sparse samples (span 200 < 800).
        resets.push((2100, 7000));
        for k in 0..3 {
            samples.push((2100 + k * 100, k as f64 * 5.0));
        }
        let cycles = split_cycles(&samples, &resets, w);
        assert_eq!(cycles.len(), 2, "殘段不得算完整週期");
        assert!(cycles[0].hit_locked, "第一週期撞到 100 應標記 locked");
        assert!(!cycles[1].hit_locked, "第二週期只到 90,未 locked");
    }

    /// §D 歷史 expected:兩條已知曲線 → t=0.5 的中位數斷言。
    #[test]
    fn historical_expected_is_the_median_at_t() {
        // Two flat cycles at 40 and 60 → median at any t is 50.
        let cycles = [flat_cycle(40.0, false), flat_cycle(60.0, false)];
        let hp = historical_pace(&cycles, 0.5, 50.0, 1000);
        assert!((hp.expected_util - 50.0).abs() < 1e-9, "median expected 50");
        // Two ramps 0→80 and 0→100: at t=0.5 they read 40 and 50 → median 45.
        let ramps = [ramp_cycle(0.0, 80.0), ramp_cycle(0.0, 100.0)];
        let hp2 = historical_pace(&ramps, 0.5, 20.0, 1000);
        assert!((hp2.expected_util - 45.0).abs() < 0.5, "median ramps ~45, got {}", hp2.expected_util);
    }

    /// run_out_probability = 撞 locked 的週期比例。
    #[test]
    fn run_out_probability_is_the_locked_fraction() {
        let cycles = [
            flat_cycle(50.0, true),
            flat_cycle(50.0, false),
            flat_cycle(50.0, true),
            flat_cycle(50.0, false),
        ];
        let hp = historical_pace(&cycles, 0.5, 50.0, 1000);
        assert!((hp.run_out_probability - 0.5).abs() < 1e-9);
    }

    /// §D 門檻回歸鎖:1 個完整週期 → basis=linear,runway 與現行 compute_runway 逐位一致。
    #[test]
    fn one_cycle_stays_linear_and_bit_identical() {
        let mut h = VecDeque::new();
        h.push_back((0, 70.0));
        h.push_back((50, 75.0));
        h.push_back((100, 80.0));
        let one = [ramp_cycle(0.0, 100.0)];
        let (runway, basis, prob) = project_runway(&one, &h, 80.0, Some(0.5), 1000);
        assert_eq!(basis, PaceBasis::Linear, "1 週期必須維持線性");
        assert_eq!(prob, None, "未達門檻不報 run_out_probability");
        assert_eq!(runway, compute_runway(&h, 80.0), "runway 必須與現行線性逐位一致");
    }

    /// ≥2 週期 → basis=historical,runway 由歷史曲線得出。
    #[test]
    fn two_cycles_switch_to_historical() {
        let h = VecDeque::new(); // empty linear history: proves the number is historical, not linear
        // Both cycles ramp 0→100 over the window; at t=0.5 base=50. From util=50,
        // reaching 100 needs +50 which the curve delivers at progress 1.0 → runway ~ 0.5*window.
        let cycles = [ramp_cycle(0.0, 100.0), ramp_cycle(0.0, 100.0)];
        let (runway, basis, prob) = project_runway(&cycles, &h, 50.0, Some(0.5), 1000);
        assert_eq!(basis, PaceBasis::Historical);
        assert_eq!(prob, Some(1.0), "兩週期都撞 locked → prob 1.0");
        let r = runway.expect("historical must project here");
        assert!((r - 500).abs() <= 5, "historical runway ~500s, got {r}");
    }

    /// §8 不可倒退:歷史曲線缺料(永不到 100)→ 退回線性 runway,不得空白。
    #[test]
    fn historical_missing_data_falls_back_to_linear() {
        // Cycles top out at 60 → from util 50 the curve never reaches 100.
        let cycles = [flat_cycle(60.0, false), flat_cycle(60.0, false)];
        let mut h = VecDeque::new();
        h.push_back((0, 40.0));
        h.push_back((50, 45.0));
        h.push_back((100, 50.0));
        let (runway, basis, prob) = project_runway(&cycles, &h, 50.0, Some(0.5), 1000);
        assert_eq!(basis, PaceBasis::Linear, "缺料必須退線性");
        assert_eq!(runway, compute_runway(&h, 50.0), "退線性後 runway 用線性值");
        assert_eq!(prob, Some(0.0), "仍回報機率(此處 0)");
    }
}
