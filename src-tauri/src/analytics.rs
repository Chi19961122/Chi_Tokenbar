//! Layer ③ analytics (UX Spec v3 §11): aggregate local JSONL into daily /
//! model / agent breakdowns, cost estimate, and stats. All from local files —
//! no undocumented API, no risk. Dates are bucketed in UTC for consistency.

use chrono::{Datelike, Timelike};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
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

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MaxDayRecord {
    pub date: String,
    pub tokens: u64,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MaxHourRecord {
    pub date: String,
    pub hour: u8,
    pub tokens: u64,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Records {
    pub max_day: MaxDayRecord,
    pub max_hour: MaxHourRecord,
    pub streak_days: u32,
    pub pr_now: bool,
}

#[derive(Serialize)]
pub struct Breakdown {
    pub input: u64,
    pub cached: u64,
    pub output: u64,
    pub reasoning: u64,
}

/// One activity-type slice (階段 C+). `kind` is a stable id the frontend maps to
/// a localized label: "edit" | "read" | "run" | "other". Claude-only — see the
/// scan-source recon note below `scan_codex`.
#[derive(Serialize)]
pub struct KindCount {
    pub kind: String,
    pub tokens: u64,
}

/// Per-project token total (階段 C+). Usage-only.
/// 隱私硬限制:不得進戰報(§0)——`buildShareData` 禁止引用 `by_project`。
/// The remainder beyond the top 8 is merged under the id "__other__".
#[derive(Serialize)]
pub struct ProjectCount {
    pub name: String,
    pub tokens: u64,
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
        .map(|d| d.format("%Y-%m-%d").to_string())
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
        }
    }

    /// `project` = a bare folder name ("" to skip project attribution).
    /// `kind` = an activity kind for classifiable providers (Claude); None for
    /// providers whose tokens aren't per-tool attributable (Codex).
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
        if total == 0 {
            return;
        }
        let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) else {
            return;
        };
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
        let local = dt.with_timezone(&chrono::Local);
        *self
            .hourly_by_day
            .entry((local.format("%Y-%m-%d").to_string(), local.hour() as u8))
            .or_default() += total;

        self.breakdown.input += input;
        self.breakdown.cached += cached;
        self.breakdown.output += output;
        self.breakdown.reasoning += reasoning;

        if !project.is_empty() {
            *self.by_project.entry(project.to_string()).or_default() += total;
        }
        if let Some(k) = kind {
            *self.by_kind.entry(k.to_string()).or_default() += total;
        }

        if self.now - ts <= 600 {
            self.recent_tokens += total;
        }
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

fn filter_accounts(
    filter: &str,
    tool_opencode: bool,
    tool_gemini: bool,
    accounts: Vec<Account>,
) -> Vec<Account> {
    accounts
        .into_iter()
        .filter(|a| match a.provider.as_str() {
            "anthropic" => scans_claude(filter),
            "codex" => scans_codex(filter),
            // 階段 E: gated by their own toggle, not the anthropic/codex filter.
            "opencode" => tool_opencode,
            "gemini" => tool_gemini,
            _ => true,
        })
        .collect()
}

/// Compute analytics for "today" or "week", scoped to the display filter.
///
/// Skips the scan outright rather than scanning then discarding: `scan_*`
/// walks a whole directory tree, and a hidden provider's files are pure waste.
pub fn compute_with(
    range: &str,
    filter: &str,
    tool_opencode: bool,
    tool_gemini: bool,
) -> Analytics {
    compute_routed(
        range,
        filter,
        scan_codex,
        scan_claude,
        scan_opencode,
        scan_gemini,
        tool_opencode,
        tool_gemini,
        detect_accounts(),
    )
}

/// The real body of `compute_with`, with every source of ambient state
/// (the two directory scans and account detection) passed in.
///
/// This split exists purely so the filter routing below is testable: the
/// scanners read the real home dir, so a test that cannot replace them can
/// only re-assert `scans_*`, which proves nothing about which branch runs.
#[allow(clippy::too_many_arguments)]
fn compute_routed<C, L, O, G>(
    range: &str,
    filter: &str,
    scan_codex_fn: C,
    scan_claude_fn: L,
    scan_opencode_fn: O,
    scan_gemini_fn: G,
    tool_opencode: bool,
    tool_gemini: bool,
    accounts: Vec<Account>,
) -> Analytics
where
    C: FnOnce(&mut Acc, i64) -> u32,
    L: FnOnce(&mut Acc, i64),
    O: FnOnce(&mut Acc, i64),
    G: FnOnce(&mut Acc, i64),
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
    // 階段 E 多工具: gated purely on their own toggle, independent of the
    // anthropic/codex `providers` filter (those two are quota pools; these are
    // separate clients). A disabled tool is never scanned; an enabled tool with
    // no local data simply contributes nothing (no fake 0 card — Acc::add drops
    // zero-token rows, and byAgent only holds keys that actually had usage).
    if tool_opencode {
        scan_opencode_fn(&mut acc, start);
    }
    if tool_gemini {
        scan_gemini_fn(&mut acc, start);
    }

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

    Analytics {
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
        accounts: filter_accounts(filter, tool_opencode, tool_gemini, accounts),
    }
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

