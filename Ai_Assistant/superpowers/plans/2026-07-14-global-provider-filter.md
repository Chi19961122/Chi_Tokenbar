# 全域平台過濾（顯示平台選擇）實作計畫

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把現有只作用於島嶼的 `island_mode`（並排／僅 Claude／僅 Codex），升級成**全域**的「顯示平台」設定 —— 選定後島嶼、面板、系統匣、通知、排名、分析頁全部只呈現該平台。

**Architecture:** 在排程器把兩個 provider 的 limits 合併後、送進 `engine.ingest()` **之前**做單一節點過濾（`lib.rs:248-250`），並且**連 poll 都跳過**被關掉的 provider（沿用 `codex_usage_source` 既有的 `matches!` 跳過模式，`lib.rs:238/243`）。分析頁不吃 Snapshot、直接讀檔（`analytics.rs:145-146`），必須另外過濾。

**Tech Stack:** Rust 2021, Tauri 2, TypeScript。

## 偵查結論（已驗證，非推測）

- **單一過濾節點存在**：`lib.rs:248` 合併 limits → `lib.rs:249` 接上 Anthropic → `lib.rs:250` `engine.ingest()`。在 249 與 250 之間過濾，下游全部自動一致。
- **下游都不必改**：
  - `panel.ts:62` 已有 `if (items.length === 0) return "";` → 空的 provider 分組自動跳過，`panel.ts:69` 還有「工具目前未在執行」fallback。**不用改。**
  - `lib.rs:310-324`（系統匣 tooltip）與 `lib.rs:338`（通知）都是直接遍歷 `snap.limits` → 自動跟著過濾。**不用改。**
  - `ranking.rs:26-51` 的 `WorstTracker::select()` 只從傳入的 limits 挑 → `worst_id` 不會選到被過濾掉的平台。**不用改。**
  - `island.ts:66-74` 本來就依 `opts.mode` 分流 → 把新設定值傳給它即可。**邏輯不用改。**
- **分析頁是唯一的例外**：`analytics.rs:138-197` 的 `compute(range)` 直接呼叫 `scan_codex()` / `scan_claude()` 掃本機檔案，**完全不經過 Snapshot**，不受任何過濾影響。必須單獨處理。

## ⚠️ 最大的坑：`#[serde(default)]` 不驗證值

`config.rs:7` 的 `#[serde(default)]` 是**容器層級**屬性 —— 它只在欄位**缺失**時套用 `impl Default`，**完全不檢查值的內容**。`island_mode: String` 會原封不動接受 `"worst"` 或任何字串。

HANDOFF.md:28 記載的「舊值 `worst` 會 fallback 成並排」**不是 serde 做的**，而是 `island.ts:66-74` 的 else 分支順手吃掉的（非 `claude`/`codex` 一律當並排）。也就是說：**目前的容錯完全依賴前端的 else 分支，後端從未驗證過這個值。**

一旦把過濾搬到後端，這個容錯就消失了。若用 `match` 且沒有 default 分支，一個殘留的 `"worst"`（或使用者手改 settings.json 打錯字）會把**兩個平台都濾掉，整個 app 變空白**。

→ **本計畫的過濾函式必須有明確的 catch-all，且「未知值 = 顯示全部」，永不回空。** Step 1 的測試專門擋這件事。

## Global Constraints

- 過濾只做一次，就在 `lib.rs` 排程器。**不得**在前端各消費點各寫一份（那種寫法遲早漏掉一處 —— 現況的 tray tooltip 和通知就是漏掉的那兩處）。
- 未知/舊設定值一律降級為「顯示全部」，永不產生空畫面。
- 跑 cargo 前 PATH 要先 prepend `%USERPROFILE%\.cargo\bin`。

---

### Task 1: 設定欄位改名 + 舊值遷移

**Files:**
- Modify: `src-tauri/src/config.rs`
- Modify: `src/types.ts`、`src/main.ts`

**設計決定（2026-07-14 由使用者拍板，勿再更動）:** 採用**單一設定**取代 `island_mode`，而非新增第二個設定。理由：兩個設定會互相矛盾 —— 全域選「僅 Claude」但島嶼設「僅 Codex」時該聽誰？多一個設定就多一組衝突狀態要處理。

**已知並接受的代價:** 失去「面板兩個都看、但島嶼只顯示一個省空間」這個組合（島嶼並排 340px、單一 270px，這是 `island_mode` 當初存在的理由）。使用者已知悉此代價並選擇單一設定 —— 不要「體貼地」把島嶼獨立選項加回來。

- [ ] **Step 1: 先寫失敗的遷移測試**

