//! Stage 1B analytics scan coordinator.
//!
//! - **TTL cache** — same `sources|range` within the window never re-parses
//! - **In-flight coalesce** — concurrent identical keys share one result
//! - **Mutual exclusion** — at most one full scan body at a time
//! - **Latest-request-wins queue** — while busy, at most one pending job; a
//!   newer different key cancels the previous pending waiters with `Superseded`
//! - **Non-blocking promote** — pending runs on a worker; leader returns first
//! - **Panic / spawn isolation** — scan panics and worker-spawn failures become
//!   `ScanFailed` and restore coordinator state

use crate::analytics::{self, Analytics};
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Short process-local cache.
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

fn default_spawn_fn() -> SpawnFn {
    Arc::new(|job| {
        std::thread::Builder::new()
            .name("atoll-analytics".into())
            .spawn(job)
            .map(|_| ())
            .map_err(|e| AnalyticsError::scan_failed(format!("worker spawn failed: {e}")))
    })
}

struct InflightEntry {
    waiters: Vec<Waiter>,
}

struct PendingJob {
    key: String,
    range: String,
    sources: Vec<String>,
    waiters: Vec<Waiter>,
}

struct Inner {
    cache: HashMap<String, (Instant, Analytics)>,
    inflight: HashMap<String, InflightEntry>,
    busy: bool,
    pending: Option<PendingJob>,
}

#[derive(Clone)]
pub struct ScanCoordinator {
    inner: Arc<Mutex<Inner>>,
    scan_fn: ScanFn,
    spawn_fn: SpawnFn,
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
        )
    }

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
        )
    }

    pub fn with_hooks(scan_fn: ScanFn, spawn_fn: SpawnFn) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                cache: HashMap::new(),
                inflight: HashMap::new(),
                busy: false,
                pending: None,
            })),
            scan_fn,
            spawn_fn,
            scan_gate: Arc::new(Mutex::new(())),
        }
    }

    pub fn get(&self, range: String, sources: Vec<String>) -> Result<Analytics, AnalyticsError> {
        let key = cache_key(&sources, &range);

        if let Some(hit) = self.cache_get(&key) {
            return Ok(hit);
        }

        enum Role {
            Leader { range: String, sources: Vec<String> },
            Follower(std::sync::mpsc::Receiver<Result<Analytics, AnalyticsError>>),
        }

        let role = {
            let mut g = lock(&self.inner);

            if let Some(hit) = cache_lookup(&g.cache, &key) {
                return Ok(hit);
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
                    }
                    Some(p) => {
                        cancel_waiters(std::mem::take(&mut p.waiters), AnalyticsError::superseded());
                        *p = PendingJob {
                            key: key.clone(),
                            range,
                            sources,
                            waiters: vec![tx],
                        };
                    }
                    None => {
                        g.pending = Some(PendingJob {
                            key: key.clone(),
                            range,
                            sources,
                            waiters: vec![tx],
                        });
                    }
                }
                Role::Follower(rx)
            } else {
                g.busy = true;
                g.inflight
                    .insert(key.clone(), InflightEntry { waiters: vec![] });
                Role::Leader { range, sources }
            }
        };

        match role {
            Role::Follower(rx) => rx.recv().map_err(|_| AnalyticsError::cancelled())?,
            Role::Leader { range, sources } => self.run_job(key, range, sources),
        }
    }

    fn cache_get(&self, key: &str) -> Option<Analytics> {
        let g = lock(&self.inner);
        cache_lookup(&g.cache, key)
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
    ) -> Result<Analytics, AnalyticsError> {
        let result = {
            let _gate = self
                .scan_gate
                .lock()
                .unwrap_or_else(|p| p.into_inner());

            if let Some(hit) = self.cache_get(&key) {
                drop(_gate);
                self.finish_job(&key, Ok(hit.clone()));
                return Ok(hit);
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
            g.cache.insert(key.clone(), (Instant::now(), a.clone()));
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
                    Some((job.key, job.range, job.sources))
                }
            }
        };

        if let Some((key, range, sources)) = promote {
            let this = self.clone();
            let spawn = Arc::clone(&self.spawn_fn);
            let fail_key = key.clone();
            if let Err(e) = spawn(Box::new(move || {
                let _ = this.run_job(key, range, sources);
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

fn cache_lookup(cache: &HashMap<String, (Instant, Analytics)>, key: &str) -> Option<Analytics> {
    let (t, a) = cache.get(key)?;
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
        let leader = thread::spawn(move || c.get("today".into(), vec!["claude".into()]));
        thread::sleep(Duration::from_millis(15));
        let c2 = coord.clone();
        let week = thread::spawn(move || c2.get("week".into(), vec!["claude".into()]));
        thread::sleep(Duration::from_millis(10));
        let c3 = coord.clone();
        let month = thread::spawn(move || c3.get("month".into(), vec!["claude".into()]));
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
        let err = match coord.get("today".into(), vec!["claude".into()]) {
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
        let coord = ScanCoordinator::with_hooks(slow_ok(40), spawn);
        let c = coord.clone();
        let leader = thread::spawn(move || c.get("today".into(), vec!["claude".into()]));
        thread::sleep(Duration::from_millis(10));
        let c2 = coord.clone();
        let pending = thread::spawn(move || c2.get("week".into(), vec!["claude".into()]));
        assert_eq!(leader.join().unwrap().unwrap().range, "today");
        let err = match pending.join().unwrap() {
            Err(e) => e,
            Ok(_) => panic!("expected spawn fail on pending"),
        };
        assert_eq!(err.code, AnalyticsErrorCode::ScanFailed);
        assert!(spawn_calls.load(Ordering::SeqCst) >= 1);
        // Coordinator reusable
        let ok = coord
            .get("month".into(), vec!["claude".into()])
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
        let err = match coord.get("today".into(), vec!["claude".into()]) {
            Err(e) => e,
            Ok(_) => panic!("expected panic as ScanFailed"),
        };
        assert_eq!(err.code, AnalyticsErrorCode::ScanFailed);
        assert_eq!(
            coord
                .get("today".into(), vec!["claude".into()])
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
                c.get("week".into(), vec!["claude".into()]).unwrap()
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
        );
        let c = coord.clone();
        let leader = thread::spawn(move || c.get("today".into(), vec!["claude".into()]));
        thread::sleep(Duration::from_millis(5));
        let c2 = coord.clone();
        let _p = thread::spawn(move || c2.get("month".into(), vec!["claude".into()]));
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
}