fn scan_codex(acc: &mut Acc, start: i64) -> u32 {
    let Some(home) = dirs::home_dir() else {
        return 0;
    };
    let pattern = home
        .join(".codex/sessions/**/rollout-*.jsonl")
        .to_string_lossy()
        .replace('\\', "/");
    let mut sessions = 0;
    let mut seen = HashSet::new();
    if let Ok(paths) = glob::glob(&pattern) {
        for p in paths.filter_map(Result::ok) {
            let ts = mtime_secs(&p);
            if ts < start {
                continue;
            }
            sessions += 1;
            if let Ok(file) = File::open(&p) {
                let project = first_cwd_basename(&p);
                scan_codex_lines(
                    acc,
                    start,
                    &project,
                    BufReader::new(file).lines().map_while(Result::ok),
                    &mut seen,
                );
            }
        }
    }
    sessions
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

fn claude_cost(
    model: &str,
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write_5m: u64,
    cache_write_1h: u64,
) -> f64 {
    let Some(r) = claude_rates(model) else {
        return ((input + output + cache_read + cache_write_5m + cache_write_1h) as f64 / 1e6)
            * rate_per_mtok(model);
    };
    (input as f64 * r.input
        + output as f64 * r.output
        + cache_read as f64 * r.cache_read
        + cache_write_5m as f64 * r.cache_write_5m
        + cache_write_1h as f64 * r.cache_write_1h)
        / 1e6
}

fn codex_cost(model: &str, input: u64, cached: u64, output: u64, reasoning: u64) -> f64 {
    let rate = rate_per_mtok(model);
    (input.saturating_sub(cached) as f64 * rate
        + cached as f64 * rate * 0.1
        + (output + reasoning) as f64 * rate)
        / 1e6
}

type CodexUsage = (u64, u64, u64, u64);

fn codex_token_event(line: &str) -> Option<(i64, CodexUsage)> {
    if !line.contains("token_count") {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let payload = v.get("payload")?;
    if payload.get("type").and_then(|x| x.as_str()) != Some("token_count") {
        return None;
    }
    let usage = payload
        .get("info")
        .and_then(|x| x.get("total_token_usage"))
        .or_else(|| payload.get("total_token_usage"))?;
    let get = |key: &str| usage.get(key).and_then(|x| x.as_u64()).unwrap_or(0);
    Some((
        v.get("timestamp").and_then(parse_epoch)?,
        (
            get("input_tokens"),
            get("cached_input_tokens"),
            get("output_tokens"),
            get("reasoning_output_tokens"),
        ),
    ))
}

fn usage_total((input, cached, output, reasoning): CodexUsage) -> u64 {
    input
        .saturating_add(cached)
        .saturating_add(output)
        .saturating_add(reasoning)
}

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
        let Some((ts, current)) = codex_token_event(&line) else {
            continue;
        };
        let total = usage_total(current);
        let duplicate = !seen.insert((ts, total));
        let prior = previous.replace(current).unwrap_or((0, 0, 0, 0));
        if duplicate || ts < start || total.saturating_sub(usage_total(prior)) == 0 {
            continue;
        }
        let (i, ca, o, r) = current;
        let (pi, pca, po, pr) = prior;
        let di = i.saturating_sub(pi);
        let dca = ca.saturating_sub(pca);
        let do_ = o.saturating_sub(po);
        let dr = r.saturating_sub(pr);
        // kind = None: Codex tokens aren't per-tool attributable (see note).
        acc.add_with_cost(
            ts,
            "gpt-5-codex",
            "Codex CLI",
            project,
            None,
            di,
            dca,
            do_,
            dr,
            codex_cost("gpt-5-codex", di, dca, do_, dr),
        );
    }
}

