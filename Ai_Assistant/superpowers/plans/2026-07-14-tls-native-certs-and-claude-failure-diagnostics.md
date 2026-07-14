# TLS 系統憑證 + Claude 失敗階段診斷 實作計畫

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 讓 TokenBar 的 HTTPS 連線在有企業代理／防毒 HTTPS 攔截／自簽根憑證的機器上也能成功；並讓 Claude 取數失敗時，**使用者當下就能用白話看懂發生什麼事、該做什麼**，開發者則能透過 `TOKENBAR_DEBUG` 看到精確的技術階段。

**背景:** 使用者在另一台機器上，Claude 一律顯示 `SourceFailed`，但同一份憑證用 PowerShell/curl 直接打 API 回 HTTP 200。根因不是 OAuth 失效，而是 `ureq` 預設走編譯期寫死的 `webpki-roots`，完全不讀 Windows 憑證存放區；PowerShell/curl 走 schannel 所以看得到那張被攔截注入的憑證。失敗階段停在 `usage_transport`。該機器的診斷結論正確，本計畫是把它移植回本 repo。

**Architecture:** Task 1 把根憑證來源從編譯期內建清單換成作業系統存放區（`ureq` 的 `native-certs` feature）。Task 2 在 `anthropic.rs` 內部把 `Option` 改成 `Result<_, FailureStage>`，並讓同一個 `FailureStage` 有**兩種輸出**：`label()` 給 `TOKENBAR_DEBUG` 的 stderr（精確技術階段，如 `usage_transport_proxy_unauthorized`），`user_hint()` 給 UI（白話，如「連不上 Claude，可能是防毒或公司網路擋住了」）。狀態機**完全不動**——仍是 §7 既有的 `SourceFailed` 單一狀態，只是把 `panel.ts` 寫死的那行字換成依原因而變。Task 3 升版打包發佈。

**Tech Stack:** Rust 2021, Tauri 2, `ureq` 2.12.1, `rustls` 0.23, `serde_json`。

## Global Constraints

- **機密鐵則（CLAUDE.md）**：`FailureStage` 只能攜帶錯誤種類與 HTTP 狀態碼。**不得**含 access token、refresh token、email、account id 或 response body，部分遮蔽也不行。
- **對外行為不變**：UI 仍只有 `SourceFailed` 單一降級狀態（UX Spec v3 §7 是唯一真相）。診斷只走 stderr，且僅在 `TOKENBAR_DEBUG=1` 時輸出，前綴沿用既有的 `[tb]`。
- **native-certs 是「取代」不是「疊加」**：見 Task 1 的驗證紀錄。啟用後內建 Mozilla 清單完全不使用。
- Claude refresh 維持 opt-in（settings `allow_token_refresh`，預設 false），本計畫不改動這個語意。
- 打包前先 `taskkill /IM tokenbar.exe /F` 與 `taskkill /IM TokenBar-portable.exe /F`（exe 檔案鎖）。
- 跑 cargo 前 PATH 要先 prepend `%USERPROFILE%\.cargo\bin`。

---

### Task 1: 根憑證改用 Windows 系統存放區 —— ✅ 已完成（2026-07-14）

**Files:**
- Modify: `src-tauri/Cargo.toml`（已改）
- Modify: `src-tauri/Cargo.lock`（連帶更新）

**Interfaces:**
- 不改任何函式簽章。純 build-time feature 切換，`get_usage` / `refresh_token` 的呼叫端無感。

- [x] **Step 1: 確認根因**

`src-tauri/Cargo.toml:25` 原為 `ureq = { version = "2", features = ["json", "tls"] }`；Cargo.lock 確認 ureq 2.12.1 且**無** `rustls-native-certs`，即走 `webpki-roots`。

- [x] **Step 2: 啟用 native-certs**

```toml
ureq = { version = "2", features = ["json", "tls", "native-certs"] }
```

- [x] **Step 3: 驗證依賴樹真的換掉了（而非只加了無作用的 flag）**

