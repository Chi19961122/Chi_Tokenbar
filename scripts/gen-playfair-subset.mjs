// Generate the local Playfair Display Italic 400 subset used by the panel's
// editorial labels. The generated woff2 is committed, so runtime has no font
// network dependency.
//
// Run: npm run gen:playfair
// Source: google/fonts, Playfair Display Italic variable font
// License: OFL-1.1 (google/fonts/ofl/playfairdisplay/OFL.txt)

import { writeFileSync, mkdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");
const FONT_URL =
  "https://github.com/google/fonts/raw/main/ofl/playfairdisplay/PlayfairDisplay-Italic%5Bwght%5D.ttf";
const OUT = join(ROOT, "public", "fonts", "playfair_italic_sub.woff2");

const GLYPHS =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 '’,.—%";

async function main() {
  console.log(`[gen-playfair] glyphs: ${[...new Set(GLYPHS)].length}`);
  console.log(`[gen-playfair] downloading ${FONT_URL}`);
  const res = await fetch(FONT_URL);
  if (!res.ok) throw new Error(`download failed: HTTP ${res.status}`);
  const srcFont = Buffer.from(await res.arrayBuffer());
  console.log(`[gen-playfair] source font: ${(srcFont.length / 1024).toFixed(0)} KB`);

  const { default: subsetFont } = await import("subset-font");
  const out = await subsetFont(srcFont, GLYPHS, { targetFormat: "woff2" });

  mkdirSync(dirname(OUT), { recursive: true });
  writeFileSync(OUT, out);
  const kb = statSync(OUT).size / 1024;
  console.log(`[gen-playfair] wrote ${OUT} (${kb.toFixed(1)} KB)`);
}

main().catch((error) => {
  console.error(`[gen-playfair] ${error.message}`);
  process.exit(1);
});
