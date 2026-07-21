//! Stateful glue: keeps per-limit sample history, derives status/pace/runway,
//! and applies most-dangerous selection to produce a Snapshot for the frontend.

use crate::burnrate::{compute_pace, project_runway, split_cycles, Cycle};
use crate::model::{Limit, LimitStatus, Snapshot};
use crate::ranking::WorstTracker;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

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

// ── Landed quota history (T-feat-007 §A) ─────────────────────────────

/// Retention: samples older than this are pruned on load and on write (§A-3).
const RETAIN_SECS: i64 = 35 * 86_400;
/// …and at most this many reset-delimited cycles are kept per limit (§A-3).
const RETAIN_CYCLES: usize = 5;
/// Landing throttle: record a new sample at most this often per limit, so the
/// file stays small (§A-2「量小」). A reset change or a util move of at least
/// `LANDING_MIN_DELTA` bypasses the throttle so cycle starts/steps aren't lost.
const LANDING_MIN_SECS: i64 = 300;
const LANDING_MIN_DELTA: f64 = 0.5;
/// On-disk format version. Bumped only on a breaking shape change; an unreadable
/// or unknown file is treated as empty (§A-3 壞檔當空).
const HISTORY_FILE_VERSION: u32 = 1;

/// Per-limit landed series: the raw `(ts, util)` samples plus the `(ts, resets_at)`
/// change points. **Privacy (§A-4): only timestamps, util% and reset instants —
/// never a token, project name, model, or any log content.**
#[derive(Serialize, Deserialize, Default, Clone)]
struct LimitLanding {
    #[serde(default)]
    samples: Vec<(i64, f64)>,
    #[serde(default)]
    resets: Vec<(i64, i64)>,
}

#[derive(Serialize, Deserialize, Default)]
struct HistoryFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    limits: HashMap<String, LimitLanding>,
}

/// Disk-backed landing store. Kept out of `Engine::new()` on purpose: the engine
/// tests (and the frozen crosscheck) build a pure in-memory engine, so `store`
/// stays None there and the projection path is byte-for-byte the linear one.
struct HistoryStore {
    path: PathBuf,
    file: HistoryFile,
}

impl HistoryStore {
    /// Load + prune, best-effort. A missing / corrupt / unknown-version file
    /// yields an empty store that re-accumulates from scratch — never a panic.
    fn load(path: &Path, now: i64) -> Self {
        let mut file = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<HistoryFile>(&raw).ok())
            .filter(|f| f.version == HISTORY_FILE_VERSION)
            .unwrap_or_default();
        for landing in file.limits.values_mut() {
            prune_landing(landing, now);
        }
        Self {
            path: path.to_path_buf(),
            file,
        }
    }

    /// Fold one live reading into the landing series. Returns true when the file
    /// changed (so the caller flushes once per ingest). Degraded/stale readings
    /// never reach here — the engine only calls this for live limits, mirroring
    /// the in-memory `history` invariant (`degraded_limits_are_not_recorded_in_history`).
    fn record(&mut self, id: &str, now: i64, util: f64, resets_at: i64) -> bool {
        let landing = self.file.limits.entry(id.to_string()).or_default();
        let mut changed = false;

        // Reset change point: a new window was issued.
        let reset_changed = landing.resets.last().map_or(true, |(_, r)| *r != resets_at);
        if reset_changed {
            landing.resets.push((now, resets_at));
            changed = true;
        }

        // Throttled sample: unless a reset just changed or util moved enough,
        // only record once per LANDING_MIN_SECS to keep the file small.
        let due = match landing.samples.last() {
            None => true,
            Some((t, u)) => {
                reset_changed || now - t >= LANDING_MIN_SECS || (util - u).abs() >= LANDING_MIN_DELTA
            }
        };
        // Guard against duplicate timestamps (same as the in-memory history).
        let dup_ts = landing.samples.last().map_or(false, |(t, _)| *t == now);
        if due && !dup_ts {
            landing.samples.push((now, util));
            changed = true;
        }

        if changed {
            prune_landing(landing, now);
        }
        changed
    }

    /// Complete cycles for a limit at the given window length (empty if unknown).
    fn cycles_for(&self, id: &str, window_secs: i64) -> Vec<Cycle> {
        match self.file.limits.get(id) {
            Some(l) => split_cycles(&l.samples, &l.resets, window_secs),
            None => Vec::new(),
        }
    }

    /// Atomic whole-file rewrite (temp + rename), mirroring the credentials
    /// write-back in providers/anthropic.rs. Best-effort: an IO error just means
    /// the next successful flush persists the accumulated series.
    fn flush(&mut self) {
        self.file.version = HISTORY_FILE_VERSION;
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = self.path.with_extension("json.tmp");
        if serde_json::to_string(&self.file)
            .ok()
            .and_then(|s| std::fs::write(&tmp, s).ok())
            .is_some()
        {
            let _ = std::fs::rename(&tmp, &self.path);
        }
    }
}