`cargo tree -p ureq` 應出現：

```text
├── rustls-native-certs v0.7.3
│   └── schannel v0.1.29
```

實測已出現，與另一台機器的報告一致。

- [x] **Step 4: 確認取代語意**

讀 `~/.cargo/registry/src/*/ureq-2.12.1/src/rtls.rs:62-86`：`#[cfg(feature = "native-certs")]` 的 `root_certs()` 從 `RootCertStore::empty()` 開始、**只**載入系統憑證；`#[cfg(not(...))]` 才用 `webpki_roots`。兩者互斥。ureq 在 `rtls.rs:75` 自帶警告：系統憑證一張都載不到時所有 HTTPS 都會失敗。Windows 根存放區必有內容，故此路徑安全。

- [x] **Step 5: 實測 TLS 握手**

以臨時 `examples/tls_check.rs`（不帶任何憑證）對 `https://api.anthropic.com/api/oauth/usage` 發一次請求，得 `TLS OK, HTTP 429` —— 傳輸層通過（429 是因為未帶 token，不影響握手結論）。驗證後臨時檔已刪除。

- [x] **Step 6: 回歸測試**

`cargo test --manifest-path src-tauri\Cargo.toml` → 33 passed, 0 failed。

- [ ] **Step 7: Commit 這個切片**

```bash
git -C C:\Coding\TokenBar\TokenBar-Src add src-tauri/Cargo.toml src-tauri/Cargo.lock
git -C C:\Coding\TokenBar\TokenBar-Src commit -m "fix: use OS root certificate store for HTTPS"
```

### Task 2: 白話失敗提示（UI）+ 精確階段（除錯用）

**Files:**
- Modify: `src-tauri/src/model.rs`（`Limit` 新增 `hint` 欄位）
- Modify: `src-tauri/src/providers/anthropic.rs`
- Modify: `src-tauri/src/providers/codex.rs`、`src-tauri/src/providers/codex_live.rs`（補 `hint: None`，機械性）
- Modify: `src/types.ts`、`src/panel.ts`、`src/mock.ts`
- Modify: `Ai_Assistant/CONFIG.md`、`Ai_Assistant/TokenBar UX Spec v3.md`

**Interfaces:**
- 新增 module-private `enum FailureStage`，含 `fn label(&self) -> String`（技術用）與 `fn user_hint(&self) -> &'static str`（白話用）。
- 新增純函式 `fn parse_creds(raw: &str) -> Result<Creds, FailureStage>`，供 `read_creds` 呼叫且可單元測試（現行 `read_creds` 直接讀 `home_dir()`，無法測）。
- `degraded_limits` 改為 `degraded_limits(stage: &FailureStage) -> Vec<Limit>`，把 `user_hint()` 寫進每條 limit 的 `hint`。
- `Limit` 新增 `pub hint: Option<String>`；除 `degraded_limits` 外所有建構點一律 `hint: None`。
- `poll` 的降級語意不變（仍回 `SourceFailed`）。

**設計理由:** 狀態機不動——UX Spec v3 §7 的「來源失效」仍是單一狀態，虛線邊的降級樣式也保持不變。改的只是那行**文案內容**：從一句寫死的字，變成依 `FailureStage` 而變的白話提示（徽章文字一併從「估算」改為「無法取得」，理由見下）。這是加欄位、不是加狀態，所以不牴觸 §7。

**必須順便修的謊言:** `panel.ts:82` 現在寫「來源失效，改用本機估算」，但 `degraded_limits` 其實只回 `util: 0.0` 的佔位值，**根本沒有做任何估算**。這句話會讓使用者以為看到的 0% 是估算數字，實際上是「沒資料」——比不顯示訊息更糟。本 Task 一併把它換成誠實的白話提示。

**已知的規格落差（本計畫不處理，記入 backlog）:** §7 第 169 行要求來源失效時應「退回本機 token 估算 + 標『估算』」，但實作從未做過本機估算。真正補上估算是獨立的功能工作；本 Task 只負責讓畫面上的字**不再說謊**。

