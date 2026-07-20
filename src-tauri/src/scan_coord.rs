//! Stage 1B analytics scan coordinator.
//!
//! - **Fingerprint cache** (T-perf-001) — same `sources|range` serves the
//!   cached result while the underlying session-log files are unchanged
//!   `(path, mtime, len)`; a changed fingerprint invalidates immediately,
//!   ignoring the TTL. `ANALYTICS_TTL` is now only a safety ceiling for the
//!   pathological case where content-relevant state changes without moving
//!   any file's mtime.
//! - **In-flight coalesce** — concurrent identical keys share one result
//! - **Mutual exclusion** — at most one full scan body at a time
//! - **Latest-request-wins queue** — while busy, at most one pending job; a
//!   newer different key cancels the previous pending waiters with `Superseded`
//! - **Non-blocking promote** — pending runs on a worker; leader returns first
//! - **Panic / spawn isolation** — scan panics and worker-spawn failures become
//!   `ScanFailed` and restore coordinator state
//! - **`force`** — bypasses the cache read entirely (still coalesces with any
//!   identical in-flight request) and always refreshes the stored fingerprint

use crate::analytics::{self, Analytics};
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Safety-ceiling TTL — no longer the primary invalidation path (see module
/// docs). Kept short: it only matters when the fingerprint fails to move.
const ANALYTICS_TTL: Duration = Duration::from_secs(60);

/// Stable error codes for Tauri + frontend (snake_case over the wire).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsErrorCode {
    Superseded,
    Cancelled,
    ScanFailed,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct AnalyticsError {
    pub code: AnalyticsErrorCode,
    pub message: String,
}

impl AnalyticsError {
    pub fn superseded() -> Self {
        Self {
            code: AnalyticsErrorCode::Superseded,
            message: "analytics request superseded".into(),
        }
    }
    pub fn cancelled() -> Self {
        Self {
            code: AnalyticsErrorCode::Cancelled,
            message: "analytics request cancelled".into(),
        }
    }
    pub fn scan_failed(message: impl Into<String>) -> Self {
        Self {
            code: AnalyticsErrorCode::ScanFailed,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for AnalyticsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Tauri surfaces command errors as strings; embed the code so the
        // frontend can decode without depending on the human message alone.
        write!(f, "analytics_error:{}:{}", code_str(&self.code), self.message)
    }
}

fn code_str(c: &AnalyticsErrorCode) -> &'static str {
    match c {
        AnalyticsErrorCode::Superseded => "superseded",
        AnalyticsErrorCode::Cancelled => "cancelled",
        AnalyticsErrorCode::ScanFailed => "scan_failed",
    }
}

/// Parse a Tauri-surfaced error string back into a structured code (tests only;
/// production frontend uses its own decoder in `analytics-error.ts`).
#[cfg(test)]
pub fn parse_analytics_error_display(s: &str) -> Option<AnalyticsErrorCode> {
    let rest = s.strip_prefix("analytics_error:")?;
    let code = rest.split(':').next()?;
    match code {
        "superseded" => Some(AnalyticsErrorCode::Superseded),
        "cancelled" => Some(AnalyticsErrorCode::Cancelled),
        "scan_failed" => Some(AnalyticsErrorCode::ScanFailed),
        _ => None,
    }
}

type Waiter = std::sync::mpsc::Sender<Result<Analytics, AnalyticsError>>;
type ScanFn = Arc<dyn Fn(&str, &[String]) -> Result<Analytics, AnalyticsError> + Send + Sync>;
/// Spawn a background job. Returns Err if the worker cannot be started.
type SpawnFn = Arc<dyn Fn(Box<dyn FnOnce() + Send>) -> Result<(), AnalyticsError> + Send + Sync>;
/// Fingerprint of the source files a `(range, sources)` scan would read.
type FingerprintFn = Arc<dyn Fn(&str, &[String]) -> u64 + Send + Sync>;

fn default_spawn_fn() -> SpawnFn {
    Arc::new(|job| {
        std::thread::Builder::new()
            .name("atoll-analytics".into())
            .spawn(job)
            .map(|_| ())
            .map_err(|e| AnalyticsError::scan_failed(format!("worker spawn failed: {e}")))
    })
}

fn default_fingerprint_fn() -> FingerprintFn {
    Arc::new(|range, sources| analytics::source_fingerprint(range, sources))
}

struct InflightEntry {
    waiters: Vec<Waiter>,
}

struct PendingJob {
    key: String,
    range: String,
    sources: Vec<String>,
    waiters: Vec<Waiter>,
    /// True if any coalesced waiter asked for `force`; OR-ed in so one forced
    /// request among several queued for the same key still forces the promoted
    /// scan to bypass the cache.
    force: bool,
}

struct Inner {
    cache: HashMap<String, (Instant, u64, Analytics)>,
    inflight: HashMap<String, InflightEntry>,
    busy: bool,
    pending: Option<PendingJob>,
}

#[derive(Clone)]
pub struct ScanCoordinator {
    inner: Arc<Mutex<Inner>>,
    scan_fn: ScanFn,
    spawn_fn: SpawnFn,
    fingerprint_fn: FingerprintFn,
    scan_gate: Arc<Mutex<()>>,
}

impl Default for ScanCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl ScanCoordinator {
    pub fn new() -> Self {
        Self::with_hooks(
            Arc::new(|range, sources| Ok(analytics::compute_with(range, sources))),
            default_spawn_fn(),
            default_fingerprint_fn(),
        )
    }

    /// Test convenience: real scan behaviour, fixed fingerprint (`0` for every
    /// key) so cache-hit/TTL/coalesce tests don't depend on the filesystem.
    /// Tests that need fingerprint semantics use `with_hooks` directly.
    #[cfg(test)]
    pub fn with_scan(scan_fn: ScanFn) -> Self {
        Self::with_hooks(
            scan_fn,
            Arc::new(|job| {
                std::thread::Builder::new()
                    .name("atoll-analytics".into())
                    .spawn(job)
                    .map(|_| ())
                    .map_err(|e| AnalyticsError::scan_failed(format!("worker spawn failed: {e}")))
            }),
            Arc::new(|_, _| 0),
        )
    }

    pub fn with_hooks(scan_fn: ScanFn, spawn_fn: SpawnFn, fingerprint_fn: FingerprintFn) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                cache: HashMap::new(),
                inflight: HashMap::new(),
                busy: false,
                pending: None,
            })),
            scan_fn,
            spawn_fn,
            fingerprint_fn,
            scan_gate: Arc::new(Mutex::new(())),
        }
    }

    pub fn get(
        &self,
        range: String,
        sources: Vec<String>,
        force: bool,
    ) -> Result<Analytics, AnalyticsError> {
        let key = cache_key(&sources, &range);
        // Computed once up front and reused for both the cache check and the
        // value stored after a recompute — see `run_job`. `force` skips only
        // the cache *read*; the coalesce/leader-follower machinery below is
        // unchanged, so concurrent forced + normal requests for the same key
        // still share one scan.
        let fp = (self.fingerprint_fn)(&range, &sources);

        if !force {
            if let Some(hit) = self.cache_get(&key, fp) {
                return Ok(hit);
            }
        }

        enum Role {
            Leader {
                range: String,
                sources: Vec<String>,
                force: bool,
            },
            Follower(std::sync::mpsc::Receiver<Result<Analytics, AnalyticsError>>),
        }

        let role = {
            let mut g = lock(&self.inner);

            if !force {
                if let Some(hit) = cache_lookup(&g.cache, &key, fp) {
                    return Ok(hit);
                }
            }

            if let Some(entry) = g.inflight.get_mut(&key) {
                let (tx, rx) = std::sync::mpsc::channel();
                entry.waiters.push(tx);
                Role::Follower(rx)
            } else if g.busy {
                let (tx, rx) = std::sync::mpsc::channel();
                match &mut g.pending {
                    Some(p) if p.key == key => {
                        p.range = range;
                        p.sources = sources;
                        p.waiters.push(tx);
                        p.force = p.force || force;
                    }
                    Some(p) => {
                        cancel_waiters(std::mem::take(&mut p.waiters), AnalyticsError::superseded());
                        *p = PendingJob {
                            key: key.clone(),
                            range,
                            sources,
                            waiters: vec![tx],
                            force,
                        };
                    }
                    None => {
                        g.pending = Some(PendingJob {
                            key: key.clone(),
                            range,
                            sources,
                            waiters: vec![tx],
                            force,
                        });
                    }
                }
                Role::Follower(rx)
            } else {
                g.busy = true;
                g.inflight
                    .insert(key.clone(), InflightEntry { waiters: vec![] });
                Role::Leader { range, sources, force }
            }
        };

        match role {
            Role::Follower(rx) => rx.recv().map_err(|_| AnalyticsError::cancelled())?,
            Role::Leader { range, sources, force } => self.run_job(key, range, sources, force),
        }
    }

    fn cache_get(&self, key: &str, fp: u64) -> Option<Analytics> {
        let g = lock(&self.inner);
        cache_lookup(&g.cache, key, fp)
    }

    pub fn invalidate_all(&self) {
        let mut g = lock(&self.inner);
        g.cache.clear();
    }

    fn run_job(
        &self,
        key: String,
        range: String,
        sources: Vec<String>,
        force: bool,
    ) -> Result<Analytics, AnalyticsError> {
        // Fingerprinted independently of whatever `get()` computed: a job
        // promoted from the pending queue never goes through `get()` at all,
        // and by the time this runs the `(range, sources)` may have been
        // merged/updated by later coalesced callers.
        let fp = (self.fingerprint_fn)(&range, &sources);
        let result = {
            let _gate = self
                .scan_gate
                .lock()
                .unwrap_or_else(|p| p.into_inner());

            if !force {
                if let Some(hit) = self.cache_get(&key, fp) {
                    drop(_gate);
                    self.finish_job(&key, Ok(hit.clone()));
                    return Ok(hit);
                }
            }

            let scan = Arc::clone(&self.scan_fn);
            let range_c = range.clone();
            let sources_c = sources.clone();
            match catch_unwind(AssertUnwindSafe(|| scan(&range_c, &sources_c))) {
                Ok(r) => r,
                Err(_) => Err(AnalyticsError::scan_failed("analytics scan panicked")),
            }
        };

        if let Ok(ref a) = result {
            let mut g = lock(&self.inner);
            g.cache.insert(key.clone(), (Instant::now(), fp, a.clone()));
        }

        self.finish_job(&key, result.clone());
        result
    }

    fn finish_job(&self, key: &str, result: Result<Analytics, AnalyticsError>) {
        let waiters = {
            let mut g = lock(&self.inner);
            g.inflight
                .remove(key)
                .map(|e| e.waiters)
                .unwrap_or_default()
        };
        for w in waiters {
            let _ = w.send(result.clone());
        }

        let promote = {
            let mut g = lock(&self.inner);
            match g.pending.take() {
                None => {
                    g.busy = false;
                    None
                }
                Some(job) => {
                    g.busy = true;
                    g.inflight.insert(
                        job.key.clone(),
                        InflightEntry {
                            waiters: job.waiters,
                        },
                    );
                    Some((job.key, job.range, job.sources, job.force))
                }
            }
        };

        if let Some((key, range, sources, force)) = promote {
            let this = self.clone();
            let spawn = Arc::clone(&self.spawn_fn);
            let fail_key = key.clone();
            if let Err(e) = spawn(Box::new(move || {
                let _ = this.run_job(key, range, sources, force);
            })) {
                // Spawn failed: resolve waiters and restore idle state.
                self.fail_promote(&fail_key, e);
            }
        }
    }

    fn fail_promote(&self, key: &str, err: AnalyticsError) {
        let waiters = {
            let mut g = lock(&self.inner);
            let w = g
                .inflight
                .remove(key)
                .map(|e| e.waiters)
                .unwrap_or_default();
            // Also clear any newer pending so we do not leave a zombie busy flag.
            if let Some(p) = g.pending.take() {
                cancel_waiters(p.waiters, err.clone());
            }
            g.busy = false;
            w
        };
        for w in waiters {
            let _ = w.send(Err(err.clone()));
        }
    }
}

