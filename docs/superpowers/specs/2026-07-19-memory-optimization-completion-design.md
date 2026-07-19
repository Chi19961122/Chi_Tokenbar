# Memory Optimization Completion Design

**Date:** 2026-07-19  
**Status:** Approved direction; implementation pending specification review  
**Scope:** Complete the analytics and share-preview memory plan after commits `c34c503`, `427a741`, `acd0195`, `988dd7d`, and `4a68987`.

## 1. Context

Stage 0, Stage 1A, and most of Stage 1B are present. The current scan coordinator coalesces compatible requests and applies a latest-wins pending policy, but superseded requests are rejected as ordinary errors. The frontend catches every analytics error and substitutes random mock data, then caches that result. Rapid range changes can therefore render and persist fabricated production data.

The provider scanners still allocate a new string for every line, deserialize broad `serde_json::Value` trees, and, for Codex, open each file twice to discover the working directory before token parsing. Share Preview retains a base64 data URL and creates additional JavaScript byte representations while saving.

The remaining work will be evidence-gated. A stage is complete when its implementation and acceptance tests pass, or when release measurements satisfy its documented gate and the stage is explicitly waived with evidence. This prevents speculative infrastructure from becoming permanent maintenance cost.

## 2. Goals

1. Make analytics cancellation and failure semantics correct: no Tauri error may become mock analytics or enter the production cache.
2. Reduce cold-scan allocation and file-I/O overhead without changing provider attribution, deduplication, date filtering, or metrics semantics.
3. Add reproducible release-build memory and latency measurement with machine-readable output.
4. Add an on-disk analytics index only if Stage 2 cannot meet the agreed latency and memory gates.
5. Replace retained Share Preview data URLs with lifecycle-managed temporary PNG files and avoid unnecessary 3x preview rendering.
6. Apply window, font, and bundle changes only when individually measured and regression-free.
7. Bring `MEMORY-OPTIMIZATION.md` and its review notes in line with the implemented current state.

## 3. Non-goals

- Changing token-cost calculations or analytics product behavior.
- Changing range definitions, provider discovery paths, or project filtering semantics.
- Replacing the current chart/UI design.
- Optimizing unrelated application modules.
- Modifying or deleting the unrelated untracked `design/preview-portfolio/` directory.

## 4. System Invariants

- A superseded request is a normal control-flow outcome, not analytics data and not a user-visible scan failure.
- Only the newest relevant range request may update the visible analytics view or its range cache.
- Mock analytics are allowed only when running without Tauri; they are never a fallback for a Tauri invocation error.
- Provider results must stay identical for supported logs, including Claude cache/tool-use deduplication and Grok sticky-model attribution across model switches.
- Invalid or truncated records are skipped and counted consistently; a typed-schema mismatch must not silently redefine the existing `json_parse_ok` metric.
- Scan coordinator state must recover after cancellation, worker panic, and worker-spawn failure.
- Temporary Share Preview files are application-owned, cannot be selected through arbitrary frontend paths, and are deleted on replacement and lifecycle teardown.

## 5. Analytics Request Contract

### 5.1 Backend result

The Tauri analytics command will expose stable structured error data with these codes:

- `superseded`: a newer pending request replaced this request.
- `cancelled`: application or coordinator teardown cancelled the request.
- `scan_failed`: the active scan failed for an operational reason.

The internal coordinator must use typed outcomes rather than matching human-readable strings. Tauri serialization may use a serializable error object or an equivalent tagged payload, but frontend behavior must depend only on the stable code.

If worker creation fails, the coordinator returns `scan_failed`, resolves all affected waiters, and restores `busy`, `inflight`, and pending state. No spawn path may panic and strand the coordinator.

### 5.2 Frontend behavior

The datasource returns one of:

- analytics data;
- an explicit superseded/cancelled no-result outcome;
- a thrown operational error.