- [ ] **Step 1: 先寫失敗的測試**

在 `anthropic.rs` 的 `mod tests` 加入。注意**白話提示也要測**——文案是這個 Task 的產品面交付物，不是註解：

```rust
#[test]
fn malformed_json_is_shape_failure() {
    assert_eq!(parse_creds("not json").unwrap_err(), FailureStage::CredentialsShape);
}

#[test]
fn missing_oauth_block_is_shape_failure() {
    assert_eq!(parse_creds(r#"{ "other": 1 }"#).unwrap_err(), FailureStage::CredentialsShape);
}

#[test]
fn empty_access_token_is_shape_failure() {
    let raw = r#"{ "claudeAiOauth": { "accessToken": "", "refreshToken": "r", "expiresAt": 1 } }"#;
    assert_eq!(parse_creds(raw).unwrap_err(), FailureStage::CredentialsShape);
}

#[test]
fn debug_labels_include_status_code() {
    assert_eq!(FailureStage::UsageHttp(403).label(), "usage_http_403");
    assert_eq!(FailureStage::RefreshHttp(401).label(), "refresh_http_401");
}

/// 白話提示不得洩漏術語,也不得空白 (§7:降級不得空白)。
#[test]
fn user_hints_are_plain_language_and_never_empty() {
    let stages = [
        FailureStage::CredentialsFile,
        FailureStage::CredentialsShape,
        FailureStage::RefreshDisabled,
        FailureStage::RefreshHttp(401),
        FailureStage::RefreshTransport("dns"),
        FailureStage::RefreshJson,
        FailureStage::UsageHttp(403),
        FailureStage::UsageHttp(429),
        FailureStage::UsageTransport("connection_failed"),
        FailureStage::UsageJson,
        FailureStage::UsageShape,
    ];
    for s in stages {
        let h = s.user_hint();
        assert!(!h.is_empty(), "{:?} 沒有提示文案", s);
        for jargon in ["TLS", "HTTP", "OAuth", "token", "transport", "JSON", "proxy"] {
            assert!(!h.contains(jargon), "{:?} 的提示含術語 {}", s, jargon);
        }
    }
}

/// 降級的 limit 必須帶著提示,否則 UI 沒東西可顯示。
#[test]
fn degraded_limits_carry_the_hint() {
    let ls = degraded_limits(&FailureStage::UsageTransport("connection_failed"));
    assert_eq!(ls.len(), 2);
    assert!(ls.iter().all(|l| l.hint.is_some()));
    assert!(ls.iter().all(|l| l.status == LimitStatus::SourceFailed));
}
```

- [ ] **Step 2: 跑測試確認失敗**

Run: `cargo test --manifest-path src-tauri\Cargo.toml anthropic::tests`

Expected: 編譯失敗,因為 `parse_creds` / `FailureStage` / `hint` 尚不存在。

- [ ] **Step 3: 實作 FailureStage 的兩種輸出**

```rust
#[derive(Debug, Clone, PartialEq)]
enum FailureStage {
    CredentialsFile,
    CredentialsShape,
    RefreshDisabled,
    RefreshHttp(u16),
    RefreshTransport(&'static str),
    RefreshJson,
    UsageHttp(u16),
    UsageTransport(&'static str),
    UsageJson,
    UsageShape,
}
```

`label()` 給開發者(精確、可含代碼),`user_hint()` 給使用者(白話、可行動)。同一個列舉兩種讀者:

