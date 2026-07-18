// Data-source abstraction: real Tauri backend, or mock in a plain browser.
// The UI only talks to this module, so it renders identically either way.

import type { Analytics, Settings, Snapshot } from "./types";
import { SCENARIOS, mockAnalytics } from "./mock";
import { nowSecs } from "./format";

const DEFAULT_SETTINGS: Settings = {
  allow_token_refresh: false,
  autostart: false,
  warn_pct: 75,
  crit_pct: 90,
  compact: false,
  providers: "both",
  codex_usage_source: "local",
  // Mirrors config.rs `Settings::default()` — pinned unless the user opts out.
  always_on_top: true,
  // Follow the OS language unless the user picks one explicitly.
  locale: "system",
  // 階段 B defaults — mirror config.rs `Settings::default()`.
  expand_default: "compact",
  island_pin_claude: "auto",
  island_pin_codex: "auto",
  island_aux: "tok_per_min",
  reset_display: "relative",
  // T-901 亮暗雙主題 — mirror config.rs `Settings::default()`.
  theme: "system",
  // 階段 D defaults — mirror config.rs `Settings::default()`.
  share_style: "statement",
  share_range: "week",
  // T-905 default — mirror config.rs `Settings::default()`.
  share_size: "auto",
  // 階段 E defaults — mirror config.rs `Settings::default()` (detect-and-show).
  tool_opencode: true,
  tool_gemini: true,
};

export const isTauri = () =>
  typeof (window as any).__TAURI_INTERNALS__ !== "undefined" ||
  typeof (window as any).__TAURI__ !== "undefined";

// Island vs expanded panel window sizes (logical px).
// Expanded height is measured once per mode entry; collapsed width shrinks
// to 270 when the island shows a single provider (see main.ts collapsedSize).
export const SIZE = {
  collapsed: { w: 340, h: 52 },
  expanded: { w: 380 },
};

type Cb = (s: Snapshot) => void;

// ── mock plumbing (browser) ──────────────────────────────────────────
let mockScenario = "safe";
const mockSubs: Cb[] = [];

export function setMockScenario(name: string) {
  if (!(name in SCENARIOS)) return;
  mockScenario = name;
  emitMock();
}

function emitMock() {
  const s = { ...SCENARIOS[mockScenario], updated_at: nowSecs() };
  for (const cb of mockSubs) cb(s);
}
export const mockScenarioNames = () => Object.keys(SCENARIOS);

// ── public API ───────────────────────────────────────────────────────

export async function getSnapshot(): Promise<Snapshot | null> {
  if (isTauri()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return (await invoke<Snapshot | null>("get_snapshot")) ?? null;
  }
  return SCENARIOS[mockScenario];
}

/** Ask the backend to poll all providers right now (mock: re-emit fresh). */
export async function refreshNow(): Promise<void> {
  if (isTauri()) {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("refresh_now");
    return;
  }
  emitMock();
}

/**
 * Launch the official `claude auth login` flow via the backend.
 *
 * Rejects with a fixed code ("claude_not_found" / "spawn_failed"); the panel
 * treats any rejection the same way — it shows the command to run by hand.
 * That path is not rare: TokenBar autostarts from Explorer with a different
 * PATH than the user's terminal, and a WSL-only Claude Code has no Windows
 * launcher at all.
 *
 * The browser preview has no backend, so it rejects — which conveniently makes
 * the fallback (the taller, layout-risky branch) the previewable one.
 */
export async function relogin(): Promise<void> {
  if (!isTauri()) throw new Error("claude_not_found");
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("relogin");
}

export async function onSnapshot(cb: Cb): Promise<void> {
  if (isTauri()) {
    const { listen } = await import("@tauri-apps/api/event");
    await listen<Snapshot>("snapshot", (e) => cb(e.payload));
    return;
  }
  mockSubs.push(cb);
}

export async function getAnalytics(range: "today" | "week" | "month"): Promise<Analytics> {
  if (isTauri()) {
    const { invoke } = await import("@tauri-apps/api/core");
    try {
      return await invoke<Analytics>("get_analytics", { range });
    } catch {
      return mockAnalytics(range); // backend command not present yet
    }
  }
  return mockAnalytics(range);
}

export async function getSettings(): Promise<Settings> {
  if (isTauri()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<Settings>("get_settings");
  }
  try {
    return { ...DEFAULT_SETTINGS, ...JSON.parse(localStorage.getItem("tb.settings") || "{}") };
  } catch {
    return DEFAULT_SETTINGS;
  }
}