```rust
#[test]
fn migrates_island_mode_to_providers() {
    let s: Settings = load_from_str(r#"{ "island_mode": "claude" }"#);
    assert_eq!(s.providers, "claude");
}

#[test]
fn explicit_providers_wins_over_legacy_island_mode() {
    let s: Settings = load_from_str(r#"{ "island_mode": "codex", "providers": "claude" }"#);
    assert_eq!(s.providers, "claude");
}

#[test]
fn missing_both_defaults_to_all() {
    assert_eq!(load_from_str("{}").providers, "both");
}
```

- [ ] **Step 2: 跑測試確認失敗**

Run: `cargo test --manifest-path src-tauri\Cargo.toml config::tests`

- [ ] **Step 3: 實作欄位與遷移**

`config.rs` 的 `Settings` 新增 `pub providers: String`（預設 `"both"`），保留 `island_mode` 為 deprecated 欄位（不可直接刪 —— 刪掉會讓舊 settings.json 的使用者偏好無聲消失）。

遷移邏輯放在 `load()`：現行是 `serde_json::from_str().ok().unwrap_or_default()`，改為先解析成 `Value`、檢查「有 `island_mode` 但沒有 `providers`」時把值搬過去，再反序列化。抽成純函式 `load_from_str(raw: &str) -> Settings` 以便測試（現行 `load()` 直接讀檔，無法測）。

- [ ] **Step 4: 設定 UI 改標籤**

`main.ts:162-166` 的 `#s-island` 下拉，標籤從「島嶼顯示」改為「**顯示平台**」，並在 `main.ts:174-185` 的 `readSettingsForm()` 改讀寫 `providers`。選項文字改為「兩個都顯示 / 只顯示 Claude / 只顯示 Codex」（原本的「並排」是島嶼視角的用語，現在是全域設定了，措辭要跟著改）。

`types.ts:39` 的 `IslandMode` 改名為 `ProviderFilter`，`types.ts:48` 對應更新。`main.ts:82` 傳給 `renderIsland()` 的 `opts.mode` 改讀 `settings?.providers ?? "both"` —— **`island.ts` 本身不用動**。

- [ ] **Step 5: 跑測試 + Commit**

```bash
git -C C:\Coding\TokenBar\TokenBar-Src add src-tauri/src/config.rs src/types.ts src/main.ts
git -C C:\Coding\TokenBar\TokenBar-Src commit -m "feat: promote island_mode to a global provider filter setting"
```

### Task 2: 排程器單一節點過濾

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 先寫失敗的過濾測試**

**這幾個測試是本計畫的核心防線** —— 特別是最後一個，它擋的正是上面「`serde(default)` 不驗證值」那個坑：

```rust
/// 未知值(含舊的 "worst"、使用者手改打錯字)一律顯示全部,永不回空。
#[test]
fn unknown_filter_value_shows_everything() {
    let all = vec![limit("codex.5h", Provider::Codex), limit("cc.5h", Provider::Anthropic)];
    assert_eq!(apply_provider_filter("worst", all.clone()).len(), 2);
    assert_eq!(apply_provider_filter("", all.clone()).len(), 2);
    assert_eq!(apply_provider_filter("CLAUDE", all).len(), 2); // 大小寫不符也不能變空
}

#[test]
fn claude_filter_drops_codex() {
    let all = vec![limit("codex.5h", Provider::Codex), limit("cc.5h", Provider::Anthropic)];
    let out = apply_provider_filter("claude", all);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].provider, Provider::Anthropic);
}

#[test]
fn codex_filter_drops_claude() {
    let all = vec![limit("codex.5h", Provider::Codex), limit("cc.5h", Provider::Anthropic)];
    let out = apply_provider_filter("codex", all);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].provider, Provider::Codex);
}

#[test]
fn both_keeps_everything() {
    let all = vec![limit("codex.5h", Provider::Codex), limit("cc.5h", Provider::Anthropic)];
    assert_eq!(apply_provider_filter("both", all).len(), 2);
}
```

- [ ] **Step 2: 跑測試確認失敗**

Run: `cargo test --manifest-path src-tauri\Cargo.toml apply_provider_filter`

- [ ] **Step 3: 實作過濾 + 跳過 poll**

```rust
/// 依「顯示平台」設定過濾。未知值一律顯示全部(絕不回空)。
pub fn apply_provider_filter(filter: &str, limits: Vec<Limit>) -> Vec<Limit> {
    match filter {
        "claude" => limits.into_iter().filter(|l| l.provider == Provider::Anthropic).collect(),
        "codex" => limits.into_iter().filter(|l| l.provider == Provider::Codex).collect(),
        _ => limits, // "both" 與任何未知值
    }
}
```

在 `lib.rs:232-237` 讀 `codex_source` / `allow_refresh` 的同一處，一併讀出 `providers`。然後：

