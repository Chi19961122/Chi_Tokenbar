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
    pub daily: Vec<DayPoint>,
    pub hourly: Vec<u64>,
    pub by_model: HashMap<String, u64>,
    pub by_agent: HashMap<String, u64>,
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
// (MCP tools, Task*, Agent, chat-only turns) is "other" — a real bucket, not a
// fabricated one. Kept deliberately small so each mapping is defensible.
fn classify_kind(name: &str) -> &'static str {
    match name {
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => "edit",
        "Read" | "Grep" | "Glob" | "LS" | "ToolSearch" | "WebSearch" | "WebFetch" => "read",
        "Bash" | "PowerShell" => "run",
        _ => "other",
    }
}

/// The single kind attributed to one assistant message, from the tools it used.
/// A message's tokens are booked whole to its dominant tool kind (ties break in
/// edit>read>run>other order); a message with no tool_use is "other".
fn message_kind(tool_names: &[String]) -> &'static str {
    if tool_names.is_empty() {
        return "other";
    }
    let mut counts = [0u32; 4]; // edit, read, run, other
    for n in tool_names {
        let idx = match classify_kind(n) {
            "edit" => 0,
            "read" => 1,
            "run" => 2,
            _ => 3,
        };
        counts[idx] += 1;
    }
    let kinds = ["edit", "read", "run", "other"];
    let mut best = 0;
    for i in 1..4 {
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
            breakdown: Breakdown { input: 0, cached: 0, output: 0, reasoning: 0 },
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
        by_project.push(ProjectCount { name: "__other__".to_string(), tokens: other_project });
    }

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
        by_kind,
        by_project,
        sessions_this_week: sessions,
        tok_per_min: (acc.recent_tokens as f64 / 10.0) as u64,
        accounts: filter_accounts(filter, accounts),
    }
}

