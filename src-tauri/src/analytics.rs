//! Layer ③ analytics (UX Spec v3 §11): aggregate local JSONL into daily /
//! model / agent breakdowns, cost estimate, and stats. All from local files —
//! no undocumented API, no risk. Dates are bucketed in UTC for consistency.

use chrono::Timelike;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayPoint {
    pub date: String,
    pub by_model: HashMap<String, u64>,
    pub by_agent: HashMap<String, u64>,
    pub cost_usd: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BestDay {
    pub date: String,
    pub cost_usd: f64,
}

#[derive(Serialize)]
pub struct Breakdown {
    pub input: u64,
    pub cached: u64,
    pub output: u64,
    pub reasoning: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub client: String,
    pub provider: String,
    pub account: String,
    pub plan: String,
}

#[derive(Serialize)]
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
    pub daily: Vec<DayPoint>,
    pub hourly: Vec<u64>,
    pub by_model: HashMap<String, u64>,
    pub by_agent: HashMap<String, u64>,
    pub breakdown: Breakdown,
    pub sessions_this_week: u32,
    pub tok_per_min: u64,
    pub accounts: Vec<Account>,
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
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

struct Acc {
    days: HashMap<String, DayAgg>,
    hourly: [u64; 24],
    breakdown: Breakdown,
    recent_tokens: u64,
    now: i64,
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
            breakdown: Breakdown { input: 0, cached: 0, output: 0, reasoning: 0 },
            recent_tokens: 0,
            now,
        }
    }

    fn add(&mut self, ts: i64, model: &str, agent: &str, input: u64, cached: u64, output: u64, reasoning: u64) {
        let total = input + cached + output + reasoning;
        if total == 0 {
            return;
        }
        let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) else {
            return;
        };
        let day = self.days.entry(dt.format("%Y-%m-%d").to_string()).or_default();
        *day.by_model.entry(model.to_string()).or_default() += total;
        *day.by_agent.entry(agent.to_string()).or_default() += total;
        day.cost += (total as f64 / 1e6) * rate_per_mtok(model);

        self.hourly[dt.hour() as usize] += total;

        self.breakdown.input += input;
        self.breakdown.cached += cached;
        self.breakdown.output += output;
        self.breakdown.reasoning += reasoning;

        if self.now - ts <= 600 {
            self.recent_tokens += total;
        }
    }
}

// ── display filter (Settings::providers) ─────────────────────────────────
//
// Analytics is the one consumer that does not read the Snapshot — it scans
// local JSONL directly — so the scheduler's single filter node cannot reach
// it and it has to honour the setting itself.
//
// Both helpers mirror lib::apply_provider_filter's contract: only an exact
// "claude"/"codex" narrows anything, every unknown value scans everything.
// Never let a stale setting empty the page.

fn scans_codex(filter: &str) -> bool {
    filter != "claude"
}

fn scans_claude(filter: &str) -> bool {
    filter != "codex"
}

fn filter_accounts(filter: &str, accounts: Vec<Account>) -> Vec<Account> {
    accounts
        .into_iter()
        .filter(|a| match a.provider.as_str() {
            "anthropic" => scans_claude(filter),
            "codex" => scans_codex(filter),
            _ => true,
        })
        .collect()
}

/// Compute analytics for "today" or "week", scoped to the display filter.
///
/// Skips the scan outright rather than scanning then discarding: `scan_*`
/// walks a whole directory tree, and a hidden provider's files are pure waste.
pub fn compute_with(range: &str, filter: &str) -> Analytics {
    compute_routed(range, filter, scan_codex, scan_claude, detect_accounts())
}

