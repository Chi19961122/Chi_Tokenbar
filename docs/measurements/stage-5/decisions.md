# Stage 5 shell optimization decisions

**Date:** 2026-07-19  
**Status:** All candidates **rejected / deferred** without code change in this completion pass.

| Candidate | Decision | Rationale |
|---|---|---|
| Disable window transparency | **Reject** | No isolated release before/after capture in this run; visual risk to island glass without evidence of material WS/Private gain. |
| Delay Playfair / font loading | **Reject** | Share card offline export depends on embedded fonts; risk of layout shift on share open. |
| Dynamic import Analytics/Share | **Reject** | First-open latency regression risk; island already loads core shell. Revisit after Stage 2 measurement baseline exists. |

No `tauri.conf.json` / `fonts.css` / lazy-chunk code retained.
