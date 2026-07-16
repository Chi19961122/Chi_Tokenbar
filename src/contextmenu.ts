// Island right-click menu (決策 D4): pin a limit per provider, switch the
// provider filter, open settings, or hide the island. Native OS menu under
// Tauri (@tauri-apps/api/menu); a DOM menu is the browser-preview fallback so
// the flow is still verifiable without a backend.
//
// Both paths render from the same neutral model (buildSections), so the two
// can't drift: the native menu and the DOM menu always offer the same items,
// checkmarks, and actions.

import type { Limit, ProviderFilter, Settings, Snapshot } from "./types";
import { isTauri } from "./datasource";
import { windowShort } from "./island";
import { t } from "./i18n";

export interface MenuCtx {
  settings: Settings;
  snap: Snapshot | null;
  /** Client coords of the contextmenu event (DOM fallback anchor). */
  x: number;
  y: number;
  /** Merge a settings patch, persist it, and re-render (main.ts owns this). */
  apply: (patch: Partial<Settings>) => void;
  /** Open the settings panel (reuses the gear flow). */
  openSettings: () => void;
  /** Send the island to the tray. */
  hide: () => void;
}

interface Leaf {
  label: string;
  checked?: boolean;
  onSelect: () => void;
}
interface Section {
  label: string;
  items: Leaf[];
}

/** Model-scoped limits (anything that isn't a 5h/weekly window) for a provider. */
function modelLimits(snap: Snapshot | null, provider: Limit["provider"]): Limit[] {
  return (snap?.limits ?? []).filter(
    (l) => l.provider === provider && !l.id.endsWith(".5h") && !l.id.endsWith(".week"),
  );
}

/** Pin options for one provider: Auto / 5h / Week + any model windows present. */
function pinItems(
  ctx: MenuCtx,
  provider: Limit["provider"],
  key: "island_pin_claude" | "island_pin_codex",
): Leaf[] {
  const current = ctx.settings[key];
  const pick = (value: string): Leaf => ({
    label:
      value === "auto"
        ? t("settings.pinAuto")
        : value === "5h"
          ? t("settings.pin5h")
          : value === "week"
            ? t("settings.pinWeek")
            : value.slice("model:".length),
    checked: current === value,
    onSelect: () => ctx.apply({ [key]: value } as Partial<Settings>),
  });
  const items = [pick("auto"), pick("5h"), pick("week")];
  for (const l of modelLimits(ctx.snap, provider)) {
    items.push({
      label: windowShort(l) || l.label,
      checked: current === `model:${l.id}`,
      onSelect: () => ctx.apply({ [key]: `model:${l.id}` } as Partial<Settings>),
    });
  }
  return items;
}

function providerItems(ctx: MenuCtx): Leaf[] {
  const cur = ctx.settings.providers;
  const opt = (value: ProviderFilter, labelKey: Parameters<typeof t>[0]): Leaf => ({
    label: t(labelKey),
    checked: cur === value,
    onSelect: () => ctx.apply({ providers: value }),
  });
  return [
    opt("both", "settings.providersBoth"),
    opt("claude", "settings.providersClaude"),
    opt("codex", "settings.providersCodex"),
  ];
}

function buildSections(ctx: MenuCtx): Section[] {
  return [
    { label: t("menu.pinClaude"), items: pinItems(ctx, "anthropic", "island_pin_claude") },
    { label: t("menu.pinCodex"), items: pinItems(ctx, "codex", "island_pin_codex") },
    { label: t("menu.provider"), items: providerItems(ctx) },
  ];
}

// ── native (Tauri) ────────────────────────────────────────────────────

async function showNative(ctx: MenuCtx): Promise<void> {
  const { Menu, Submenu, CheckMenuItem, MenuItem, PredefinedMenuItem } = await import(
    "@tauri-apps/api/menu"
  );
  const sections = buildSections(ctx);

  const submenus = await Promise.all(
    sections.map(async (sec) => {
      const items = await Promise.all(
        sec.items.map((leaf) =>
          CheckMenuItem.new({
            text: leaf.label,
            checked: !!leaf.checked,
            action: () => leaf.onSelect(),
          }),
        ),
      );
      return Submenu.new({ text: sec.label, items });
    }),
  );

  const sep = await PredefinedMenuItem.new({ item: "Separator" });
  const settingsItem = await MenuItem.new({
    text: t("menu.settings"),
    action: () => ctx.openSettings(),
  });
  const hideItem = await MenuItem.new({
    text: t("menu.hide"),
    action: () => ctx.hide(),
  });

  const menu = await Menu.new({
    items: [...submenus, sep, settingsItem, hideItem],
  });
  await menu.popup();
}

// ── DOM fallback (browser preview) ───────────────────────────────────

let openDom: HTMLElement | null = null;

function closeDom(): void {
  openDom?.remove();
  openDom = null;
  document.removeEventListener("pointerdown", onOutside, true);
  window.removeEventListener("blur", closeDom);
}
function onOutside(e: PointerEvent): void {
  if (openDom && !openDom.contains(e.target as Node)) closeDom();
}

function showDom(ctx: MenuCtx): void {
  closeDom();
  const menu = document.createElement("div");
  menu.className = "ctxmenu";
  menu.setAttribute("role", "menu");

  const addLeaf = (leaf: Leaf, sub = false) => {
    const b = document.createElement("button");
    b.className = `ctx-item${sub ? " ctx-sub" : ""}${leaf.checked ? " on" : ""}`;
    b.setAttribute("role", "menuitem");
    b.textContent = `${leaf.checked ? "✓ " : ""}${leaf.label}`;
    b.addEventListener("click", () => {
      leaf.onSelect();
      closeDom();
    });
    menu.appendChild(b);
  };

  for (const sec of buildSections(ctx)) {
    const h = document.createElement("div");
    h.className = "ctx-head";
    h.textContent = sec.label;
    menu.appendChild(h);
    for (const leaf of sec.items) addLeaf(leaf, true);
  }

  const sep = document.createElement("div");
  sep.className = "ctx-sep";
  menu.appendChild(sep);
  addLeaf({ label: t("menu.settings"), onSelect: ctx.openSettings });
  addLeaf({ label: t("menu.hide"), onSelect: ctx.hide });

  // Anchor at the cursor, then nudge back inside the viewport.
  menu.style.left = `${ctx.x}px`;
  menu.style.top = `${ctx.y}px`;
  document.body.appendChild(menu);
  const r = menu.getBoundingClientRect();
  if (r.right > innerWidth) menu.style.left = `${Math.max(0, innerWidth - r.width)}px`;
  if (r.bottom > innerHeight) menu.style.top = `${Math.max(0, innerHeight - r.height)}px`;

  openDom = menu;
  document.addEventListener("pointerdown", onOutside, true);
  window.addEventListener("blur", closeDom);
}

/**
 * Show the island context menu. Native OS menu under Tauri; DOM fallback in the
 * browser preview. Any failure in the native path (permission gaps, API drift)
 * degrades to the DOM menu rather than leaving the right-click dead.
 */
export async function showIslandMenu(ctx: MenuCtx): Promise<void> {
  if (isTauri()) {
    try {
      await showNative(ctx);
      return;
    } catch {
      /* fall through to the DOM menu */
    }
  }
  showDom(ctx);
}