| 階段 | `label()`(TOKENBAR_DEBUG) | `user_hint()`(UI 顯示給使用者) |
|---|---|---|
| `CredentialsFile` | `credentials_file` | 找不到 Claude 的登入資料,請先登入 Claude Code |
| `CredentialsShape` | `credentials_shape` | Claude 的登入資料讀不出來,請重新登入 Claude Code |
| `RefreshDisabled` | `refresh_disabled` | Claude 登入已過期,請重新登入(或在設定開啟自動更新) |
| `RefreshHttp(401\|403)` | `refresh_http_401` | Claude 登入已失效,請重新登入 Claude Code |
| `RefreshHttp(其他)` | `refresh_http_500` | Claude 登入更新失敗,稍後會自動再試 |
| `RefreshTransport(_)` | `refresh_transport_dns` | 連不上 Claude,請檢查網路連線 |
| `RefreshJson` | `refresh_json` | Claude 回應的格式不認得,可能需要更新 TokenBar |
| `UsageHttp(401\|403)` | `usage_http_403` | Claude 不接受這個帳號的查詢,請重新登入 Claude Code |
| `UsageHttp(429)` | `usage_http_429` | 查詢太頻繁,稍後會自動再試 |
| `UsageHttp(5xx)` | `usage_http_503` | Claude 伺服器暫時有狀況,稍後會自動再試 |
| `UsageTransport(_)` | `usage_transport_connection_failed` | **連不上 Claude。請檢查網路;若有公司網路或防毒軟體,可能擋住了連線** |
| `UsageJson` / `UsageShape` | `usage_json` / `usage_shape` | Claude 回應的格式不認得,可能需要更新 TokenBar |

文案原則(照使用者要求):不出現 TLS / HTTP / OAuth / token / 憑證 / 傳輸層 / proxy 等字眼;每句都要能讓使用者**知道下一步做什麼**,或明確告知「會自動再試、不用動作」。粗體那條就是另一台機器實際遇到的情況——使用者看到它就會直接去查防毒/公司網路,而不是白費力氣重跑 `/login`。

`parse_creds` 明確檢查:JSON 可解析 → 有 `claudeAiOauth` → `accessToken` 存在且**非空** → `refreshToken` 存在且非空 → 讀 `expiresAt`。任一項不符回 `CredentialsShape`;`read_creds` 只負責讀檔(讀不到 → `CredentialsFile`)後轉呼叫 `parse_creds`。

`UsageShape` 用於「連線成功、JSON 正常,但解析結果是空的」——即 `parse_usage` 回空 `Vec`。這代表官方改了 schema。**這條不能省**:Codex 的 schema 今年已變過兩次(7/13 那次讓 5h 視窗整個消失),Claude 的 `limits` 陣列同樣會改;沒有這條的話畫面會顯示「沒有額度」而不是「來源壞了」,那比 `SourceFailed` 更難察覺,因為它看起來像正常狀態。

`transport_kind_label(kind: ureq::ErrorKind) -> &'static str` 把 kind 映射成 `dns` / `connection_failed` / `proxy_connect` / `proxy_unauthorized` / `bad_header` / `io` 等短字串,**僅供 `label()` 使用**(使用者提示不分這麼細)。**注意:實作前先確認 ureq 2.12.1 的 `ErrorKind` 實際變體名稱**(本計畫未逐一驗證),並保留 `_ => "other"` catch-all 以免升版編譯失敗。

HTTP 錯誤要靠 `ureq::Error::Status(code, _)` 取得狀態碼——現行 `.call().ok()?` 會把 429/403 一律吞成 `None`,這正是原本無法分辨的原因。**只取 `code`,絕不碰 response body。**

- [ ] **Step 4: `Limit` 新增 hint 欄位**

`src-tauri/src/model.rs`:

```rust
/// 來源失效時給使用者看的白話提示 (§7);正常狀態為 None。
#[serde(skip_serializing_if = "Option::is_none")]
pub hint: Option<String>,
```

`src/types.ts` 的 `Limit` 同步加 `hint?: string`。其餘所有 `Limit` 建構點(`anthropic.rs` 的 `window()`、`codex.rs`、`codex_live.rs`、各測試 helper)一律補 `hint: None` —— 純機械性修改,適合交給 mech-executor。

- [ ] **Step 5: 內部改用 Result,兩種輸出各就各位**

