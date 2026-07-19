//! Stage 1B analytics scan coordinator (partial → tightened after review).
//!
//! - **TTL cache** — same `sources|range` within the window never re-parses
//! - **In-flight coalesce** — concurrent identical keys share one result
//! - **Mutual exclusion** — at most one full scan body at a time
//! - **Latest-request-wins queue** — while busy, at most one pending job; a
//!   newer different key cancels the previous pending waiters
//! - **Non-blocking promote** — after the leader finishes, pending runs on a
//!   worker thread so the leader returns immediately
//! - **Panic isolation** — scan panics become `Err` and reset busy/inflight
//!
//! Month-from-narrow **derive** is still not implemented; queue priority is
//! latest pending key only (not "always month first").

use crate::analytics::{self, Analytics};
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Short process-local cache. Long enough to fold island 60s refresh + tab
/// flips; short enough that a real session keeps data feeling live.
const ANALYTICS_TTL: Duration = Duration::from_secs(60);

type Waiter = std::sync::mpsc::Sender<Result<Analytics, String>>;
type ScanFn = Arc<dyn Fn(&str, &[String]) -> Result<Analytics, String> + Send + Sync>;

struct InflightEntry {
    waiters: Vec<Waiter>,
}

/// Single pending job (global latest-wins while a scan is busy).
struct PendingJob {
    key: String,
    range: String,
    sources: Vec<String>,
    waiters: Vec<Waiter>,
}

struct Inner {
    cache: HashMap<String, (Instant, Analytics)>,
    inflight: HashMap<String, InflightEntry>,
    /// True while a scan body is running or a worker is about to start one.
    busy: bool,
    /// At most one queued job; replaced when a newer different key arrives.
    pending: Option<PendingJob>,
}

/// Shared coordinator managed by Tauri state.
#[derive(Clone)]
pub struct ScanCoordinator {
    inner: Arc<Mutex<Inner>>,
    scan_fn: ScanFn,
    scan_gate: Arc<Mutex<()>>,
}

impl Default for ScanCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl ScanCoordinator {
    pub fn new() -> Self {
        Self::with_scan(Arc::new(|range, sources| {
            Ok(analytics::compute_with(range, sources))
        }))
    }

    /// Test / injection hook: replace the real disk scan.
    pub fn with_scan(scan_fn: ScanFn) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                cache: HashMap::new(),
                inflight: HashMap::new(),
                busy: false,
                pending: None,
            })),
            scan_fn,
            scan_gate: Arc::new(Mutex::new(())),
        }
    }

    /// Run or join an analytics scan for `(sources, range)`.
    pub fn get(&self, range: String, sources: Vec<String>) -> Result<Analytics, String> {
        let key = cache_key(&sources, &range);

        if let Some(hit) = self.cache_get(&key) {
            return Ok(hit);
        }

        enum Role {
            Leader { range: String, sources: Vec<String> },
            Follower(std::sync::mpsc::Receiver<Result<Analytics, String>>),
        }

        let role = {
            let mut g = lock(&self.inner);

            if let Some(hit) = cache_lookup(&g.cache, &key) {
                return Ok(hit);
            }

            // Same key already scanning → coalesce.
            if let Some(entry) = g.inflight.get_mut(&key) {
                let (tx, rx) = std::sync::mpsc::channel();
                entry.waiters.push(tx);
                Role::Follower(rx)
            } else if g.busy {
                let (tx, rx) = std::sync::mpsc::channel();
                match &mut g.pending {
                    Some(p) if p.key == key => {
                        // Same key queued → coalesce waiters (latest range/sources).
                        p.range = range;
                        p.sources = sources;
                        p.waiters.push(tx);
                    }
                    Some(p) => {
                        // Different key → latest-wins: cancel previous pending.
                        cancel_waiters(
                            std::mem::take(&mut p.waiters),
                            "analytics request superseded",
                        );
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
            Role::Follower(rx) => rx
                .recv()
                .map_err(|_| "analytics scan cancelled".to_string())?,
            Role::Leader { range, sources } => self.run_job(key, range, sources),
        }
    }

    fn cache_get(&self, key: &str) -> Option<Analytics> {
        let g = lock(&self.inner);
        cache_lookup(&g.cache, key)
    }

    /// Drop all cached analytics (call only when the cache key domain changes,
    /// i.e. `sources` — theme/locale/share style must not wipe).
    pub fn invalidate_all(&self) {
        let mut g = lock(&self.inner);
        g.cache.clear();
    }

    fn run_job(
        &self,
        key: String,
        range: String,
        sources: Vec<String>,
    ) -> Result<Analytics, String> {
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

            // Isolate panics so busy/inflight cannot stick forever.
            let scan = Arc::clone(&self.scan_fn);
            let range_c = range.clone();
            let sources_c = sources.clone();
            match catch_unwind(AssertUnwindSafe(|| scan(&range_c, &sources_c))) {
                Ok(r) => r,
                Err(_) => Err("analytics scan panicked".into()),
            }
        };

        if let Ok(ref a) = result {
            let mut g = lock(&self.inner);
            g.cache.insert(key.clone(), (Instant::now(), a.clone()));
        }

        self.finish_job(&key, result.clone());
        result
    }

    /// Notify waiters for `key`, then either clear `busy` or spawn a worker for
    /// the single pending job — without blocking the current caller on that work.
    fn finish_job(&self, key: &str, result: Result<Analytics, String>) {
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
                    // Still busy: worker will run next job.
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
            std::thread::spawn(move || {
                let _ = this.run_job(key, range, sources);
            });
        }
    }
}

