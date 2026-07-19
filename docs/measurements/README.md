# Memory measurements

Release-build process-tree samples for Atoll (Working Set + Private Bytes).

## Scripts

```powershell
# Validate helpers (no app launch)
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/measure-memory.ps1 -SelfTest
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/summarize-memory.ps1 -SelfTest

# Capture a run (refuses to overwrite OutDir)
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/measure-memory.ps1 `
  -Exe path\to\atoll.exe -OutDir docs/measurements/stage-1b/<run-id>/idle-r1 `
  -Scenario idle -DurationSec 60 -Run 1

powershell -NoProfile -ExecutionPolicy Bypass -File scripts/summarize-memory.ps1 `
  -InputDir docs/measurements/stage-1b/<run-id>/idle-r1
```

## Layout

```
docs/measurements/
  stage-1b/<run-id>/   # post-Stage-1B baseline
  stage-2/<run-id>/    # post-Stage-2 comparison
  stage-5/decisions.md
```

Commit **metadata.json** and **summary.csv** only. Large raw `samples.csv` may stay local; record SHA-256 in metadata.

## Gates (Stage 3 decision)

Activate SQLite index only if post-Stage-2 release results fail any of:

- warm backend p95 &lt; 50 ms
- cold week p95 ≤ 1.5 s
- cold month p95 ≤ 3 s
- no monotonic Private Bytes growth after 60 s recovery