把 `fetch` 拆成 `fetch_inner(&self, allow_refresh: bool) -> Result<Vec<Limit>, FailureStage>`:

```rust
fn fetch(&self, allow_refresh: bool) -> Option<Vec<Limit>> {
    match self.fetch_inner(allow_refresh) {
        Ok(limits) => Some(limits),
        Err(stage) => {
            if std::env::var("TOKENBAR_DEBUG").is_ok() {
                eprintln!("[tb] anthropic fetch failed: {}", stage.label());
            }
            Some(degraded_limits(&stage))   // 白話提示隨降級資料一起帶給 UI
        }
    }
}
```

注意 `fetch` 現在回 `Some(degraded_limits(..))` 而非 `None`,所以 `poll` 的 `unwrap_or_else(degraded_limits)` 要改為 `unwrap_or_else(|| degraded_limits(&FailureStage::CredentialsFile))` 或直接讓 `fetch` 永不回 `None`。**擇一即可,但別讓兩條路徑都產生降級資料**,否則會出現沒有 hint 的降級 limit(Step 1 的 `degraded_limits_carry_the_hint` 測試擋不到從 `poll` 那條路進來的)。

- [ ] **Step 6: 前端顯示白話提示**

`src/panel.ts:81-82` 現在是:

```ts
} else if (l.status === "source_failed") {
  sub = `<span class="badge">估算</span> 來源失效，改用本機估算`;
```

改為顯示真實原因。**「估算」徽章要拿掉**——沒有估算這回事,留著就是繼續說謊:

```ts
} else if (l.status === "source_failed") {
  sub = `<span class="badge">無法取得</span> ${escapeHtml(l.hint ?? "暫時取不到 Claude 用量")}`;
```

`hint` 來自後端字串,插進 innerHTML 前**必須跳脫**(現有文案是寫死的常數所以沒這問題)。確認 `panel.ts` 既有的跳脫工具;若無則加一個。虛線邊樣式(`styles.css` 的 `.lrow.status-source_failed`)保持不變。

`src/mock.ts:55-56` 的 `source_failed` 情境補上 `hint`,讓瀏覽器 preview 的 devbar 能實際看到文案排版(長句在 340px 島嶼寬度下會不會爆版,只有這裡看得出來)。

- [ ] **Step 7: 跑測試 + 實際看過文案**

Run: `cargo test --manifest-path src-tauri\Cargo.toml; npm run build`

Expected: 既有 33 個 + 新增 6 個全過,Vite exit 0。

接著 `npm run dev` 開瀏覽器 preview,devbar 切到 `degraded` 情境,**用眼睛確認**最長的那句(`UsageTransport` 那條)不爆版、不截斷。這步不能只靠測試——文案長度是視覺問題。

- [ ] **Step 8: 文件化**

`Ai_Assistant/CONFIG.md:104` 的 `TOKENBAR_DEBUG` 說明後補一行:啟用時 Claude 取數失敗會多印 `[tb] anthropic fetch failed: <stage>`,並列出所有 stage 字串與意義。

`Ai_Assistant/TokenBar UX Spec v3.md` §7 第 169 行的「來源失效」列,把卡片欄更新為「虛線邊 + 白話原因提示」,並記下**實作尚未做本機估算**這個既存落差(見 Task 2 開頭的說明)。規格是唯一真相,不能讓它繼續描述一個不存在的行為。

- [ ] **Step 9: Commit 這個切片**

```bash
git -C C:\Coding\TokenBar\TokenBar-Src add src-tauri/src/model.rs src-tauri/src/providers src/types.ts src/panel.ts src/mock.ts Ai_Assistant/CONFIG.md "Ai_Assistant/TokenBar UX Spec v3.md"
git -C C:\Coding\TokenBar\TokenBar-Src commit -m "feat: show plain-language reason when Claude usage is unavailable"
```

### Task 3: 主動通知 + 一鍵重新登入