1. **跳過不需要的 poll**（沿用 `lib.rs:238/243` 既有的 `matches!` 模式）：`providers == "claude"` 時不呼叫 `codex_live.poll()` 也不呼叫 `codex::read_limits()`；`providers == "codex"` 時不呼叫 `anthropic.poll()`。這不只是省資源 —— 選了「只用 Claude」卻還在背景打 Codex 的 API，行為上說不過去。
2. **仍然在 `lib.rs:249` 與 `250` 之間套一次 `apply_provider_filter`** 作為保險。跳過 poll 是最佳化，過濾才是正確性保證；只靠前者的話，日後有人加了第三個 provider 就會漏。

- [ ] **Step 4: 確認切換時沒有殘留狀態**

`engine.ingest()` 內部持有 burn-rate 歷史。從「兩個都顯示」切到「僅 Claude」再切回來時，Codex 的歷史會有一段空窗。**驗證這不會產生錯誤的燃速或 runway**（例如把空窗當成「都沒用量」而算出假的低燃速）。若有問題，切換時清掉該 provider 的歷史比留著半截更誠實。

Run: `npm run tauri dev`，在設定裡來回切換，用 `TOKENBAR_DEBUG=1` 看 stderr 的 `[tb]` 數值是否合理。

- [ ] **Step 5: 跑測試 + Commit**

```bash
git -C C:\Coding\TokenBar\TokenBar-Src add src-tauri/src/lib.rs
git -C C:\Coding\TokenBar\TokenBar-Src commit -m "feat: filter providers globally in the scheduler"
```

### Task 3: 分析頁過濾

**Files:**
- Modify: `src-tauri/src/analytics.rs`

> 分析頁是**唯一**不吃 Snapshot 的消費者（`analytics.rs:145-146` 直接掃 `~/.codex/sessions/**/rollout-*.jsonl` 與 `~/.claude/projects/**/*.jsonl`），所以前面兩個 Task 的過濾對它完全無效。使用者說的「全部都只使用選擇的那一個」包含分析頁，這個 Task 不能省。

- [ ] **Step 1: 先寫失敗的測試**

```rust
#[test]
fn claude_filter_skips_codex_scan() {
    // compute_with(range, filter) 在 filter="claude" 時,結果不含任何 Codex 歸因
}
```

- [ ] **Step 2: 實作**

把 `compute(range)` 改為 `compute_with(range: &str, filter: &str)`，在 `analytics.rs:145-146` 依 filter 跳過 `scan_codex()` 或 `scan_claude()`。Tauri 指令端讀 `config::load().providers` 後傳入，保留 `compute(range)` 作為薄包裝以免破壞既有呼叫點（`lib.rs:260` 的 debug 輸出也會呼叫）。

**跳過掃描而非掃完再濾** —— `scan_*` 是掃整個目錄樹的檔案 I/O，選了「只用 Claude」卻還去掃 Codex 的 session 檔是白費工。

- [ ] **Step 3: 跑測試 + Commit**

```bash
git -C C:\Coding\TokenBar\TokenBar-Src add src-tauri/src/analytics.rs
git -C C:\Coding\TokenBar\TokenBar-Src commit -m "feat: honor the provider filter in analytics"
```

### Task 4: 文件與實機驗證

- [ ] **Step 1: 實機走一遍三種設定**

`npm run tauri dev`，逐一切換「兩個都顯示 / 只顯示 Claude / 只顯示 Codex」，每種都確認：島嶼（含寬度 340↔270 切換）、面板清單、系統匣 tooltip、通知、分析頁**五處全部一致**。這是本計畫唯一能證明「單一節點過濾真的涵蓋所有下游」的檢查。

- [ ] **Step 2: 驗證舊設定檔遷移**

手動把 `%APPDATA%\TokenBar\settings.json` 改成含 `"island_mode": "claude"` 且無 `providers`，重啟，確認自動變成「只顯示 Claude」而非退回預設。再測一次 `"island_mode": "worst"`，確認顯示全部而非空白。

- [ ] **Step 3: 更新文件**

`Ai_Assistant/CONFIG.md` 的設定總表：`island_mode` 標為 deprecated，新增 `providers` 並註明遷移行為與「未知值 = 顯示全部」。

`Ai_Assistant/TokenBar UX Spec v3.md`：`island_mode` 的描述改為全域「顯示平台」，並註明它同時影響分析頁。

`Ai_Assistant/HANDOFF.md`：新增本次變更，**特別記錄 `serde(default)` 不驗證值這個坑**（HANDOFF.md:28 現有的「舊值 worst fallback」說法是錯的 —— 那是前端 else 分支的副作用，不是 serde 的行為，要一併更正）。

- [ ] **Step 4: Commit**

```bash
git -C C:\Coding\TokenBar\TokenBar-Src add Ai_Assistant/
git -C C:\Coding\TokenBar\TokenBar-Src commit -m "docs: record the global provider filter and the serde(default) pitfall"
```