export async function setSettings(settings: Settings): Promise<void> {
  if (isTauri()) {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("set_settings", { settings });
    return;
  }
  localStorage.setItem("tb.settings", JSON.stringify(settings));
}

/** Begin an OS window drag (Tauri only; no-op in browser). */
export async function startWindowDrag(): Promise<void> {
  if (!isTauri()) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().startDragging();
}

/**
 * Hide the island, leaving only the tray icon (§ island hide button).
 *
 * Done from the frontend rather than through a new Tauri command: the
 * `core:window:allow-hide` capability is already granted, and a command would
 * only wrap the same one call. The window stays *hidden*, not closed, so the
 * tray's "Show / Hide" brings it back — `toggle_action(visible=false, …)`
 * returns Show, and `skipTaskbar: true` makes that the only way back, so
 * nothing here may ever `close()`.
 *
 * No-op in the browser preview (no window to hide), like startWindowDrag.
 */
export async function hideWindow(): Promise<void> {
  if (!isTauri()) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().hide();
}

/** Work area (excludes taskbar) with a fallback for older tauri-api versions. */
function workAreaOf(mon: any): { position: { x: number; y: number }; size: { width: number; height: number } } {
  return mon.workArea ?? { position: mon.position, size: mon.size };
}

/** Snap the island to the nearest work-area edge after a drag settles (§10 docking). */
export async function setupEdgeSnap(): Promise<void> {
  if (!isTauri()) return;
  const { getCurrentWindow, currentMonitor, PhysicalPosition } = await import(
    "@tauri-apps/api/window"
  );
  const win = getCurrentWindow();
  let timer: ReturnType<typeof setTimeout> | undefined;

  await win.onMoved(() => {
    clearTimeout(timer);
    timer = setTimeout(async () => {
      try {
        const mon = await currentMonitor();
        if (!mon) return;
        const size = await win.outerSize();
        const pos = await win.outerPosition();
        const margin = Math.round(8 * mon.scaleFactor);
        const snap = Math.round(40 * mon.scaleFactor);

        const wa = workAreaOf(mon);
        const mx = wa.position.x;
        const my = wa.position.y;
        const mw = wa.size.width;
        const mh = wa.size.height;
        let { x, y } = pos;

        if (y - my < snap) y = my + margin; // top
        if (my + mh - (y + size.height) < snap) y = my + mh - size.height - margin; // bottom
        if (x - mx < snap) x = mx + margin; // left
        if (mx + mw - (x + size.width) < snap) x = mx + mw - size.width - margin; // right

        if (x !== pos.x || y !== pos.y) await win.setPosition(new PhysicalPosition(x, y));
      } catch {
        /* querying window geometry may be permission-gated; ignore */
      }
    }, 250);
  });
}

// Serialize resize calls so rapid re-fits can't interleave size/position writes.
let resizeChain: Promise<void> = Promise.resolve();

/**
 * Resize the window to `w`×`h` (logical px) keeping its bottom-right corner
 * fixed, so the panel grows upward/leftward from wherever the island sits.
 * Clamped into the monitor work area. No-op in the browser.
 */
export function resizeAnchored(w: number, h: number): Promise<void> {
  if (!isTauri()) return Promise.resolve();
  resizeChain = resizeChain.then(async () => {
    try {
      const { getCurrentWindow, currentMonitor, PhysicalPosition, PhysicalSize } =
        await import("@tauri-apps/api/window");
      const win = getCurrentWindow();
      const mon = await currentMonitor();
      const scale = mon?.scaleFactor ?? 1;
      const margin = Math.round(8 * scale);
      const wa = mon ? workAreaOf(mon) : null;

      const pw = Math.round(w * scale);
      let ph = Math.round(h * scale);
      if (wa) ph = Math.min(ph, wa.size.height - margin * 2);

      const pos = await win.outerPosition();
      const size = await win.outerSize();
      let x = pos.x + size.width - pw;
      let y = pos.y + size.height - ph;
      if (wa) {
        x = Math.min(Math.max(x, wa.position.x + margin), wa.position.x + wa.size.width - pw - margin);
        y = Math.min(Math.max(y, wa.position.y + margin), wa.position.y + wa.size.height - ph - margin);
      }
      // x/y are the anchored top-left computed from the *current* geometry and
      // the target size above — they don't depend on the resize landing first,
      // so fire both writes together instead of serializing two IPC round-trips.
      await Promise.all([
        win.setSize(new PhysicalSize(pw, ph)),
        win.setPosition(new PhysicalPosition(x, y)),
      ]);
    } catch {
      /* geometry queries may fail transiently; next fit will retry */
    }
  });
  return resizeChain;
}