> **前置實測(2026-07-14,直接跑 `claude` CLI 的 `--help` 確認,非推測):**
>
> - `claude auth login [--claudeai|--console|--sso] [--email <email>]` — 官方子指令,「Sign in to your Anthropic account」,外部程式可直接啟動。
> - `claude auth status --json` — 唯讀,回 `{loggedIn, authMethod, apiProvider, email, orgId, orgName, subscriptionType}`,**不含任何 token**。
> - `claude setup-token` — 長效 token(需訂閱)。本計畫不使用。
>
> **這徹底排除了「TokenBar 自己實作 OAuth 流程」的方案。** 官方既然提供了 `claude auth login`,就不該去碰 `CLIENT_ID`、自行改寫 `.credentials.json`、承擔把使用者的 Claude Code 登出的風險(該風險已明載於 `anthropic.rs:7-10`)。

**Files:**
- Modify: `src-tauri/src/lib.rs`(通知分支 + 新 Tauri 指令)
- Modify: `src/panel.ts`、`src/mock.ts`
- Modify: `Ai_Assistant/CONFIG.md`

**Interfaces:**
- 新增 Tauri 指令 `relogin() -> Result<(), String>`,啟動 `claude auth login --claudeai`。
- `fire_notifications` 新增來源失效分支。

**⚠️ 委派給 security-executor:** 本 Task 同時涉及**啟動外部行程**與**憑證流程**,依全域 CLAUDE.md 規定不得在主 session 直接實作。Task 1/2 可由 executor 或 mech-executor 處理,本 Task 必須交給 security-executor。

- [ ] **Step 1: 先寫失敗的測試(通知去重)**

現況的坑:`fire_notifications` 只在 `util >= warn`(75)、`util >= crit`(90) 或 `Locked` 時發通知,而 `SourceFailed` 的 `util` 恆為 `0.0` —— **永遠不會觸發任何通知**。這正是使用者「不打開面板就永遠不知道壞了」的原因。

同時 `cc.5h` 與 `cc.week` 會**同時**失效,現行迴圈逐 limit 發通知會一次跳兩則一模一樣的訊息。把判斷抽成純函式來測:

```rust
/// 來源失效時該發哪一則通知(每個 provider 最多一則)。
/// 回傳 (去重key, 通知內文);沒有失效的來源時回空。
fn source_failed_notices(snap: &Snapshot) -> Vec<(String, String)>

#[test]
fn two_failed_limits_of_one_provider_produce_one_notice() {
    let snap = snapshot_with(vec![
        failed("cc.5h", "Claude 登入已失效，請重新登入 Claude Code"),
        failed("cc.week", "Claude 登入已失效，請重新登入 Claude Code"),
    ]);
    assert_eq!(source_failed_notices(&snap).len(), 1);
}

#[test]
fn notice_body_is_the_user_hint() {
    let snap = snapshot_with(vec![failed("cc.5h", "連不上 Claude。請檢查網路")]);
    assert_eq!(source_failed_notices(&snap)[0].1, "連不上 Claude。請檢查網路");
}

#[test]
fn healthy_snapshot_produces_no_notice() {
    assert!(source_failed_notices(&snapshot_with(vec![normal("cc.5h", 20.0)])).is_empty());
}
```

- [ ] **Step 2: 跑測試確認失敗**

Run: `cargo test --manifest-path src-tauri\Cargo.toml source_failed_notices`

Expected: 編譯失敗,函式不存在。

- [ ] **Step 3: 實作通知分支**

去重 key 用 `format!("{:?}.source_failed", provider)`(每個 provider 一則),內文直接用 Task 2 的 `hint` —— 白話文案只寫一次,面板和通知共用,不會走針。

抑制策略**不可沿用既有的 30 分鐘**:`NOTIFY_SUPPRESS_SECS` 是為「額度快用完」設計的,那種提醒重複有意義;但「請重新登入」是要使用者動手的事,每半小時彈一次是騷擾。改為:

