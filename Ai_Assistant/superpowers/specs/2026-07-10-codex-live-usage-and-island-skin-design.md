# Codex Live Usage and Island Skin Design

## Goal

Let the user choose the Codex quota source, including a live account read that matches the Codex app, and make the island appearance identical in portable and installed builds.

## Decisions

- Add `codex_usage_source` to persisted settings with `local` as the default for new installs, `live` for an explicit account read, and `auto` for live-first with a local fallback.
- Live reads use `~/.codex/auth.json` only in memory, call `https://chatgpt.com/backend-api/wham/usage`, and send neither credentials nor response bodies to logs.
- Live reads are cached for 180 seconds and respect the existing five-second forced-refresh floor. A failed `live` read returns degraded Codex limits; `auto` falls back to the local provider.
- The response maps `primary_window` to `codex.5h` and `secondary_window` to `codex.week`; `used_percent`, `limit_window_seconds`, and `reset_at` map directly to the canonical `Limit` fields.
- The common island palette is a black, opaque pill with amber near-state border and glow, matching the supplied portable-build screenshot. All colors remain CSS tokens in `src/styles.css`, so NSIS/MSI and the portable executable package the same frontend assets.

## Error Handling and Verification

- Missing, malformed, or rejected Codex credentials never expose their contents and render a `SourceFailed` Codex state when the live-only source is selected.
- Parser tests use a fixture with 15% used for the 5-hour window and 3% for the weekly window; they also cover malformed/missing windows.
- Frontend mock settings cover the new field; production build plus Rust tests confirm both bundles compile from the same assets.
