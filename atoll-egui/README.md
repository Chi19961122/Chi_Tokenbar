# Atoll · egui experiment

**Branch:** `exp/egui-shell`  
**Purpose:** Local spike only — see memory / feel of a **no-WebView2** shell.  
**Not production.** Main app stays Tauri under `src-tauri/`.

## What you get

- Standalone binary `atoll-egui.exe`
- Compact “island” bar (Claude / Codex remaining %)
- Expandable limit list (demo data)
- **No** live APIs, **no** log scanners, **no** Share card, **no** tray yet

## Run

```powershell
cd atoll-egui
cargo run --release
```

## Package (separate from Atoll-release)

```powershell
cd atoll-egui
cargo build --release
# optional collect
$out = Join-Path (Resolve-Path ..\..).Path "Atoll-egui-release"
New-Item -ItemType Directory -Force -Path $out | Out-Null
Copy-Item target\release\atoll-egui.exe $out\Atoll-egui-portable.exe -Force
```

Or from repo root:

```powershell
powershell -NoProfile -File atoll-egui/collect-release.ps1
```

Output folder: `TokenBar/Atoll-egui-release/` (sibling of `TokenBar-Src`, **not** mixed into `Atoll-release/`).

## Compare memory

1. Run production `Atoll-portable.exe` — note WebView2 GPU / tree WS  
2. Run `Atoll-egui-portable.exe` — usually **no** `msedgewebview2` children  
3. Do **not** treat demo UI parity as a ship decision; this is a shell feasibility check

## Decision later

- Keep Tauri → abandon / freeze this branch  
- Rewrite → grow this crate (wire real providers, tray, analytics) or start Slint instead  

## License

Same as parent repo.
