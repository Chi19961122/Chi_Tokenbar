//! Scan cache (T-perf-004): a per-source-file cache of *parsed events*, so an
//! unchanged session log is not re-parsed on the next scan.
//!
//! ── SCHEMA BUMP RULE (hard, do not remove) ────────────────────────────────
//! Any future ticket that changes how a session log is **parsed** or how those
//! parsed events **aggregate** (i.e. touches `analytics.rs` parse_* / book_* /
//! `Acc` math / the event structs below) MUST bump `SCHEMA`. The cache stores
//! parsed events, not raw messages; if their meaning or aggregation changes,
//! stale entries would silently produce wrong numbers. Bumping `SCHEMA` makes
//! the whole file invalid on next load and it is rebuilt from scratch.
//!
//! Robustness: a missing / corrupt / truncated / wrong-schema / too-large file
//! yields an **empty** cache (rebuild), never a panic. Writes are atomic
//! (temp + rename). The file lives under %LOCALAPPDATA%\Atoll — it is a
//! machine-local *aggregation* artifact, not user settings.
//!
//! Why *events* and not per-file daily/hourly lump aggregates (ticket 規格 1):
//! the Claude global dedup (T-fix-001) and the Codex/Grok cumulative-diff
//! baselines are **cross-file** state. A lump aggregate cannot be de-duplicated
//! against another file at merge time, so it cannot satisfy 規格 5 (the ticket's
//! stated soul). Parsed events *can* be replayed through the exact production
//! booking logic with the global dedup set, giving byte-identical results while
//! still skipping the expensive JSON parse and staying far smaller than the raw
//! logs (numbers + short ids, gzip-compressed on disk).

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Bump this whenever parse/aggregation logic changes (see header rule).
pub const SCHEMA: u32 = 1;

/// Hard ceiling on the on-disk (compressed) cache file. Above this we prune to
/// the current working set, and failing that skip the write, so the cache can
/// never grow without bound (ticket 規格 4).
const MAX_CACHE_BYTES: u64 = 32 * 1024 * 1024;

/// Bytes sampled from each end of a file for the fingerprint hash.
const SAMPLE_BYTES: usize = 4096;

// ── parsed-event payloads (built by analytics::parse_*) ───────────────────

/// One booked-candidate Claude assistant message. Everything `book_claude_event`
/// needs to dedup, cost, and aggregate — cost is intentionally *not* stored so a
/// pricing-override change is honoured without invalidating the cache.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClaudeEvent {
    /// requestId → message.id → uuid (T-fix-001 priority), or None.
    pub dedup_key: Option<String>,
    pub ts: i64,
    pub model: String,
    /// Activity kind (`message_kind` result) — Claude is the only classifiable source.
    pub kind: String,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write_5m: u64,
    pub cache_write_1h: u64,
    /// cache_read + cache_creation — the `cached` dimension booked into breakdown.
    pub cached: u64,
}

/// One Codex `token_count` event: the *cumulative* usage tuple at a timestamp.
/// The per-file diff + global (ts,total) replay guard run at book time.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CodexEvent {
    pub ts: i64,
    pub input: u64,
    pub cached: u64,
    pub output: u64,
    pub reasoning: u64,
}

/// One Grok update line: an optional sticky model id and/or a cumulative token
/// total at a timestamp. The per-file baseline + global replay guard run at book time.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GrokEvent {
    pub model_update: Option<String>,
    /// (ts, cumulative totalTokens) when this line carried a token reading.
    pub token: Option<(i64, u64)>,
}

/// Per-file parsed payload. Project is stored only for Codex, whose project is
/// discovered from file *content* (cwd); Claude/Grok derive project from the
/// path, so it is recomputed on hit and never cached.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CachedFile {
    Claude(Vec<ClaudeEvent>),
    Codex { project: String, events: Vec<CodexEvent> },
    Grok(Vec<GrokEvent>),
}

// ── fingerprint ───────────────────────────────────────────────────────────

/// (size, mtime, head-4KB hash, tail-4KB hash). Any mismatch → re-parse. An
/// actively-appended JSONL always changes size and the tail hash, so a live log
/// invalidates naturally — exactly the append-only shape of these files.
///
/// Hash is `DefaultHasher` (SipHash13, fixed zero seed → stable across process
/// runs, so a cache written by one launch is still valid on the next). This is a
/// local consistency check, not a security boundary, so a 64-bit sampled hash is
/// enough; the project has no direct `sha2` dependency and the one-line Cargo
/// budget is spent on `flate2` for compression.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fingerprint {
    pub size: u64,
    pub mtime: i64,
    pub head: u64,
    pub tail: u64,
}

fn hash_bytes(b: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    b.hash(&mut h);
    h.finish()
}