fn cancel_waiters(waiters: Vec<Waiter>, err: AnalyticsError) {
    for w in waiters {
        let _ = w.send(Err(err.clone()));
    }
}

fn lock(m: &Mutex<Inner>) -> std::sync::MutexGuard<'_, Inner> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn cache_key(sources: &[String], range: &str) -> String {
    let mut src: Vec<&str> = sources.iter().map(|s| s.as_str()).collect();
    src.sort_unstable();
    format!("{}|{}", src.join(","), range)
}

/// Hit only if the fingerprint still matches *and* the TTL ceiling hasn't
/// passed. A changed fingerprint is an immediate miss regardless of TTL —
/// per T-perf-001, the fingerprint is the primary invalidation signal and TTL
/// is only a safety fallback for the case where content-relevant state
/// changes without any source file's mtime moving.
fn cache_lookup(
    cache: &HashMap<String, (Instant, u64, Analytics)>,
    key: &str,
    fp: u64,
) -> Option<Analytics> {
    let (t, cached_fp, a) = cache.get(key)?;
    if *cached_fp != fp {
        return None;
    }
    if t.elapsed() > ANALYTICS_TTL {
        return None;
    }
    Some(a.clone())
}

pub fn sources_equal(a: &[String], b: &[String]) -> bool {
    let mut aa: Vec<&str> = a.iter().map(|s| s.as_str()).collect();
    let mut bb: Vec<&str> = b.iter().map(|s| s.as_str()).collect();
    aa.sort_unstable();
    bb.sort_unstable();
    aa == bb
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    fn sample_analytics(range: &str) -> Analytics {
        Analytics {
            range: range.into(),
            range_start_day: "2026-07-01".into(),
            total_tokens: 1,
            total_cost_usd: 0.0,
            best_day: analytics::BestDay {
                date: String::new(),
                cost_usd: 0.0,
            },
            active_days: 0,
            records: Default::default(),
            daily: vec![],
            hourly: vec![0; 24],
            hourly_cost: vec![0.0; 24],
            by_model: HashMap::new(),
            by_agent: HashMap::new(),
            by_model_cost: HashMap::new(),
            by_agent_cost: HashMap::new(),
            breakdown: analytics::Breakdown {
                input: 0,
                cached: 0,
                output: 0,
                reasoning: 0,
            },
            by_kind: vec![],
            by_project: vec![],
            sessions_this_week: 0,
            tok_per_min: 0,
            accounts: vec![],
        }
    }

    fn slow_ok(delay_ms: u64) -> ScanFn {
        Arc::new(move |range, _| {
            thread::sleep(Duration::from_millis(delay_ms));
            Ok(sample_analytics(range))
        })
    }

    fn default_spawn() -> SpawnFn {
        Arc::new(|job| {
            std::thread::Builder::new()
                .spawn(job)
                .map(|_| ())
                .map_err(|e| AnalyticsError::scan_failed(e.to_string()))
        })
    }

    #[test]
    fn superseded_returns_typed_code() {
        let coord = ScanCoordinator::with_scan(slow_ok(80));
        let c = coord.clone();
        let leader = thread::spawn(move || c.get("today".into(), vec!["claude".into()], false));
        thread::sleep(Duration::from_millis(15));
        let c2 = coord.clone();
        let week = thread::spawn(move || c2.get("week".into(), vec!["claude".into()], false));
        thread::sleep(Duration::from_millis(10));
        let c3 = coord.clone();
        let month = thread::spawn(move || c3.get("month".into(), vec!["claude".into()], false));
        leader.join().unwrap().unwrap();
        let week_err = match week.join().unwrap() {
            Err(e) => e,
            Ok(_) => panic!("expected superseded"),
        };
        assert_eq!(week_err.code, AnalyticsErrorCode::Superseded);
        assert_eq!(month.join().unwrap().unwrap().range, "month");
    }

    #[test]
    fn scan_failed_is_typed_not_string_match() {
        let coord = ScanCoordinator::with_scan(Arc::new(|_, _| {
            Err(AnalyticsError::scan_failed("disk full"))
        }));
        let err = match coord.get("today".into(), vec!["claude".into()], false) {
            Err(e) => e,
            Ok(_) => panic!("expected scan_failed"),
        };
        assert_eq!(err.code, AnalyticsErrorCode::ScanFailed);
        assert!(err.message.contains("disk full"));
    }

    #[test]
    fn spawn_failure_resolves_waiters_and_recovers() {
        let spawn_calls = Arc::new(AtomicUsize::new(0));
        let spawn_calls2 = Arc::clone(&spawn_calls);
        let spawn: SpawnFn = Arc::new(move |_job| {
            spawn_calls2.fetch_add(1, Ordering::SeqCst);
            Err(AnalyticsError::scan_failed("spawn denied"))
        });
        let coord = ScanCoordinator::with_hooks(slow_ok(40), spawn, Arc::new(|_, _| 0));
        let c = coord.clone();
        let leader = thread::spawn(move || c.get("today".into(), vec!["claude".into()], false));
        thread::sleep(Duration::from_millis(10));
        let c2 = coord.clone();
        let pending = thread::spawn(move || c2.get("week".into(), vec!["claude".into()], false));
        assert_eq!(leader.join().unwrap().unwrap().range, "today");
        let err = match pending.join().unwrap() {
            Err(e) => e,
            Ok(_) => panic!("expected spawn fail on pending"),
        };
        assert_eq!(err.code, AnalyticsErrorCode::ScanFailed);
        assert!(spawn_calls.load(Ordering::SeqCst) >= 1);
        // Coordinator reusable
        let ok = coord
            .get("month".into(), vec!["claude".into()], false)
            .expect("after spawn fail");
        assert_eq!(ok.range, "month");
    }

    #[test]
    fn panic_surfaces_scan_failed_then_recovers() {
        let n = Arc::new(AtomicUsize::new(0));
        let n2 = Arc::clone(&n);
        let coord = ScanCoordinator::with_scan(Arc::new(move |range, _| {
            if n2.fetch_add(1, Ordering::SeqCst) == 0 {
                panic!("boom");
            }
            Ok(sample_analytics(range))
        }));
        let err = match coord.get("today".into(), vec!["claude".into()], false) {
            Err(e) => e,
            Ok(_) => panic!("expected panic as ScanFailed"),
        };
        assert_eq!(err.code, AnalyticsErrorCode::ScanFailed);
        assert_eq!(
            coord
                .get("today".into(), vec!["claude".into()], false)
                .unwrap()
                .range,
            "today"
        );
    }

    #[test]
    fn display_roundtrips_code() {
        let e = AnalyticsError::superseded();
        assert_eq!(
            parse_analytics_error_display(&e.to_string()),
            Some(AnalyticsErrorCode::Superseded)
        );
        let e = AnalyticsError::scan_failed("x");
        assert_eq!(
            parse_analytics_error_display(&e.to_string()),
            Some(AnalyticsErrorCode::ScanFailed)
        );
    }

    #[test]
    fn coalesce_same_key_scans_once() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls2 = Arc::clone(&calls);
        let coord = ScanCoordinator::with_scan(Arc::new(move |range, _| {
            calls2.fetch_add(1, Ordering::SeqCst);
            thread::sleep(Duration::from_millis(50));
            Ok(sample_analytics(range))
        }));
        let mut hs = vec![];
        for _ in 0..4 {
            let c = coord.clone();
            hs.push(thread::spawn(move || {
                c.get("week".into(), vec!["claude".into()], false).unwrap()
            }));
        }
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn leader_returns_before_pending_finishes() {
        let coord = ScanCoordinator::with_hooks(
            Arc::new(|range, _| {
                if range == "today" {
                    thread::sleep(Duration::from_millis(30));
                } else {
                    thread::sleep(Duration::from_millis(200));
                }
                Ok(sample_analytics(range))
            }),
            default_spawn(),
            Arc::new(|_, _| 0),
        );
        let c = coord.clone();
        let leader = thread::spawn(move || c.get("today".into(), vec!["claude".into()], false));
        thread::sleep(Duration::from_millis(5));
        let c2 = coord.clone();
        let _p = thread::spawn(move || c2.get("month".into(), vec!["claude".into()], false));
        let t0 = Instant::now();
        assert_eq!(leader.join().unwrap().unwrap().range, "today");
        assert!(t0.elapsed().as_millis() < 150);
    }

    #[test]
    fn sources_equal_ignores_order() {
        assert!(sources_equal(
            &["codex".into(), "claude".into()],
            &["claude".into(), "codex".into()]
        ));
    }

    // ── T-perf-001: fingerprint-driven invalidation ──────────────────────

    #[test]
    fn same_fingerprint_serves_cache_within_ttl() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls2 = Arc::clone(&calls);
        let coord = ScanCoordinator::with_hooks(
            Arc::new(move |range, _| {
                calls2.fetch_add(1, Ordering::SeqCst);
                Ok(sample_analytics(range))
            }),
            default_spawn(),
            // Source files never change between calls.
            Arc::new(|_, _| 42),
        );
        coord.get("week".into(), vec!["claude".into()], false).unwrap();
        coord.get("week".into(), vec!["claude".into()], false).unwrap();
        coord.get("week".into(), vec!["claude".into()], false).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn changed_fingerprint_forces_rescan_ignoring_ttl() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls2 = Arc::clone(&calls);
        let seen_fp = Arc::new(AtomicUsize::new(0));
        let seen_fp2 = Arc::clone(&seen_fp);
        let coord = ScanCoordinator::with_hooks(
            Arc::new(move |range, _| {
                calls2.fetch_add(1, Ordering::SeqCst);
                Ok(sample_analytics(range))
            }),
            default_spawn(),
            // Each call reports a new fingerprint, as if a source log's mtime
            // moved between requests — well within the 60s TTL.
            Arc::new(move |_, _| seen_fp2.fetch_add(1, Ordering::SeqCst) as u64),
        );
        coord.get("week".into(), vec!["claude".into()], false).unwrap();
        coord.get("week".into(), vec!["claude".into()], false).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn force_rescans_even_with_unchanged_fingerprint() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls2 = Arc::clone(&calls);
        let coord = ScanCoordinator::with_hooks(
            Arc::new(move |range, _| {
                calls2.fetch_add(1, Ordering::SeqCst);
                Ok(sample_analytics(range))
            }),
            default_spawn(),
            Arc::new(|_, _| 7),
        );
        coord.get("week".into(), vec!["claude".into()], false).unwrap();
        coord.get("week".into(), vec!["claude".into()], true).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        // And the refreshed fingerprint is cached: a normal follow-up hits it.
        coord.get("week".into(), vec!["claude".into()], false).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
