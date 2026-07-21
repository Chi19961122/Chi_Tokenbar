//! Layer ③ analytics (UX Spec v3 §11): aggregate local JSONL into daily /
//! model / agent breakdowns, cost estimate, and stats. All from local files —
//! no undocumented API, no risk. Days and hours are bucketed in the user's
//! local timezone so the charts read on the same clock as their labels (F-15).

use crate::pricing::{PricingOverride, RateSpec};
use crate::scan_cache::{self, CacheStats, CachedFile, ClaudeEvent, CodexEvent, GrokEvent, ScanCache};
use chrono::{Datelike, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

/// Loaded scan cache plus this round's hit/parse counters, shared (via a
/// `RefCell`) across the three per-source scanners so one on-disk cache serves
/// them all and the debug line can report a single hit/parsed total.
struct CacheRun {
    cache: ScanCache,
    stats: CacheStats,
}

/// Per-scan counters for opt-in diagnostics (`TOKENBAR_DEBUG`).
/// Does not change product behaviour — only logged when the env var is set.
///
/// Field semantics (kept identical across Claude / Codex / Grok):
/// - `files_considered` — paths from glob (before mtime gate)
/// - `files_read` — files opened for the main scan pass
/// - `eligible_file_bytes` — sum of `metadata().len()` for those files
///   (**not** bytes actually read; excludes Codex `first_cwd_basename` head-read)
/// - `lines_read` — lines seen on the main pass
/// - `candidate_lines` — lines that passed the cheap string prefilter
/// - `json_parse_ok` — candidate lines where `serde_json::from_str` succeeded
///   (before domain filters like "has usage" / "is token_count event")
#[derive(Default, Debug, Clone)]
struct ScanStats {
    files_considered: u64,
    files_read: u64,
    eligible_file_bytes: u64,
    lines_read: u64,
    candidate_lines: u64,
    json_parse_ok: u64,
}

fn log_scan_stats(range: &str, sources: &[String], stats: &ScanStats, elapsed_ms: u128) {
    if std::env::var_os("TOKENBAR_DEBUG").is_none() {
        return;
    }
    eprintln!(
        "[atoll:analytics] range={} sources={:?} files_considered={} files_read={} eligible_file_bytes={} lines_read={} candidate_lines={} json_parse_ok={} elapsed_ms={}",
        range,
        sources,
        stats.files_considered,
        stats.files_read,
        stats.eligible_file_bytes,
        stats.lines_read,
        stats.candidate_lines,
        stats.json_parse_ok,
        elapsed_ms
    );
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DayPoint {
    pub date: String,
    pub by_model: HashMap<String, u64>,
    pub by_agent: HashMap<String, u64>,
    pub cost_usd: f64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BestDay {
    pub date: String,
    pub cost_usd: f64,
}

#[derive(Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MaxDayRecord {
    pub date: String,
    pub tokens: u64,
}

#[derive(Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MaxHourRecord {
    pub date: String,
    pub hour: u8,
    pub tokens: u64,
}

#[derive(Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Records {
    pub max_day: MaxDayRecord,
    pub max_hour: MaxHourRecord,
    pub streak_days: u32,
    pub pr_now: bool,
}

#[derive(Serialize, Clone)]
pub struct Breakdown {
    pub input: u64,
    pub cached: u64,
    pub output: u64,
    pub reasoning: u64,
}

/// One activity-type slice (階段 C+). `kind` is a stable id the frontend maps to
/// a localized label: "edit" | "read" | "run" | "other". Claude-only — see the
/// scan-source recon note below `scan_codex`.
#[derive(Serialize, Clone)]
pub struct KindCount {
    pub kind: String,
    pub tokens: u64,
}

/// Per-project token total (階段 C+). Usage-only.
/// 隱私硬限制:不得進戰報(§0)——`buildShareData` 禁止引用 `by_project`。
/// The remainder beyond the top 8 is merged under the id "__other__".
#[derive(Serialize, Clone)]
pub struct ProjectCount {
    pub name: String,
    pub tokens: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub client: String,
    pub provider: String,
    pub account: String,
    pub plan: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Analytics {
    pub range: String,
    /// Earliest day actually shown. Normally the window start, but when local
    /// logs are shorter than the requested window (e.g. a "month" with only a
    /// week of history) it is the first day that has any activity — the caller
    /// annotates "from {date}" when this is later than the nominal start.
    pub range_start_day: String,
    pub total_tokens: u64,
    pub total_cost_usd: f64,
    pub best_day: BestDay,
    pub active_days: u32,
    pub records: Records,
    pub daily: Vec<DayPoint>,
    pub hourly: Vec<u64>,
    /// Per-hour cost, indexed like `hourly` (len 24). Lets the hourly chart and
    /// the metric toggle draw $ where `hourly` only ever held tokens.
    pub hourly_cost: Vec<f64>,
    pub by_model: HashMap<String, u64>,
    pub by_agent: HashMap<String, u64>,
    /// Range-total cost per model / per agent, keyed identically to
    /// `by_model` / `by_agent`. Gives the "share" price mode a real cost split.
    pub by_model_cost: HashMap<String, f64>,
    pub by_agent_cost: HashMap<String, f64>,
    pub breakdown: Breakdown,
    /// Activity-type breakdown (Claude tool usage). Empty when nothing is
    /// classifiable — the frontend then omits the whole section (no fake kinds).
    pub by_kind: Vec<KindCount>,
    /// Per-project token totals, top 8 + merged "__other__". Usage-only.
    /// 不得進戰報(§0):`buildShareData` 禁止引用此欄位。
    pub by_project: Vec<ProjectCount>,
    pub sessions_this_week: u32,
    pub tok_per_min: u64,
    pub accounts: Vec<Account>,
}

// ── activity-type classification (階段 C+, Claude only) ───────────────────
//
// Buckets a Claude tool name into a coarse activity kind. Anything unrecognised
// is "other" — a real bucket, not a fabricated one. Kept deliberately small so
// each mapping is defensible from an observed or documented tool name.
fn classify_kind(name: &str) -> &'static str {
    match name {
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => "edit",
        "Read" => "read",
        "Grep" | "Glob" | "LS" | "ToolSearch" => "search",
        "Bash" | "PowerShell" => "run",
        "WebFetch" | "WebSearch" => "web",
        "Task" => "agent",
        _ if name.starts_with("Agent") => "agent",
        _ if name.starts_with("mcp__") => "mcp",
        _ => "other",
    }
}

/// The single kind attributed to one assistant message, from the tools it used.
/// A message's tokens are booked whole to its dominant tool kind (ties break in
/// edit>read>search>run>web>agent>mcp>other order); a message with no tool_use
/// is "other".
fn message_kind(tool_names: &[String]) -> &'static str {
    if tool_names.is_empty() {
        return "other";
    }
    let mut counts = [0u32; 8];
    for n in tool_names {
        let idx = match classify_kind(n) {
            "edit" => 0,
            "read" => 1,
            "search" => 2,
            "run" => 3,
            "web" => 4,
            "agent" => 5,
            "mcp" => 6,
            _ => 7,
        };
        counts[idx] += 1;
    }
    let kinds = [
        "edit", "read", "search", "run", "web", "agent", "mcp", "other",
    ];
    let mut best = 0;
    for i in 1..counts.len() {
        if counts[i] > counts[best] {
            best = i;
        }
    }
    kinds[best]
}

/// Last path component of a cwd / project path (no separators kept, so it is a
/// bare folder name, never a full path). "" when there is nothing to take.
fn basename(path: &str) -> String {
    path.rsplit(|c| c == '/' || c == '\\')
        .find(|s| !s.is_empty())
        .unwrap_or("")
        .to_string()
}

/// Blended $/Mtok estimate per model family (clearly an estimate; §11).
fn rate_per_mtok(model: &str) -> f64 {
    let m = model.to_lowercase();
    if m.contains("opus") {
        9.0
    } else if m.contains("sonnet") {
        3.5
    } else if m.contains("mini") {
        1.0
    } else if m.contains("codex") || m.contains("gpt") {
        5.0
    } else {
        4.0
    }
}