/// The real body of `compute_with`, with every source of ambient state
/// (the two directory scans and account detection) passed in.
///
/// This split exists purely so the filter routing below is testable: the
/// scanners read the real home dir, so a test that cannot replace them can
/// only re-assert `scans_*`, which proves nothing about which branch runs.
fn compute_routed<C, L>(
    range: &str,
    filter: &str,
    scan_codex_fn: C,
    scan_claude_fn: L,
    accounts: Vec<Account>,
) -> Analytics
where
    C: FnOnce(&mut Acc, i64) -> u32,
    L: FnOnce(&mut Acc, i64),
{
    let now = chrono::Utc::now().timestamp();
    let days_back: i64 = match range {
        "today" => 0,
        "month" => 29, // last 30 days including today
        _ => 6,        // "week"
    };
    let utc_midnight = now - now.rem_euclid(86400);
    let start = utc_midnight - days_back * 86400;

    let mut acc = Acc::new(now);
    let sessions = if scans_codex(filter) {
        scan_codex_fn(&mut acc, start)
    } else {
        0
    };
    if scans_claude(filter) {
        scan_claude_fn(&mut acc, start);
    }

    let mut daily: Vec<DayPoint> = Vec::new();
    let mut by_model: HashMap<String, u64> = HashMap::new();
    let mut by_agent: HashMap<String, u64> = HashMap::new();
    let mut total_tokens = 0u64;
    let mut total_cost = 0.0;
    let mut best = BestDay { date: String::new(), cost_usd: 0.0 };

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
            best = BestDay { date: date.clone(), cost_usd: agg.cost };
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

    // Actual start: the first day with activity, so a "month" backed by only a
    // few days of logs reports its true reach instead of claiming 30. Falls
    // back to the nominal window start when nothing was recorded at all.
    let range_start_day = daily
        .iter()
        .find(|d| !d.by_agent.is_empty())
        .or_else(|| daily.first())
        .map(|d| d.date.clone())
        .unwrap_or_default();

    Analytics {
        range: range.to_string(),
        range_start_day,
        total_tokens,
        total_cost_usd: total_cost,
        best_day: best,
        active_days,
        daily,
        hourly: acc.hourly.to_vec(),
        by_model,
        by_agent,
        breakdown: acc.breakdown,
        sessions_this_week: sessions,
        tok_per_min: (acc.recent_tokens as f64 / 10.0) as u64,
        accounts: filter_accounts(filter, accounts),
    }
}

// ── Codex: tail-read each recent session for its cumulative total ────────

fn scan_codex(acc: &mut Acc, start: i64) -> u32 {
    let Some(home) = dirs::home_dir() else {
        return 0;
    };
    let pattern = home
        .join(".codex/sessions/**/rollout-*.jsonl")
        .to_string_lossy()
        .replace('\\', "/");
    let mut sessions = 0;
    if let Ok(paths) = glob::glob(&pattern) {
        for p in paths.filter_map(Result::ok) {
            let ts = mtime_secs(&p);
            if ts < start {
                continue;
            }
            sessions += 1;
            if let Some((i, ca, o, r)) = last_total_usage(&p) {
                acc.add(ts, "gpt-5-codex", "Codex CLI", i, ca, o, r);
            }
        }
    }
    sessions
}

