//! 本機可覆寫價目表(T-feat-006)。vendored 價目表(analytics::claude_rates)仍是
//! 預設與最終 fallback,本模組只加一層使用者可手編的 override:
//! `%APPDATA%\Atoll\pricing.json`(與 settings.json 同目錄)。
//!
//! 硬規定(容錯載入):任何檔案層級的壞掉都退回「無 override」(純 vendored),
//! 絕不因 override 檔壞而讓成本欄位變 0 或消失。單一條目壞只跳過該條目,其餘照用。
//! 零外連:本模組只讀本機檔,不發任何網路請求。

use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

/// override 檔大小上限。超過就整包忽略(避免有人不小心把巨檔丟進來拖慢每輪掃描)。
const MAX_BYTES: u64 = 1024 * 1024;

/// 解析後的單一 model 費率。兩種形態,對應查價鏈最終要嘛走分項五欄、要嘛走單一混合率。
/// Copy:查價每筆事件都會取一次,複製 5 個 f64 比 Arc 還便宜。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RateSpec {
    /// 分項五欄($/Mtok)。缺的 cache 欄位在解析時已用 input 比例補齊。
    Component {
        input: f64,
        output: f64,
        cache_read: f64,
        cache_write_5m: f64,
        cache_write_1h: f64,
    },
    /// 單一混合率($/Mtok),整筆 token 一律套同一個數字。
    Blended(f64),
}

/// 一份載入好的 override 表。key 一律以小寫存放,查價時大小寫不敏感。
///
/// entries 依 key 排序,讓 substring 比對在多個 key 都命中時結果穩定(可重現)。
pub struct PricingOverride {
    entries: Vec<(String, RateSpec)>,
}

impl PricingOverride {
    /// 空表 = 沒有任何 override,查價一律落到 vendored / blended。
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// 查一個 model id 的 override 費率:精確命中(大小寫不敏感)優先,再退 substring
    /// 命中(override key 是 model id 的子字串,同 vendored 家族表的 `contains` 風格)。
    /// 兩層都沒有 → None,交給呼叫端往 vendored / blended 走。
    pub fn lookup(&self, model: &str) -> Option<RateSpec> {
        let m = model.to_lowercase();
        if let Some((_, spec)) = self.entries.iter().find(|(k, _)| *k == m) {
            return Some(*spec);
        }
        self.entries
            .iter()
            .find(|(k, _)| m.contains(k.as_str()))
            .map(|(_, spec)| *spec)
    }

    /// 從 JSON 字串建表(可注入,測試不必碰真實 %APPDATA%)。
    ///
    /// 容錯:整檔 parse 失敗或 >1MB → 空表(純 vendored);單一條目壞 → 跳過並記
    /// `[tb] pricing override: skipped <key>`,其餘條目照收。缺 `models` 物件也視為
    /// 空表(沒有東西要覆寫,不算錯)。
    pub fn from_json_str(raw: &str) -> Self {
        if raw.len() as u64 > MAX_BYTES {
            eprintln!("[tb] pricing override: ignored whole file (exceeds 1MB)");
            return Self::empty();
        }
        let Ok(value) = serde_json::from_str::<Value>(raw) else {
            eprintln!("[tb] pricing override: ignored whole file (invalid JSON)");
            return Self::empty();
        };
        let Some(models) = value.get("models").and_then(|m| m.as_object()) else {
            return Self::empty();
        };
        let mut entries: Vec<(String, RateSpec)> = Vec::new();
        for (key, entry) in models {
            match parse_entry(entry) {
                Some(spec) => entries.push((key.to_lowercase(), spec)),
                None => eprintln!("[tb] pricing override: skipped {key}"),
            }
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        Self { entries }
    }
}

/// 解析單一 model 條目。回 None = 壞條目(缺必要欄、非數字、負數),呼叫端負責跳過。
///
/// 形態判定:先看 `blended`(有就走單一混合率),否則走分項。分項必須有 `input`
/// 與 `output`(推不出就當壞條目);三個 cache 欄可省,省的用 input 比例補齊
/// —— 比例(0.1 / 1.25 / 2.0)正好是 vendored 家族表遵循的關係,補值不會亂真。
fn parse_entry(v: &Value) -> Option<RateSpec> {
    let obj = v.as_object()?;
    if let Some(b) = obj.get("blended") {
        return Some(RateSpec::Blended(valid_rate(b)?));
    }
    let input = valid_rate(obj.get("input")?)?;
    let output = valid_rate(obj.get("output")?)?;
    let cache_read = optional_rate(obj.get("cache_read"), input * 0.1)?;
    let cache_write_5m = optional_rate(obj.get("cache_write_5m"), input * 1.25)?;
    let cache_write_1h = optional_rate(obj.get("cache_write_1h"), input * 2.0)?;
    Some(RateSpec::Component {
        input,
        output,
        cache_read,
        cache_write_5m,
        cache_write_1h,
    })
}

/// 一個合法費率:必須是有限的非負數。非數字 / 負數 / NaN / Inf 一律判壞。
/// (0 是使用者刻意設的免費費率,允許;容錯規定針對的是「壞檔」而非「刻意的 0」。)
fn valid_rate(v: &Value) -> Option<f64> {
    let n = v.as_f64()?;
    (n.is_finite() && n >= 0.0).then_some(n)
}

/// 可省欄位:缺 → 用 default;有但壞 → None(讓整條目被跳過,不默默吞掉壞值)。
fn optional_rate(v: Option<&Value>, default: f64) -> Option<f64> {
    match v {
        None => Some(default),
        Some(val) => valid_rate(val),
    }
}

/// override 檔路徑:與 settings.json 同目錄(config.rs 的 `dirs::config_dir()?/Atoll`)。
fn override_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("Atoll").join("pricing.json"))
}