// ── Codex: tail-read each recent session for its cumulative total ────────
//
// 階段 C+ 資料源勘察結論(2026-07-17,本機真實 log 抽樣,只看結構):
//
// Codex rollout-*.jsonl 每行 `{timestamp,type,payload}`。可用於本階段的欄位:
//   · payload.type == "token_count" 帶 `total_token_usage`(累計值,取最後一筆
//     即為本 session 總量;現行 tail-read 即靠此)。
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
    if let Ok(paths) = glob::glob(&pattern) {
        for p in paths.filter_map(Result::ok) {
            let ts = mtime_secs(&p);
            if ts < start {
                continue;
            }
            sessions += 1;
            if let Some((i, ca, o, r)) = last_total_usage(&p) {
                let project = first_cwd_basename(&p);
                // kind = None: Codex tokens aren't per-tool attributable (see note).
                acc.add(ts, "gpt-5-codex", "Codex CLI", &project, None, i, ca, o, r);
            }
        }
    }
    sessions
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
            if let Some(cwd) = v.get("payload").and_then(|p| p.get("cwd")).and_then(|c| c.as_str())
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
            // Activity kind from this message's tool_use names.
            let mut tools: Vec<String> = Vec::new();
            if let Some(content) = msg.and_then(|m| m.get("content")).and_then(|c| c.as_array()) {
                for it in content {
                    if it.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        if let Some(name) = it.get("name").and_then(|n| n.as_str()) {
                            tools.push(name.to_string());
                        }
                    }
                }
            }
            let kind = message_kind(&tools);
            let input = usage.get("input_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
            let output = usage.get("output_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
            let cached = usage.get("cache_read_input_tokens").and_then(|x| x.as_u64()).unwrap_or(0)
                + usage.get("cache_creation_input_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
            acc.add(ts, model, "Claude Code", &project, Some(kind), input, cached, output, 0);
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
        acc.add(acc.now, "gpt-5-codex", CODEX_AGENT, "", None, 100, 0, 0, 0);
        7
    }

    fn stub_claude(acc: &mut Acc, _start: i64) {
        acc.add(acc.now, "claude-opus", CLAUDE_AGENT, "", None, 200, 0, 0, 0);
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
                acc.add(acc.now - k * 86400, "claude-opus", CLAUDE_AGENT, "", None, 100, 0, 0, 0);
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

    // ── activity types + projects (階段 C+) ──────────────────────────────

    #[test]
    fn classify_maps_tools_to_kinds() {
        assert_eq!(classify_kind("Edit"), "edit");
        assert_eq!(classify_kind("Write"), "edit");
        assert_eq!(classify_kind("Read"), "read");
        assert_eq!(classify_kind("Grep"), "read");
        assert_eq!(classify_kind("Bash"), "run");
        assert_eq!(classify_kind("PowerShell"), "run");
        assert_eq!(classify_kind("Agent"), "other");
        assert_eq!(classify_kind("mcp__whatever"), "other");
    }

    #[test]
    fn message_kind_picks_dominant_tool_and_defaults_to_other() {
        assert_eq!(message_kind(&[]), "other");
        assert_eq!(
            message_kind(&["Edit".to_string(), "Read".to_string(), "Edit".to_string()]),
            "edit"
        );
        // Tie between edit and read breaks to edit (fixed priority order).
        assert_eq!(message_kind(&["Read".to_string(), "Edit".to_string()]), "edit");
        assert_eq!(message_kind(&["Read".to_string(), "Read".to_string()]), "read");
    }

    #[test]
    fn basename_takes_the_last_folder_only() {
        assert_eq!(basename("C:\\Coding\\TokenBar"), "TokenBar");
        assert_eq!(basename("/home/me/proj/"), "proj");
        assert_eq!(basename(""), "");
    }

    /// Claude activity across two projects, several kinds.
    fn stub_activity(acc: &mut Acc, _start: i64) {
        acc.add(acc.now, "claude-opus", CLAUDE_AGENT, "proj-a", Some("edit"), 100, 0, 0, 0);
        acc.add(acc.now, "claude-opus", CLAUDE_AGENT, "proj-a", Some("read"), 50, 0, 0, 0);
        acc.add(acc.now, "claude-opus", CLAUDE_AGENT, "proj-b", Some("edit"), 30, 0, 0, 0);
    }

    #[test]
    fn by_kind_aggregates_claude_activity_sorted_desc() {
        let a = compute_routed("today", "claude", no_codex, stub_activity, Vec::new());
        assert_eq!(a.by_kind[0].kind, "edit"); // 130 > 50
        assert_eq!(a.by_kind[0].tokens, 130);
        let read = a.by_kind.iter().find(|k| k.kind == "read").unwrap();
        assert_eq!(read.tokens, 50);
    }

    #[test]
    fn by_project_aggregates_and_sorts() {
        let a = compute_routed("today", "claude", no_codex, stub_activity, Vec::new());
        assert_eq!(a.by_project[0].name, "proj-a"); // 150 > 30
        assert_eq!(a.by_project[0].tokens, 150);
        assert_eq!(a.by_project[1].name, "proj-b");
    }

    /// Codex contributes to projects (via cwd) but never to by_kind.
    #[test]
    fn codex_is_absent_from_by_kind() {
        fn codex_only(acc: &mut Acc, _start: i64) -> u32 {
            acc.add(acc.now, "gpt-5-codex", CODEX_AGENT, "proj-x", None, 100, 0, 0, 0);
            1
        }
        let a = compute_routed("today", "codex", codex_only, |_, _| {}, Vec::new());
        assert!(a.by_kind.is_empty(), "Codex must not produce activity kinds");
        assert_eq!(a.by_project[0].name, "proj-x");
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
        let a = compute_routed("today", "claude", no_codex, many, Vec::new());
        assert_eq!(a.by_project.len(), 9, "8 named projects + merged remainder");
        assert_eq!(a.by_project.last().unwrap().name, "__other__");
        // The remainder holds the two smallest (p08=60, p09=55).
        assert_eq!(a.by_project.last().unwrap().tokens, 115);
    }

    #[test]
    fn empty_project_is_not_recorded() {
        // A provider with no cwd ("") still counts its tokens but adds no project.
        let a = compute_routed("today", "claude", no_codex, stub_recent(1), Vec::new());
        assert_eq!(a.total_tokens, 100);
        assert!(a.by_project.is_empty());
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
