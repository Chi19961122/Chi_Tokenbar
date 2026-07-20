# Atoll · Slint experiment

**Purpose:** Local spike only — RAM-footprint decision gate for a **no-WebView2, no-GPU** shell.
**Not production.** Main app stays Tauri under `src-tauri/`.

## Why software renderer, specifically

The sibling `atoll-egui/` spike (see its README) already proved a native shell drops
WebView2's cost, but it still opens a GPU GL context via `glow` and builds a font
atlas texture — together these cost roughly 100MB of private working set baseline,
eating most of the expected RAM win.

This spike exists to test the next lever: **skip the GPU path entirely.** Slint ships
a CPU software rasterizer (`renderer-software`) as an alternative to its GPU backends
(`renderer-femtovg`, `renderer-skia`). `Cargo.toml` disables Slint's default features
and enables only:

```toml
slint = { version = "1", default-features = false, features = [
    "compat-1-2",
    "std",
    "backend-winit",
    "renderer-software",
] }
```

- `backend-winit` — window/event-loop backend (still needed for a desktop window;
  it does not itself pull in a GPU renderer)
- `renderer-software` — the CPU rasterizer; no femtovg/skia, no OpenGL/Vulkan context
- `compat-1-2`, `std` — required baseline features for a Rust std desktop build

No `renderer-femtovg`, `renderer-skia`, `renderer-skia-opengl`, or `renderer-skia-vulkan`
feature is enabled anywhere in the dependency tree, so no GPU context or GPU font atlas
should be created at runtime. **If a memory profile of the built exe shows a GPU driver
DLL loaded (e.g. `d3d11.dll`/`opengl32.dll` client-side allocations beyond what Windows
loads by default) or a `dxgi`/`d3d` swapchain, that is a sign the software renderer did
not actually get selected — re-check feature flags before trusting the RAM number.**

## What you get

- Standalone binary `atoll-slint.exe`
- Compact "island" bar (Claude / Codex remaining %), frameless, ~340x56
- Click to expand to a ~380x420 panel listing mock limit rows; click a row to
  toggle its detail (provider / status / runway)
- Dark theme, matches the visual scope of `atoll-egui/`
- **No** live APIs, **no** log scanners, **no** Share card, **no** tray, **no** analytics

## Files

- `Cargo.toml` — crate manifest; software-renderer-only Slint feature set; same
  release profile (`lto`, `codegen-units = 1`, `strip`) as `atoll-egui/`
- `build.rs` — invokes `slint-build` to compile `ui/atoll.slint` into generated Rust
- `ui/atoll.slint` — the UI markup: island pill, expandable panel, limit cards
- `src/main.rs` — wires mock data into the generated `AtollWindow` component and
  handles the expand/collapse/row-toggle callbacks
- `src/mock.rs` — demo Claude/Codex/Grok limit data (ported from `atoll-egui/src/mock.rs`)

## Run

```powershell
cd atoll-slint
cargo run --release
```

## Build

```powershell
cd atoll-slint
cargo build --release
```

Or from repo root:

```powershell
cargo build --release --manifest-path atoll-slint/Cargo.toml
```

## Compare memory

1. Run production `Atoll-portable.exe` — note WebView2 GPU / tree WS
2. Run `atoll-egui.exe` — no `msedgewebview2` children, but a GPU GL context + font atlas
3. Run `atoll-slint.exe` — should have neither WebView2 nor a GPU context; compare
   private working set against both of the above
4. Do **not** treat demo UI parity as a ship decision; this is a shell feasibility check

## Decision later

- Keep Tauri → abandon / freeze this branch
- Rewrite → grow this crate (wire real providers, tray, analytics) or fall back to
  the egui shell if the software renderer proves too limited visually

## License

Same as parent repo.