/// Prune a landing series to the retention bound (§A-3): drop samples older than
/// 35 days, then keep only the most recent `RETAIN_CYCLES` reset-delimited
/// cycles. Reset change points are pruned in step so the two stay consistent.
fn prune_landing(landing: &mut LimitLanding, now: i64) {
    let cutoff = now - RETAIN_SECS;
    landing.samples.retain(|(t, _)| *t >= cutoff);
    landing.resets.retain(|(t, _)| *t >= cutoff);

    // 5-cycle bound, approximated by reset boundaries: if more than RETAIN_CYCLES
    // resets survive, drop everything before the start of the RETAIN_CYCLES-th
    // most recent one. (35-day is the hard bound; this is the secondary cap.)
    if landing.resets.len() > RETAIN_CYCLES {
        let keep_from = landing.resets[landing.resets.len() - RETAIN_CYCLES].0;
        landing.samples.retain(|(t, _)| *t >= keep_from);
        landing.resets.retain(|(t, _)| *t >= keep_from);
    }
}

pub struct Engine {
    history: HashMap<String, VecDeque<(i64, f64)>>,
    tracker: WorstTracker,
    /// Disk-backed landing; None keeps the engine pure (tests / crosscheck).
    store: Option<HistoryStore>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            history: HashMap::new(),
            tracker: WorstTracker::default(),
            store: None,
        }
    }

    /// Attach the on-disk landing store (`%APPDATA%\Atoll\quota-history.json`),
    /// loading + pruning any existing history. Called once by the scheduler; the
    /// engine stays in-memory-only when no config dir is available.
    pub fn attach_disk_history(&mut self, now: i64) {
        if let Some(dir) = dirs::config_dir() {
            let path = dir.join("Atoll").join("quota-history.json");
            self.store = Some(HistoryStore::load(&path, now));
        }
    }

    /// Window progress 0..1 for a limit (same placement `compute_pace` uses), or
    /// None when the window is unknown — historical projection needs this to know
    /// *where on the curve* we are.
    fn window_progress(l: &Limit, now: i64) -> Option<f64> {
        if l.window_secs <= 0 || l.resets_at <= 0 {
            return None;
        }
        let start = l.resets_at - l.window_secs;
        let elapsed = (now - start) as f64;
        if elapsed <= 0.0 {
            return None;
        }
        Some((elapsed / l.window_secs as f64).clamp(0.0, 1.0))
    }

    /// Fold a fresh set of limits into state and return the snapshot to emit.
    pub fn ingest(&mut self, mut limits: Vec<Limit>, now: i64) -> Snapshot {
        let mut store_dirty = false;
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

                // Land the live sample to disk (§A) — same degraded-excluded
                // invariant as the in-memory history above. Record first so the
                // freshest sample is part of the cycle split below.
                if let Some(store) = self.store.as_mut() {
                    if store.record(&l.id, now, l.util, l.resets_at) {
                        store_dirty = true;
                    }
                }

                // Linear vs historical projection (§C-8). With no store (tests /
                // <2 cycles / no window) this is byte-for-byte the old
                // `compute_runway` linear path, and pace_basis stays Linear.
                let cycles = self
                    .store
                    .as_ref()
                    .map(|s| s.cycles_for(&l.id, l.window_secs))
                    .unwrap_or_default();
                let t = Self::window_progress(l, now);
                let (runway, basis, prob) =
                    project_runway(&cycles, hist, l.util, t, l.window_secs);
                l.runway_secs = runway;
                if let Some(pace) = l.pace.as_mut() {
                    pace.pace_basis = basis;
                    pace.run_out_probability = prob;
                }
            }
        }

        if store_dirty {
            if let Some(store) = self.store.as_mut() {
                store.flush();
            }
        }

        let worst_id = self.tracker.select(&limits, now);
        Snapshot {
            limits,
            worst_id,
            updated_at: now,
            next_fetch_in: 0, // scheduler overwrites with the real cadence
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
            action: None,
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

    // ── Landed history persistence (T-feat-007 §A) ───────────────────

    /// Unique temp path per test so parallel runs don't collide.
    fn temp_history_path(tag: &str) -> PathBuf {
        let name = format!(
            "atoll-hist-{tag}-{:x}.json",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        std::env::temp_dir().join(name)
    }

    /// §D 落地 round-trip:寫 → 載 → 修剪(35 天界)。舊於 35 天的樣本必須在
    /// 載入時被剪掉,較新的保留;且不 panic。
    #[test]
    fn landed_history_round_trips_and_prunes_at_35_days() {
        let path = temp_history_path("roundtrip");
        let now: i64 = 1_800_000_000;
        let day = 86_400i64;

        // Write a series spanning >35 days: one ancient sample and recent ones.
        {
            let mut store = HistoryStore::load(&path, now - 40 * day);
            // Old sample, 40 days before `now` (should not survive a load at `now`).
            store.record("codex.5h", now - 40 * day, 10.0, 5000);
            // Recent samples within the window.
            store.record("codex.5h", now - 2 * day, 20.0, 5000);
            store.record("codex.5h", now - day, 30.0, 5000);
            store.record("codex.5h", now, 40.0, 5000);
            store.flush();
        }
        assert!(path.is_file(), "history file must be written");

        // Reload at `now`: the 40-day-old sample is pruned, recents remain.
        let reloaded = HistoryStore::load(&path, now);
        let landing = reloaded
            .file
            .limits
            .get("codex.5h")
            .expect("limit must round-trip");
        assert!(
            landing.samples.iter().all(|(t, _)| *t >= now - RETAIN_SECS),
            "35 天以外的樣本必須被修剪:{:?}",
            landing.samples
        );
        assert!(
            landing.samples.iter().any(|(t, _)| *t == now),
            "視窗內的樣本必須保留"
        );
        assert!(
            !landing.samples.iter().any(|(t, _)| *t == now - 40 * day),
            "40 天前的樣本不該還在"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// §D 壞檔:亂寫 JSON → 空歷史、不 panic,且之後仍能正常累積並覆寫成有效檔。
    #[test]
    fn corrupt_history_file_is_treated_as_empty_without_panicking() {
        let path = temp_history_path("corrupt");
        std::fs::write(&path, b"{ this is not valid json ]]").unwrap();

        let now: i64 = 1_800_000_000;
        let mut store = HistoryStore::load(&path, now);
        assert!(store.file.limits.is_empty(), "壞檔必須當空歷史");

        // Accumulation still works and rewrites a valid file.
        store.record("cc.5h", now, 55.0, 6000);
        store.flush();
        let reloaded = HistoryStore::load(&path, now);
        assert!(
            reloaded.file.limits.contains_key("cc.5h"),
            "壞檔覆寫後必須能正常讀回"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Landing must exclude degraded readings, mirroring the in-memory invariant:
    /// a store-backed engine fed only degraded limits records nothing.
    #[test]
    fn store_backed_engine_does_not_land_degraded_samples() {
        let path = temp_history_path("degraded");
        let now: i64 = 1_800_000_000;
        let mut e = Engine::new();
        e.store = Some(HistoryStore::load(&path, now));
        for status in [LimitStatus::SourceFailed, LimitStatus::Stale, LimitStatus::Idle] {
            e.ingest(vec![limit_with_status("a", 0.0, status)], now);
        }
        let landed = e
            .store
            .as_ref()
            .and_then(|s| s.file.limits.get("a"))
            .map_or(true, |l| l.samples.is_empty());
        assert!(landed, "degraded 樣本不得落地");
        let _ = std::fs::remove_file(&path);
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
