# Memory Optimization Completion Implementation Plan

> **For Codex:** Execute this plan with `superpowers:subagent-driven-development`. Use a fresh implementer for each task, then a focused reviewer. The primary agent owns all integration decisions and the final audit.

**Goal:** Finish the approved memory-optimization design without allowing cancelled analytics requests to fabricate data, while reducing scan and Share Preview memory with measured release-build evidence.

**Architecture:** Stabilize the request contract first, then measure the current Stage 1B build. Optimize the existing raw scanners using typed streaming parsing and measure again. Add SQLite only if the approved release gates fail. Move Share Preview retention to backend-owned temporary PNGs, and keep Stage 5 shell changes only when measured.

**Tech stack:** Rust 2021, Tauri 2, TypeScript 5.6, Vite 6, Vitest 4, PowerShell, optional SQLite through `rusqlite` only if the Stage 3 gate activates.

**Design spec:** `docs/superpowers/specs/2026-07-19-memory-optimization-completion-design.md`

---

## Global execution rules

- Work in an isolated git worktree created from `3845703` or its direct descendant.
- Preserve the unrelated `design/preview-portfolio/` directory in the original worktree.
- Follow red-green-refactor for every behavior change.
- Commit each task separately using Conventional Commits.
- Do not activate Stage 3 or retain a Stage 5 candidate without the required measurement evidence.
- After every task, run its focused tests and review the committed diff before continuing.
- Before completion, run the full Rust, TypeScript, build, warning, documentation, and release-measurement checks.

## Task 1: Structured analytics cancellation contract

**Files:**

- Modify: `src-tauri/src/scan_coord.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/datasource.ts`
- Modify: `src/main.ts`
- Create: `src/datasource.test.ts`
- Create: `src/analytics-request.ts`
- Create: `src/analytics-request.test.ts`

### Step 1: Add failing Rust tests

Add tests that assert:

- replacing a pending request returns the exact `superseded` code;
- a scan failure returns `scan_failed` without string matching;
- an injected worker-spawn failure resolves affected waiters and restores coordinator state;
- the next request succeeds after panic or spawn failure.

Run:

```powershell
Set-Location src-tauri
cargo test scan_coord::tests -- --nocapture
```

Expected: new tests fail because coordinator errors are strings and worker spawn is not injectable.

### Step 2: Implement typed coordinator errors and safe spawning

Introduce a serializable error contract equivalent to:

```rust
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum AnalyticsErrorCode {
    Superseded,
    Cancelled,
    ScanFailed,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
struct AnalyticsError {
    code: AnalyticsErrorCode,
    message: String,
}
```

Use typed internal outcomes throughout `ScanCoordinator`. Add a small worker-spawner abstraction or injected callback for tests. On spawn failure, remove the inserted inflight entry, clear/advance pending state correctly, resolve every waiter, and leave the coordinator reusable.

Expose the serializable error unchanged through the Tauri command.

### Step 3: Add failing frontend tests

`src/datasource.test.ts` must prove:

- non-Tauri mode may return mock analytics;
- Tauri `superseded` and `cancelled` return an explicit no-result outcome;
- Tauri `scan_failed` and unknown rejections throw;
- no Tauri rejection calls `mockAnalytics`.

`src/analytics-request.test.ts` must simulate `today -> week -> month` with out-of-order promises and prove only `month` can call cache/render callbacks.

Run:

```powershell
npm test -- src/datasource.test.ts src/analytics-request.test.ts
```

Expected: tests fail against the current catch-all mock fallback and unguarded cache write.

### Step 4: Implement frontend request gating

In `datasource.ts`, distinguish browser mode from Tauri mode before invoking. Decode stable Tauri error codes without depending on human-readable messages. Return `null` or a tagged no-result for superseded/cancelled requests and throw operational errors.

Extract the request-generation/commit decision from `main.ts` into `analytics-request.ts`. A request captures its generation; only the current generation may write `analyticsCache`, update range data, or paint. Preserve the last valid view on operational failure.

### Step 5: Verify and commit

Run:

```powershell
npm test -- src/datasource.test.ts src/analytics-request.test.ts
Set-Location src-tauri
cargo test scan_coord::tests -- --nocapture
```