`fetchAnalytics` verifies request identity before rendering or writing the cache. Superseded/cancelled requests do neither. Operational errors preserve the last valid view and follow existing UI error reporting. In non-Tauri browser development only, the datasource may return mock data.

The key race test is `today -> week -> month`: after out-of-order completion, only `month` may paint or enter the cache.

## 6. Stage 2: Streaming Typed Parsers

### 6.1 Shared scan mechanics

- Replace `BufRead::lines()` with `read_line(&mut String)` and reuse one line buffer per open file.
- Deserialize provider-specific minimal typed envelopes. Large message/content bodies and unknown fields are ignored unless a small field is required for existing semantics.
- Borrow strings where practical, while avoiding lifetimes that force data to outlive the reusable line buffer.
- Preserve duplicate suppression, session reset/replay handling, date boundaries, file ordering, and aggregate counters.

### 6.2 Codex

Each file is opened once. Up to the first eight lines are retained temporarily while looking for `session_meta.cwd`; those retained lines are then processed for tokens before continuing from the same `BufReader`. If cwd is absent, existing attribution fallback behavior remains unchanged.

This bounded prefix is deliberate: repository samples place cwd before token events, and it removes the existing metadata probe reopen without buffering the whole file.

### 6.3 Claude

The typed representation contains only the fields needed for usage extraction, cache accounting, tool-use/dedup identity, timestamp/session handling, and model attribution. The current usage-versus-content precedence and deduplication behavior remain unchanged.

### 6.4 Grok

Content is ignored. Sticky model state remains per file/session and must support split records, mid-stream model changes, reset/replay input, and token records that omit a model. Model attribution never leaks between unrelated files.

### 6.5 Metrics

`json_parse_ok` continues to mean syntactically valid JSON unless a separate schema metric is introduced. A syntactically valid record with an unsupported shape must not be reported as malformed JSON. Any metric change requires an explicit rename and documentation update.

## 7. Measurement Harness and Gates

Add a PowerShell release-measurement script under `scripts/` that:

- launches or attaches to the release Atoll process;
- samples the Atoll process tree once per second;
- records timestamp, scenario, process count, Working Set, and Private Bytes to CSV;
- records available analytics timing markers and scenario milestones;
- supports repeated runs and produces median/p95 summaries without overwriting raw samples.

Required scenarios, each run three times after a clean launch:

1. cold startup and idle stabilization;
2. cold `today`, `week`, and `month` analytics requests;
3. rapid range switching;
4. ten Share Preview open/close cycles;
5. memory recovery at 30 and 60 seconds after close.

Record a post-Stage-1B baseline before Stage 2 changes and a post-Stage-2 result on the same machine/configuration. Environment and dataset size are recorded with results.

### Stage 3 index gate

Stage 3 is required if any repeatable post-Stage-2 release result fails these gates:

- warm cached backend request p95: less than 50 ms;
- cold `week` request p95: at most 1.5 seconds;
- cold `month` request p95: at most 3 seconds;
- repeated scans: no monotonic Private Bytes growth after the 60-second recovery window.

If all gates pass, Stage 3 is closed as an evidence-backed waiver and the measurement artifact is linked from the plan. The thresholds are performance gates for this project, not claims that every machine has identical absolute timings.

## 8. Stage 3: Optional Persistent Index

If the gate fails, implement a local SQLite index owned by the Rust backend. The index stores normalized aggregate events or daily/provider/project buckets sufficient to answer current ranges; it must not store large message content.

Each source file has a fingerprint/watermark including provider, canonical path identity, length, modification time, and processed offset plus the minimal parser continuation state required by that provider. Changed/truncated files are rebuilt safely. New suffixes are ingested transactionally. Schema versioning supports full local rebuild when incompatible.

Index reads remain behind the existing analytics command contract, so frontend code does not depend on SQLite. Cold rebuild and corrupted-index recovery fall back to a safe rebuild rather than fabricated or partial totals.