/// Fingerprint a file: metadata + a 4KB sample from each end. `None` if the file
/// cannot be stat'd/opened/read — the caller then skips it (same as the old scan
/// treating a metadata/open failure as "not scannable").
pub fn fingerprint(path: &Path) -> Option<Fingerprint> {
    let meta = fs::metadata(path).ok()?;
    let size = meta.len();
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    let mut file = File::open(path).ok()?;

    let head_len = size.min(SAMPLE_BYTES as u64) as usize;
    let mut head_buf = vec![0u8; head_len];
    read_exact_at(&mut file, 0, &mut head_buf)?;
    let head = hash_bytes(&head_buf);

    let tail_len = size.min(SAMPLE_BYTES as u64) as usize;
    let tail_start = size.saturating_sub(tail_len as u64);
    let mut tail_buf = vec![0u8; tail_len];
    read_exact_at(&mut file, tail_start, &mut tail_buf)?;
    let tail = hash_bytes(&tail_buf);

    Some(Fingerprint {
        size,
        mtime,
        head,
        tail,
    })
}

fn read_exact_at(file: &mut File, offset: u64, buf: &mut [u8]) -> Option<()> {
    if buf.is_empty() {
        return Some(());
    }
    file.seek(SeekFrom::Start(offset)).ok()?;
    file.read_exact(buf).ok()?;
    Some(())
}

// ── on-disk container ─────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Entry {
    size: u64,
    mtime: i64,
    head: u64,
    tail: u64,
    data: CachedFile,
}

impl Entry {
    fn matches(&self, fp: &Fingerprint) -> bool {
        self.size == fp.size && self.mtime == fp.mtime && self.head == fp.head && self.tail == fp.tail
    }
}

#[derive(Serialize, Deserialize)]
struct Persisted {
    schema: u32,
    entries: HashMap<String, Entry>,
}

/// Loaded cache plus in-memory bookkeeping for this scan round.
pub struct ScanCache {
    entries: HashMap<String, Entry>,
    /// Paths touched (hit or inserted) this round — the working set kept when the
    /// file must be pruned to fit `MAX_CACHE_BYTES`.
    live: HashSet<String>,
}

/// Per-round hit/parse counters for `TOKENBAR_DEBUG`.
#[derive(Default, Clone, Copy, Debug)]
pub struct CacheStats {
    pub hit: u64,
    pub parsed: u64,
}

impl ScanCache {
    pub fn empty() -> Self {
        ScanCache {
            entries: HashMap::new(),
            live: HashSet::new(),
        }
    }

    /// Load the machine-local cache, or an empty cache on any problem
    /// (missing / too large / unreadable / bad gzip / bad JSON / schema mismatch).
    pub fn load() -> Self {
        match cache_path() {
            Some(p) => Self::load_from(&p),
            None => Self::empty(),
        }
    }

    /// Testable core of `load`.
    pub fn load_from(path: &Path) -> Self {
        let Ok(meta) = fs::metadata(path) else {
            return Self::empty();
        };
        if meta.len() > MAX_CACHE_BYTES {
            // Oversized on disk → discard and rebuild (ticket 規格 4).
            return Self::empty();
        }
        let Ok(bytes) = fs::read(path) else {
            return Self::empty();
        };
        let Some(json) = gunzip(&bytes) else {
            return Self::empty();
        };
        let Ok(p) = serde_json::from_slice::<Persisted>(&json) else {
            return Self::empty();
        };
        if p.schema != SCHEMA {
            // Parser/aggregation changed under us → whole file invalid.
            return Self::empty();
        }
        ScanCache {
            entries: p.entries,
            live: HashSet::new(),
        }
    }

    /// Cached payload for `path` iff its fingerprint still matches. Records the
    /// path as live for this round regardless of hit/miss.
    pub fn get_matching(&mut self, path: &str, fp: &Fingerprint) -> Option<&CachedFile> {
        self.live.insert(path.to_string());
        match self.entries.get(path) {
            Some(e) if e.matches(fp) => Some(&e.data),
            _ => None,
        }
    }

    /// Store freshly-parsed events for `path`.
    pub fn insert(&mut self, path: String, fp: Fingerprint, data: CachedFile) {
        self.live.insert(path.clone());
        self.entries.insert(
            path,
            Entry {
                size: fp.size,
                mtime: fp.mtime,
                head: fp.head,
                tail: fp.tail,
                data,
            },
        );
    }

    /// Persist atomically. Prunes entries whose source file has vanished, and if
    /// the compressed image would exceed `MAX_CACHE_BYTES`, shrinks to this
    /// round's working set (and skips the write if even that is too large).
    /// Never propagates an error — caching is best-effort.
    pub fn save_best_effort(&self) {
        let Some(path) = cache_path() else {
            return;
        };
        // Drop entries for deleted source files.
        let mut entries: HashMap<String, Entry> = self
            .entries
            .iter()
            .filter(|(p, _)| Path::new(p).exists())
            .map(|(p, e)| (p.clone(), e.clone()))
            .collect();

        let mut bytes = match encode(SCHEMA, &entries) {
            Some(b) => b,
            None => return,
        };
        if bytes.len() as u64 > MAX_CACHE_BYTES {
            // Shrink to the paths actually used this round.
            entries.retain(|p, _| self.live.contains(p));
            bytes = match encode(SCHEMA, &entries) {
                Some(b) => b,
                None => return,
            };
            if bytes.len() as u64 > MAX_CACHE_BYTES {
                return; // give up rather than write an oversized file
            }
        }
        let _ = atomic_write(&path, &bytes);
    }