fn date_str(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

struct Acc {
    days: HashMap<String, DayAgg>,
    hourly: [u64; 24],
    /// Per-hour cost, accumulated alongside `hourly` (same `dt.hour()` index).
    hourly_cost: [f64; 24],
    hourly_by_day: HashMap<(String, u8), u64>,
    /// Range-total cost per model / per agent, keyed like the per-day token maps
    /// (nothing needs per-day-per-model cost, so these live directly on Acc).
    by_model_cost: HashMap<String, f64>,
    by_agent_cost: HashMap<String, f64>,
    breakdown: Breakdown,
    /// Range-total activity-type buckets (Claude only). Summed like `breakdown`.
    by_kind: HashMap<String, u64>,
    /// Range-total per-project buckets (both providers). Usage-only (§0).
    by_project: HashMap<String, u64>,
    recent_tokens: u64,
    now: i64,
    stats: ScanStats,
    /// 查價用的本機 override 表(T-feat-006)。預設空表 = 純 vendored,所以既有
    /// 測試與 stub 路徑行為不變;真實掃描由 `compute_with` 塞入 `pricing::current()`。
    pricing: Arc<PricingOverride>,
}

#[derive(Default)]
struct DayAgg {
    by_model: HashMap<String, u64>,
    by_agent: HashMap<String, u64>,
    cost: f64,
}

impl Acc {
    fn new(now: i64) -> Self {
        Acc {
            days: HashMap::new(),
            hourly: [0; 24],
            hourly_cost: [0.0; 24],
            hourly_by_day: HashMap::new(),
            by_model_cost: HashMap::new(),
            by_agent_cost: HashMap::new(),
            breakdown: Breakdown {
                input: 0,
                cached: 0,
                output: 0,
                reasoning: 0,
            },
            by_kind: HashMap::new(),
            by_project: HashMap::new(),
            recent_tokens: 0,
            now,
            stats: ScanStats::default(),
            pricing: Arc::new(PricingOverride::empty()),
        }
    }

    /// `project` = a bare folder name ("" to skip project attribution).
    /// `kind` = an activity kind for classifiable providers (Claude); None for
    /// providers whose tokens aren't per-tool attributable (Codex).
    ///
    /// Test-only since T-917: the real scanners route through `add_with_cost`
    /// (vendor-priced) or `add_total_only` (Grok); this blended-cost wrapper now
    /// only backs the aggregation test stubs.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn add(
        &mut self,
        ts: i64,
        model: &str,
        agent: &str,
        project: &str,
        kind: Option<&str>,
        input: u64,
        cached: u64,
        output: u64,
        reasoning: u64,
    ) {
        let cost = ((input + cached + output + reasoning) as f64 / 1e6) * rate_per_mtok(model);
        self.add_with_cost(
            ts, model, agent, project, kind, input, cached, output, reasoning, cost,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn add_with_cost(
        &mut self,
        ts: i64,
        model: &str,
        agent: &str,
        project: &str,
        kind: Option<&str>,
        input: u64,
        cached: u64,
        output: u64,
        reasoning: u64,
        cost: f64,
    ) {
        let total = input + cached + output + reasoning;
        // `book` records every total dimension; breakdown/by_kind are added only
        // when the row was actually booked (non-zero, valid timestamp).
        if !self.book(ts, model, agent, project, total, cost) {
            return;
        }
        self.breakdown.input += input;
        self.breakdown.cached += cached;
        self.breakdown.output += output;
        self.breakdown.reasoning += reasoning;
        if let Some(k) = kind {
            *self.by_kind.entry(k.to_string()).or_default() += total;
        }
    }

    /// T-916 Grok: a source that reports only a cumulative *total* per event with
    /// no input/cached/output/reasoning split. Book the total into every total
    /// dimension (total_tokens, by_model, by_agent, hourly, by_project, recent),
    /// but deliberately NOT into `breakdown` or `by_kind` — inventing an
    /// input/output category for Grok would be a lie (硬規定:無法拆分/無法分類
    /// 就不出假類別). Caller passes cost 0.0 (no public Grok pricing), so est.
    /// cost knowingly undercounts when Grok is selected.
    fn add_total_only(&mut self, ts: i64, model: &str, agent: &str, project: &str, total: u64, cost: f64) {
        self.book(ts, model, agent, project, total, cost);
    }

    /// Shared booking for every total dimension. Returns whether the row was
    /// booked (false when the total is zero or the timestamp is invalid), so the
    /// caller knows whether to add its breakdown/kind detail. This is exactly the
    /// aggregation `add_with_cost` did inline before Grok needed a total-only path
    /// — order preserved so existing behaviour is unchanged.
    fn book(&mut self, ts: i64, model: &str, agent: &str, project: &str, total: u64, cost: f64) -> bool {
        if total == 0 {
            return false;
        }
        let Some(dt_utc) = chrono::DateTime::from_timestamp(ts, 0) else {
            return false;
        };
        // Attribute the row to the user's local day/hour (F-15): the daily and
        // hourly charts, and the busiest-hour record, must all read on one clock.
        let dt = dt_utc.with_timezone(&chrono::Local);
        let day = self
            .days
            .entry(dt.format("%Y-%m-%d").to_string())
            .or_default();
        *day.by_model.entry(model.to_string()).or_default() += total;
        *day.by_agent.entry(agent.to_string()).or_default() += total;
        day.cost += cost;

        // Range-total cost dimensions, mirrored on the token equivalents above.
        *self.by_model_cost.entry(model.to_string()).or_default() += cost;
        *self.by_agent_cost.entry(agent.to_string()).or_default() += cost;

        self.hourly[dt.hour() as usize] += total;
        self.hourly_cost[dt.hour() as usize] += cost;
        *self
            .hourly_by_day
            .entry((dt.format("%Y-%m-%d").to_string(), dt.hour() as u8))
            .or_default() += total;

        if !project.is_empty() {
            *self.by_project.entry(project.to_string()).or_default() += total;
        }

        if self.now - ts <= 600 {
            self.recent_tokens += total;
        }
        true
    }
}

fn records_for(acc: &Acc, today: chrono::NaiveDate, current_hour: u8) -> Records {
    let mut by_day: HashMap<&str, u64> = HashMap::new();
    let mut max_hour = MaxHourRecord::default();
    let mut historical_hour_max = 0u64;
    let today_s = today.format("%Y-%m-%d").to_string();
    let current_key = (today_s.as_str(), current_hour);

    for ((date, hour), tokens) in &acc.hourly_by_day {
        *by_day.entry(date.as_str()).or_default() += *tokens;
        if *tokens > max_hour.tokens {
            max_hour = MaxHourRecord {
                date: date.clone(),
                hour: *hour,
                tokens: *tokens,
            };
        }
        if (date.as_str(), *hour) != current_key {
            historical_hour_max = historical_hour_max.max(*tokens);
        }
    }

    let max_day = by_day
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
        .map(|(date, tokens)| MaxDayRecord {
            date: (*date).to_string(),
            tokens: *tokens,
        })
        .unwrap_or_default();

    let mut cursor = if by_day.contains_key(today_s.as_str()) {
        today
    } else {
        today.pred_opt().unwrap_or(today)
    };
    let mut streak_days = 0;
    while by_day.contains_key(cursor.format("%Y-%m-%d").to_string().as_str()) {
        streak_days += 1;
        let Some(previous) = cursor.pred_opt() else {
            break;
        };
        cursor = previous;
    }

    let current_tokens = acc
        .hourly_by_day
        .get(&(today_s, current_hour))
        .copied()
        .unwrap_or(0);
    Records {
        max_day,
        max_hour,
        streak_days,
        pr_now: current_tokens > historical_hour_max && current_tokens > 0,
    }
}

// ── source gating (Settings::sources) ────────────────────────────────────
//
// Analytics is the one consumer that does not read the Snapshot — it scans
// local JSONL directly — so the scheduler's single filter node cannot reach it
// and it has to honour the selection itself. T-916 unified the old
// providers-filter + tool toggles into one `sources` list: a source is scanned
// iff it is a member. Membership is explicit — an absent source is never
// scanned, and an empty list yields an honest empty page (no "unknown ⇒ show
// everything" fallback; that lived in the string filter this replaced).

fn wants(sources: &[String], id: &str) -> bool {
    sources.iter().any(|s| s == id)
}

fn filter_accounts(sources: &[String], accounts: Vec<Account>) -> Vec<Account> {
    accounts
        .into_iter()
        .filter(|a| match a.provider.as_str() {
            "anthropic" => wants(sources, "claude"),
            "codex" => wants(sources, "codex"),
            "grok" => wants(sources, "grok"),
            _ => true,
        })
        .collect()
}

fn days_back_for(range: &str) -> i64 {
    match range {
        "today" => 0,
        "month" => 29, // last 30 days including today
        _ => 6,        // "week"
    }
}

/// Window start (unix seconds) for `range`, aligned to the user's LOCAL
/// midnight (F-15) — the same boundary `compute_routed` scans from. Shared
/// with `source_fingerprint` so the fingerprint covers exactly the files a
/// scan of this range would actually read.
fn range_start(range: &str) -> i64 {
    let now = chrono::Utc::now().timestamp();
    let days_back = days_back_for(range);
    let local_midnight = chrono::Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|naive| chrono::Local.from_local_datetime(&naive).single())
        .map(|dt| dt.timestamp())
        .unwrap_or_else(|| now - now.rem_euclid(86400));
    local_midnight - days_back * 86400
}

/// Glob one source's session-log tree, same pattern each `scan_*` uses.
/// Returns `PathBuf`s only — no file is opened here.
fn glob_source(pattern: &Path) -> Vec<PathBuf> {
    let pattern = pattern.to_string_lossy().replace('\\', "/");
    match glob::glob(&pattern) {
        Ok(paths) => paths.filter_map(Result::ok).collect(),
        Err(_) => Vec::new(),
    }
}

/// T-perf-001: a cheap fingerprint of every session-log file the current
/// `sources` selection would actually read for `range` — `(path, mtime, len)`
/// per file, combined into one hash. Two calls with the same fingerprint mean
/// "no source file changed"; the cache layer (`scan_coord`) uses that to skip
/// a full re-scan instead of relying on TTL alone.
///
/// Metadata only (`fs::metadata` mtime/len) — never opens or reads a file, so
/// this cannot touch credential/token content (CLAUDE.md 機密鐵則).
/// Missing home dir / unreadable file metadata contribute nothing rather than
/// panicking — a fingerprint must be as robust as the scan it stands in for.
pub fn source_fingerprint(range: &str, sources: &[String]) -> u64 {
    use std::hash::{Hash, Hasher};

    let Some(home) = dirs::home_dir() else {
        return 0;
    };
    let start = range_start(range);

    let mut entries: Vec<(String, i64, u64)> = Vec::new();
    let mut collect = |pattern: PathBuf| {
        for p in glob_source(&pattern) {
            if mtime_secs(&p) < start {
                continue;
            }
            let len = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            entries.push((p.to_string_lossy().into_owned(), mtime_secs(&p), len));
        }
    };
    if wants(sources, "codex") {
        collect(home.join(".codex/sessions/**/rollout-*.jsonl"));
    }
    if wants(sources, "claude") {
        collect(home.join(".claude/projects/**/*.jsonl"));
    }
    if wants(sources, "grok") {
        collect(home.join(".grok/sessions/**/updates.jsonl"));
    }

    // Sort so the hash is independent of glob/filesystem enumeration order.
    entries.sort();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    entries.hash(&mut hasher);
    hasher.finish()
}

/// Compute analytics for a range, scoped to the selected `sources`.
///
/// Skips the scan outright rather than scanning then discarding: `scan_*` walks
/// a whole directory tree, and an unselected source's files are pure waste.
pub fn compute_with(range: &str, sources: &[String]) -> Analytics {
    // 每輪掃描前取一次 override 表:pricing::current() 內部 stat mtime,沒變就回快取,
    // 變了才重讀(檔案不存在 = 空表 = 純 vendored,零成本路徑)。
    //
    // T-perf-004: load the per-file scan cache once, share it across the three
    // scanners, persist only if something was re-parsed this round. The cache
    // stores parsed events (not raw messages); cost is recomputed per-scan so a
    // pricing-override change is honoured without invalidating the cache.
    let run = RefCell::new(CacheRun {
        cache: ScanCache::load(),
        stats: CacheStats::default(),
    });
    let out = compute_routed(
        range,
        sources,
        |acc, start| scan_codex_cached(acc, start, &run),
        |acc, start| scan_claude_cached(acc, start, &run),
        |acc, start| scan_grok_cached(acc, start, &run),
        detect_accounts(),
        crate::pricing::current(),
    );
    let run = run.into_inner();
    // Only write when this round actually parsed a file (ticket 規格 4).
    if run.stats.parsed > 0 {
        run.cache.save_best_effort();
    }
    if std::env::var_os("TOKENBAR_DEBUG").is_some() {
        eprintln!(
            "[tb] scan cache: {} hit / {} parsed",
            run.stats.hit, run.stats.parsed
        );
    }
    out
}

/// The real body of `compute_with`, with every source of ambient state (the
/// directory scans and account detection) passed in.
///
/// This split exists purely so the source routing below is testable: the
/// scanners read the real home dir, so a test that cannot replace them can only
/// re-assert `wants`, which proves nothing about which branch runs.
#[allow(clippy::too_many_arguments)]
fn compute_routed<C, L, K>(
    range: &str,
    sources: &[String],
    scan_codex_fn: C,
    scan_claude_fn: L,
    scan_grok_fn: K,
    accounts: Vec<Account>,
    pricing: Arc<PricingOverride>,
) -> Analytics
where
    C: FnOnce(&mut Acc, i64) -> u32,
    L: FnOnce(&mut Acc, i64),
    K: FnOnce(&mut Acc, i64),
{
    let t0 = Instant::now();
    let now = chrono::Utc::now().timestamp();
    let days_back = days_back_for(range);
    let start = range_start(range);

    let mut acc = Acc::new(now);
    // 在任何 scanner 跑之前塞入 override 表,cost 才會走 override → vendored → blended 鏈。
    acc.pricing = pricing;
    // Each source scans iff selected. A source with no local data simply
    // contributes nothing (no fake 0 card — `book` drops zero-token rows and
    // byAgent only holds keys that actually had usage).
    let sessions = if wants(sources, "codex") {
        scan_codex_fn(&mut acc, start)
    } else {
        0
    };
    if wants(sources, "claude") {
        scan_claude_fn(&mut acc, start);
    }
    if wants(sources, "grok") {
        scan_grok_fn(&mut acc, start);
    }
    let scan_stats = std::mem::take(&mut acc.stats);

    let mut daily: Vec<DayPoint> = Vec::new();
    let mut by_model: HashMap<String, u64> = HashMap::new();
    let mut by_agent: HashMap<String, u64> = HashMap::new();
    let mut total_tokens = 0u64;
    let mut total_cost = 0.0;
    let mut best = BestDay {
        date: String::new(),
        cost_usd: 0.0,
    };

    for i in 0..=days_back {
        let date = date_str(start + i * 86400);
        let agg = acc.days.remove(&date).unwrap_or_default();
        for (k, v) in &agg.by_model {
            *by_model.entry(k.clone()).or_default() += v;
            total_tokens += v;
        }
        for (k, v) in &agg.by_agent {
            *by_agent.entry(k.clone()).or_default() += v;
        }
        total_cost += agg.cost;
        if agg.cost > best.cost_usd {
            best = BestDay {
                date: date.clone(),
                cost_usd: agg.cost,
            };
        }
        daily.push(DayPoint {
            date,
            by_model: agg.by_model,
            by_agent: agg.by_agent,
            cost_usd: agg.cost,
        });
    }
    if best.date.is_empty() && !daily.is_empty() {
        best.date = daily.last().unwrap().date.clone();
    }

    let active_days = daily.iter().filter(|d| !d.by_agent.is_empty()).count() as u32;
    let local_now = chrono::Local::now();
    let records = records_for(
        &acc,
        chrono::NaiveDate::from_ymd_opt(local_now.year(), local_now.month(), local_now.day())
            .unwrap(),
        local_now.hour() as u8,
    );

    // Actual start: the first day with activity, so a "month" backed by only a
    // few days of logs reports its true reach instead of claiming 30. Falls
    // back to the nominal window start when nothing was recorded at all.
    let range_start_day = daily
        .iter()
        .find(|d| !d.by_agent.is_empty())
        .or_else(|| daily.first())
        .map(|d| d.date.clone())
        .unwrap_or_default();

    // Activity types (Claude only), sorted by tokens desc for a stable donut.
    let mut by_kind: Vec<KindCount> = acc
        .by_kind
        .into_iter()
        .map(|(kind, tokens)| KindCount { kind, tokens })
        .collect();
    by_kind.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.kind.cmp(&b.kind)));

    // Projects: top 8 by tokens, the rest merged under "__other__".
    let mut projects: Vec<(String, u64)> = acc.by_project.into_iter().collect();
    projects.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut by_project: Vec<ProjectCount> = Vec::new();
    let mut other_project = 0u64;
    for (i, (name, tokens)) in projects.into_iter().enumerate() {
        if i < 8 {
            by_project.push(ProjectCount { name, tokens });
        } else {
            other_project += tokens;
        }
    }
    if other_project > 0 {
        by_project.push(ProjectCount {
            name: "__other__".to_string(),
            tokens: other_project,
        });
    }

    let out = Analytics {
        range: range.to_string(),
        range_start_day,
        total_tokens,
        total_cost_usd: total_cost,
        best_day: best,
        active_days,
        records,
        daily,
        hourly: acc.hourly.to_vec(),
        hourly_cost: acc.hourly_cost.to_vec(),
        by_model,
        by_agent,
        by_model_cost: acc.by_model_cost,
        by_agent_cost: acc.by_agent_cost,
        breakdown: acc.breakdown,
        by_kind,
        by_project,
        sessions_this_week: sessions,
        tok_per_min: (acc.recent_tokens as f64 / 10.0) as u64,
        accounts: filter_accounts(sources, accounts),
    };
    log_scan_stats(
        range,
        sources,
        &scan_stats,
        t0.elapsed().as_millis(),
    );
    out
}

// ── Codex: cumulative token events converted to per-event deltas ─────────
//
// 階段 C+ 資料源勘察結論(2026-07-17,本機真實 log 抽樣,只看結構):
//
// Codex rollout-*.jsonl 每行 `{timestamp,type,payload}`。可用於本階段的欄位:
//   · payload.type == "token_count" 帶 `total_token_usage` 累計值；同檔逐筆
//     差分後，依各事件 timestamp 歸屬。
//   · session_meta(通常首行)/ turn_context 的 `payload.cwd` = 專案工作目錄。
//     → 專案維度:取 cwd 的 basename 當專案名(見 first_cwd_basename)。
//
// 活動類型「無法可靠分類」:Codex 的用量是每回合模型輸出的累計 total,和工具
//   事件(function_call/custom_tool_call `exec`、patch_apply_end、web_search_end
//   …)是分開的記錄,token 無法歸屬到個別工具。故 **by_kind 不含 Codex**
//   (計畫硬規定:無法分類就不出假類別),donut 只反映 Claude 活動。

fn scan_codex_cached(acc: &mut Acc, start: i64, run: &RefCell<CacheRun>) -> u32 {
    let Some(home) = dirs::home_dir() else {
        return 0;
    };
    let pattern = home
        .join(".codex/sessions/**/rollout-*.jsonl")
        .to_string_lossy()
        .replace('\\', "/");
    let Ok(paths) = glob::glob(&pattern) else {
        return 0;
    };
    let mut seen = HashSet::new();
    scan_codex_paths(acc, start, paths.filter_map(Result::ok), Some(run), &mut seen)
}