/// The session's project folder, from the first `payload.cwd` in the file
/// (session_meta / turn_context). "" when none is found in the opening lines.
fn first_cwd_basename(path: &PathBuf) -> String {
    let Ok(file) = File::open(path) else {
        return String::new();
    };
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    // cwd lives near the top (session_meta first, turn_context soon after); a
    // handful of lines is plenty and keeps this to one cheap head-read.
    for _ in 0..8 {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line.trim()) {
            if let Some(cwd) = v
                .get("payload")
                .and_then(|p| p.get("cwd"))
                .and_then(|c| c.as_str())
            {
                return basename(cwd);
            }
        }
    }
    String::new()
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
    let mut seen = HashSet::new();
    for p in paths.filter_map(Result::ok) {
        if mtime_secs(&p) < start {
            continue;
        }
        // Project = the immediate parent directory's slug name.
        let project = p
            .parent()
            .and_then(|d| d.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let Ok(file) = File::open(&p) else {
            continue;
        };
        scan_claude_lines(
            acc,
            start,
            &project,
            BufReader::new(file).lines().map_while(Result::ok),
            &mut seen,
        );
    }
}

fn scan_claude_lines(
    acc: &mut Acc,
    start: i64,
    project: &str,
    lines: impl Iterator<Item = String>,
    seen: &mut HashSet<String>,
) {
    for line in lines {
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
        let dedup_key = v
            .get("requestId")
            .and_then(|x| x.as_str())
            .or_else(|| msg.and_then(|m| m.get("id")).and_then(|x| x.as_str()))
            .or_else(|| v.get("uuid").and_then(|x| x.as_str()));
        if dedup_key.is_some_and(|key| !seen.insert(key.to_string())) {
            continue;
        }
        let model = msg
            .and_then(|m| m.get("model"))
            .and_then(|m| m.as_str())
            .unwrap_or("claude");
        // Activity kind from this message's tool_use names.
        let mut tools: Vec<String> = Vec::new();
        if let Some(content) = msg
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
        {
            for it in content {
                if it.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                    if let Some(name) = it.get("name").and_then(|n| n.as_str()) {
                        tools.push(name.to_string());
                    }
                }
            }
        }
        let kind = message_kind(&tools);
        let input = usage
            .get("input_tokens")
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        let output = usage
            .get("output_tokens")
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        let cache_read = usage
            .get("cache_read_input_tokens")
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        let cache_creation = usage
            .get("cache_creation_input_tokens")
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        let cache_creation_detail = usage.get("cache_creation");
        let cache_write_1h = cache_creation_detail
            .and_then(|x| x.get("ephemeral_1h_input_tokens"))
            .and_then(|x| x.as_u64())
            .unwrap_or(0);
        let cache_write_5m = cache_creation_detail
            .and_then(|x| x.get("ephemeral_5m_input_tokens"))
            .and_then(|x| x.as_u64())
            .unwrap_or_else(|| cache_creation.saturating_sub(cache_write_1h));
        let cached = cache_read + cache_creation;
        let cost = claude_cost(
            model,
            input,
            output,
            cache_read,
            cache_write_5m,
            cache_write_1h,
        );
        acc.add_with_cost(
            ts,
            model,
            "Claude Code",
            &project,
            Some(kind),
            input,
            cached,
            output,
            0,
            cost,
        );
    }
}

// ── OpenCode: one JSON file per message (階段 E) ──────────────────────────
//
// 勘察結論(2026-07-17,見 data-sources-findings.md §4.1):本機**未安裝**
// OpenCode。scanner 依「文件化格式」實作,執行期目錄不存在即回空:
//   storage/message/<sessionID>/<messageID>.json —— assistant 訊息帶 `role`、
//   `modelID`、`time.created`(epoch 毫秒)、`tokens.{input,output,reasoning,
//   cache.read,cache.write}`。基底目錄取 XDG data + 常見備援。
// Limits:OpenCode 本機無官方 limit 檔(額度歸後端 provider)→ 僅 Usage。

/// Candidate OpenCode storage roots (first existing wins is *not* assumed — all
/// are scanned so a mirrored/legacy layout is still picked up).
fn opencode_bases() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(home) = dirs::home_dir() {
        v.push(home.join(".local/share/opencode"));
        v.push(home.join(".opencode"));
        v.push(home.join(".config/opencode"));
    }
    if let Some(local) = dirs::data_local_dir() {
        v.push(local.join("opencode"));
    }
    v
}