/// (mtime 秒, 位元組大小)—— 重載判斷的指紋,同 codex 快照的 mtime 思路。
type Stamp = (i64, u64);

#[derive(Default)]
struct CacheState {
    /// 已載入過至少一次(用來區分「從沒載過」與「載過但檔案不存在」)。
    loaded: bool,
    /// 上次見到的 (mtime, size);None = 上次檔案不存在。
    stamp: Option<Stamp>,
    over: Option<Arc<PricingOverride>>,
}

fn cache() -> &'static Mutex<CacheState> {
    static C: OnceLock<Mutex<CacheState>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(CacheState::default()))
}

fn file_stamp(path: &PathBuf) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Some((mtime, meta.len()))
}

/// 每輪掃描前呼叫:stat 檔案 mtime/size,沒變就回上次快取的表,變了才重讀。
/// 檔案不存在 = 空表(純 vendored,零成本路徑),且不會自動建立範本檔。
pub fn current() -> Arc<PricingOverride> {
    let Some(path) = override_path() else {
        return Arc::new(PricingOverride::empty());
    };
    let stamp = file_stamp(&path);
    let mut g = cache().lock().unwrap_or_else(|p| p.into_inner());
    if g.loaded && g.stamp == stamp {
        if let Some(over) = &g.over {
            return over.clone();
        }
    }
    let over = Arc::new(load_from(&path, stamp));
    g.loaded = true;
    g.stamp = stamp;
    g.over = Some(over.clone());
    over
}

/// 依指紋決定怎麼載:不存在 → 空;>1MB → 空(不讀內容);否則讀檔交給 from_json_str。
/// 讀檔失敗(權限等)也退空表 —— 任何檔案層級問題都不得炸、不得清空成本。
fn load_from(path: &PathBuf, stamp: Option<Stamp>) -> PricingOverride {
    match stamp {
        None => PricingOverride::empty(),
        Some((_, size)) if size > MAX_BYTES => {
            eprintln!("[tb] pricing override: ignored whole file (exceeds 1MB)");
            PricingOverride::empty()
        }
        Some(_) => match std::fs::read_to_string(path) {
            Ok(raw) => PricingOverride::from_json_str(&raw),
            Err(_) => PricingOverride::empty(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_lookup_is_case_insensitive() {
        let o = PricingOverride::from_json_str(
            r#"{ "version": 1, "models": { "Fable-5": { "blended": 4.0 } } }"#,
        );
        assert_eq!(o.lookup("fable-5"), Some(RateSpec::Blended(4.0)));
        assert_eq!(o.lookup("FABLE-5"), Some(RateSpec::Blended(4.0)));
        assert_eq!(o.lookup("unrelated"), None);
    }

    #[test]
    fn missing_cache_columns_are_derived_from_input_ratio() {
        // 只給 input/output,三個 cache 欄用 input 比例補齊(0.1 / 1.25 / 2.0)。
        let o = PricingOverride::from_json_str(
            r#"{ "models": { "m": { "input": 10.0, "output": 50.0 } } }"#,
        );
        assert_eq!(
            o.lookup("m"),
            Some(RateSpec::Component {
                input: 10.0,
                output: 50.0,
                cache_read: 1.0,
                cache_write_5m: 12.5,
                cache_write_1h: 20.0,
            })
        );
    }

    #[test]
    fn bad_entries_are_skipped_not_fatal() {
        // 缺 output、負數、非數字各壞一條;好的那條照收。
        let o = PricingOverride::from_json_str(
            r#"{ "models": {
                "good":       { "input": 3.0, "output": 15.0 },
                "no_output":  { "input": 3.0 },
                "negative":   { "input": -1.0, "output": 5.0 },
                "not_number": { "blended": "free" }
            } }"#,
        );
        assert!(matches!(o.lookup("good"), Some(RateSpec::Component { .. })));
        assert_eq!(o.lookup("no_output"), None);
        assert_eq!(o.lookup("negative"), None);
        assert_eq!(o.lookup("not_number"), None);
    }

    #[test]
    fn whole_bad_file_yields_empty_table() {
        assert_eq!(PricingOverride::from_json_str("not json {{{").lookup("x"), None);
        assert_eq!(PricingOverride::from_json_str("").lookup("x"), None);
        // 缺 models 物件也視為空表。
        assert_eq!(
            PricingOverride::from_json_str(r#"{ "version": 1 }"#).lookup("x"),
            None
        );
    }

    #[test]
    fn oversize_file_is_ignored_whole() {
        let big = format!(
            r#"{{ "models": {{ "m": {{ "blended": 1.0, "pad": "{}" }} }} }}"#,
            "x".repeat(MAX_BYTES as usize)
        );
        assert_eq!(PricingOverride::from_json_str(&big).lookup("m"), None);
    }
}