fn mtime_secs(p: &PathBuf) -> i64 {
    fs::metadata(p)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn last_total_usage(path: &PathBuf) -> Option<(u64, u64, u64, u64)> {
    let mut f = File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let start = len.saturating_sub(512 * 1024);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);

    let key = "total_token_usage\":";
    let at = text.rfind(key)?;
    let brace = text[at + key.len()..].find('{')?;
    let obj_start = at + key.len() + brace;
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut end = None;
    for i in obj_start..bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let obj: serde_json::Value = serde_json::from_str(&text[obj_start..end?]).ok()?;
    Some((
        obj.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        obj.get("cached_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        obj.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
        obj.get("reasoning_output_tokens").and_then(|v| v.as_u64()).unwrap_or(0),
    ))
}

// ── Claude: per-message usage from projects/*.jsonl ──────────────────────

fn scan_claude(acc: &mut Acc, start: i64) {
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
    for p in paths.filter_map(Result::ok) {
        if mtime_secs(&p) < start {
            continue;
        }
        let Ok(file) = File::open(&p) else {
            continue;
        };
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            if !line.contains("\"usage\"") {
                continue;
            }
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            let msg = v.get("message");
            let Some(usage) = msg.and_then(|m| m.get("usage")) else {
                continue;
            };
            let ts = v
                .get("timestamp")
                .and_then(|t| t.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.timestamp())
                .unwrap_or(0);
            if ts < start {
                continue;
            }
            let model = msg
                .and_then(|m| m.get("model"))
                .and_then(|m| m.as_str())
                .unwrap_or("claude");
            let input = usage.get("input_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
            let output = usage.get("output_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
            let cached = usage.get("cache_read_input_tokens").and_then(|x| x.as_u64()).unwrap_or(0)
                + usage.get("cache_creation_input_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
            acc.add(ts, model, "Claude Code", input, cached, output, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scan helpers hit the real home dir, so these assert on the pure
    /// routing decision instead: which scans the filter authorises.
    #[test]
    fn claude_filter_skips_codex_scan() {
        assert!(!scans_codex("claude"));
        assert!(scans_claude("claude"));
    }

    #[test]
    fn codex_filter_skips_claude_scan() {
        assert!(scans_codex("codex"));
        assert!(!scans_claude("codex"));
    }

    #[test]
    fn both_scans_everything() {
        assert!(scans_codex("both"));
        assert!(scans_claude("both"));
    }

    /// Same catch-all rule as lib::apply_provider_filter: an unknown value
    /// must never silently produce an empty analytics page.
    #[test]
    fn unknown_filter_scans_everything() {
        for f in ["worst", "", "CLAUDE", "nonsense"] {
            assert!(scans_codex(f), "codex scan dropped for {f:?}");
            assert!(scans_claude(f), "claude scan dropped for {f:?}");
        }
    }

    #[test]
    fn accounts_follow_the_filter() {
        let only_claude = filter_accounts(
            "claude",
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
        acc.add(acc.now, "gpt-5-codex", CODEX_AGENT, 100, 0, 0, 0);
        7
    }

    fn stub_claude(acc: &mut Acc, _start: i64) {
        acc.add(acc.now, "claude-opus", CLAUDE_AGENT, 200, 0, 0, 0);
    }

    /// Agents that actually got scanned, for `filter`, sorted.
    fn scanned_agents(filter: &str) -> Vec<String> {
        let a = compute_routed("today", filter, stub_codex, stub_claude, Vec::new());
        let mut names: Vec<String> = a.by_agent.keys().cloned().collect();
        names.sort();
        names
    }

    #[test]
    fn claude_filter_routes_to_claude_scan_only() {
        assert_eq!(scanned_agents("claude"), vec![CLAUDE_AGENT.to_string()]);
    }

    #[test]
    fn codex_filter_routes_to_codex_scan_only() {
        assert_eq!(scanned_agents("codex"), vec![CODEX_AGENT.to_string()]);
    }

    #[test]
    fn both_filter_routes_to_every_scan() {
        assert_eq!(
            scanned_agents("both"),
            vec![CLAUDE_AGENT.to_string(), CODEX_AGENT.to_string()]
        );
    }

    /// The core guard: a stale or unknown setting must never route to "scan
    /// nothing" and leave the analytics page blank.
    #[test]
    fn unknown_filter_routes_to_every_scan() {
        for f in ["worst", "", "CLAUDE", "nonsense"] {
            assert_eq!(
                scanned_agents(f),
                vec![CLAUDE_AGENT.to_string(), CODEX_AGENT.to_string()],
                "unknown filter {f:?} did not scan both providers"
            );
        }
    }

    /// Totals, not just agent names: a skipped scan must take its tokens and
    /// its session count with it.
    #[test]
    fn skipped_codex_scan_drops_its_tokens_and_sessions() {
        let claude_only = compute_routed("today", "claude", stub_codex, stub_claude, Vec::new());
        assert_eq!(claude_only.total_tokens, 200);
        assert_eq!(claude_only.sessions_this_week, 0);

        let codex_only = compute_routed("today", "codex", stub_codex, stub_claude, Vec::new());
        assert_eq!(codex_only.total_tokens, 100);
        assert_eq!(codex_only.sessions_this_week, 7);

        let everything = compute_routed("today", "both", stub_codex, stub_claude, Vec::new());
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
                acc.add(acc.now - k * 86400, "claude-opus", CLAUDE_AGENT, 100, 0, 0, 0);
            }
        }
    }

    #[test]
    fn month_range_spans_thirty_daily_buckets() {
        let a = compute_routed("month", "claude", no_codex, stub_recent(3), Vec::new());
        assert_eq!(a.daily.len(), 30);
        assert_eq!(a.total_tokens, 300); // 3 days × 100
    }

    #[test]
    fn month_range_reports_actual_start_when_history_is_short() {
        let a = compute_routed("month", "claude", no_codex, stub_recent(3), Vec::new());
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
        let a = compute_routed("month", "claude", no_codex, |_, _| {}, Vec::new());
        assert_eq!(a.range_start_day, a.daily.first().unwrap().date);
    }
}

fn detect_accounts() -> Vec<Account> {
    let mut out = Vec::new();
    let home = dirs::home_dir();
    if home.as_ref().map_or(false, |h| h.join(".claude/.credentials.json").exists()) {
        out.push(Account {
            client: "Claude Code".into(),
            provider: "anthropic".into(),
            account: "—".into(),
            plan: "Claude".into(),
        });
    }
    if home.as_ref().map_or(false, |h| h.join(".codex/sessions").exists()) {
        out.push(Account {
            client: "Codex CLI".into(),
            provider: "codex".into(),
            account: "—".into(),
            plan: "—".into(),
        });
    }
    out
}