fn cancel_waiters(waiters: Vec<Waiter>, msg: &str) {
    for w in waiters {
        let _ = w.send(Err(msg.to_string()));
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

/// Sorted sources equality for settings invalidation.
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

    fn slow_scan(
        calls: Arc<AtomicUsize>,
        concurrent: Arc<AtomicUsize>,
        max_concurrent: Arc<AtomicUsize>,
        delay_ms: u64,
    ) -> ScanFn {
        Arc::new(move |range, _sources| {
            calls.fetch_add(1, Ordering::SeqCst);
            let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
            max_concurrent.fetch_max(now, Ordering::SeqCst);
            thread::sleep(Duration::from_millis(delay_ms));
            concurrent.fetch_sub(1, Ordering::SeqCst);
            Ok(sample_analytics(range))
        })
    }

    #[test]
    fn cache_key_sorts_sources() {
        assert_eq!(
            cache_key(&["codex".into(), "claude".into()], "week"),
            cache_key(&["claude".into(), "codex".into()], "week")
        );
    }

    #[test]
    fn sources_equal_ignores_order() {
        assert!(sources_equal(
            &["codex".into(), "claude".into()],
            &["claude".into(), "codex".into()]
        ));
        assert!(!sources_equal(
            &["claude".into()],
            &["claude".into(), "codex".into()]
        ));
    }

    #[test]
    fn ttl_hit_and_expiry() {
        let mut cache = HashMap::new();
        let fake = sample_analytics("today");
        cache.insert("claude|today".into(), (Instant::now(), fake.clone()));
        assert!(cache_lookup(&cache, "claude|today").is_some());
        cache.insert(
            "claude|old".into(),
            (
                Instant::now() - ANALYTICS_TTL - Duration::from_secs(1),
                fake,
            ),
        );
        assert!(cache_lookup(&cache, "claude|old").is_none());
    }

    #[test]
    fn coalesce_same_key_scans_once() {
        let calls = Arc::new(AtomicUsize::new(0));
        let concurrent = Arc::new(AtomicUsize::new(0));
        let max_c = Arc::new(AtomicUsize::new(0));
        let coord = ScanCoordinator::with_scan(slow_scan(
            Arc::clone(&calls),
            Arc::clone(&concurrent),
            Arc::clone(&max_c),
            80,
        ));
        let mut handles = vec![];
        for _ in 0..4 {
            let c = coord.clone();
            handles.push(thread::spawn(move || {
                c.get("week".into(), vec!["claude".into()]).unwrap()
            }));
        }
        for h in handles {
            assert_eq!(h.join().unwrap().range, "week");
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(max_c.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn exclusion_max_one_concurrent_scan() {
        let calls = Arc::new(AtomicUsize::new(0));
        let concurrent = Arc::new(AtomicUsize::new(0));
        let max_c = Arc::new(AtomicUsize::new(0));
        let coord = ScanCoordinator::with_scan(slow_scan(
            Arc::clone(&calls),
            Arc::clone(&concurrent),
            Arc::clone(&max_c),
            50,
        ));
        let c1 = coord.clone();
        let c2 = coord.clone();
        let h1 = thread::spawn(move || c1.get("today".into(), vec!["claude".into()]));
        thread::sleep(Duration::from_millis(10));
        let h2 = thread::spawn(move || c2.get("week".into(), vec!["claude".into()]));
        h1.join().unwrap().unwrap();
        h2.join().unwrap().unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(max_c.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn latest_wins_cancels_stale_pending_key() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls2 = Arc::clone(&calls);
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen2 = Arc::clone(&seen);
        let coord = ScanCoordinator::with_scan(Arc::new(move |range, _| {
            calls2.fetch_add(1, Ordering::SeqCst);
            seen2.lock().unwrap().push(range.to_string());
            thread::sleep(Duration::from_millis(80));
            Ok(sample_analytics(range))
        }));

        let c_leader = coord.clone();
        let leader = thread::spawn(move || {
            c_leader
                .get("today".into(), vec!["claude".into()])
                .unwrap()
        });
        thread::sleep(Duration::from_millis(15));

        let c_week = coord.clone();
        let week = thread::spawn(move || c_week.get("week".into(), vec!["claude".into()]));
        thread::sleep(Duration::from_millis(10));

        let c_month = coord.clone();
        let month = thread::spawn(move || c_month.get("month".into(), vec!["claude".into()]));

        assert_eq!(leader.join().unwrap().range, "today");
        // week was superseded by month
        let week_res = week.join().unwrap();
        assert!(
            week_res.is_err(),
            "stale week should be cancelled"
        );
        assert_eq!(month.join().unwrap().unwrap().range, "month");

        // today + month only (week cancelled before scan)
        let order = seen.lock().unwrap().clone();
        assert_eq!(order, vec!["today".to_string(), "month".to_string()]);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn leader_returns_before_pending_finishes() {
        let phase = Arc::new(AtomicUsize::new(0));
        let phase2 = Arc::clone(&phase);
        let coord = ScanCoordinator::with_scan(Arc::new(move |range, _| {
            if range == "today" {
                phase2.store(1, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(40));
                phase2.store(2, Ordering::SeqCst);
            } else {
                // pending month — long
                phase2.store(3, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(200));
                phase2.store(4, Ordering::SeqCst);
            }
            Ok(sample_analytics(range))
        }));

        let c = coord.clone();
        let leader = thread::spawn(move || c.get("today".into(), vec!["claude".into()]));
        thread::sleep(Duration::from_millis(10));
        let c2 = coord.clone();
        let _pending = thread::spawn(move || c2.get("month".into(), vec!["claude".into()]));

        let t0 = Instant::now();
        let a = leader.join().unwrap().unwrap();
        let leader_ms = t0.elapsed().as_millis();
        assert_eq!(a.range, "today");
        // Leader must not wait for the 200ms pending body.
        assert!(
            leader_ms < 150,
            "leader blocked on pending: {leader_ms}ms"
        );
        // Pending may still be running
        assert!(phase.load(Ordering::SeqCst) >= 2);
    }

    #[test]
    fn panic_in_scan_recovers_for_next_request() {
        let n = Arc::new(AtomicUsize::new(0));
        let n2 = Arc::clone(&n);
        let coord = ScanCoordinator::with_scan(Arc::new(move |range, _| {
            let i = n2.fetch_add(1, Ordering::SeqCst);
            if i == 0 {
                panic!("boom");
            }
            Ok(sample_analytics(range))
        }));
        match coord.get("today".into(), vec!["claude".into()]) {
            Err(err) => assert!(err.contains("panic"), "unexpected err: {err}"),
            Ok(_) => panic!("expected panic to surface as Err"),
        }
        let ok = coord
            .get("today".into(), vec!["claude".into()])
            .expect("second request works");
        assert_eq!(ok.range, "today");
    }

    #[test]
    fn concurrent_cache_hits_are_stable() {
        let coord = ScanCoordinator::new();
        {
            let mut g = lock(&coord.inner);
            g.cache.insert(
                cache_key(&["claude".into()], "today"),
                (Instant::now(), sample_analytics("today")),
            );
        }
        let hits = Arc::new(AtomicUsize::new(0));
        let mut handles = vec![];
        for _ in 0..8 {
            let c = coord.clone();
            let hits = Arc::clone(&hits);
            handles.push(thread::spawn(move || {
                let a = c
                    .get("today".into(), vec!["claude".into()])
                    .expect("cached");
                assert_eq!(a.range, "today");
                hits.fetch_add(1, Ordering::SeqCst);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(hits.load(Ordering::SeqCst), 8);
    }
}