Commit:

```text
fix(analytics): make cancellation outcomes explicit
```

## Task 2: Reproducible release memory harness and Stage 1B baseline

**Files:**

- Create: `scripts/measure-memory.ps1`
- Create: `scripts/summarize-memory.ps1`
- Create: `docs/measurements/README.md`
- Create: `docs/measurements/stage-1b/<run-id>/metadata.json`
- Create: `docs/measurements/stage-1b/<run-id>/summary.csv`
- Do not commit large raw per-second CSV files; retain their path and checksum in metadata.

### Step 1: Add script-level validation mode

The scripts must support a `-SelfTest` or fixture-input mode that validates:

- process-tree aggregation;
- one-second sample schema;
- median and nearest-rank p95 calculation;
- separate scenario/run grouping;
- refusal to overwrite an existing run directory.

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/measure-memory.ps1 -SelfTest
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/summarize-memory.ps1 -SelfTest
```

Expected: validation initially fails because scripts do not exist.

### Step 2: Implement the harness

`measure-memory.ps1` accepts an explicit executable, output directory, scenario name, duration, and optional existing PID. Resolve only the launched PID and descendants. Record ISO timestamp, elapsed seconds, scenario, run, PID count, Working Set bytes, Private Bytes, and a milestone field.

`summarize-memory.ps1` consumes raw CSV and writes per-scenario/run plus aggregate median/p95 summaries. All paths must be resolved and constrained to the requested measurement directory.

### Step 3: Build and capture the Stage 1B baseline

Run:

```powershell
npm run build
Set-Location src-tauri
cargo build --release
```

On the same build and dataset, capture three runs of:

- cold startup and idle;
- cold today/week/month;
- rapid range switching;
- ten Share Preview open/close cycles;
- 30/60 second recovery.

Record commit, OS, CPU, RAM, release executable hash, analytics file count/bytes, exact commands, and raw CSV checksum. Commit the small metadata and summary only.

### Step 4: Verify and commit

Run both script self-tests again and inspect that all required scenarios have three runs.

Commit:

```text
perf: add release memory measurement harness
```

## Task 3: Shared Stage 2 streaming parser mechanics

**Files:**

- Modify: `src-tauri/src/analytics.rs`

### Step 1: Add failing scanner-mechanics tests

Add focused tests/fixtures for:

- malformed JSON versus syntactically valid unknown schema;
- truncated final line;
- unknown fields containing large content;
- cross-day filtering;
- stable existing `json_parse_ok` semantics.

Where direct open-count observation is needed, extract a small reader-based helper so tests exercise parsing without filesystem timing assumptions.

Run:

```powershell
Set-Location src-tauri
cargo test analytics::tests -- --nocapture
```

### Step 2: Replace per-line allocation

Replace `BufRead::lines()` in all provider scanners with a reusable `String` and `read_line`. Clear the buffer only after the current borrowed deserialization result is no longer used.

Introduce provider-specific minimal envelopes with `Deserialize`; retain `serde_json::Value` only for genuinely dynamic small subtrees required by existing behavior. Unknown large content must be ignored.

Keep syntax metrics separate from supported-schema decisions.

### Step 3: Verify parity and commit

Run all analytics tests, then the full Rust suite.

Commit:

```text
perf(analytics): reuse streaming scan buffers
```

## Task 4: Codex single-pass typed scan

**Files:**

- Modify: `src-tauri/src/analytics.rs`

### Step 1: Add failing Codex tests

Cover:

- cwd in each position within the eight-line prefix;
- missing cwd fallback;
- token events before and after cwd;
- malformed prefix records;
- duplicate/session replay behavior;
- current delta and cross-day totals.

Use an injected reader/open helper or equivalent deterministic seam to prove one open/one reader path.

### Step 2: Implement bounded-prefix single pass

Open each Codex file once. Retain at most eight prefix lines while discovering cwd, process those saved lines through the normal token parser, then continue on the same reader and reusable buffer. Deserialize only session metadata and token-event fields required by current aggregation.

Remove the now-unused probe/reopen path and the `codex_token_event` warning source.

### Step 3: Verify and commit

Run Codex-focused tests and the complete Rust suite.

Commit:

```text
perf(analytics): scan codex logs in one pass
```

## Task 5: Claude and Grok minimal typed scans

**Files:**

- Modify: `src-tauri/src/analytics.rs`

### Step 1: Add failing Claude parity tests

Cover usage/content precedence, cache accounting, tool-use deduplication, replay/reset, missing optional fields, large ignored content, and cross-day totals.

### Step 2: Implement Claude typed envelopes

Deserialize only timestamp/session/model/usage/cache/dedup fields. Preserve current precedence and duplicate suppression exactly.

### Step 3: Add failing Grok parity tests

Cover split records, token records without model, mid-stream model switch, reset/replay, file isolation, malformed records, and cross-day totals.

### Step 4: Implement Grok typed envelopes

Ignore content. Keep sticky model state scoped to the current file/session and update it only from supported model-bearing records.

### Step 5: Verify and commit

Run provider-focused tests and the complete Rust suite.

Commit:

```text
perf(analytics): deserialize minimal provider records
```

## Task 6: Stage 2 measurements and Stage 3 decision

**Files:**

- Create: `docs/measurements/stage-2/<run-id>/metadata.json`
- Create: `docs/measurements/stage-2/<run-id>/summary.csv`
- Modify: `docs/MEMORY-OPTIMIZATION.md`

### Step 1: Capture post-Stage-2 release measurements

Build the release executable and repeat the exact Stage 1B scenarios, dataset, durations, and run count. Compare cold week/month latency, warm backend cache latency, and 60-second Private Bytes recovery.

### Step 2: Apply the approved gate

Activate Task 7 if any repeatable result fails:

- warm backend p95 below 50 ms;
- cold week p95 at most 1.5 seconds;
- cold month p95 at most 3 seconds;
- no monotonic Private Bytes growth after 60 seconds.

If all pass, update the plan with an evidence-backed Stage 3 waiver and skip Task 7.

### Step 3: Commit the decision evidence

Commit:

```text
docs: record stage 2 memory measurements
```

## Task 7: Conditional SQLite analytics index

Execute this task only when Task 6 activates Stage 3.

**Files:**

- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Create: `src-tauri/src/analytics_index.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/analytics.rs`

### Step 1: Add failing parity and lifecycle tests

Cover initial build, unchanged reuse, append, truncation/replacement, provider continuation state, transaction rollback, schema-version rebuild, corrupt DB recovery, date ranges, project filter, and exact parity with raw scanners.

### Step 2: Add the smallest SQLite dependency

Use `rusqlite` with an appropriate bundled/system feature decision documented in the commit. Do not add an ORM. Store only normalized aggregate events/buckets and source fingerprints; never store message content.

### Step 3: Implement versioned incremental ingestion

Use canonical source identity, provider, size, modification time, processed offset, and minimal continuation state. Apply suffix ingestion transactionally. Rebuild a file on truncation/replacement and rebuild the database on incompatible schema or corruption.

Keep the existing Tauri analytics contract unchanged.

### Step 4: Verify performance and commit

Run parity/recovery tests, full Rust tests, and the release measurement scenarios again. The index must improve the failed gate without introducing monotonic memory growth.

Commit:

```text
perf(analytics): add incremental local index
```

## Task 8: File-backed Share Preview

**Files:**

- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/share-preview.json` if protocol permissions require it
- Modify: `src/share-panel.ts`
- Modify: `src/share-preview.ts`
- Modify: `src/share-panel.test.ts`
- Modify: `src/share-preview.test.ts`

