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