Tests cover initial build, unchanged reuse, append, truncate/replace, schema rebuild, corruption recovery, date ranges, project filtering, and parity with the streaming scanners.

## 9. Stage 4: File-backed Share Preview

The frontend still renders the card, but the retained preview state becomes an opaque backend-owned temporary-file identifier rather than a base64 data URL.

Flow:

1. Render preview at 1x for the modal.
2. Transfer PNG bytes through the narrowest supported Tauri binary path; if the local Tauri version cannot accept raw bytes, a transient data URL may cross one invocation boundary but is not stored in frontend or backend state.
3. Rust writes the PNG atomically into an application-specific temporary directory and returns an opaque preview identifier plus a safe display URL.
4. Replacing or closing the preview deletes the previous file.
5. Window teardown removes active files; startup removes stale files older than a documented TTL.
6. Saving/exporting renders at 3x only when high-resolution output is requested and writes directly through the backend.

The backend resolves identifiers only inside its own preview directory. The frontend cannot submit an arbitrary path for deletion or saving. Save failures keep the preview available and report the error without leaking temporary files indefinitely.

## 10. Stage 5: Measurement-gated Shell Optimizations

Evaluate these independently, retaining only changes with a material release-build benefit and no visual or interaction regression:

- window transparency/compositing settings;
- delayed or reduced font loading;
- dynamic imports for Analytics and Share modules.

For every retained change, capture before/after startup or memory evidence. For every rejected change, document the measured result or regression. Transparency is not disabled merely because it is a suspected cost; Playfair is not delayed if it causes visible layout shift; bundle splitting is not kept if it only moves cost while degrading first-open latency.

## 11. Testing Strategy

Development follows red-green-refactor per task.

### Rust

- exact structured coordinator outcomes;
- coalescing, latest-wins, leader completion, waiter resolution;
- panic and injected worker-spawn failure recovery;
- malformed, truncated, unknown-shape, and cross-day records;
- Codex cwd prefix and single-open behavior;
- Claude usage/cache/tool-use dedup parity;
- Grok sticky/switch/reset/replay parity;
- index parity and recovery tests if Stage 3 is activated;
- Share Preview identifier confinement and cleanup.

### TypeScript

- browser-only mock behavior;
- Tauri superseded/cancelled no-result behavior;
- Tauri operational errors never produce mock data;
- rapid range changes update only the newest cache/view;
- preview replacement, close, failed save, and cleanup requests.

### Final verification

- complete Rust test suite;
- complete Vitest suite;
- production frontend build;
- release Rust build/check as appropriate;
- release measurement scenarios and artifact review;
- no new compiler warnings;
- final diff and documentation audit by the primary agent.

## 12. Delivery Sequence

1. Correct the analytics request contract and coordinator recovery.
2. Capture the post-Stage-1B release baseline.
3. Implement and verify Stage 2 parsers.
4. Capture post-Stage-2 measurements and decide the Stage 3 gate.
5. Implement Stage 3 only if required; otherwise record the waiver.
6. Implement Stage 4 Share Preview lifecycle.
7. Measure and decide each Stage 5 candidate.
8. Update memory-plan documentation and remove stale current-state claims.
9. Run final verification and primary-agent review.

Each implementation task is isolated, starts from a failing test where behavior changes, and receives a focused code review before integration. Delegated implementation does not replace the final primary-agent audit.

## 13. Acceptance Criteria

The work is complete when:

- no production analytics error can synthesize or cache mock data;
- cancellation and coordinator failure paths are deterministic and tested;
- Stage 2 provider parity tests and full existing suites pass;
- release measurement raw data and summaries are reproducible;
- Stage 3 is either passing with index parity/recovery tests or formally waived by the stated gates;
- Share Preview uses bounded, file-backed state with verified cleanup;
- each Stage 5 decision has evidence;
- documentation describes the actual current architecture and remaining limitations;
- the final primary-agent review finds no unresolved correctness or high-risk memory issue.