```rust
/// 來源失效通知:恢復前只提醒一次(不像額度警告那樣重複提醒)。
const SOURCE_FAIL_SUPPRESS_SECS: i64 = 6 * 3600;
```

並在該 provider 的 limits **不再是 `SourceFailed` 時,把去重 key 從 `notified` map 移除** —— 這樣「壞掉→修好→又壞掉」會正確地再通知一次,而不是被 6 小時的抑制吃掉。

- [ ] **Step 4: 實作重新登入指令**

```rust
/// 啟動官方登入流程。不碰憑證檔、不自行實作 OAuth。
#[tauri::command]
fn relogin() -> Result<(), String> {
    std::process::Command::new("cmd")
        .args(["/C", "start", "", "claude", "auth", "login", "--claudeai"])
        .spawn()
        .map(|_| ())
        .map_err(|_| "找不到 claude 指令".to_string())
}
```

安全要點:
- 參數全為**編譯期常數**,無任何使用者輸入拼接 → 無注入面。維持這個性質,不要為了「順便帶 email」而把外部字串接進 args(`--email` 看似方便,但那是個人資料且會讓參數變成動態的)。
- **不得捕捉或記錄子行程的 stdout/stderr** —— 登入流程的輸出可能含機密。
- 錯誤訊息只回固定字串,不得把 `io::Error` 的內容往外送。

**已知限制(必須誠實處理,不可假裝不存在):**
- **`claude` 不一定在 TokenBar 的 PATH 上。** TokenBar 是從檔案總管/開機自動啟動的 GUI 程式,繼承的環境和使用者的終端機不同。`spawn` 失敗時 UI 必須降級成「請手動執行 `claude auth login`」並把指令**顯示出來讓使用者能複製**,不能只跳一個沒有出路的錯誤。
- 若使用者的 Claude Code 跑在 WSL 裡,Windows 端可能根本沒有 `claude`。此時上述降級路徑就是唯一正解。
- 這顆按鈕啟動的是**新的登入流程**,不是把 `/login` 送進使用者正在跑的那個 Claude Code 會話(技術上不可能)。登入完成後 TokenBar 會在下一輪輪詢(≤180s)自動恢復,或使用者按面板的 ⟳ 立即重整。

- [ ] **Step 5: 面板按鈕**

`panel.ts` 的 `source_failed` 分支,在 Task 2 的白話提示後面加一顆按鈕,**只在提示屬於「登入類」時顯示** —— 連不上 Claude(網路/防毒問題)時給「重新登入」按鈕是誤導,使用者按了也沒用,反而會以為是自己帳號有問題。

判斷方式:後端在 `Limit` 再加一個 `action: Option<String>` 欄位(值為 `"relogin"`),由 `FailureStage` 決定 —— 只有 `CredentialsFile` / `CredentialsShape` / `RefreshDisabled` / `RefreshHttp(401|403)` / `UsageHttp(401|403)` 這幾個階段帶 `relogin`。**別在前端用字串比對 hint 內容來猜** —— 文案一改就壞,而且那是把顯示層的字當成邏輯用。

- [ ] **Step 6: 測試 + 實機驗證**

Run: `cargo test --manifest-path src-tauri\Cargo.toml; npm run build`

實機:`npm run tauri dev`,devbar 切 `degraded` 情境確認按鈕只在登入類提示出現。**實際按一次按鈕**,確認真的叫得起 `claude auth login`。

**注意:實測時不要真的跑完登入流程**(會輪替 refresh token,可能影響使用者當下的 Claude Code 會話 —— 見 `anthropic.rs:7-10`);確認瀏覽器/終端機有跳出來即可,然後取消。

- [ ] **Step 7: 文件化 + Commit**

`Ai_Assistant/CONFIG.md` 記錄新指令與 `SOURCE_FAIL_SUPPRESS_SECS`。