/// Shared per-file Codex loop. `run` = Some to use the disk cache, None to always
/// parse (the full-scan reference path used by the golden test). Returns the
/// session count (every in-range file, matching the pre-cache behaviour).
fn scan_codex_paths<I>(
    acc: &mut Acc,
    start: i64,
    paths: I,
    run: Option<&RefCell<CacheRun>>,
    seen: &mut HashSet<(i64, u64)>,
) -> u32
where
    I: Iterator<Item = PathBuf>,
{
    let mut sessions = 0;
    for p in paths {
        acc.stats.files_considered += 1;
        if mtime_secs(&p) < start {
            continue;
        }
        sessions += 1;
        let pstr = p.to_string_lossy().into_owned();
        // Fingerprint once when caching; None means unreadable → skip (the old
        // scan treated a metadata/open failure the same way).
        let fp = match run {
            Some(_) => match scan_cache::fingerprint(&p) {
                Some(fp) => Some(fp),
                None => continue,
            },
            None => None,
        };

        if let (Some(r), Some(fp)) = (run, fp.as_ref()) {
            let hit = {
                let mut g = r.borrow_mut();
                match g.cache.get_matching(&pstr, fp) {
                    Some(CachedFile::Codex { project, events }) => {
                        Some((project.clone(), events.clone()))
                    }
                    _ => None,
                }
            };
            if let Some((project, events)) = hit {
                r.borrow_mut().stats.hit += 1;
                let mut previous: Option<CodexUsage> = None;
                for ev in &events {
                    book_codex_event(acc, start, &project, ev, &mut previous, seen);
                }
                continue;
            }
        }

        // Miss (or caching disabled): parse the file.
        let Ok(meta) = fs::metadata(&p) else {
            continue;
        };
        let Ok(file) = File::open(&p) else {
            continue;
        };
        acc.stats.files_read += 1;
        acc.stats.eligible_file_bytes += meta.len();
        let (project, events) = parse_codex_file(file, &mut acc.stats);
        let mut previous: Option<CodexUsage> = None;
        for ev in &events {
            book_codex_event(acc, start, &project, ev, &mut previous, seen);
        }
        if let (Some(r), Some(fp)) = (run, fp) {
            let mut g = r.borrow_mut();
            g.stats.parsed += 1;
            g.cache
                .insert(pstr, fp, CachedFile::Codex { project, events });
        }
    }
    sessions
}