    /// Test helper: write to an explicit path (analytics golden tests use a temp
    /// file instead of the real `%LOCALAPPDATA%` cache).
    #[cfg(test)]
    pub fn save_to(&self, path: &Path) {
        self.save_with_schema(path, SCHEMA);
    }

    /// Test helper: write with an arbitrary schema tag, to exercise the
    /// schema-bump-invalidates-everything path.
    #[cfg(test)]
    pub fn save_with_schema(&self, path: &Path, schema: u32) {
        if let Some(bytes) = encode(schema, &self.entries) {
            let _ = atomic_write(path, &bytes);
        }
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn encode(schema: u32, entries: &HashMap<String, Entry>) -> Option<Vec<u8>> {
    let persisted = PersistedRef { schema, entries };
    let json = serde_json::to_vec(&persisted).ok()?;
    gzip(&json)
}

/// Borrowed mirror of `Persisted` so `save` serializes without cloning the map.
#[derive(Serialize)]
struct PersistedRef<'a> {
    schema: u32,
    entries: &'a HashMap<String, Entry>,
}

fn gzip(data: &[u8]) -> Option<Vec<u8>> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    enc.write_all(data).ok()?;
    enc.finish().ok()
}

fn gunzip(data: &[u8]) -> Option<Vec<u8>> {
    use flate2::read::GzDecoder;
    let mut out = Vec::new();
    GzDecoder::new(data).read_to_end(&mut out).ok()?;
    Some(out)
}

/// temp + rename in the same dir so a reader never sees a half-written file, and
/// a crash mid-write cannot corrupt the previous good cache.
fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", unique_suffix()));
    {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all().ok();
    }
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

fn unique_suffix() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(std::process::id() as u64);
    h.write_i64(chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
    format!("{:016x}", h.finish())
}

/// `%LOCALAPPDATA%\Atoll\scan-cache.json.gz` on Windows (data_local_dir). This is
/// a machine-local aggregation product, deliberately **not** the roaming config
/// dir. `None` only if no local-data dir exists, in which case caching is off.
pub fn cache_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("Atoll").join("scan-cache.json.gz"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_from_missing_file_is_empty() {
        let p = std::env::temp_dir().join("atoll-scan-cache-test-missing.json.gz");
        let _ = fs::remove_file(&p);
        assert!(ScanCache::load_from(&p).is_empty());
    }

    #[test]
    fn corrupt_file_loads_as_empty() {
        let p = std::env::temp_dir().join("atoll-scan-cache-test-corrupt.json.gz");
        fs::write(&p, b"this is not gzip and not json {{{").unwrap();
        assert!(ScanCache::load_from(&p).is_empty(), "corrupt cache must rebuild");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn wrong_schema_loads_as_empty() {
        let p = std::env::temp_dir().join("atoll-scan-cache-test-schema.json.gz");
        // Hand-encode a valid gzip'd Persisted with a bumped schema number.
        let entries: HashMap<String, Entry> = HashMap::new();
        let bytes = encode(SCHEMA + 1, &entries).unwrap();
        fs::write(&p, &bytes).unwrap();
        assert!(
            ScanCache::load_from(&p).is_empty(),
            "a schema mismatch must invalidate the whole cache"
        );
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn fingerprint_changes_when_content_is_appended() {
        let p = std::env::temp_dir().join("atoll-scan-cache-test-fp.jsonl");
        fs::write(&p, b"line one\n").unwrap();
        let a = fingerprint(&p).unwrap();
        fs::write(&p, b"line one\nline two\n").unwrap();
        let b = fingerprint(&p).unwrap();
        assert_ne!(a, b, "append must move the fingerprint (size + tail)");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn roundtrip_through_gzip_preserves_entries() {
        let entries: HashMap<String, Entry> = HashMap::from([(
            "p".to_string(),
            Entry {
                size: 10,
                mtime: 20,
                head: 30,
                tail: 40,
                data: CachedFile::Claude(vec![ClaudeEvent {
                    dedup_key: Some("r1".into()),
                    ts: 100,
                    model: "claude-opus".into(),
                    kind: "edit".into(),
                    input: 5,
                    output: 6,
                    cache_read: 7,
                    cache_write_5m: 8,
                    cache_write_1h: 9,
                    cached: 7,
                }]),
            },
        )]);
        let bytes = encode(SCHEMA, &entries).unwrap();
        let json = gunzip(&bytes).unwrap();
        let p: Persisted = serde_json::from_slice(&json).unwrap();
        assert_eq!(p.schema, SCHEMA);
        assert_eq!(p.entries.len(), 1);
        assert!(p.entries.contains_key("p"));
    }
}
