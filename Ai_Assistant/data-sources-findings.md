# 資料來源實測結果(2026-07-09,於本機 Windows 驗證)

開工前驗證的兩個資料事實,已用真實檔案確認。實作 parser 依此。

---

## 1. Codex — `rate_limits`(✅ 確認,最穩)

來源檔:`%USERPROFILE%\.codex\sessions\YYYY\MM\DD\rollout-*.jsonl`(每 session 一檔,可達數十 MB)。
`rate_limits` 物件內嵌於 session 記錄中,**每次 API 回應重複出現**,取**最後一筆**即為最新狀態:

```json
"rate_limits": {
  "limit_id": "codex",
  "limit_name": null,
  "primary":   { "used_percent": 4.0, "window_minutes": 300,   "resets_at": 1782590353 },
  "secondary": { "used_percent": 7.0, "window_minutes": 10080, "resets_at": 1782976756 },
  "credits": null,
  "individual_limit": null,
  "plan_type": "plus",
  "rate_limit_reached_type": null
}
```

**對應關係**:
- `primary`  = **5h 視窗**(`window_minutes: 300` = 5h)。
- `secondary`= **週視窗**(`window_minutes: 10080` = 7 天)。
- `used_percent` = 直接就是 util%(0–100)。**不需自己算**。
- `resets_at` = **Unix epoch 秒**。
- `window_minutes` → 視窗長度,故 `window_start = resets_at − window_minutes×60`,配速 `f` 可直接算(§4)。
- `credits` = 額度餘額(此 plus 帳號為 `null`;有 credit 時應會有值 → §9 的「credit 餘額」原標「本機檔通常沒有」可**修正為:欄位存在,值視方案而定**)。
- `plan_type` = `"plus"`(可直接顯示方案別)。

> 影響:Codex 的 5h/週 限制**完全可從本機檔算出,零未公開 API**。pace/runway 皆可直接推導。

### token_count(供第三層分析,✅ 確認)

同檔內 `type:"token_count"` 事件帶累計用量:

```json
"total_token_usage": {
  "input_tokens": 40003,
  "cached_input_tokens": 29440,
  "output_tokens": 1244,
  "reasoning_output_tokens": 516,
  "total_tokens": 41247
}
```
累計值,取差分得每回合用量。含 `reasoning_output_tokens`(Codex 特有)。

### 其他可用檔
- `%USERPROFILE%\.codex\auth.json`(Codex OAuth,若日後需呼叫 OpenAI API)。
- `%USERPROFILE%\.codex\session_index.jsonl`(session 索引,可加速找最新 session)。

---

## 2. Anthropic — OAuth token(✅ 定位,但需 refresh)

Token 檔:`%USERPROFILE%\.claude\.credentials.json`

結構(值已遮蔽):
```
claudeAiOauth:
  accessToken:  <108 字元, sk-ant-…>
  refreshToken: <108 字元>
  expiresAt:    1779139493792   (Unix 毫秒)
  scopes:       [user:file_upload, user:inference, user:mcp_servers, user:profile, user:sessions:claude_code]
```

**關鍵發現**:
- scopes **含 `user:profile`** → 滿足 `GET /api/oauth/usage` 需求。
- **實測當下 `accessToken` 已過期**(expiresAt = 2026-05-19,驗證時為 2026-07-09)。
  → **anthropic provider 必須先用 `refreshToken` 換新 accessToken 才能呼叫 endpoint**,不能直接用檔裡的 accessToken。
  → 這增加 OAuth refresh 流程與失敗面 → 再次印證:Anthropic 路徑最脆弱,MVP 鏈路先走 Codex 是對的。

**呼叫**(取得有效 token 後):
```
GET https://api.anthropic.com/api/oauth/usage
Authorization: Bearer <fresh_access_token>
anthropic-beta: oauth-2025-04-20
```
回應含 `five_hour` / `seven_day` / `seven_day_opus`(可能 null) / `extra_usage`。

**2026-07-10 更新:API 已改版,新增結構化 `limits` 陣列(TokenBar 現在優先讀這個,舊欄位當 fallback):**
```json
"limits": [
  { "kind": "session",       "group": "session", "percent": 25, "severity": "normal",
    "resets_at": "2026-07-10T06:19:59+00:00", "scope": null, "is_active": true },
  { "kind": "weekly_all",    "group": "weekly",  "percent": 3,  "severity": "normal",
    "resets_at": "2026-07-14T02:59:59+00:00", "scope": null, "is_active": false },
  { "kind": "weekly_scoped", "group": "weekly",  "percent": 5,  "severity": "normal",
    "resets_at": "2026-07-14T02:59:59+00:00",
    "scope": { "model": { "id": null, "display_name": "Fable" }, "surface": null }, "is_active": false }
]
```
- `weekly_scoped` = 模型專屬週限制(目前為 **Fable**;舊的 `seven_day_opus`/`seven_day_sonnet` 頂層欄位在此帳號為 null)。
- 頂層另有 `spend`(extra usage 的新表示法)與一堆 null 的實驗欄位(`seven_day_cowork`、`tangelo` 等 codename),忽略即可。
- 對應解析:`providers/anthropic.rs` `parse_limits_array`(session→`cc.5h`、weekly_all→`cc.week`、weekly_scoped→Opus 沿用 `cc.opus`、其他模型→`cc.w.<slug>`)。