### Step 1: Add failing Rust lifecycle/security tests

Extract a filesystem-backed preview store that is testable without a WebView. Test:

- replace deletes the previous file;
- clear deletes the active file;
- stale TTL cleanup touches only the app preview directory;
- opaque identifiers cannot escape the directory;
- failed atomic write preserves the prior valid preview;
- save failure does not prematurely delete the preview.

Use a test-specific directory under the system temp directory and validate its resolved path before recursive cleanup.

### Step 2: Implement the backend preview store

Replace `Option<String>` data-URL state with metadata for one backend-owned PNG. Write through a temporary sibling then atomically rename. Return an opaque ID and a safe display URL. Delete on replacement, explicit close, preview-window destruction, and stale startup cleanup.

Prefer raw `Uint8Array`/binary IPC supported by the installed Tauri 2 API. If unavailable, accept a transient data URL only at the update command boundary and immediately decode/write it; never retain the base64 string.

### Step 3: Add failing frontend lifecycle tests

Update tests to prove:

- modal preview rasterization uses `pixelRatio: 1`;
- payload/state uses an opaque preview reference, not retained data URL;
- replacement and close commands follow lifecycle order;
- dedicated preview ignores stale async pulls;
- high-resolution export alone uses `pixelRatio: 3`;
- failed preview/save paths clean up correctly.