/// Parse one Codex rollout file into (project, token events). The expensive JSON
/// pass — cached so an unchanged file skips it. Discovers cwd from the first 8
/// lines, then collects every `token_count` event (including those lines).
fn parse_codex_file(file: File, stats: &mut ScanStats) -> (String, Vec<CodexEvent>) {
    let mut reader = BufReader::new(file);
    let mut buf = String::new();
    let mut project = String::new();
    let mut events = Vec::new();
    let mut idx = 0u32;
    loop {
        buf.clear();
        match reader.read_line(&mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        let line = buf.trim_end_matches(['\r', '\n']);
        if idx < 8 && project.is_empty() {
            if let Ok(env) = serde_json::from_str::<CodexEnvelope>(line.trim()) {
                if let Some(p) = codex_cwd_from_envelope(&env) {
                    project = p;
                }
            }
        }
        if let Some(ev) = parse_codex_line(line, stats) {
            events.push(ev);
        }
        idx += 1;
    }
    (project, events)
}

/// Parse half of the old `process_codex_line`: line → cumulative token event,
/// with no cross-file state. Diffing/dedup/booking happen in `book_codex_event`.
fn parse_codex_line(line: &str, stats: &mut ScanStats) -> Option<CodexEvent> {
    stats.lines_read += 1;
    if !line.contains("token_count") {
        return None;
    }
    stats.candidate_lines += 1;
    // Typed envelope — unknown large fields ignored by serde.
    let env = serde_json::from_str::<CodexEnvelope>(line.trim()).ok()?;
    stats.json_parse_ok += 1;
    let (ts, (input, cached, output, reasoning)) = codex_token_from_envelope(&env)?;
    Some(CodexEvent {
        ts,
        input,
        cached,
        output,
        reasoning,
    })
}

/// Book half: the per-file cumulative diff + the global (ts,total) replay guard.
/// `previous` is per-file (reset each file); `seen` is the cross-file guard, so a
/// fork/resume that replays an earlier prefix advances the baseline but books
/// nothing (T-fix-002) — identical whether the events came fresh or from cache.
fn book_codex_event(
    acc: &mut Acc,
    start: i64,
    project: &str,
    ev: &CodexEvent,
    previous: &mut Option<CodexUsage>,
    seen: &mut HashSet<(i64, u64)>,
) {
    let current = (ev.input, ev.cached, ev.output, ev.reasoning);
    let total = usage_total(current);
    let duplicate = !seen.insert((ev.ts, total));
    let prior = previous.replace(current).unwrap_or((0, 0, 0, 0));
    if duplicate || ev.ts < start || total.saturating_sub(usage_total(prior)) == 0 {
        return;
    }
    let (i, ca, o, r) = current;
    let (pi, pca, po, pr) = prior;
    let di = i.saturating_sub(pi);
    let dca = ca.saturating_sub(pca);
    let do_ = o.saturating_sub(po);
    let dr = r.saturating_sub(pr);
    // 先算成本再 add:add_with_cost 借 &mut acc,無法在同一呼叫裡再借 acc.pricing。
    let cost = codex_cost(&acc.pricing, "gpt-5-codex", di, dca, do_, dr);
    acc.add_with_cost(
        ev.ts,
        "gpt-5-codex",
        "Codex CLI",
        project,
        None,
        di,
        dca,
        do_,
        dr,
        cost,
    );
}

/// Kept so the `scan_codex_lines` test helper still drives the exact production
/// path: parse one line, then book it with the per-file / cross-file state.
#[cfg(test)]
fn process_codex_line(
    acc: &mut Acc,
    start: i64,
    project: &str,
    line: &str,
    previous: &mut Option<CodexUsage>,
    seen: &mut HashSet<(i64, u64)>,
) {
    if let Some(ev) = parse_codex_line(line, &mut acc.stats) {
        book_codex_event(acc, start, project, &ev, previous, seen);
    }
}

#[derive(Clone, Copy)]
struct ClaudeRates {
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write_5m: f64,
    cache_write_1h: f64,
}

/// Vendored Anthropic API prices in $/Mtok, cached 2026-06-24 (§11).
fn claude_rates(model: &str) -> Option<ClaudeRates> {
    let m = model.to_lowercase();
    let rates = if m.contains("fable-5") || m.contains("mythos-5") {
        ClaudeRates {
            input: 10.00,
            output: 50.00,
            cache_read: 1.00,
            cache_write_5m: 12.50,
            cache_write_1h: 20.00,
        }
    } else if m.contains("opus") {
        ClaudeRates {
            input: 5.00,
            output: 25.00,
            cache_read: 0.50,
            cache_write_5m: 6.25,
            cache_write_1h: 10.00,
        }
    } else if m.contains("sonnet") {
        ClaudeRates {
            input: 3.00,
            output: 15.00,
            cache_read: 0.30,
            cache_write_5m: 3.75,
            cache_write_1h: 6.00,
        }
    } else if m.contains("haiku") {
        ClaudeRates {
            input: 1.00,
            output: 5.00,
            cache_read: 0.10,
            cache_write_5m: 1.25,
            cache_write_1h: 2.00,
        }
    } else {
        return None;
    };
    Some(rates)
}

/// 單一查價入口(T-feat-006):override 精確 → override substring → vendored
/// `claude_rates` 家族表 → blended `rate_per_mtok`。所有計價都收斂走這條鏈,新增
/// 的 override 層只是插在最前面;沒有 override 檔時每一層都回退到原本的行為,故既有
/// 金額斷言一字不變。
fn resolve_rates(over: &PricingOverride, model: &str) -> RateSpec {
    if let Some(spec) = over.lookup(model) {
        return spec;
    }
    if let Some(r) = claude_rates(model) {
        return RateSpec::Component {
            input: r.input,
            output: r.output,
            cache_read: r.cache_read,
            cache_write_5m: r.cache_write_5m,
            cache_write_1h: r.cache_write_1h,
        };
    }
    RateSpec::Blended(rate_per_mtok(model))
}

fn claude_cost(
    over: &PricingOverride,
    model: &str,
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write_5m: u64,
    cache_write_1h: u64,
) -> f64 {
    match resolve_rates(over, model) {
        RateSpec::Component {
            input: ri,
            output: ro,
            cache_read: rcr,
            cache_write_5m: rc5,
            cache_write_1h: rc1,
        } => {
            (input as f64 * ri
                + output as f64 * ro
                + cache_read as f64 * rcr
                + cache_write_5m as f64 * rc5
                + cache_write_1h as f64 * rc1)
                / 1e6
        }
        RateSpec::Blended(rate) => {
            ((input + output + cache_read + cache_write_5m + cache_write_1h) as f64 / 1e6) * rate
        }
    }
}

/// Codex 的 `input` 累計已含 cached,計價把非快取部分與 cached 分開:blended 時 cached
/// 打 0.1 折(沿用既有語意),分項時 cached 走 override 的 cache_read 費率,
/// output+reasoning 走 output 費率。無 override 時 "gpt-5-codex" 落到 blended,行為不變。
fn codex_cost(
    over: &PricingOverride,
    model: &str,
    input: u64,
    cached: u64,
    output: u64,
    reasoning: u64,
) -> f64 {
    match resolve_rates(over, model) {
        RateSpec::Blended(rate) => {
            (input.saturating_sub(cached) as f64 * rate
                + cached as f64 * rate * 0.1
                + (output + reasoning) as f64 * rate)
                / 1e6
        }
        RateSpec::Component {
            input: ri,
            output: ro,
            cache_read: rcr,
            ..
        } => {
            (input.saturating_sub(cached) as f64 * ri
                + cached as f64 * rcr
                + (output + reasoning) as f64 * ro)
                / 1e6
        }
    }
}

type CodexUsage = (u64, u64, u64, u64);

/// Minimal Codex JSONL envelope — ignores large unknown content fields.
#[derive(Deserialize)]
struct CodexEnvelope {
    timestamp: Option<serde_json::Value>,
    payload: Option<CodexPayload>,
}

#[derive(Deserialize)]
struct CodexPayload {
    #[serde(rename = "type")]
    kind: Option<String>,
    cwd: Option<String>,
    info: Option<CodexInfo>,
    total_token_usage: Option<CodexUsageFields>,
}

#[derive(Deserialize)]
struct CodexInfo {
    total_token_usage: Option<CodexUsageFields>,
}

#[derive(Deserialize)]
struct CodexUsageFields {
    input_tokens: Option<u64>,
    cached_input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    reasoning_output_tokens: Option<u64>,
}

fn codex_token_from_envelope(env: &CodexEnvelope) -> Option<(i64, CodexUsage)> {
    let payload = env.payload.as_ref()?;
    if payload.kind.as_deref() != Some("token_count") {
        return None;
    }
    let usage = payload
        .info
        .as_ref()
        .and_then(|i| i.total_token_usage.as_ref())
        .or(payload.total_token_usage.as_ref())?;
    let ts = env.timestamp.as_ref().and_then(parse_epoch)?;
    Some((
        ts,
        (
            usage.input_tokens.unwrap_or(0),
            usage.cached_input_tokens.unwrap_or(0),
            usage.output_tokens.unwrap_or(0),
            usage.reasoning_output_tokens.unwrap_or(0),
        ),
    ))
}

fn codex_cwd_from_envelope(env: &CodexEnvelope) -> Option<String> {
    env.payload
        .as_ref()
        .and_then(|p| p.cwd.as_ref())
        .map(|c| basename(c))
}

fn usage_total((input, cached, output, reasoning): CodexUsage) -> u64 {
    input
        .saturating_add(cached)
        .saturating_add(output)
        .saturating_add(reasoning)
}

/// Test helper: feed pre-split lines (no filesystem) through the same
/// `process_codex_line` path production uses.
#[cfg(test)]
fn scan_codex_lines<I>(
    acc: &mut Acc,
    start: i64,
    project: &str,
    lines: I,
    seen: &mut HashSet<(i64, u64)>,
) where
    I: Iterator<Item = String>,
{
    let mut previous: Option<CodexUsage> = None;
    for line in lines {
        process_codex_line(acc, start, project, &line, &mut previous, seen);
    }
}

fn mtime_secs(p: &PathBuf) -> i64 {
    fs::metadata(p)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ── Claude: per-message usage from projects/*.jsonl ──────────────────────
//
// 階段 C+ 資料源勘察結論(2026-07-17,本機真實 log 抽樣,只看結構):
//
// ~/.claude/projects/<slug>/<uuid>.jsonl,每行一則記錄。assistant 訊息帶
//   `message.usage`(現行聚合來源)與 `message.content[]`,其中 type=="tool_use"
//   的項目有 `name`(標準工具名:Edit/Write/Read/Grep/Glob/Bash/PowerShell/
//   WebSearch… 及 mcp__* / Task* / Agent)。
//   → 活動類型:依 tool name 分類(classify_kind),把該訊息 token 記到主要
//     工具類別;無 tool_use 的訊息 → "other"。Claude **可分類**。
//   → 專案維度:檔案的上層目錄名(<slug>)即專案。
//
// (工具名是工具 schema 的一部分,非對話內容;此處只讀 name 與目錄名,不觸碰
//  訊息文字/參數/檔案路徑細節。)

fn scan_claude_cached(acc: &mut Acc, start: i64, run: &RefCell<CacheRun>) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let pattern = home
        .join(".claude/projects/**/*.jsonl")
        .to_string_lossy()
        .replace('\\', "/");
    let Ok(paths) = glob::glob(&pattern) else {
        return;
    };
    let mut seen = HashSet::new();
    scan_claude_paths(acc, start, paths.filter_map(Result::ok), Some(run), &mut seen);
}

/// Shared per-file Claude loop. `run` = Some to use the disk cache, None to
/// always parse (the full-scan reference path used by the golden test). Project
/// is derived from the path (parent slug), so it is recomputed on a hit rather
/// than cached.
fn scan_claude_paths<I>(
    acc: &mut Acc,
    start: i64,
    paths: I,
    run: Option<&RefCell<CacheRun>>,
    seen: &mut HashSet<String>,
) where
    I: Iterator<Item = PathBuf>,
{
    for p in paths {
        acc.stats.files_considered += 1;
        if mtime_secs(&p) < start {
            continue;
        }
        // Project = the immediate parent directory's slug name.
        let project = p
            .parent()
            .and_then(|d| d.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let pstr = p.to_string_lossy().into_owned();
        let fp = match run {
            Some(_) => match scan_cache::fingerprint(&p) {
                Some(fp) => Some(fp),
                None => continue,
            },
            None => None,
        };

        if let (Some(r), Some(fp)) = (run, fp.as_ref()) {
            let hit = {
                let mut g = r.borrow_mut();
                match g.cache.get_matching(&pstr, fp) {
                    Some(CachedFile::Claude(events)) => Some(events.clone()),
                    _ => None,
                }
            };
            if let Some(events) = hit {
                r.borrow_mut().stats.hit += 1;
                for ev in &events {
                    book_claude_event(acc, start, &project, ev, seen);
                }
                continue;
            }
        }

        // Miss (or caching disabled): parse the file.
        let Ok(meta) = fs::metadata(&p) else {
            continue;
        };
        let Ok(file) = File::open(&p) else {
            continue;
        };
        acc.stats.files_read += 1;
        acc.stats.eligible_file_bytes += meta.len();
        let events = parse_claude_file(file, &mut acc.stats);
        for ev in &events {
            book_claude_event(acc, start, &project, ev, seen);
        }
        if let (Some(r), Some(fp)) = (run, fp) {
            let mut g = r.borrow_mut();
            g.stats.parsed += 1;
            g.cache.insert(pstr, fp, CachedFile::Claude(events));
        }
    }
}

/// Parse one Claude session file into booked-candidate events (the expensive
/// JSON pass, cached so an unchanged file skips it).
fn parse_claude_file(file: File, stats: &mut ScanStats) -> Vec<ClaudeEvent> {
    let mut reader = BufReader::new(file);
    let mut buf = String::new();
    let mut out = Vec::new();
    loop {
        buf.clear();
        match reader.read_line(&mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        let line = buf.trim_end_matches(['\r', '\n']);
        if let Some(ev) = parse_claude_line(line, stats) {
            out.push(ev);
        }
    }
    out
}

#[cfg(test)]
fn scan_claude_lines(
    acc: &mut Acc,
    start: i64,
    project: &str,
    lines: impl Iterator<Item = String>,
    seen: &mut HashSet<String>,
) {
    for line in lines {
        scan_claude_line(acc, start, project, &line, seen);
    }
}

/// Minimal Claude JSONL fields — large message content text is not retained
/// as owned strings beyond tool names we need for kind classification.
#[derive(Deserialize)]
struct ClaudeEnvelope {
    timestamp: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    uuid: Option<String>,
    message: Option<ClaudeMessage>,
}

#[derive(Deserialize)]
struct ClaudeMessage {
    id: Option<String>,
    model: Option<String>,
    usage: Option<ClaudeUsage>,
    content: Option<Vec<ClaudeContentPart>>,
}

#[derive(Deserialize)]
struct ClaudeUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_creation: Option<ClaudeCacheCreation>,
}

#[derive(Deserialize)]
struct ClaudeCacheCreation {
    ephemeral_1h_input_tokens: Option<u64>,
    ephemeral_5m_input_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct ClaudeContentPart {
    #[serde(rename = "type")]
    kind: Option<String>,
    name: Option<String>,
}

/// Parse half: line → booked-candidate event, no cross-file state. The `ts<start`
/// gate and the dedup live in `book_claude_event` so a cached event stays
/// range-independent (one cache serves today/week/month).
fn parse_claude_line(line: &str, stats: &mut ScanStats) -> Option<ClaudeEvent> {
    stats.lines_read += 1;
    if !line.contains("\"usage\"") {
        return None;
    }
    stats.candidate_lines += 1;
    let env = serde_json::from_str::<ClaudeEnvelope>(line).ok()?;
    stats.json_parse_ok += 1;
    let msg = env.message.as_ref();
    let usage = msg.and_then(|m| m.usage.as_ref())?;
    let ts = env
        .timestamp
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.timestamp())
        .unwrap_or(0);
    let dedup_key = env
        .request_id
        .as_deref()
        .or_else(|| msg.and_then(|m| m.id.as_deref()))
        .or(env.uuid.as_deref())
        .map(|s| s.to_string());
    let model = msg
        .and_then(|m| m.model.as_deref())
        .unwrap_or("claude")
        .to_string();
    let mut tools: Vec<String> = Vec::new();
    if let Some(content) = msg.and_then(|m| m.content.as_ref()) {
        for it in content {
            if it.kind.as_deref() == Some("tool_use") {
                if let Some(name) = it.name.as_ref() {
                    tools.push(name.clone());
                }
            }
        }
    }
    let kind = message_kind(&tools).to_string();
    let input = usage.input_tokens.unwrap_or(0);
    let output = usage.output_tokens.unwrap_or(0);
    let cache_read = usage.cache_read_input_tokens.unwrap_or(0);
    let cache_creation = usage.cache_creation_input_tokens.unwrap_or(0);
    let cache_write_1h = usage
        .cache_creation
        .as_ref()
        .and_then(|c| c.ephemeral_1h_input_tokens)
        .unwrap_or(0);
    let cache_write_5m = usage
        .cache_creation
        .as_ref()
        .and_then(|c| c.ephemeral_5m_input_tokens)
        .unwrap_or_else(|| cache_creation.saturating_sub(cache_write_1h));
    let cached = cache_read + cache_creation;
    Some(ClaudeEvent {
        dedup_key,
        ts,
        model,
        kind,
        input,
        output,
        cache_read,
        cache_write_5m,
        cache_write_1h,
        cached,
    })
}

/// Book half: the global dedup (T-fix-001 — requestId/message.id/uuid priority
/// already resolved into `dedup_key`) + cost + aggregation. `seen` is the
/// cross-file set, so a resume/fork copy of a message is counted once whether the
/// event came fresh from a parse or from the cache — this is ticket 規格 5.
fn book_claude_event(
    acc: &mut Acc,
    start: i64,
    project: &str,
    ev: &ClaudeEvent,
    seen: &mut HashSet<String>,
) {
    // ts<start is checked before the dedup insert (as the pre-cache scan did), so
    // an out-of-range copy never reserves a key an in-range copy needs.
    if ev.ts < start {
        return;
    }
    if let Some(key) = ev.dedup_key.as_deref() {
        if !seen.insert(key.to_string()) {
            return;
        }
    }
    let cost = claude_cost(
        &acc.pricing,
        &ev.model,
        ev.input,
        ev.output,
        ev.cache_read,
        ev.cache_write_5m,
        ev.cache_write_1h,
    );
    acc.add_with_cost(
        ev.ts,
        &ev.model,
        "Claude Code",
        project,
        Some(ev.kind.as_str()),
        ev.input,
        ev.cached,
        ev.output,
        0,
        cost,
    );
}

/// Kept so the `scan_claude_lines` test helper still drives the exact production
/// path: parse one line, then book it with the cross-file dedup set.
#[cfg(test)]
fn scan_claude_line(
    acc: &mut Acc,
    start: i64,
    project: &str,
    line: &str,
    seen: &mut HashSet<String>,
) {
    if let Some(ev) = parse_claude_line(line, &mut acc.stats) {
        book_claude_event(acc, start, project, &ev, seen);
    }
}

// ── Grok CLI: cumulative session totals (T-916) ───────────────────────────
//
// 資料源(勘察已定案,見 T-916 brief; 語意複核 2026-07-19):
//   ~/.grok/sessions/<url-encoded-cwd>/<session-id>/updates.jsonl
//   每行一個 JSON 物件,`timestamp` = unix epoch 秒。
//
//   Token 累計(主路徑,實測 2026-07-19):
//     `params._meta.totalTokens` — 從 session 起算的累計 u64。
//     (舊/簡化形狀也可能在頂層 `_meta.totalTokens`。)
//
//   Model id **不在 token 同行**(真實檔 0 筆 co-locate):
//     出現在先前的 `params.update._meta.modelId`(如 user_message_chunk)。
//     每檔先出現 model update,再出現 token event。Scanner 必須保留 per-file
//     `current_model`,token 行缺 model 時沿用;同檔中途換 model 則更新。
//
// 和 Codex 一樣把累計值逐筆差分成單筆增量(同 monotonic-diff);但重置(數值下降)
// 的處理不同:視為新 baseline —— 把當前累計值當成增量,而非 saturating 到 0。
//
// 專案維度:對 <url-encoded-cwd> 路徑段做百分號解碼後取 basename(usage-only;
// §0 天然不進戰報,shares 不讀 by_project)。
//
// 誠實取捨:
//   · 無 input/output/cache 拆分 → 走 add_total_only(整筆記進總量,不進 breakdown
//     的假類別)。
//   · 無公開定價 → cost 0.0(含 Grok 時 est. 成本會低估,刻意不臆造費率)。
//   · 無法可靠分類 → 不進 by_kind(同 Codex 的硬規定)。

/// Percent-decode a single URL path segment (`%2F` → `/`, `%3A` → `:`, …). Any
/// malformed escape is left literal. Small and dependency-free so it stays
/// unit-testable; only used to recover the project folder from an encoded cwd.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Project folder for a Grok session from its file path:
/// `.grok/sessions/<url-encoded-cwd>/<session-id>/updates.jsonl` → decode the
/// `<url-encoded-cwd>` segment (the file's grandparent dir) and take its
/// basename. "" when the layout doesn't match.
fn grok_project_from_path(path: &Path) -> String {
    let encoded = path
        .parent() // <session-id>/
        .and_then(|p| p.parent()) // <url-encoded-cwd>/
        .and_then(|d| d.file_name())
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    basename(&percent_decode(&encoded))
}

/// Minimal Grok updates.jsonl fields (content bodies ignored by serde).
#[derive(Deserialize)]
struct GrokEnvelope {
    timestamp: Option<serde_json::Value>,
    #[serde(rename = "_meta")]
    meta: Option<GrokMeta>,
    params: Option<GrokParams>,
}

#[derive(Deserialize)]
struct GrokParams {
    #[serde(rename = "_meta")]
    meta: Option<GrokMeta>,
    update: Option<GrokUpdate>,
}

#[derive(Deserialize)]
struct GrokUpdate {
    #[serde(rename = "_meta")]
    meta: Option<GrokMeta>,
}

#[derive(Deserialize, Default)]
struct GrokMeta {
    #[serde(rename = "totalTokens")]
    total_tokens: Option<u64>,
    #[serde(rename = "modelId")]
    model_id: Option<String>,
}

fn grok_model_id_from_env(env: &GrokEnvelope) -> Option<String> {
    env.params
        .as_ref()
        .and_then(|p| p.update.as_ref())
        .and_then(|u| u.meta.as_ref())
        .and_then(|m| m.model_id.clone())
        .or_else(|| {
            env.params
                .as_ref()
                .and_then(|p| p.meta.as_ref())
                .and_then(|m| m.model_id.clone())
        })
        .or_else(|| env.meta.as_ref().and_then(|m| m.model_id.clone()))
        .filter(|s| !s.is_empty())
}

fn grok_token_from_env(env: &GrokEnvelope) -> Option<(i64, u64)> {
    let total = env
        .meta
        .as_ref()
        .and_then(|m| m.total_tokens)
        .or_else(|| {
            env.params
                .as_ref()
                .and_then(|p| p.meta.as_ref())
                .and_then(|m| m.total_tokens)
        })?;
    let ts = env.timestamp.as_ref().and_then(parse_epoch)?;
    Some((ts, total))
}

#[cfg(test)]
fn grok_event(line: &str) -> Option<(i64, u64, Option<String>)> {
    if !line.contains("totalTokens") {
        return None;
    }
    let env: GrokEnvelope = serde_json::from_str(line.trim()).ok()?;
    let (ts, total) = grok_token_from_env(&env)?;
    Some((ts, total, grok_model_id_from_env(&env)))
}

#[cfg(test)]
fn scan_grok_lines<I>(
    acc: &mut Acc,
    start: i64,
    project: &str,
    lines: I,
    seen: &mut HashSet<(i64, u64)>,
) where
    I: Iterator<Item = String>,
{
    let mut previous: u64 = 0;
    let mut current_model = String::from("grok");
    for line in lines {
        process_grok_line(
            acc,
            start,
            project,
            &line,
            &mut previous,
            &mut current_model,
            seen,
        );
    }
}

/// Parse half: line → Grok event (optional sticky model id and/or a cumulative
/// token reading). Baseline diff + replay guard run in `book_grok_event`.
fn parse_grok_line(line: &str, stats: &mut ScanStats) -> Option<GrokEvent> {
    stats.lines_read += 1;
    if !line.contains("totalTokens") && !line.contains("modelId") {
        return None;
    }
    stats.candidate_lines += 1;
    let env = serde_json::from_str::<GrokEnvelope>(line.trim()).ok()?;
    stats.json_parse_ok += 1;
    Some(GrokEvent {
        model_update: grok_model_id_from_env(&env),
        token: grok_token_from_env(&env),
    })
}

/// Book half: sticky per-file model + per-file cumulative baseline + the global
/// (ts,total) replay guard. A reset (value drop) is a new baseline, not a
/// saturating-to-zero (T-916). `previous`/`current_model` are per-file; `seen` is
/// cross-file — identical whether events came fresh or from cache.
fn book_grok_event(
    acc: &mut Acc,
    start: i64,
    project: &str,
    ev: &GrokEvent,
    previous: &mut u64,
    current_model: &mut String,
    seen: &mut HashSet<(i64, u64)>,
) {
    if let Some(model) = ev.model_update.as_ref() {
        *current_model = model.clone();
    }
    let Some((ts, cumulative)) = ev.token else {
        return;
    };
    let duplicate = !seen.insert((ts, cumulative));
    let delta = if cumulative >= *previous {
        cumulative - *previous
    } else {
        cumulative
    };
    *previous = cumulative;
    if duplicate || ts < start || delta == 0 {
        return;
    }
    acc.add_total_only(ts, current_model, "Grok CLI", project, delta, 0.0);
}

/// Kept so the `scan_grok_lines` test helper drives the exact production path.
#[cfg(test)]
fn process_grok_line(
    acc: &mut Acc,
    start: i64,
    project: &str,
    line: &str,
    previous: &mut u64,
    current_model: &mut String,
    seen: &mut HashSet<(i64, u64)>,
) {
    if let Some(ev) = parse_grok_line(line, &mut acc.stats) {
        book_grok_event(acc, start, project, &ev, previous, current_model, seen);
    }
}

/// Parse one Grok updates file into events (the expensive JSON pass, cached).
fn parse_grok_file(file: File, stats: &mut ScanStats) -> Vec<GrokEvent> {
    let mut reader = BufReader::new(file);
    let mut buf = String::new();
    let mut out = Vec::new();
    loop {
        buf.clear();
        match reader.read_line(&mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        let line = buf.trim_end_matches(['\r', '\n']);
        if let Some(ev) = parse_grok_line(line, stats) {
            out.push(ev);
        }
    }
    out
}

fn scan_grok_cached(acc: &mut Acc, start: i64, run: &RefCell<CacheRun>) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let pattern = home
        .join(".grok/sessions/**/updates.jsonl")
        .to_string_lossy()
        .replace('\\', "/");
    let Ok(paths) = glob::glob(&pattern) else {
        return;
    };
    let mut seen = HashSet::new();
    scan_grok_paths(acc, start, paths.filter_map(Result::ok), Some(run), &mut seen);
}

/// Shared per-file Grok loop. `run` = Some to use the disk cache, None to always
/// parse. Project is derived from the path, so it is recomputed on a hit.
fn scan_grok_paths<I>(
    acc: &mut Acc,
    start: i64,
    paths: I,
    run: Option<&RefCell<CacheRun>>,
    seen: &mut HashSet<(i64, u64)>,
) where
    I: Iterator<Item = PathBuf>,
{
    for p in paths {
        acc.stats.files_considered += 1;
        if mtime_secs(&p) < start {
            continue;
        }
        let project = grok_project_from_path(&p);
        let pstr = p.to_string_lossy().into_owned();
        let fp = match run {
            Some(_) => match scan_cache::fingerprint(&p) {
                Some(fp) => Some(fp),
                None => continue,
            },
            None => None,
        };

        if let (Some(r), Some(fp)) = (run, fp.as_ref()) {
            let hit = {
                let mut g = r.borrow_mut();
                match g.cache.get_matching(&pstr, fp) {
                    Some(CachedFile::Grok(events)) => Some(events.clone()),
                    _ => None,
                }
            };
            if let Some(events) = hit {
                r.borrow_mut().stats.hit += 1;
                let mut previous: u64 = 0;
                let mut current_model = String::from("grok");
                for ev in &events {
                    book_grok_event(
                        acc,
                        start,
                        &project,
                        ev,
                        &mut previous,
                        &mut current_model,
                        seen,
                    );
                }
                continue;
            }
        }

        let Ok(meta) = fs::metadata(&p) else {
            continue;
        };
        let Ok(file) = File::open(&p) else {
            continue;
        };
        acc.stats.files_read += 1;
        acc.stats.eligible_file_bytes += meta.len();
        let events = parse_grok_file(file, &mut acc.stats);
        let mut previous: u64 = 0;
        let mut current_model = String::from("grok");
        for ev in &events {
            book_grok_event(
                acc,
                start,
                &project,
                ev,
                &mut previous,
                &mut current_model,
                seen,
            );
        }
        if let (Some(r), Some(fp)) = (run, fp) {
            let mut g = r.borrow_mut();
            g.stats.parsed += 1;
            g.cache.insert(pstr, fp, CachedFile::Grok(events));
        }
    }
}

/// Epoch seconds from a JSON value that may be epoch millis, epoch seconds, or
/// an RFC3339 string. Millis are distinguished by magnitude (> ~Sat 2286 in
/// seconds), which is safe for any realistic log timestamp.
fn parse_epoch(v: &serde_json::Value) -> Option<i64> {
    if let Some(n) = v.as_i64() {
        return Some(if n > 10_000_000_000 { n / 1000 } else { n });
    }
    if let Some(s) = v.as_str() {
        return chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|d| d.timestamp());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_sources(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    /// Convenience wrapper: run `compute_routed` with the Grok scanner disabled,
    /// so every codex/claude test reads exactly as before. The Grok tests call
    /// `compute_routed` directly.
    fn routed<C, L>(
        range: &str,
        sources: &[&str],
        codex: C,
        claude: L,
        accounts: Vec<Account>,
    ) -> Analytics
    where
        C: FnOnce(&mut Acc, i64) -> u32,
        L: FnOnce(&mut Acc, i64),
    {
        compute_routed(
            range,
            &to_sources(sources),
            codex,
            claude,
            |_, _| {},
            accounts,
            Arc::new(PricingOverride::empty()),
        )
    }

    /// The scan helpers hit the real home dir, so these assert on the pure
    /// membership decision instead: which scans the source list authorises.
    #[test]
    fn membership_gates_each_scan() {
        let claude_only = to_sources(&["claude"]);
        assert!(!wants(&claude_only, "codex"));
        assert!(wants(&claude_only, "claude"));

        let codex_only = to_sources(&["codex"]);
        assert!(wants(&codex_only, "codex"));
        assert!(!wants(&codex_only, "claude"));

        let both = to_sources(&["claude", "codex"]);
        assert!(wants(&both, "codex"));
        assert!(wants(&both, "claude"));

        // Empty means nothing is scanned — an honest empty page, no fallback.
        let none: Vec<String> = Vec::new();
        for id in ["claude", "codex", "grok"] {
            assert!(!wants(&none, id), "empty sources must not scan {id}");
        }
    }

    #[test]
    fn accounts_follow_the_sources() {
        let only_claude = filter_accounts(
            &to_sources(&["claude"]),
            vec![
                Account {
                    client: "Claude Code".into(),
                    provider: "anthropic".into(),
                    account: "—".into(),
                    plan: "Claude".into(),
                },
                Account {
                    client: "Codex CLI".into(),
                    provider: "codex".into(),
                    account: "—".into(),
                    plan: "—".into(),
                },
            ],
        );
        assert_eq!(only_claude.len(), 1);
        assert_eq!(only_claude[0].provider, "anthropic");
    }

    // ── real routing: which scan `compute_routed` actually runs ──────────
    //
    // Stub scanners stand in for the two directory walks, so these observe
    // the branch that ran instead of restating the predicate. No user data
    // is touched. Each stub tags its tokens with its own agent name, and the
    // agents present in the output name the scans that happened.

    const CODEX_AGENT: &str = "Codex CLI";
    const CLAUDE_AGENT: &str = "Claude Code";

    fn stub_codex(acc: &mut Acc, _start: i64) -> u32 {
        acc.add(acc.now, "gpt-5-codex", CODEX_AGENT, "", None, 100, 0, 0, 0);
        7
    }

    fn stub_claude(acc: &mut Acc, _start: i64) {
        acc.add(acc.now, "claude-opus", CLAUDE_AGENT, "", None, 200, 0, 0, 0);
    }

    /// Agents that actually got scanned, for `sources`, sorted.
    fn scanned_agents(sources: &[&str]) -> Vec<String> {
        let a = routed("today", sources, stub_codex, stub_claude, Vec::new());
        let mut names: Vec<String> = a.by_agent.keys().cloned().collect();
        names.sort();
        names
    }

    #[test]
    fn claude_source_routes_to_claude_scan_only() {
        assert_eq!(scanned_agents(&["claude"]), vec![CLAUDE_AGENT.to_string()]);
    }

    #[test]
    fn codex_source_routes_to_codex_scan_only() {
        assert_eq!(scanned_agents(&["codex"]), vec![CODEX_AGENT.to_string()]);
    }

    #[test]
    fn both_sources_route_to_every_scan() {
        assert_eq!(
            scanned_agents(&["claude", "codex"]),
            vec![CLAUDE_AGENT.to_string(), CODEX_AGENT.to_string()]
        );
    }

    /// The core gating guarantee: a source absent from the list is never
    /// scanned, and an empty list scans nothing at all.
    #[test]
    fn absent_source_is_never_scanned() {
        assert_eq!(scanned_agents(&["claude"]), vec![CLAUDE_AGENT.to_string()]);
        assert_eq!(scanned_agents(&["codex"]), vec![CODEX_AGENT.to_string()]);
        assert!(scanned_agents(&[]).is_empty(), "empty sources scanned something");
    }

    /// Totals, not just agent names: a skipped scan must take its tokens and
    /// its session count with it.
    #[test]
    fn skipped_codex_scan_drops_its_tokens_and_sessions() {
        let claude_only = routed("today", &["claude"], stub_codex, stub_claude, Vec::new());
        assert_eq!(claude_only.total_tokens, 200);
        assert_eq!(claude_only.sessions_this_week, 0);

        let codex_only = routed("today", &["codex"], stub_codex, stub_claude, Vec::new());
        assert_eq!(codex_only.total_tokens, 100);
        assert_eq!(codex_only.sessions_this_week, 7);

        let everything = routed("today", &["claude", "codex"], stub_codex, stub_claude, Vec::new());
        assert_eq!(everything.total_tokens, 300);
        assert_eq!(everything.sessions_this_week, 7);
    }

    // ── month range (階段 C) ─────────────────────────────────────────────
    //
    // The month window is 30 daily buckets, but local logs are often shorter.
    // These pin the two facts the frontend relies on: the bucket count, and
    // `range_start_day` reporting the true earliest day of data.

    fn no_codex(_acc: &mut Acc, _start: i64) -> u32 {
        0
    }

    /// Claude activity only on the last `days` days (today, yesterday, …).
    fn stub_recent(days: i64) -> impl Fn(&mut Acc, i64) {
        move |acc: &mut Acc, _start: i64| {
            for k in 0..days {
                acc.add(
                    acc.now - k * 86400,
                    "claude-opus",
                    CLAUDE_AGENT,
                    "",
                    None,
                    100,
                    0,
                    0,
                    0,
                );
            }
        }
    }

    #[test]
    fn month_range_spans_thirty_daily_buckets() {
        let a = routed("month", &["claude"], no_codex, stub_recent(3), Vec::new());
        assert_eq!(a.daily.len(), 30);
        assert_eq!(a.total_tokens, 300); // 3 days × 100
    }

    #[test]
    fn month_range_reports_actual_start_when_history_is_short() {
        let a = routed("month", &["claude"], no_codex, stub_recent(3), Vec::new());
        // daily runs oldest→newest over 30 buckets: today is [29], so the
        // earliest of the last three active days is [27].
        assert_eq!(a.range_start_day, a.daily[27].date);
        assert_ne!(
            a.range_start_day, a.daily[0].date,
            "a short history must not claim the full-window start day"
        );
    }

    #[test]
    fn range_start_day_is_window_start_when_no_activity() {
        let a = routed("month", &["claude"], no_codex, |_, _| {}, Vec::new());
        assert_eq!(a.range_start_day, a.daily.first().unwrap().date);
    }

    // ── activity types + projects (階段 C+) ──────────────────────────────

    #[test]
    fn classify_maps_tools_to_kinds() {
        assert_eq!(classify_kind("Edit"), "edit");
        assert_eq!(classify_kind("Write"), "edit");
        assert_eq!(classify_kind("MultiEdit"), "edit");
        assert_eq!(classify_kind("NotebookEdit"), "edit");
        assert_eq!(classify_kind("Read"), "read");
        assert_eq!(classify_kind("Grep"), "search");
        assert_eq!(classify_kind("Glob"), "search");
        assert_eq!(classify_kind("LS"), "search");
        assert_eq!(classify_kind("ToolSearch"), "search");
        assert_eq!(classify_kind("Bash"), "run");
        assert_eq!(classify_kind("PowerShell"), "run");
        assert_eq!(classify_kind("WebFetch"), "web");
        assert_eq!(classify_kind("WebSearch"), "web");
        assert_eq!(classify_kind("Task"), "agent");
        assert_eq!(classify_kind("Agent"), "agent");
        assert_eq!(classify_kind("AgentExplore"), "agent");
        assert_eq!(classify_kind("mcp__whatever"), "mcp");
        assert_eq!(classify_kind("AskUserQuestion"), "other");
    }

    #[test]
    fn message_kind_picks_dominant_tool_and_defaults_to_other() {
        assert_eq!(message_kind(&[]), "other");
        assert_eq!(
            message_kind(&["Edit".to_string(), "Read".to_string(), "Edit".to_string()]),
            "edit"
        );
        // Tie between edit and read breaks to edit (fixed priority order).
        assert_eq!(
            message_kind(&["Read".to_string(), "Edit".to_string()]),
            "edit"
        );
        assert_eq!(
            message_kind(&["Read".to_string(), "Read".to_string()]),
            "read"
        );
        assert_eq!(
            message_kind(&["WebFetch".to_string(), "AgentPlan".to_string()]),
            "web"
        );
        assert_eq!(
            message_kind(&["mcp__server__tool".to_string(), "mcp__other".to_string()]),
            "mcp"
        );
    }

    #[test]
    fn basename_takes_the_last_folder_only() {
        assert_eq!(basename("C:\\Coding\\TokenBar"), "TokenBar");
        assert_eq!(basename("/home/me/proj/"), "proj");
        assert_eq!(basename(""), "");
    }

    /// Claude activity across two projects, several kinds.
    fn stub_activity(acc: &mut Acc, _start: i64) {
        acc.add(
            acc.now,
            "claude-opus",
            CLAUDE_AGENT,
            "proj-a",
            Some("edit"),
            100,
            0,
            0,
            0,
        );
        acc.add(
            acc.now,
            "claude-opus",
            CLAUDE_AGENT,
            "proj-a",
            Some("read"),
            50,
            0,
            0,
            0,
        );
        acc.add(
            acc.now,
            "claude-opus",
            CLAUDE_AGENT,
            "proj-b",
            Some("edit"),
            30,
            0,
            0,
            0,
        );
    }

    #[test]
    fn by_kind_aggregates_claude_activity_sorted_desc() {
        let a = routed("today", &["claude"], no_codex, stub_activity, Vec::new());
        assert_eq!(a.by_kind[0].kind, "edit"); // 130 > 50
        assert_eq!(a.by_kind[0].tokens, 130);
        let read = a.by_kind.iter().find(|k| k.kind == "read").unwrap();
        assert_eq!(read.tokens, 50);
    }

    #[test]
    fn by_project_aggregates_and_sorts() {
        let a = routed("today", &["claude"], no_codex, stub_activity, Vec::new());
        assert_eq!(a.by_project[0].name, "proj-a"); // 150 > 30
        assert_eq!(a.by_project[0].tokens, 150);
        assert_eq!(a.by_project[1].name, "proj-b");
    }

    /// Codex contributes to projects (via cwd) but never to by_kind.
    #[test]
    fn codex_is_absent_from_by_kind() {
        fn codex_only(acc: &mut Acc, _start: i64) -> u32 {
            acc.add(
                acc.now,
                "gpt-5-codex",
                CODEX_AGENT,
                "proj-x",
                None,
                100,
                0,
                0,
                0,
            );
            1
        }
        let a = routed("today", &["codex"], codex_only, |_, _| {}, Vec::new());
        assert!(
            a.by_kind.is_empty(),
            "Codex must not produce activity kinds"
        );
        assert_eq!(a.by_project[0].name, "proj-x");
    }

    fn codex_line(ts: &str, total: u64) -> String {
        serde_json::json!({
            "timestamp": ts,
            "payload": {
                "type": "token_count",
                "info": { "total_token_usage": { "input_tokens": total } }
            }
        })
        .to_string()
    }

    /// An RFC3339 timestamp at a given LOCAL wall-clock time, so hour/day bucket
    /// assertions hold on any machine timezone (F-15: buckets are local now).
    fn local_ts(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> String {
        chrono::Local
            .with_ymd_and_hms(y, mo, d, h, mi, 0)
            .unwrap()
            .to_rfc3339()
    }

    fn codex_detailed_line(ts: &str, input: u64, cached: u64, output: u64) -> String {
        serde_json::json!({
            "timestamp": ts,
            "payload": {
                "type": "token_count",
                "info": { "total_token_usage": {
                    "input_tokens": input,
                    "cached_input_tokens": cached,
                    "output_tokens": output
                } }
            }
        })
        .to_string()
    }

    fn scan_fake_codex_files(files: Vec<Vec<String>>) -> Acc {
        let mut acc = Acc::new(1_783_000_000);
        let mut seen = HashSet::new();
        for lines in files {
            scan_codex_lines(&mut acc, 0, "test-project", lines.into_iter(), &mut seen);
        }
        acc
    }

    #[test]
    fn codex_cumulative_events_become_timestamped_deltas() {
        let acc = scan_fake_codex_files(vec![vec![
            codex_line(&local_ts(2026, 7, 17, 1, 0), 100),
            codex_line(&local_ts(2026, 7, 17, 2, 0), 250),
            codex_line(&local_ts(2026, 7, 17, 3, 0), 250),
            codex_line(&local_ts(2026, 7, 17, 4, 0), 400),
        ]]);
        assert_eq!(acc.breakdown.input, 400);
        assert_eq!(acc.hourly[1], 100);
        assert_eq!(acc.hourly[2], 150);
        assert_eq!(acc.hourly[3], 0);
        assert_eq!(acc.hourly[4], 150);
    }

    #[test]
    fn codex_cached_input_uses_discounted_delta_cost() {
        let acc = scan_fake_codex_files(vec![vec![
            codex_detailed_line("2026-07-17T01:00:00Z", 500, 300, 50),
            codex_detailed_line("2026-07-17T02:00:00Z", 1000, 800, 100),
        ]]);
        let cost: f64 = acc.days.values().map(|d| d.cost).sum();
        // (input-cached)=200 at $5 + cached=800 at $0.50 + output=100 at $5.
        assert!((cost - 0.0019).abs() < 1e-12);
    }

    #[test]
    fn codex_without_cached_breakdown_keeps_blended_cost() {
        let acc = scan_fake_codex_files(vec![vec![codex_line("2026-07-17T01:00:00Z", 1000)]]);
        let cost: f64 = acc.days.values().map(|d| d.cost).sum();
        assert!((cost - 0.005).abs() < 1e-12);
    }

    #[test]
    fn codex_events_across_midnight_are_booked_to_separate_days() {
        let acc = scan_fake_codex_files(vec![vec![
            codex_line(&local_ts(2026, 7, 16, 23, 59), 100),
            codex_line(&local_ts(2026, 7, 17, 0, 1), 250),
        ]]);
        assert_eq!(acc.days["2026-07-16"].by_agent[CODEX_AGENT], 100);
        assert_eq!(acc.days["2026-07-17"].by_agent[CODEX_AGENT], 150);
    }

    #[test]
    fn codex_fork_replay_prefix_counts_once() {
        let parent = vec![
            codex_line("2026-07-17T01:00:00Z", 100),
            codex_line("2026-07-17T02:00:00Z", 250),
        ];
        let fork = vec![
            codex_line("2026-07-17T01:00:00Z", 100),
            codex_line("2026-07-17T02:00:00Z", 250),
            codex_line("2026-07-17T03:00:00Z", 400),
        ];
        let acc = scan_fake_codex_files(vec![parent, fork]);
        assert_eq!(acc.breakdown.input, 400);
    }

    #[test]
    fn codex_decreasing_total_saturates_to_zero() {
        let acc = scan_fake_codex_files(vec![vec![
            codex_line(&local_ts(2026, 7, 17, 1, 0), 250),
            codex_line(&local_ts(2026, 7, 17, 2, 0), 100),
        ]]);
        assert_eq!(acc.breakdown.input, 250);
        assert_eq!(acc.hourly[2], 0);
    }

    #[test]
    fn by_project_merges_beyond_top_eight() {
        fn many(acc: &mut Acc, _start: i64) {
            for i in 0..10u64 {
                acc.add(
                    acc.now,
                    "claude-opus",
                    CLAUDE_AGENT,
                    &format!("p{i:02}"),
                    None,
                    100 - i * 5,
                    0,
                    0,
                    0,
                );
            }
        }
        let a = routed("today", &["claude"], no_codex, many, Vec::new());
        assert_eq!(a.by_project.len(), 9, "8 named projects + merged remainder");
        assert_eq!(a.by_project.last().unwrap().name, "__other__");
        // The remainder holds the two smallest (p08=60, p09=55).
        assert_eq!(a.by_project.last().unwrap().tokens, 115);
    }

    fn scan_fake_claude_files(files: Vec<Vec<String>>) -> Acc {
        let mut acc = Acc::new(1_782_000_000);
        let mut seen = HashSet::new();
        for lines in files {
            scan_claude_lines(&mut acc, 0, "test-project", lines.into_iter(), &mut seen);
        }
        acc
    }

    #[test]
    fn claude_opus_usage_uses_component_prices() {
        let line = r#"{"timestamp":"2026-07-17T00:00:00Z","message":{"model":"claude-opus-4-8","usage":{"input_tokens":1000,"output_tokens":500,"cache_read_input_tokens":100000,"cache_creation_input_tokens":2000}}}"#.to_string();
        let acc = scan_fake_claude_files(vec![vec![line]]);
        let cost: f64 = acc.days.values().map(|d| d.cost).sum();
        // 0.005 + 0.0125 + 0.05 + 0.0125 = 0.08.
        assert!((cost - 0.08).abs() < 1e-12);
    }

    #[test]
    fn claude_cache_creation_1h_uses_one_hour_price() {
        let line = r#"{"timestamp":"2026-07-17T00:00:00Z","message":{"model":"claude-opus-4-8","usage":{"cache_creation_input_tokens":2000,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":2000}}}}"#.to_string();
        let acc = scan_fake_claude_files(vec![vec![line]]);
        let cost: f64 = acc.days.values().map(|d| d.cost).sum();
        assert!((cost - 0.02).abs() < 1e-12);
    }

    #[test]
    fn claude_duplicate_message_id_across_files_counts_once() {
        let line = r#"{"timestamp":"2026-07-17T00:00:00Z","message":{"id":"message-a","model":"claude-test","usage":{"input_tokens":100}}}"#.to_string();
        let acc = scan_fake_claude_files(vec![vec![line.clone()], vec![line]]);
        assert_eq!(acc.breakdown.input, 100);
    }

    #[test]
    fn claude_request_id_takes_priority_over_message_id() {
        let first = r#"{"requestId":"request-a","timestamp":"2026-07-17T00:00:00Z","message":{"id":"message-a","model":"claude-test","usage":{"input_tokens":100}}}"#.to_string();
        let second = r#"{"requestId":"request-a","timestamp":"2026-07-17T00:00:01Z","message":{"id":"message-b","model":"claude-test","usage":{"input_tokens":200}}}"#.to_string();
        let acc = scan_fake_claude_files(vec![vec![first], vec![second]]);
        assert_eq!(acc.breakdown.input, 100);
    }

    #[test]
    fn claude_messages_without_ids_all_count() {
        let line = r#"{"timestamp":"2026-07-17T00:00:00Z","message":{"model":"claude-test","usage":{"input_tokens":100}}}"#.to_string();
        let acc = scan_fake_claude_files(vec![vec![line.clone()], vec![line]]);
        assert_eq!(acc.breakdown.input, 200);
    }

    #[test]
    fn empty_project_is_not_recorded() {
        // A provider with no cwd ("") still counts its tokens but adds no project.
        let a = routed("today", &["claude"], no_codex, stub_recent(1), Vec::new());
        assert_eq!(a.total_tokens, 100);
        assert!(a.by_project.is_empty());
    }

    // ── cost dimensions: hourly / per-model / per-agent cost ─────────────
    //
    // The metric/group toggles need a cost equivalent for every token
    // dimension. These pin the three invariants the frontend relies on:
    // hourly_cost mirrors the day-cost sum, the range-total cost maps sum to
    // total_cost_usd, and every cost map carries the same keys as its token map.

    #[test]
    fn hourly_cost_sums_match_day_cost_sums() {
        let acc = scan_fake_codex_files(vec![vec![
            codex_detailed_line("2026-07-17T01:00:00Z", 500, 300, 50),
            codex_detailed_line("2026-07-17T02:00:00Z", 1000, 800, 100),
        ]]);
        let day_cost: f64 = acc.days.values().map(|d| d.cost).sum();
        let hourly_cost: f64 = acc.hourly_cost.iter().sum();
        assert!((day_cost - hourly_cost).abs() < 1e-9);

        let claude = scan_fake_claude_files(vec![vec![
            r#"{"timestamp":"2026-07-17T00:00:00Z","message":{"model":"claude-opus-4-8","usage":{"input_tokens":1000,"output_tokens":500,"cache_read_input_tokens":100000,"cache_creation_input_tokens":2000}}}"#.to_string(),
        ]]);
        let cday: f64 = claude.days.values().map(|d| d.cost).sum();
        let chour: f64 = claude.hourly_cost.iter().sum();
        assert!((cday - chour).abs() < 1e-9);
    }

    #[test]
    fn cost_maps_total_to_range_cost_and_share_keys_with_tokens() {
        let a = routed("today", &["claude", "codex"], stub_codex, stub_claude, Vec::new());

        // (b) each range-total cost map sums to total_cost_usd.
        let model_sum: f64 = a.by_model_cost.values().sum();
        let agent_sum: f64 = a.by_agent_cost.values().sum();
        let hourly_sum: f64 = a.hourly_cost.iter().sum();
        assert!((model_sum - a.total_cost_usd).abs() < 1e-9);
        assert!((agent_sum - a.total_cost_usd).abs() < 1e-9);
        assert!((hourly_sum - a.total_cost_usd).abs() < 1e-9);

        // (c) cost maps carry exactly the keys of their token maps.
        let mk: HashSet<&String> = a.by_model.keys().collect();
        let mck: HashSet<&String> = a.by_model_cost.keys().collect();
        assert_eq!(mk, mck, "by_model_cost keys must match by_model");
        let ak: HashSet<&String> = a.by_agent.keys().collect();
        let ack: HashSet<&String> = a.by_agent_cost.keys().collect();
        assert_eq!(ak, ack, "by_agent_cost keys must match by_agent");
    }

    /// A selected source with no local data must not surface (the empty-scan
    /// case): a selected-but-empty scanner contributes nothing, so no
    /// agent/legend entry and no fake 0 card.
    #[test]
    fn selected_but_empty_source_adds_nothing() {
        let a = compute_routed(
            "today",
            &to_sources(&["grok"]),
            no_codex,
            |_, _| {},
            |_, _| {},
            Vec::new(),
            Arc::new(PricingOverride::empty()),
        );
        assert!(a.by_agent.is_empty());
        assert_eq!(a.total_tokens, 0);
    }

    // ── T-916 Grok: cumulative deltas, reset guard, total-only mapping ───

    fn grok_ts(rfc3339: &str) -> i64 {
        chrono::DateTime::parse_from_rfc3339(rfc3339).unwrap().timestamp()
    }

    fn grok_line(ts: i64, total: u64, model: &str) -> String {
        serde_json::json!({
            "timestamp": ts,
            "_meta": { "totalTokens": total, "modelId": model }
        })
        .to_string()
    }

    #[test]
    fn grok_event_falls_back_to_params_meta_when_top_meta_is_empty() {
        // Real 2026-07-18 file shape: top-level `_meta` present but EMPTY, the
        // usable payload nested under `params._meta`. Co-located modelId still
        // surfaces as Some so scan can stick it without a prior update row.
        let line = serde_json::json!({
            "timestamp": 1_784_343_597,
            "method": "session/update",
            "_meta": {},
            "params": { "_meta": { "totalTokens": 10_093u64, "modelId": "grok-4.5" } }
        })
        .to_string();
        let (ts, total, model) = grok_event(&line).unwrap();
        assert_eq!(ts, 1_784_343_597);
        assert_eq!(total, 10_093);
        assert_eq!(model.as_deref(), Some("grok-4.5"));

        // Neither location has totalTokens → skipped, not fatal.
        let empty = serde_json::json!({
            "timestamp": 1_784_343_597,
            "_meta": {},
            "params": { "_meta": {}, "note": "totalTokens" }
        })
        .to_string();
        assert!(grok_event(&empty).is_none());
    }

    fn scan_fake_grok_files(files: Vec<Vec<String>>) -> Acc {
        let mut acc = Acc::new(1_783_000_000);
        let mut seen = HashSet::new();
        for lines in files {
            scan_grok_lines(&mut acc, 0, "grok-project", lines.into_iter(), &mut seen);
        }
        acc
    }

    fn grok_agent_total(acc: &Acc) -> u64 {
        acc.days
            .values()
            .map(|d| d.by_agent.get("Grok CLI").copied().unwrap_or(0))
            .sum()
    }

    #[test]
    fn grok_event_parses_epoch_seconds_total_and_model() {
        // A realistic 2026 epoch-seconds value stays as-is (below the millis
        // threshold), proving the epoch-seconds handling the brief calls out.
        let secs = grok_ts("2026-07-17T02:00:00Z");
        let (ts, total, model) = grok_event(&grok_line(secs, 500, "grok-4.5")).unwrap();
        assert_eq!(ts, secs);
        assert_eq!(total, 500);
        assert_eq!(model.as_deref(), Some("grok-4.5"));

        // modelId absent on the token line → None here; scan sticks "grok" (or
        // a prior update row's model) instead of inventing one at parse time.
        let no_model = r#"{ "timestamp": 1784253600, "_meta": { "totalTokens": 10 } }"#;
        assert_eq!(grok_event(no_model).unwrap().2, None);

        // No cumulative total → skipped (the `totalTokens` prefilter also guards this).
        assert!(grok_event(r#"{ "timestamp": 1, "_meta": { "modelId": "grok" } }"#).is_none());
    }

    /// Real 2026-07-19 Grok shape: `modelId` lives on an earlier
    /// `params.update._meta` row; token rows only carry `params._meta.totalTokens`.
    /// Without sticky per-file model every event fell back to the generic "grok".
    fn grok_model_update_line(ts: i64, model: &str) -> String {
        serde_json::json!({
            "timestamp": ts,
            "method": "session/update",
            "params": {
                "sessionId": "test-session",
                "update": {
                    "sessionUpdate": "user_message_chunk",
                    "content": { "type": "text", "text": "hi" },
                    "_meta": { "modelId": model, "promptIndex": 0 }
                },
                "_meta": { "eventId": "evt-model" }
            }
        })
        .to_string()
    }

    fn grok_token_only_line(ts: i64, total: u64) -> String {
        serde_json::json!({
            "timestamp": ts,
            "method": "session/update",
            "params": {
                "sessionId": "test-session",
                "update": {
                    "sessionUpdate": "agent_thought_chunk",
                    "content": { "type": "text", "text": "thinking" }
                },
                "_meta": { "totalTokens": total, "eventId": "evt-tok" }
            }
        })
        .to_string()
    }

    #[test]
    fn grok_split_model_update_then_token_books_real_model() {
        let t0 = grok_ts(&local_ts(2026, 7, 17, 1, 0));
        let t1 = grok_ts(&local_ts(2026, 7, 17, 2, 0));
        let t2 = grok_ts(&local_ts(2026, 7, 17, 3, 0));
        let acc = scan_fake_grok_files(vec![vec![
            grok_model_update_line(t0, "grok-4.5"),
            grok_token_only_line(t1, 100),
            grok_token_only_line(t2, 400),
        ]]);
        assert_eq!(grok_agent_total(&acc), 400);
        let day = acc.days.values().next().unwrap();
        assert_eq!(day.by_model.get("grok-4.5").copied(), Some(400));
        assert!(
            !day.by_model.contains_key("grok"),
            "must not collapse to the generic fallback when a prior model update exists"
        );
    }

    #[test]
    fn grok_token_without_prior_model_falls_back_to_generic() {
        let t1 = grok_ts(&local_ts(2026, 7, 17, 2, 0));
        let acc = scan_fake_grok_files(vec![vec![grok_token_only_line(t1, 250)]]);
        assert_eq!(grok_agent_total(&acc), 250);
        let day = acc.days.values().next().unwrap();
        assert_eq!(day.by_model.get("grok").copied(), Some(250));
    }

    #[test]
    fn grok_mid_file_model_switch_applies_to_later_tokens() {
        let t0 = grok_ts(&local_ts(2026, 7, 17, 1, 0));
        let t1 = grok_ts(&local_ts(2026, 7, 17, 2, 0));
        let t2 = grok_ts(&local_ts(2026, 7, 17, 3, 0));
        let t3 = grok_ts(&local_ts(2026, 7, 17, 4, 0));
        let acc = scan_fake_grok_files(vec![vec![
            grok_model_update_line(t0, "grok-4.5"),
            grok_token_only_line(t1, 100),
            grok_model_update_line(t2, "grok-4"),
            grok_token_only_line(t3, 300),
        ]]);
        // deltas: 100 on 4.5, then 200 on 4
        let day = acc.days.values().next().unwrap();
        assert_eq!(day.by_model.get("grok-4.5").copied(), Some(100));
        assert_eq!(day.by_model.get("grok-4").copied(), Some(200));
        assert_eq!(grok_agent_total(&acc), 300);
    }

    #[test]
    fn grok_cumulative_events_become_timestamped_deltas() {
        let acc = scan_fake_grok_files(vec![vec![
            grok_line(grok_ts(&local_ts(2026, 7, 17, 1, 0)), 100, "grok-4.5"),
            grok_line(grok_ts(&local_ts(2026, 7, 17, 2, 0)), 250, "grok-4.5"),
            grok_line(grok_ts(&local_ts(2026, 7, 17, 3, 0)), 250, "grok-4.5"),
            grok_line(grok_ts(&local_ts(2026, 7, 17, 4, 0)), 400, "grok-4.5"),
        ]]);
        assert_eq!(acc.hourly[1], 100);
        assert_eq!(acc.hourly[2], 150);
        assert_eq!(acc.hourly[3], 0);
        assert_eq!(acc.hourly[4], 150);
        assert_eq!(grok_agent_total(&acc), 400);
    }

    /// Total-only mapping: Grok's tokens count toward every *total* dimension but
    /// never fabricate a breakdown category or an activity kind (硬規定).
    #[test]
    fn grok_is_total_only_no_breakdown_no_kind() {
        let acc = scan_fake_grok_files(vec![vec![
            grok_line(grok_ts("2026-07-17T01:00:00Z"), 100, "grok-4.5"),
            grok_line(grok_ts("2026-07-17T02:00:00Z"), 400, "grok-4.5"),
        ]]);
        // Booked to the totals…
        assert_eq!(grok_agent_total(&acc), 400);
        assert_eq!(*acc.days.values().next().unwrap().by_model.get("grok-4.5").unwrap(), 400);
        assert!(acc.by_project.contains_key("grok-project"));
        // …but NOT to breakdown categories or by_kind.
        assert_eq!(acc.breakdown.input, 0);
        assert_eq!(acc.breakdown.cached, 0);
        assert_eq!(acc.breakdown.output, 0);
        assert_eq!(acc.breakdown.reasoning, 0);
        assert!(acc.by_kind.is_empty(), "Grok must not produce activity kinds");
    }

    /// No public Grok pricing → 0.0 cost (est. cost knowingly undercounts).
    #[test]
    fn grok_contributes_zero_cost() {
        let acc = scan_fake_grok_files(vec![vec![grok_line(
            grok_ts("2026-07-17T01:00:00Z"),
            1_000_000,
            "grok-4.5",
        )]]);
        let cost: f64 = acc.days.values().map(|d| d.cost).sum();
        assert_eq!(cost, 0.0);
        assert!(acc.by_model_cost.values().all(|&c| c == 0.0));
    }

    /// A cumulative *drop* is a new session baseline (whole current value is the
    /// delta) — not a saturating-to-zero like Codex, and never negative.
    #[test]
    fn grok_reset_is_treated_as_a_new_baseline() {
        let acc = scan_fake_grok_files(vec![vec![
            grok_line(grok_ts(&local_ts(2026, 7, 17, 1, 0)), 300, "grok-4.5"),
            grok_line(grok_ts(&local_ts(2026, 7, 17, 2, 0)), 120, "grok-4.5"),
        ]]);
        assert_eq!(acc.hourly[1], 300);
        assert_eq!(acc.hourly[2], 120); // the drop counts as a fresh 120, not 0
        assert_eq!(grok_agent_total(&acc), 420);
    }

    /// A fork/replay that repeats an earlier prefix counts each (ts,total) once.
    #[test]
    fn grok_fork_replay_prefix_counts_once() {
        let parent = vec![
            grok_line(grok_ts("2026-07-17T01:00:00Z"), 100, "grok-4.5"),
            grok_line(grok_ts("2026-07-17T02:00:00Z"), 250, "grok-4.5"),
        ];
        let fork = vec![
            grok_line(grok_ts("2026-07-17T01:00:00Z"), 100, "grok-4.5"),
            grok_line(grok_ts("2026-07-17T02:00:00Z"), 250, "grok-4.5"),
            grok_line(grok_ts("2026-07-17T03:00:00Z"), 400, "grok-4.5"),
        ];
        let acc = scan_fake_grok_files(vec![parent, fork]);
        assert_eq!(grok_agent_total(&acc), 400);
    }

    #[test]
    fn percent_decode_recovers_paths_and_leaves_bad_escapes_literal() {
        assert_eq!(percent_decode("C%3A%5CCoding%5CTokenBar"), "C:\\Coding\\TokenBar");
        assert_eq!(percent_decode("%2Fhome%2Fme%2Fproj"), "/home/me/proj");
        assert_eq!(percent_decode("no-escapes"), "no-escapes");
        assert_eq!(percent_decode("50%"), "50%"); // trailing lone '%' left literal
    }

    #[test]
    fn grok_project_is_decoded_basename_of_the_cwd_segment() {
        let win = PathBuf::from(
            "/root/.grok/sessions/C%3A%5CCoding%5CTokenBar/abc123/updates.jsonl",
        );
        assert_eq!(grok_project_from_path(&win), "TokenBar");
        let unix = PathBuf::from("/root/.grok/sessions/%2Fhome%2Fme%2Fmyproj/sess/updates.jsonl");
        assert_eq!(grok_project_from_path(&unix), "myproj");
    }

    fn record_acc(now: i64, entries: &[(&str, u8, u64)]) -> Acc {
        let mut acc = Acc::new(now);
        for (date, hour, tokens) in entries {
            acc.hourly_by_day.insert(((*date).into(), *hour), *tokens);
        }
        acc
    }

    #[test]
    fn streak_falls_back_to_yesterday_and_stops_at_gap() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 7, 17).unwrap();
        let acc = record_acc(0, &[("2026-07-16", 8, 10), ("2026-07-15", 9, 10)]);
        assert_eq!(records_for(&acc, today, 10).streak_days, 2);

        let gap = record_acc(0, &[("2026-07-15", 9, 10)]);
        assert_eq!(records_for(&gap, today, 10).streak_days, 0);
    }

    #[test]
    fn max_hour_is_one_date_hour_not_cross_day_bucket() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 7, 17).unwrap();
        let acc = record_acc(
            0,
            &[
                ("2026-07-15", 9, 60),
                ("2026-07-16", 9, 60),
                ("2026-07-16", 10, 100),
            ],
        );
        let records = records_for(&acc, today, 11);
        assert_eq!(records.max_hour.tokens, 100);
        assert_eq!(records.max_hour.hour, 10);
    }

    #[test]
    fn pr_now_excludes_current_hour_from_history() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 7, 17).unwrap();
        let acc = record_acc(0, &[("2026-07-16", 9, 100), ("2026-07-17", 10, 101)]);
        assert!(records_for(&acc, today, 10).pr_now);

        let tied = record_acc(0, &[("2026-07-16", 9, 101), ("2026-07-17", 10, 101)]);
        assert!(!records_for(&tied, today, 10).pr_now);
    }

    // ── T-feat-006 pricing override: the override → vendored → blended chain ──
    //
    // These drive real cost through `scan_claude_line` (the same path production
    // uses) with an injected override table, and assert the resulting $ amount —
    // a lookup that never reaches the cost is the bug this ticket guards against,
    // so the assertions are on money, not on `lookup` in isolation.

    /// Total est. cost for one input-only Claude message of `tokens` under `over`.
    /// Input-only so the amount is just `tokens × input_rate / 1e6`, making the
    /// active rate directly readable from the assertion.
    fn claude_cost_with(over: Arc<PricingOverride>, model: &str, tokens: u64) -> f64 {
        let mut acc = Acc::new(1_782_000_000);
        acc.pricing = over;
        let mut seen = HashSet::new();
        let line = serde_json::json!({
            "timestamp": "2026-07-17T00:00:00Z",
            "message": { "model": model, "usage": { "input_tokens": tokens } }
        })
        .to_string();
        scan_claude_lines(&mut acc, 0, "p", vec![line].into_iter(), &mut seen);
        acc.days.values().map(|d| d.cost).sum()
    }

    /// 精確命中蓋過 vendored:opus vendored input = $5/Mtok,override 到 $10/Mtok。
    #[test]
    fn override_exact_hit_overrides_vendored_amount() {
        let vendored = claude_cost_with(
            Arc::new(PricingOverride::empty()),
            "claude-opus-4-8",
            1_000_000,
        );
        assert!((vendored - 5.0).abs() < 1e-9, "vendored opus 應為 $5:{vendored}");

        let over = Arc::new(PricingOverride::from_json_str(
            r#"{ "models": { "claude-opus-4-8": { "input": 10.0, "output": 50.0 } } }"#,
        ));
        let overridden = claude_cost_with(over, "claude-opus-4-8", 1_000_000);
        assert!((overridden - 10.0).abs() < 1e-9, "override 應蓋成 $10:{overridden}");
    }

    /// 混合檔:壞條目(負 input)被跳過、好條目生效,兩者互不影響。
    #[test]
    fn override_mixed_file_skips_bad_keeps_good() {
        let over = Arc::new(PricingOverride::from_json_str(
            r#"{ "models": {
                "claude-opus-4-8": { "input": 10.0, "output": 50.0 },
                "claude-sonnet-4-x": { "input": -3.0, "output": 15.0 }
            } }"#,
        ));
        // 好條目:opus 走 override input=10 → $10。
        let good = claude_cost_with(over.clone(), "claude-opus-4-8", 1_000_000);
        assert!((good - 10.0).abs() < 1e-9, "好條目應生效:{good}");
        // 壞條目(負 input)被跳過:sonnet 落回 vendored input=3 → $3。
        let fell_back = claude_cost_with(over, "claude-sonnet-4-x", 1_000_000);
        assert!((fell_back - 3.0).abs() < 1e-9, "壞條目應退 vendored $3:{fell_back}");
    }

    /// 整檔壞 JSON → 全部退 vendored,結果與無檔一致(逐 model 比對成本)。
    #[test]
    fn override_whole_bad_json_matches_no_file() {
        let bad = Arc::new(PricingOverride::from_json_str("not json {{{"));
        let none = Arc::new(PricingOverride::empty());
        for model in ["claude-opus-4-8", "claude-sonnet-4-x", "claude-haiku-x"] {
            let with_bad = claude_cost_with(bad.clone(), model, 1_000_000);
            let no_file = claude_cost_with(none.clone(), model, 1_000_000);
            assert!(
                (with_bad - no_file).abs() < 1e-12,
                "壞檔成本必須與無檔一致:{model} {with_bad} vs {no_file}"
            );
        }
    }

    /// 優先序:精確 > override substring > vendored(sonnet vendored=3,substring=7,精確=20)。
    #[test]
    fn override_priority_exact_beats_substring_beats_vendored() {
        let model = "claude-sonnet-4-x";
        let vendored = claude_cost_with(Arc::new(PricingOverride::empty()), model, 1_000_000);
        assert!((vendored - 3.0).abs() < 1e-9, "vendored sonnet 應為 $3:{vendored}");

        // substring key "sonnet" 蓋過 vendored(7 > 3)。
        let sub = Arc::new(PricingOverride::from_json_str(
            r#"{ "models": { "sonnet": { "input": 7.0, "output": 15.0 } } }"#,
        ));
        let sub_cost = claude_cost_with(sub, model, 1_000_000);
        assert!((sub_cost - 7.0).abs() < 1e-9, "substring 應蓋成 $7:{sub_cost}");

        // 同時有 substring(7)與精確 id(20)→ 精確勝出。
        let both = Arc::new(PricingOverride::from_json_str(
            r#"{ "models": {
                "sonnet": { "input": 7.0, "output": 15.0 },
                "claude-sonnet-4-x": { "input": 20.0, "output": 15.0 }
            } }"#,
        ));
        let exact_cost = claude_cost_with(both, model, 1_000_000);
        assert!((exact_cost - 20.0).abs() < 1e-9, "精確應蓋成 $20:{exact_cost}");
    }

    // ── T-perf-004 scan cache: golden equality, incremental, dedup, rebuild ──
    //
    // These drive the real Claude scan over on-disk fixture files (temp dir) so
    // the fingerprint, gzip round-trip and cross-file dedup are all exercised.
    // Equality is on the serialized Analytics (serde normalises HashMap key
    // order to a sorted object), so "逐欄位相等" is a single Value comparison.

    fn unique_temp_dir(tag: &str) -> PathBuf {
        let n = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let d = std::env::temp_dir().join(format!(
            "atoll-scan-test-{tag}-{}-{}",
            std::process::id(),
            n
        ));
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    fn claude_usage_line(ts: i64, req: Option<&str>, model: &str, input: u64) -> String {
        let tsr = chrono::DateTime::from_timestamp(ts, 0).unwrap().to_rfc3339();
        let mut v = serde_json::json!({
            "timestamp": tsr,
            "message": { "model": model, "usage": { "input_tokens": input } }
        });
        if let Some(r) = req {
            v["requestId"] = serde_json::json!(r);
        }
        v.to_string()
    }

    /// Three files across two project-slug dirs. All timestamps are >600s old so
    /// `recent_tokens`/`tok_per_min` stay 0 and no now-boundary flake creeps into
    /// the field-for-field comparison.
    fn write_claude_fixture(dir: &Path) -> Vec<PathBuf> {
        let now = chrono::Utc::now().timestamp();
        let proj_a = dir.join("proj-a");
        let proj_b = dir.join("proj-b");
        fs::create_dir_all(&proj_a).unwrap();
        fs::create_dir_all(&proj_b).unwrap();
        let f1 = proj_a.join("sess1.jsonl");
        fs::write(
            &f1,
            format!(
                "{}\n{}\n",
                claude_usage_line(now - 7200, Some("req-1"), "claude-opus-4-8", 1000),
                claude_usage_line(now - 7000, Some("req-2"), "claude-sonnet-4-x", 500),
            ),
        )
        .unwrap();
        let f2 = proj_a.join("sess2.jsonl");
        fs::write(
            &f2,
            format!(
                "{}\n",
                claude_usage_line(now - 6000, Some("req-3"), "claude-opus-4-8", 200),
            ),
        )
        .unwrap();
        let f3 = proj_b.join("sess3.jsonl");
        fs::write(
            &f3,
            format!(
                "{}\n{}\n",
                claude_usage_line(now - 5000, None, "claude-haiku-x", 50),
                claude_usage_line(now - 4000, Some("req-4"), "claude-opus-4-8", 777),
            ),
        )
        .unwrap();
        vec![f1, f2, f3]
    }

    fn fresh_run(cache: ScanCache) -> RefCell<CacheRun> {
        RefCell::new(CacheRun {
            cache,
            stats: CacheStats::default(),
        })
    }

    /// Real Claude scan over an explicit path list (temp fixtures) so the golden
    /// tests never touch the user's home dir. `run` = None → full scan.
    fn compute_claude_paths(
        range: &str,
        paths: &[PathBuf],
        run: Option<&RefCell<CacheRun>>,
    ) -> Analytics {
        compute_routed(
            range,
            &to_sources(&["claude"]),
            no_codex,
            |acc, start| {
                let mut seen = HashSet::new();
                scan_claude_paths(acc, start, paths.iter().cloned(), run, &mut seen);
            },
            |_, _| {},
            Vec::new(),
            Arc::new(PricingOverride::empty()),
        )
    }

    fn value_of(a: &Analytics) -> serde_json::Value {
        serde_json::to_value(a).unwrap()
    }

    /// The soul of the ticket: a full scan and a "build cache, reload from disk,
    /// second round all-hit" scan must produce byte-identical Analytics.
    #[test]
    fn cache_all_hit_matches_full_scan_field_for_field() {
        let dir = unique_temp_dir("golden");
        let files = write_claude_fixture(&dir);
        let cache_file = dir.join("scan-cache.json.gz");

        let full = compute_claude_paths("month", &files, None);

        // Round 1: empty cache → every file parsed; already equals full.
        {
            let run = fresh_run(ScanCache::empty());
            let a1 = compute_claude_paths("month", &files, Some(&run));
            assert_eq!(value_of(&a1), value_of(&full), "round-1 parse must equal full scan");
            let run = run.into_inner();
            assert_eq!(run.stats.parsed as usize, files.len());
            assert_eq!(run.stats.hit, 0);
            run.cache.save_to(&cache_file);
        }

        // Round 2: reload from disk (gzip + serde round-trip) → every file a hit.
        let run = fresh_run(ScanCache::load_from(&cache_file));
        let a2 = compute_claude_paths("month", &files, Some(&run));
        assert_eq!(
            value_of(&a2),
            value_of(&full),
            "second-round all-hit must equal full scan byte for byte"
        );
        let run = run.into_inner();
        assert_eq!(run.stats.hit as usize, files.len(), "every file must hit");
        assert_eq!(run.stats.parsed, 0, "nothing should re-parse on the second round");

        cleanup(&dir);
    }

    /// Append a line to one file → only that file re-parses; total equals a fresh
    /// full scan of the appended set.
    #[test]
    fn appending_a_line_reparses_only_that_file() {
        let dir = unique_temp_dir("append");
        let files = write_claude_fixture(&dir);
        let cache_file = dir.join("scan-cache.json.gz");

        {
            let run = fresh_run(ScanCache::empty());
            compute_claude_paths("month", &files, Some(&run));
            run.into_inner().cache.save_to(&cache_file);
        }

        let now = chrono::Utc::now().timestamp();
        {
            use std::io::Write;
            let mut f = fs::OpenOptions::new().append(true).open(&files[0]).unwrap();
            writeln!(
                f,
                "{}",
                claude_usage_line(now - 3600, Some("appended-req"), "claude-opus-4-8", 4242)
            )
            .unwrap();
        }

        let full = compute_claude_paths("month", &files, None);
        let run = fresh_run(ScanCache::load_from(&cache_file));
        let a = compute_claude_paths("month", &files, Some(&run));
        assert_eq!(value_of(&a), value_of(&full), "incremental result must equal a fresh full scan");
        let run = run.into_inner();
        assert_eq!(run.stats.parsed, 1, "only the appended file re-parses");
        assert_eq!(run.stats.hit as usize, files.len() - 1, "the rest hit");

        cleanup(&dir);
    }

    /// Cross-file duplicate requestId (fork/resume) must count once on the cache
    /// path — ticket 規格 5, the biggest correctness risk.
    #[test]
    fn cross_file_duplicate_request_id_counts_once_on_cache_hit() {
        let dir = unique_temp_dir("dup");
        let now = chrono::Utc::now().timestamp();
        let proj = dir.join("proj");
        fs::create_dir_all(&proj).unwrap();
        let f1 = proj.join("a.jsonl");
        let f2 = proj.join("b.jsonl");
        // "dup-req" appears in both files with the same tokens (a real fork copy).
        fs::write(
            &f1,
            format!(
                "{}\n",
                claude_usage_line(now - 7200, Some("dup-req"), "claude-opus-4-8", 1000)
            ),
        )
        .unwrap();
        fs::write(
            &f2,
            format!(
                "{}\n{}\n",
                claude_usage_line(now - 7200, Some("dup-req"), "claude-opus-4-8", 1000),
                claude_usage_line(now - 6000, Some("uniq-req"), "claude-opus-4-8", 500),
            ),
        )
        .unwrap();
        let files = vec![f1, f2];
        let cache_file = dir.join("scan-cache.json.gz");

        let full = compute_claude_paths("month", &files, None);
        assert_eq!(full.total_tokens, 1500, "dup counted once (1000) + 500");

        {
            let run = fresh_run(ScanCache::empty());
            compute_claude_paths("month", &files, Some(&run));
            run.into_inner().cache.save_to(&cache_file);
        }
        let run = fresh_run(ScanCache::load_from(&cache_file));
        let cached = compute_claude_paths("month", &files, Some(&run));
        assert_eq!(
            cached.total_tokens, 1500,
            "cache-hit path must not double-count a cross-file requestId"
        );
        assert_eq!(value_of(&cached), value_of(&full));
        assert_eq!(run.into_inner().stats.hit, 2);

        cleanup(&dir);
    }

    /// A corrupt/truncated cache file must silently rebuild to the correct result.
    #[test]
    fn corrupt_cache_file_rebuilds_to_correct_result() {
        let dir = unique_temp_dir("corrupt");
        let files = write_claude_fixture(&dir);
        let cache_file = dir.join("scan-cache.json.gz");
        fs::write(&cache_file, b"garbage, not gzip, not json {{{").unwrap();

        let full = compute_claude_paths("month", &files, None);
        let run = fresh_run(ScanCache::load_from(&cache_file));
        let a = compute_claude_paths("month", &files, Some(&run));
        assert_eq!(value_of(&a), value_of(&full), "corrupt cache must rebuild correctly");
        let run = run.into_inner();
        assert_eq!(run.stats.hit, 0, "corrupt cache → all miss");
        assert_eq!(run.stats.parsed as usize, files.len());

        cleanup(&dir);
    }

    /// A bumped schema invalidates the whole cache: a file written under one
    /// schema is discarded when the code's `SCHEMA` no longer matches, forcing a
    /// full rebuild. Simulated by loading a cache whose stored schema differs.
    #[test]
    fn schema_mismatch_invalidates_and_rebuilds() {
        let dir = unique_temp_dir("schema");
        let files = write_claude_fixture(&dir);
        let cache_file = dir.join("scan-cache.json.gz");

        // Build a valid cache, then corrupt only its schema tag by re-writing it
        // with a bumped schema via the cache module's own encoder path.
        {
            let run = fresh_run(ScanCache::empty());
            compute_claude_paths("month", &files, Some(&run));
            run.into_inner().cache.save_with_schema(&cache_file, scan_cache::SCHEMA + 1);
        }

        let full = compute_claude_paths("month", &files, None);
        let run = fresh_run(ScanCache::load_from(&cache_file));
        let a = compute_claude_paths("month", &files, Some(&run));
        assert_eq!(value_of(&a), value_of(&full), "schema bump must rebuild correctly");
        let run = run.into_inner();
        assert_eq!(run.stats.hit, 0, "schema mismatch → whole cache dropped");
        assert_eq!(run.stats.parsed as usize, files.len());

        cleanup(&dir);
    }
}

fn detect_accounts() -> Vec<Account> {
    let mut out = Vec::new();
    let home = dirs::home_dir();
    if home
        .as_ref()
        .map_or(false, |h| h.join(".claude/.credentials.json").exists())
    {
        out.push(Account {
            client: "Claude Code".into(),
            provider: "anthropic".into(),
            account: "—".into(),
            plan: "Claude".into(),
        });
    }
    if home
        .as_ref()
        .map_or(false, |h| h.join(".codex/sessions").exists())
    {
        out.push(Account {
            client: "Codex CLI".into(),
            provider: "codex".into(),
            account: "—".into(),
            plan: "—".into(),
        });
    }
    // Grok is intentionally NOT surfaced as an account on mere directory
    // existence (the plan bans 0 cards); it appears via usage-driven byAgent
    // instead, and its context-fill limit shows on the limits page.
    // `filter_accounts` still gates a "grok" provider key should a future
    // account source add one.
    out
}
