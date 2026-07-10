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

/// Compute analytics for "today" or "week".
pub fn compute(range: &str) -> Analytics {
    let now = chrono::Utc::now().timestamp();
    let days_back: i64 = if range == "today" { 0 } else { 6 };
    let utc_midnight = now - now.rem_euclid(86400);
    let start = utc_midnight - days_back * 86400;

    let mut acc = Acc::new(now);
    let sessions = scan_codex(&mut acc, start);
    scan_claude(&mut acc, start);

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

    Analytics {
        range: range.to_string(),
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
        accounts: detect_accounts(),
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
