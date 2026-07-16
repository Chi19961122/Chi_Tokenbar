// Generate a Noto Sans TC subset containing only the CJK glyphs the zh-TW UI
// actually uses, so English users never download a CJK font and zh users
// download ~tens of KB instead of megabytes (決策 D1: zh 模式載入).
//
// Pipeline:
//   1. Scrape every unique CJK / fullwidth code point out of src/i18n.ts
//      (the zh-TW dictionary — plus Chinese comments, harmless if included),
//      then add date glyphs 階段 B's clock format will need (明/週/日 + 一..六).
//   2. Download the Noto Sans TC variable font (Traditional Chinese subset)
//      from Fontsource on unpkg — SIL Open Font License 1.1.
//   3. Re-subset it to just those glyphs with `subset-font` (harfbuzz wasm,
//      no Python) and write public/fonts/noto_tc_sub.woff2.
//
// Run: npm run gen:noto   (commit the resulting .woff2 into the repo)
//
// Source font: google/fonts — the single-file Noto Sans TC variable font
//              (Fontsource ships the CJK range pre-split into ~120 partial
//              files, which can't be re-subset as one whole.)
// License:     OFL-1.1 (google/fonts ofl/notosanstc/OFL.txt)

import { readFileSync, writeFileSync, mkdirSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");

const FONT_URL =
  "https://github.com/google/fonts/raw/main/ofl/notosanstc/NotoSansTC%5Bwght%5D.ttf";
const OUT = join(ROOT, "public", "fonts", "noto_tc_sub.woff2");

// CJK ideographs + CJK symbols/punctuation + fullwidth forms.
const CJK_RE = /[　-〿一-鿿＀-￯]/g;

// Date glyphs the 階段 B clock format ({reset} 時刻,跨日補日標)will need but
// which may not yet appear in the dictionary. Seeded now so the subset is
// stable across階段 B without regenerating.
const DATE_GLYPHS = "明週日一二三四五六";

function collectChars() {
  const src = readFileSync(join(ROOT, "src", "i18n.ts"), "utf8");
  const set = new Set();
  for (const ch of src.match(CJK_RE) ?? []) set.add(ch);
  for (const ch of DATE_GLYPHS) set.add(ch);
  return [...set].sort().join("");
}

async function main() {
  const text = collectChars();
  console.log(`[gen-noto] unique CJK/fullwidth glyphs: ${[...text].length}`);

  console.log(`[gen-noto] downloading ${FONT_URL}`);
  const res = await fetch(FONT_URL);
  if (!res.ok) throw new Error(`download failed: HTTP ${res.status}`);
  const srcFont = Buffer.from(await res.arrayBuffer());
  console.log(`[gen-noto] source font: ${(srcFont.length / 1024).toFixed(0)} KB`);

  // Lazy import so `--help` / char-count runs don't require the dep installed.
  const { default: subsetFont } = await import("subset-font");
  const out = await subsetFont(srcFont, text, { targetFormat: "woff2" });

  mkdirSync(dirname(OUT), { recursive: true });
  writeFileSync(OUT, out);
  const kb = statSync(OUT).size / 1024;
  console.log(`[gen-noto] wrote ${OUT} (${kb.toFixed(1)} KB)`);
  if (kb > 100) console.warn(`[gen-noto] WARNING: subset exceeds 100 KB target`);
}

main().catch((e) => {
  console.error(`[gen-noto] ${e.message}`);
  process.exit(1);
});
