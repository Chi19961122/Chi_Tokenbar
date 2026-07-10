# Codex Live Usage and Island Skin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an opt-in/selectable live Codex usage source and standardize the island visual skin to the supplied portable-build appearance.

**Architecture:** A `CodexLiveProvider` mirrors the existing cached Anthropic provider but obtains a read-only access token and account ID from `~/.codex/auth.json`. The scheduler selects live, local, or auto source from persisted settings; the frontend renders the source selector and shares one CSS token palette in all packages.

**Tech Stack:** Rust 2021, Tauri 2, `ureq`, `serde_json`, TypeScript, Vite, CSS.

## Global Constraints

- Never print, log, or persist values from `~/.codex/auth.json`.
- Live quota reads are opt-in by selection and must not generate model completions or rotate credentials.
- Use canonical `util` internally and `% left` in the island UI.
- Preserve local-rollout behavior when `codex_usage_source` is `local`.
- Package the same `dist` assets for NSIS, MSI, and the portable executable.

---

### Task 1: Add the selectable Codex usage setting

**Files:**
- Modify: `src-tauri/src/config.rs`
- Modify: `src/types.ts`
- Modify: `src/datasource.ts`
- Modify: `src/main.ts`
- Modify: `docs/CONFIG.md`

**Interfaces:**
- Produces `Settings.codex_usage_source: String` in Rust and `"live" | "auto" | "local"` in TypeScript.
- The scheduler in Task 3 consumes `Settings.codex_usage_source`.

- [ ] **Step 1: Write the failing settings tests**

Add this test module to `src-tauri/src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_local_codex_usage() {
        assert_eq!(Settings::default().codex_usage_source, "local");
    }

    #[test]
    fn missing_source_deserializes_to_local() {
        let s: Settings = serde_json::from_str(r#"{ "autostart": true }"#).unwrap();
        assert_eq!(s.codex_usage_source, "local");
    }
}
```

- [ ] **Step 2: Run the targeted test and verify it fails**

Run: `cargo test --manifest-path src-tauri\\Cargo.toml config::tests::defaults_to_live_codex_usage`

Expected: compilation failure because `codex_usage_source` does not exist.

- [ ] **Step 3: Add the persisted field and UI selector**

Add this field and default to `Settings` in `src-tauri/src/config.rs`:

```rust
/// Codex quota source: "live" (account API), "auto" (live then local), or "local".
pub codex_usage_source: String,
// Default initializer:
codex_usage_source: "local".into(),
```

Extend the TypeScript `Settings` interface and `DEFAULT_SETTINGS`:

```ts
export type CodexUsageSource = "live" | "auto" | "local";
// Settings:
codex_usage_source: CodexUsageSource;
// DEFAULT_SETTINGS:
codex_usage_source: "local",
```

In `renderSettings`, add a select after the island mode selector:

```html
<div class="srow">Codex 用量來源 <select id="s-codex-source">
  <option value="live">即時帳號用量</option>
  <option value="auto">自動（即時優先）</option>
  <option value="local">本機 session 快照</option>
</select></div>
```

Select the stored value with `selected`, and add this to `readSettingsForm`:

```ts
codex_usage_source: (($("s-codex-source") as HTMLSelectElement).value || "live") as Settings["codex_usage_source"],
```

Document the three values and their fallback behavior in the settings table in `docs/CONFIG.md`.

- [ ] **Step 4: Run the targeted test and verify it passes**

Run: `cargo test --manifest-path src-tauri\\Cargo.toml config::tests`

Expected: both configuration tests pass.

- [ ] **Step 5: Commit the settings slice**

```bash
git add src-tauri/src/config.rs src/types.ts src/datasource.ts src/main.ts docs/CONFIG.md
git commit -m "feat: add selectable Codex usage source"
```

### Task 2: Implement and test the live Codex provider

**Files:**
- Create: `src-tauri/src/providers/codex_live.rs`
- Modify: `src-tauri/src/providers/mod.rs`

**Interfaces:**
- Produces `CodexLiveProvider::new()`, `poll(now: i64, force: bool) -> Option<Vec<Limit>>`, and `parse_usage(&Value) -> Option<Vec<Limit>>`.
- Task 3 uses `poll` to obtain `codex.5h` and `codex.week` limits.

- [ ] **Step 1: Write the failing parser tests**

Create `src-tauri/src/providers/codex_live.rs` with this test fixture and tests before implementation:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_live_usage_windows() {
        let usage = json!({
            "rate_limit": {
                "primary_window": { "used_percent": 15, "limit_window_seconds": 18000, "reset_at": 1783697640i64 },
                "secondary_window": { "used_percent": 3, "limit_window_seconds": 604800, "reset_at": 1784252456i64 }
            }
        });
        let limits = parse_usage(&usage).expect("valid response");
        assert_eq!(limits[0].id, "codex.5h");
        assert_eq!(limits[0].util, 15.0);
        assert_eq!(limits[0].window_secs, 18_000);
        assert_eq!(limits[1].id, "codex.week");
        assert_eq!(limits[1].util, 3.0);
    }

    #[test]
    fn rejects_missing_primary_window() {
        assert!(parse_usage(&json!({ "rate_limit": {} })).is_none());
    }
}
```

- [ ] **Step 2: Run the targeted test and verify it fails**

Run: `cargo test --manifest-path src-tauri\\Cargo.toml codex_live::tests::parses_live_usage_windows`

Expected: failure because the provider module and `parse_usage` do not exist.

- [ ] **Step 3: Implement the read-only, cached provider**

Implement these exact pieces in `codex_live.rs`:

```rust
const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const REFRESH_SECS: i64 = 180;
const FORCE_MIN_SECS: i64 = 5;