### Step 4: Implement frontend flow

Separate `rasterizePreview` from `rasterizeExport`. Render preview at 1x, transfer bytes once, and render the backend-provided safe URL in the dedicated window. Keep browser download fallback functional without Tauri.

### Step 5: Verify and commit

Run:

```powershell
npm test -- src/share-panel.test.ts src/share-preview.test.ts
Set-Location src-tauri
cargo test share_preview -- --nocapture
```

Then run the full TypeScript and Rust suites.

Commit:

```text
perf(share): use file-backed preview lifecycle
```

## Task 9: Stage 5 shell experiments

**Files:**

- Potentially modify: `src-tauri/tauri.conf.json`
- Potentially modify: `src/fonts.css`
- Potentially modify: `src/main.ts`
- Potentially modify: `src/analytics.ts`
- Potentially modify: `src/share-panel.ts`
- Create: `docs/measurements/stage-5/decisions.md`

### Step 1: Establish independent experiment branches/diffs

Measure one candidate at a time:

1. window transparency/compositing;
2. delayed/reduced font loading;
3. Analytics/Share dynamic imports.

For each candidate, capture release startup memory, stabilized memory, first-open latency, build chunk sizes, and visual behavior. Revert the candidate if benefit is immaterial or a visual/interaction regression appears.

### Step 2: Add regression tests for retained candidates

- For dynamic imports, assert the production build has the intended lazy chunk and UI first-open still succeeds.
- For font loading, assert class/application behavior and visually inspect for layout shift.
- For transparency, visually inspect window background, shadows, and click/drag behavior.

### Step 3: Commit decisions

Commit evidence even if all candidates are rejected. If code is retained, include only independently justified changes.

Commit:

```text
perf(shell): apply measured memory optimizations
```

or, if no code survives:

```text
docs: record stage 5 optimization decisions
```

## Task 10: Documentation reconciliation and final verification

**Files:**

- Modify: `docs/MEMORY-OPTIMIZATION.md`
- Modify: `docs/MEMORY-OPTIMIZATION-REVIEW.md`
- Modify: relevant measurement metadata/decision documents

### Step 1: Add a documentation consistency checklist

Correct stale statements about:

- `warmAnalytics` triple scanning;
- parallel range scans;
- preview state surviving close;
- analytics cache behavior;
- Stage 1B status and current anchors.

Label historical pre-1A/1B behavior explicitly. Record whether Stage 3 was implemented or waived and link the exact measurement summaries.

### Step 2: Run complete fresh verification

From the repository root:

```powershell
npm test
npm run build
Set-Location src-tauri
cargo test
cargo check --all-targets
cargo build --release
```

Re-run script self-tests and the final release measurement scenarios. Capture command output, test counts, warnings, commit, and measurement artifact paths.

### Step 3: Primary-agent final review

The primary agent reviews every commit and the aggregate diff for:

- request-race correctness;
- coordinator waiter/state recovery;
- parser parity and metric semantics;
- index correctness if activated;
- temporary-file confinement and cleanup;
- measurement validity and honest gates;
- unrelated worktree changes;
- documentation accuracy.

Resolve all high/medium findings and rerun affected verification. Do not claim completion from stale or partial test output.

### Step 4: Commit final documentation

Commit:

```text
docs: finalize memory optimization plan
```

## Completion handoff

After every task and final verification pass, use `superpowers:finishing-a-development-branch` to present integration choices. Do not merge or discard the isolated worktree without explicit user direction.
