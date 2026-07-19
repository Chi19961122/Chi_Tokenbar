//! Stage 1B analytics scan coordinator.
//!
//! Mutex alone is *not* enough (today→week→month would still be three full
//! scans). This module provides:
//! - **TTL cache** — same `sources|range` within the window never re-parses
//! - **In-flight coalesce** — concurrent identical keys share one result
//! - **Mutual exclusion** — at most one full `compute_with` at a time
//! - **Queue policy** — while busy, new keys wait; empty-waiter keys drop;
//!   prefer promoting `month` when several are pending
//! - **Pre-scan recheck** — after waiting, serve cache if now warm
//!
//! Decisions happen *before* the expensive scan body; we do not abort mid-scan.

use crate::analytics::{self, Analytics};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Short process-local cache. Long enough to fold island 60s refresh + tab
/// flips; short enough that a real session keeps data feeling live.
const ANALYTICS_TTL: Duration = Duration::from_secs(60);

type Waiter = std::sync::mpsc::Sender<Result<Analytics, String>>;

struct InflightEntry {
    waiters: Vec<Waiter>,
}

struct Pending {
    range: String,
    sources: Vec<String>,
    waiters: Vec<Waiter>,
}

struct Inner {
    cache: HashMap<String, (Instant, Analytics)>,
    /// Keys currently being scanned. Followers park here.
    inflight: HashMap<String, InflightEntry>,
    /// True while any full scan runs (or a pending promote is about to run).
    busy: bool,
    /// Requests that arrived while `busy`, deduped by key.
    pending: HashMap<String, Pending>,
}

/// Shared coordinator managed by Tauri state.
#[derive(Clone)]
pub struct ScanCoordinator {
    inner: Arc<Mutex<Inner>>,
    /// Serializes full scan bodies.
    scan_gate: Arc<Mutex<()>>,
}

impl Default for ScanCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl ScanCoordinator {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                cache: HashMap::new(),
                inflight: HashMap::new(),
                busy: false,
                pending: HashMap::new(),
            })),
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
                // Queue: merge into pending for this key; do not start another scan.
                let (tx, rx) = std::sync::mpsc::channel();
                match g.pending.get_mut(&key) {
                    Some(p) => {
                        p.range = range;
                        p.sources = sources;
                        p.waiters.push(tx);
                    }
                    None => {
                        g.pending.insert(
                            key.clone(),
                            Pending {
                                range,
                                sources,
                                waiters: vec![tx],
                            },
                        );
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
            Role::Leader { range, sources } => self.run_leader(key, range, sources),
        }
    }

    fn cache_get(&self, key: &str) -> Option<Analytics> {
        let g = lock(&self.inner);
        cache_lookup(&g.cache, key)
    }

    /// Drop all cached analytics (e.g. when the user changes `sources`).
    pub fn invalidate_all(&self) {
        let mut g = lock(&self.inner);
        g.cache.clear();
    }

    fn run_leader(
        &self,
        key: String,
        range: String,
        sources: Vec<String>,
    ) -> Result<Analytics, String> {
        // Hold the gate only around the scan body so after_scan can promote the
        // next pending key without deadlocking on re-entry.
        let result = {
            let _gate = self
                .scan_gate
                .lock()
                .unwrap_or_else(|p| p.into_inner());

            // Another completed scan may have filled the cache while we waited.
            if let Some(hit) = self.cache_get(&key) {
                drop(_gate);
                self.broadcast_and_clear_inflight(&key, Ok(hit.clone()));
                self.after_scan();
                return Ok(hit);
            }

            Ok(analytics::compute_with(&range, &sources))
        };

        if let Ok(ref a) = result {
            let mut g = lock(&self.inner);
            g.cache.insert(key.clone(), (Instant::now(), a.clone()));
        }

        self.broadcast_and_clear_inflight(&key, result.clone());
        self.after_scan();
        result
    }

    fn broadcast_and_clear_inflight(&self, key: &str, result: Result<Analytics, String>) {
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
    }

    /// Clear busy or promote one pending key (prefer month).
    fn after_scan(&self) {
        let promote = {
            let mut g = lock(&self.inner);
            // Drop pending entries with no waiters (nobody cares anymore).
            g.pending.retain(|_, p| !p.waiters.is_empty());

            let pick = pick_pending_key(&g.pending);
            match pick {
                None => {
                    g.busy = false;
                    None
                }
                Some(k) => {
                    let p = g.pending.remove(&k).expect("just picked");
                    g.busy = true;
                    g.inflight.insert(
                        k.clone(),
                        InflightEntry {
                            waiters: p.waiters,
                        },
                    );
                    Some((k, p.range, p.sources))
                }
            }
        };

        if let Some((key, range, sources)) = promote {
            // Chain on this thread: only one scan body at a time via scan_gate.
            let _ = self.run_leader(key, range, sources);
        }
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

/// Prefer month (widest window) when several keys are queued.
fn pick_pending_key(pending: &HashMap<String, Pending>) -> Option<String> {
    if pending.is_empty() {
        return None;
    }
    if let Some((k, _)) = pending.iter().find(|(_, p)| p.range == "month") {
        return Some(k.clone());
    }
    pending.keys().next().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

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

    #[test]
    fn cache_key_sorts_sources() {
        assert_eq!(
            cache_key(&["codex".into(), "claude".into()], "week"),
            cache_key(&["claude".into(), "codex".into()], "week")
        );
        assert_ne!(
            cache_key(&["claude".into()], "week"),
            cache_key(&["claude".into()], "month")
        );
    }

    #[test]
    fn ttl_hit_and_expiry() {
        let mut cache = HashMap::new();
        let fake = sample_analytics("today");
        cache.insert("claude|today".into(), (Instant::now(), fake.clone()));
        assert_eq!(
            cache_lookup(&cache, "claude|today").unwrap().range,
            "today"
        );
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
    fn pick_pending_prefers_month() {
        let mut pending = HashMap::new();
        pending.insert(
            "a|week".into(),
            Pending {
                range: "week".into(),
                sources: vec![],
                waiters: vec![],
            },
        );
        pending.insert(
            "a|month".into(),
            Pending {
                range: "month".into(),
                sources: vec![],
                waiters: vec![],
            },
        );
        pending.insert(
            "a|today".into(),
            Pending {
                range: "today".into(),
                sources: vec![],
                waiters: vec![],
            },
        );
        assert_eq!(pick_pending_key(&pending).unwrap(), "a|month");
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
