// Generate the local Playfair Display ROMAN (upright) subset used by the share
// cards' serif masthead and hero numbers (statement / wa, T-915 redesign). The
// existing italic subset (gen-playfair-subset.mjs) stays for the panel's gauge
// label; this adds the upright weights the redesign leans on. The generated
// woff2 is committed, so runtime has no font network dependency (offline export).
//
// Run: npm run gen:playfair-roman
// Source: google/fonts, Playfair Display roman variable font (wght 400–900)
// License: OFL-1.1 (google/fonts/ofl/playfairdisplay/OFL.txt)

import { writeFileSync, mkdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");
const FONT_URL =
  "https://github.com/google/fonts/raw/main/ofl/playfairdisplay/PlayfairDisplay%5Bwght%5D.ttf";
const OUT = join(ROOT, "public", "fonts", "playfair_roman_sub.woff2");

// Full printable ASCII (letters, digits, common punctuation) + the extra marks
// the cards render in serif: en/em dash, middle dot, ellipsis, curly quotes.
const ASCII = Array.from({ length: 0x7e - 0x20 + 1 }, (_, i) =>
  String.fromCharCode(0x20 + i),
).join("");
const EXTRA = "–—·…‘’“”€";
const GLYPHS = ASCII + EXTRA;

async function main() {
  console.log(`[gen-playfair-roman] glyphs: ${[...new Set(GLYPHS)].length}`);
  console.log(`[gen-playfair-roman] downloading ${FONT_URL}`);
  const res = await fetch(FONT_URL);
  if (!res.ok) throw new Error(`download failed: HTTP ${res.status}`);
  const srcFont = Buffer.from(await res.arrayBuffer());
  console.log(`[gen-playfair-roman] source font: ${(srcFont.length / 1024).toFixed(0)} KB`);

  const { default: subsetFont } = await import("subset-font");
  // Keep the weight axis (400–900) so a single @font-face can serve every weight
  // the cards use (masthead 800, numbers 800, period 500…).
  const out = await subsetFont(srcFont, GLYPHS, {
    targetFormat: "woff2",
    variationAxes: { wght: { min: 400, max: 900 } },
  });

  mkdirSync(dirname(OUT), { recursive: true });
  writeFileSync(OUT, out);
  const kb = statSync(OUT).size / 1024;
  console.log(`[gen-playfair-roman] wrote ${OUT} (${kb.toFixed(1)} KB)`);
}

main().catch((error) => {
  console.error(`[gen-playfair-roman] ${error.message}`);
  process.exit(1);
});