/// Parse one OpenCode message object into `(ts_secs, model, input, cached,
/// output, reasoning)`. `None` for non-assistant messages, malformed objects,
/// or ones with no token counts — the scanner then skips them.
fn oc_record(v: &serde_json::Value) -> Option<(i64, String, u64, u64, u64, u64)> {
    if v.get("role").and_then(|r| r.as_str()) != Some("assistant") {
        return None;
    }
    let tokens = v.get("tokens")?;
    let get = |k: &str| tokens.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
    let input = get("input");
    let output = get("output");
    let reasoning = get("reasoning");
    let cache = tokens.get("cache");
    let cache_get = |k: &str| {
        cache
            .and_then(|c| c.get(k))
            .and_then(|x| x.as_u64())
            .unwrap_or(0)
    };
    let cached = cache_get("read") + cache_get("write");
    if input + output + reasoning + cached == 0 {
        return None;
    }
    let ts = v
        .get("time")
        .and_then(|t| t.get("created"))
        .and_then(parse_epoch)
        .unwrap_or(0);
    let model = v
        .get("modelID")
        .and_then(|m| m.as_str())
        .unwrap_or("opencode")
        .to_string();
    Some((ts, model, input, cached, output, reasoning))
}

fn scan_opencode(acc: &mut Acc, start: i64) {
    for base in opencode_bases() {
        let pattern = base
            .join("storage/message/**/*.json")
            .to_string_lossy()
            .replace('\\', "/");
        let Ok(paths) = glob::glob(&pattern) else {
            continue;
        };
        for p in paths.filter_map(Result::ok) {
            if mtime_secs(&p) < start {
                continue;
            }
            let Ok(text) = fs::read_to_string(&p) else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            if let Some((ts, model, i, ca, o, r)) = oc_record(&v) {
                if ts < start {
                    continue;
                }
                // kind = None: OpenCode tokens aren't per-tool attributable here;
                // project = "": message files don't carry cwd (see findings §4.1).
                acc.add(ts, &model, "OpenCode", "", None, i, ca, o, r);
            }
        }
    }
}

// ── Gemini CLI: documented JSONL usage records (階段 E) ────────────────────
//
// 勘察結論(2026-07-17,見 data-sources-findings.md §4.2):本機 `~/.gemini/`
// 只有 Antigravity IDE 的 protobuf 資料,**無** Gemini CLI 用量 log。scanner 依
// 一個文件化的 JSONL 用量形狀掃 `~/.gemini/**/*.jsonl`(每行 `{timestamp,model,
// tokens:{input,output,cached,thoughts}}`),掃不到即回空。**只吃 *.jsonl**,因此
// 天然避開 `oauth_creds.json` / `settings.json`(憑證/設定,鐵則:連讀都不讀)。
// Limits:Gemini CLI 本機無官方 limit 檔 → 僅 Usage。

/// Parse one Gemini usage log line into `(ts_secs, model, input, cached,
/// output, reasoning)`. `None` for a blank/corrupt line or one with no tokens,
/// so a malformed line is skipped rather than aborting the file.
fn gemini_record(line: &str) -> Option<(i64, String, u64, u64, u64, u64)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let tokens = v.get("tokens")?;
    let get = |k: &str| tokens.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
    let input = get("input");
    let output = get("output");
    let cached = get("cached");
    let thoughts = get("thoughts"); // Gemini's reasoning-equivalent field
    if input + output + cached + thoughts == 0 {
        return None;
    }
    let ts = v.get("timestamp").and_then(parse_epoch)?;
    let model = v
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("gemini")
        .to_string();
    Some((ts, model, input, cached, output, thoughts))
}