```bash
git -C C:\Coding\TokenBar\TokenBar-Src add src-tauri/src/lib.rs src/panel.ts src/mock.ts Ai_Assistant/CONFIG.md
git -C C:\Coding\TokenBar\TokenBar-Src commit -m "feat: notify on source failure and offer one-click re-login"
```

## 順帶解掉的 backlog(記錄,本計畫不做)

`claude auth status --json` 回傳的 `email` / `subscriptionType` / `orgName` 正好能填掉 backlog 第 2 項(Stats 頁帳號 email/方案目前是佔位 `—`),且**不含 token**,比原本設想的「讀 `.claude.json`」乾淨得多。`loggedIn` 也可以當成比解析憑證檔更可靠的登入狀態來源。這是獨立的功能工作,不塞進本計畫。

### Task 4: 升版、打包、發佈

> Task 1 的修正**要重編才會生效**。目前常駐的是已安裝版 `%LOCALAPPDATA%\TokenBar\tokenbar.exe`（v0.1.3，行程名 `tokenbar`）。

**Files:**
- Modify: `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`（版本號三處，目前一致為 `0.1.3`）
- Modify: `Ai_Assistant/HANDOFF.md`

- [ ] **Step 1: 三處版本號同步升到 0.1.4**

`0.1.3` 已發佈為 Release，故新產物必須升版。三處務必一致。

- [ ] **Step 2: 前端建置**

Run: `npm run build`

Expected: Vite exit 0。

- [ ] **Step 3: 關掉常駐行程再打包**

```powershell
taskkill /IM tokenbar.exe /F
taskkill /IM TokenBar-portable.exe /F
npm run build:release
```

Expected: NSIS setup + MSI + portable 產出並由 collect-installers.mjs 集中到 `..\TokenBar-release\`，0.1.3 的舊安裝檔自動移入 `archive/`。

**踩雷提醒**：若出現 build 失敗（os error 3 / os error 1224），先 `cargo clean` 再全量重編；1224 是 Windows 檔案映射鎖，代表還有行程沒關乾淨。

- [ ] **Step 4: 實機驗證**

啟動新產出的 portable，確認 Claude 四條與 Codex 週視窗都有值（非 `SourceFailed`）。這是唯一能證明 native-certs 沒有在真實環境搞砸的檢查——單元測試涵蓋不到根憑證載入。

- [ ] **Step 5: 更新 HANDOFF.md**

在頂端新增 `## 2026-07-14：HTTPS 根憑證改用系統存放區（v0.1.4）` 區塊，記錄根因（webpki-roots 不讀 Windows 憑證存放區 → 有 HTTPS 攔截的機器 `usage_transport` 失敗）、修正、取代語意的坑，以及**「不要把另一台 `Downloads/Chi_Tokenbar-main/` 的檔案整包蓋回來」**這個警告（該基底較舊，會吃掉 v0.1.2 的 Codex 視窗長度分類修正與 v0.1.3 的單一實例鎖）。

- [ ] **Step 6: Commit + push + 發 Release**

```bash
git -C C:\Coding\TokenBar\TokenBar-Src add -A
git -C C:\Coding\TokenBar\TokenBar-Src commit -m "release: v0.1.4"
git -C C:\Coding\TokenBar\TokenBar-Src push origin main
```

GitHub Release v0.1.4（Latest），附 portable + setup + MSI。

## 不採用的項目（來自另一台機器的報告）

- **第 6 項「保留 OAuth 自動更新」**：本 repo `anthropic.rs:93-136` 已有完整的 refresh + 原子寫回，無需改動。
- **第 8 項「現代 limits 陣列解析」**：本 repo `parse_limits_array` 已支援 `cc.5h` / `cc.week` / `cc.w.fable` / `cc.extra`，無需改動。
- **第 7 項「Header 強化」**：`get_usage` 已帶 Authorization / anthropic-beta / User-Agent，只缺 `Accept` 與 `Accept-Encoding: identity`。本專案未啟用 ureq 的 gzip feature，本來就不送壓縮請求，故 `identity` 幾乎無實質作用。判定為低價值，不做。