pub struct CodexLiveProvider {
    last_fetch: i64,
    cached: Option<Vec<Limit>>,
}
```

Read only `tokens.access_token` and `tokens.account_id` from `~/.codex/auth.json`, then call `USAGE_URL` with `Authorization: Bearer <access_token>`, `ChatGPT-Account-Id: <account_id>`, and `User-Agent: tokenbar`. Return `None` for all filesystem, parsing, HTTP, and JSON errors. Do not add token refresh or any logging.

Implement `parse_usage` by reading `/rate_limit/primary_window` and `/rate_limit/secondary_window`; build `Limit`s with `Provider::Codex`, IDs/labels `codex.5h`/`Codex·5h` and `codex.week`/`Codex·週`, `LimitStatus::Normal`, no absolute values, no pace, and no runway. Require `used_percent`, `limit_window_seconds`, and `reset_at` for both windows.

Export the module from `src-tauri/src/providers/mod.rs`:

```rust
pub mod codex_live;
```

- [ ] **Step 4: Run parser tests and verify they pass**

Run: `cargo test --manifest-path src-tauri\\Cargo.toml codex_live::tests`

Expected: 2 passed, 0 failed.

- [ ] **Step 5: Commit the provider slice**

```bash
git add src-tauri/src/providers/codex_live.rs src-tauri/src/providers/mod.rs
git commit -m "feat: add live Codex usage provider"
```

### Task 3: Select the configured source in the scheduler

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/providers/codex_live.rs`

**Interfaces:**
- Consumes `Settings.codex_usage_source` and `CodexLiveProvider::poll`.
- Produces one Codex limit set per scheduler tick with no duplicate `codex.*` IDs.

- [ ] **Step 1: Write the failing source-selection test**

Add a pure helper in `src-tauri/src/providers/codex_live.rs` and test it:

```rust
#[test]
fn auto_keeps_local_limits_when_live_result_is_missing() {
    let local = vec![limit("codex.5h", 42.0), limit("codex.week", 5.0)];
    assert_eq!(choose_limits("auto", None, local.clone())[0].util, 42.0);
    assert!(choose_limits("live", None, local).is_empty());
}
```

- [ ] **Step 2: Run the targeted test and verify it fails**

Run: `cargo test --manifest-path src-tauri\\Cargo.toml codex_live::tests::auto_keeps_local_limits_when_live_result_is_missing`

Expected: failure because `choose_limits` does not exist.

- [ ] **Step 3: Implement selection and wire the scheduler**

Implement this helper in `codex_live.rs`:

```rust
pub fn choose_limits(source: &str, live: Option<Vec<Limit>>, local: Vec<Limit>) -> Vec<Limit> {
    match source {
        "local" => local,
        "auto" => live.unwrap_or(local),
        _ => live.unwrap_or_default(),
    }
}
```

In `spawn_scheduler`, construct `let mut codex_live = providers::codex_live::CodexLiveProvider::new();`. On every loop, read the current setting through `app.try_state::<AppData>()`, get the live result only for `live` and `auto`, get the local result only for `local` and `auto`, then call `choose_limits`. Keep the existing `anthropic.poll(now, force)` behavior unchanged.

- [ ] **Step 4: Run scheduler and provider tests**

Run: `cargo test --manifest-path src-tauri\\Cargo.toml providers::codex_live`

Expected: all live-provider and source-selection tests pass.

- [ ] **Step 5: Commit the scheduler slice**

```bash
git add src-tauri/src/lib.rs src-tauri/src/providers/codex_live.rs
git commit -m "feat: use configured Codex quota source"
```

### Task 4: Standardize the island skin and verify build artifacts

**Files:**
- Modify: `src/styles.css`
- Modify: `docs/CONFIG.md`

**Interfaces:**
- Uses only CSS custom properties from `:root`; no executable-specific conditionals.

- [ ] **Step 1: Add a visual regression checklist to the CSS comment**

Above the island rules, add:

```css
/* Release invariant: portable, NSIS, and MSI all package this same Vite CSS.
   Reference skin: opaque #090b0e pill, amber near border/glow, high-contrast text. */
```

- [ ] **Step 2: Verify the pre-change frontend build**

Run: `npm run build`

Expected: Vite completes with exit code 0.

- [ ] **Step 3: Apply the shared reference palette**

Set these root variables and near-state rules:

```css
--pill-bg: #090b0e;
--border: rgba(255, 255, 255, 0.16);
--text: #e8edf5;
--text-dim: #a4adba;
--track: rgba(255, 255, 255, 0.17);

.island.status-near {
  border-color: rgba(251, 191, 36, 0.72);
  box-shadow: 0 2px 14px rgba(251, 191, 36, 0.28);
}
```

Keep transparent window backgrounds unchanged and do not introduce executable/path checks. Update `docs/CONFIG.md` to state that portable, NSIS, and MSI use `src/styles.css` as the single island skin source.

- [ ] **Step 4: Build and run the full Rust suite**

Run: `npm run build; cargo test --manifest-path src-tauri\\Cargo.toml`

Expected: Vite exits 0 and every Rust test passes.

- [ ] **Step 5: Package one release and compare its bundled frontend asset**

Run: `npm run tauri build`

Expected: NSIS, MSI, and `src-tauri\\target\\release\\tokenbar.exe` are produced from the same `dist` directory; no build command selects a different CSS file for portable mode.

- [ ] **Step 6: Commit the skin slice**

```bash
git add src/styles.css docs/CONFIG.md
git commit -m "style: unify island capsule skin"
```