fn scan_gemini(acc: &mut Acc, start: i64) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let pattern = home
        .join(".gemini/**/*.jsonl")
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
            if let Some((ts, model, i, ca, o, r)) = gemini_record(&line) {
                if ts < start {
                    continue;
                }
                acc.add(ts, &model, "Gemini CLI", "", None, i, ca, o, r);
            }
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

    /// Convenience wrapper: run `compute_routed` with the 階段 E scanners disabled,
    /// so every pre-E test reads exactly as before (OpenCode/Gemini contribute
    /// nothing). 階段 E's own tests call `compute_routed` directly.
    fn routed<C, L>(
        range: &str,
        filter: &str,
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
            filter,
            codex,
            claude,
            |_, _| {},
            |_, _| {},
            false,
            false,
            accounts,
        )
    }

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
            true,
            true,
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

    /// Agents that actually got scanned, for `filter`, sorted.
    fn scanned_agents(filter: &str) -> Vec<String> {
        let a = routed("today", filter, stub_codex, stub_claude, Vec::new());
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
        let claude_only = routed("today", "claude", stub_codex, stub_claude, Vec::new());
        assert_eq!(claude_only.total_tokens, 200);
        assert_eq!(claude_only.sessions_this_week, 0);

        let codex_only = routed("today", "codex", stub_codex, stub_claude, Vec::new());
        assert_eq!(codex_only.total_tokens, 100);
        assert_eq!(codex_only.sessions_this_week, 7);

        let everything = routed("today", "both", stub_codex, stub_claude, Vec::new());
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
        let a = routed("month", "claude", no_codex, stub_recent(3), Vec::new());
        assert_eq!(a.daily.len(), 30);
        assert_eq!(a.total_tokens, 300); // 3 days × 100
    }

    #[test]
    fn month_range_reports_actual_start_when_history_is_short() {
        let a = routed("month", "claude", no_codex, stub_recent(3), Vec::new());
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
        let a = routed("month", "claude", no_codex, |_, _| {}, Vec::new());
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
        let a = routed("today", "claude", no_codex, stub_activity, Vec::new());
        assert_eq!(a.by_kind[0].kind, "edit"); // 130 > 50
        assert_eq!(a.by_kind[0].tokens, 130);
        let read = a.by_kind.iter().find(|k| k.kind == "read").unwrap();
        assert_eq!(read.tokens, 50);
    }

    #[test]
    fn by_project_aggregates_and_sorts() {
        let a = routed("today", "claude", no_codex, stub_activity, Vec::new());
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
        let a = routed("today", "codex", codex_only, |_, _| {}, Vec::new());
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
            codex_line("2026-07-17T01:00:00Z", 100),
            codex_line("2026-07-17T02:00:00Z", 250),
            codex_line("2026-07-17T03:00:00Z", 250),
            codex_line("2026-07-17T04:00:00Z", 400),
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
            codex_line("2026-07-16T23:59:00Z", 100),
            codex_line("2026-07-17T00:01:00Z", 250),
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
            codex_line("2026-07-17T01:00:00Z", 250),
            codex_line("2026-07-17T02:00:00Z", 100),
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
        let a = routed("today", "claude", no_codex, many, Vec::new());
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
        let a = routed("today", "claude", no_codex, stub_recent(1), Vec::new());
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
        let a = routed("today", "both", stub_codex, stub_claude, Vec::new());

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

    // ── 階段 E: OpenCode / Gemini parsers + toggles ──────────────────────
    //
    // The parsers are the unit under test (the dir walks read the real home
    // dir); fake data proves a well-formed record parses and a broken/empty one
    // is skipped rather than crashing.

    #[test]
    fn oc_record_parses_an_assistant_message() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{ "role": "assistant", "modelID": "claude-sonnet",
                 "time": { "created": 1782590000000 },
                 "tokens": { "input": 100, "output": 20, "reasoning": 5,
                             "cache": { "read": 10, "write": 3 } } }"#,
        )
        .unwrap();
        let (ts, model, input, cached, output, reasoning) = oc_record(&v).unwrap();
        assert_eq!(ts, 1782590000); // ms → s
        assert_eq!(model, "claude-sonnet");
        assert_eq!(input, 100);
        assert_eq!(cached, 13); // read + write
        assert_eq!(output, 20);
        assert_eq!(reasoning, 5);
    }

    #[test]
    fn oc_record_skips_non_assistant_and_tokenless() {
        // A user message: no usage to book.
        let user = serde_json::json!({ "role": "user", "tokens": { "input": 5 } });
        assert!(oc_record(&user).is_none());
        // An assistant message whose tokens are all zero adds nothing.
        let empty = serde_json::json!({ "role": "assistant", "tokens": { "input": 0 } });
        assert!(oc_record(&empty).is_none());
        // Missing the tokens object entirely.
        let bare = serde_json::json!({ "role": "assistant" });
        assert!(oc_record(&bare).is_none());
    }

    #[test]
    fn gemini_record_parses_a_usage_line() {
        let line = r#"{ "timestamp": 1782590000000, "model": "gemini-2.5-pro",
                        "tokens": { "input": 200, "output": 40, "cached": 15, "thoughts": 8 } }"#;
        let (ts, model, input, cached, output, reasoning) = gemini_record(line).unwrap();
        assert_eq!(ts, 1782590000);
        assert_eq!(model, "gemini-2.5-pro");
        assert_eq!(input, 200);
        assert_eq!(cached, 15);
        assert_eq!(output, 40);
        assert_eq!(reasoning, 8); // thoughts → reasoning
    }

    #[test]
    fn gemini_record_parses_rfc3339_timestamp() {
        let line = r#"{ "timestamp": "2026-07-10T06:19:59+00:00", "model": "gemini",
                        "tokens": { "input": 10 } }"#;
        let (ts, _, input, ..) = gemini_record(line).unwrap();
        assert_eq!(input, 10);
        assert!(ts > 0, "RFC3339 timestamp should parse to a positive epoch");
    }

    #[test]
    fn gemini_record_skips_corrupt_and_empty_lines() {
        assert!(gemini_record("").is_none());
        assert!(gemini_record("   ").is_none());
        assert!(gemini_record("{ not valid json").is_none()); // a truncated/broken line
                                                              // Valid JSON but no tokens → nothing to book.
        assert!(gemini_record(r#"{ "timestamp": 1, "model": "gemini" }"#).is_none());
        // Valid JSON, all-zero tokens → skipped.
        assert!(gemini_record(r#"{ "timestamp": 1, "tokens": { "input": 0 } }"#).is_none());
    }

    /// The toggles gate the 階段 E scanners independently of the codex/claude
    /// filter: off means the scanner never runs, so its tokens never appear.
    #[test]
    fn tool_toggles_gate_opencode_and_gemini_scans() {
        fn oc(acc: &mut Acc, _s: i64) {
            acc.add(acc.now, "oc-model", "OpenCode", "", None, 100, 0, 0, 0);
        }
        fn gem(acc: &mut Acc, _s: i64) {
            acc.add(acc.now, "gemini", "Gemini CLI", "", None, 50, 0, 0, 0);
        }

        // Both on: both agents present, both quota-pool scans off so nothing else.
        let on = compute_routed(
            "today",
            "both",
            no_codex,
            |_, _| {},
            oc,
            gem,
            true,
            true,
            Vec::new(),
        );
        assert_eq!(on.by_agent.get("OpenCode"), Some(&100));
        assert_eq!(on.by_agent.get("Gemini CLI"), Some(&50));

        // Both off: neither scan runs, neither agent appears (no fake 0 card).
        let off = compute_routed(
            "today",
            "both",
            no_codex,
            |_, _| {},
            oc,
            gem,
            false,
            false,
            Vec::new(),
        );
        assert!(off.by_agent.get("OpenCode").is_none());
        assert!(off.by_agent.get("Gemini CLI").is_none());
        assert_eq!(off.total_tokens, 0);
    }

    /// A tool with no local data must not surface (the empty-scan case): an
    /// enabled-but-empty scanner contributes nothing, so no agent/legend entry.
    #[test]
    fn enabled_but_empty_tool_adds_nothing() {
        let a = compute_routed(
            "today",
            "both",
            no_codex,
            |_, _| {},
            |_, _| {},
            |_, _| {},
            true,
            true,
            Vec::new(),
        );
        assert!(a.by_agent.is_empty());
        assert_eq!(a.total_tokens, 0);
    }

    /// Accounts for the new tools follow their own toggle, not the provider
    /// filter (which only narrows anthropic/codex).
    #[test]
    fn tool_accounts_follow_their_toggle() {
        let accts = || {
            vec![
                Account {
                    client: "OpenCode".into(),
                    provider: "opencode".into(),
                    account: "—".into(),
                    plan: "—".into(),
                },
                Account {
                    client: "Gemini CLI".into(),
                    provider: "gemini".into(),
                    account: "—".into(),
                    plan: "—".into(),
                },
            ]
        };
        assert_eq!(filter_accounts("both", true, true, accts()).len(), 2);
        assert_eq!(filter_accounts("both", false, false, accts()).len(), 0);
        let only_oc = filter_accounts("both", true, false, accts());
        assert_eq!(only_oc.len(), 1);
        assert_eq!(only_oc[0].provider, "opencode");
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
    // 階段 E: OpenCode/Gemini are intentionally NOT surfaced as accounts on mere
    // directory existence — `~/.gemini/` in particular is shared with Antigravity
    // and would fabricate a "Gemini CLI" card with no usage (the plan bans 0
    // cards). They appear via usage-driven byAgent instead; `filter_accounts`
    // still gates their `provider` keys should a future account source add them.
    out
}
