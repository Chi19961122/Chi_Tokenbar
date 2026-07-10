// Provider brand icons (island pill, panel headers), bundled locally from
// lobehub/lobe-icons v1.91.0 (MIT) — src/assets/*.svg. No CDN: the app must
// render offline. Codex's white tile background was stripped to sit on our
// dark pill; Claude's fill (#D97757) matches --claude.

import claudeRaw from "./assets/claude-color.svg?raw";
import codexRaw from "./assets/codex-color.svg?raw";

export type ProviderKey = "anthropic" | "codex";

// SVG paint servers (gradients) are looked up by document-global id, and defs
// inside a display:none subtree don't render. The island and the panel each
// embed a copy of the Codex icon, and the collapsed island is hidden while the
// panel shows — so a shared id would resolve to the hidden copy and the
// gradient would vanish. Give every instance its own id.
let instance = 0;

/** The lobe SVGs are 1em-sized; pin them to `size` px, tag with .picon,
 *  and uniquify internal ids (gradients) per instance. */
function sized(raw: string, size: number): string {
  const uid = ++instance;
  return raw
    .replace("<svg ", `<svg class="picon" `)
    .replace('height="1em"', `height="${size}"`)
    .replace('width="1em"', `width="${size}"`)
    .replace(/lobe-icons-[\w-]+/g, (id) => `${id}-i${uid}`);
}

export function providerIcon(p: ProviderKey, size = 12): string {
  return sized(p === "anthropic" ? claudeRaw : codexRaw, size);
}