**安全**:accessToken/refreshToken 為機密,**任何情況不得寫入 log、不得回顯**。僅在記憶體使用。

本機 fallback:`%USERPROFILE%\.claude\projects\**\*.jsonl`(token 拆解,供 SourceFailed 降級與第三層分析)。

---

## 3. 工具鏈現況
- Node `v24.11.1`、npm `11.6.4` ✅
- **Rust/cargo 未安裝** ❌ → 需安裝 rustup(+ Windows MSVC build tools + WebView2)。

---

## 4. 多工具:OpenCode / Gemini CLI(階段 E,2026-07-17 本機勘察)

> 只探查 session/message/log 的**目錄結構與欄位名**,不讀 auth/api key 檔、不記任何 token 值或對話內容。兩者本機用量若無穩定、預設開啟的官方檔案,則**僅做 Usage、不做 Limits**(計畫預設值)。本機探查結果:兩者皆**無**可用的本機用量資料,scanner 仍依「文件化格式」實作(假資料測試),執行期目錄不存在即回空。

### 4.1 OpenCode(client: OpenCode)— 本機未安裝
- **本機狀態**:`~/.local/share/opencode/`、`~/.opencode/`、`~/.config/opencode/`、`%LOCALAPPDATA%\opencode\`、`%APPDATA%\opencode\` 全部**不存在**——本機根本沒裝。此為結論。
- **文件化格式(scanner 依此寫)**:OpenCode 以「一訊息一 JSON 檔」存本機儲存,基底目錄為 XDG data(`~/.local/share/opencode/storage/`,另備援 `~/.opencode/`)。
  - `storage/message/<sessionID>/<messageID>.json`:assistant 訊息帶 `role`、`modelID`、`providerID`、`cost`、`time.created`(epoch **毫秒**)、以及 **`tokens` 物件**:`tokens.input` / `tokens.output` / `tokens.reasoning` / `tokens.cache.read` / `tokens.cache.write`。
  - token 欄位:**有**(見上)。cwd/專案歸屬:message 檔本身不帶,session 檔另存,本階段不做專案歸屬(project="")。
- **Limits 可靠性判準**:OpenCode 本機**無官方 limit 檔案**——它只記自己的用量,額度歸屬到後端 provider(Codex/Copilot,見 §8 多對多)。→ **僅 Usage,不做 Limits**。

### 4.2 Gemini CLI(client: Gemini CLI)— 本機無 CLI 用量檔
- **本機狀態**:`~/.gemini/` **存在**,但內容是 Google **Antigravity IDE**(`antigravity/conversations/*.pb` 為 protobuf 二進位、`annotations/*.pbtxt`、`user_settings.pb`)加一個空的 `GEMINI.md`;**沒有** Gemini CLI 的 `tmp/<hash>/`、`sessions`、`logs.json`、telemetry log。即本機沒有 Gemini CLI 的本機用量記錄(Antigravity 的 `.pb` 為未公開 proto schema、且不帶可解析的 token 欄位,不採用)。
- **文件化格式(scanner 依此寫,標記為「文件化但本機未驗證」)**:Gemini CLI 預設開啟的本機檔多為設定/憑證(`settings.json`、`oauth_creds.json`〔**憑證,不讀**〕)或不帶 token 的 chat log。**唯一**帶 per-turn token 的是 **opt-in OpenTelemetry**(`gen_ai.usage.input_tokens` / `output_tokens` 等屬性),預設關閉、無穩定 on-by-default 的 JSON 落檔。scanner 依一個文件化的 JSONL 用量記錄形狀掃 `~/.gemini/**/*.jsonl`:每行 `{ timestamp(epoch 毫秒或 RFC3339), model, tokens:{ input, output, cached, thoughts } }`;掃不到(常態)即回空。
- **Limits 可靠性判準**:Gemini CLI 本機**無官方 limit 檔案**(免費層的每日 request 限制不落地成可讀 limit 欄位)。→ **僅 Usage,不做 Limits**。

### 4.3 落地決策
- 兩家各一個 agent key:`OpenCode`、`Gemini CLI`(沿用「Claude Code / Codex CLI」的顯示名慣例,圖例直接顯示此字串;colors.ts 的 `keyColor` 以子字串 `opencode`/`gemini` 給固定色)。
- 設定 `tool_opencode` / `tool_gemini`(bool,預設 true = 偵測到就顯示);關掉即完全不掃、不進 byAgent/圖例/帳號。
- **本機無資料不出 0 假卡片**:沿用既有「tokens>0 才入桶、掃不到即不出現」機制(analytics `Acc::add` 的 `total==0` 早退、byAgent 只含實際有量的 key)。
