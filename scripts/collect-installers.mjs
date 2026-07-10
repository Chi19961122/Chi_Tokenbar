// Copy build artifacts from Tauri's deep bundle path into a sibling folder
// OUTSIDE the repo (../TokenBar-release), so build output never clutters the
// project tree or git. Run via `npm run build:release` (or standalone after
// any `npm run tauri build`).

import { cpSync, mkdirSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const rel = (...p) => join(root, ...p);

const bundleDir = rel("src-tauri", "target", "release", "bundle");
// Sibling of the project (e.g. C:\Coding\TokenBar-release); namespaced so it
// won't collide with other projects under the same parent directory.
const outDir = join(root, "..", "TokenBar-release");
const outName = "TokenBar-release";
mkdirSync(outDir, { recursive: true });

const picks = [
  { dir: join(bundleDir, "nsis"), ext: ".exe" }, // 安裝版（推薦）
  { dir: join(bundleDir, "msi"), ext: ".msi" }, // MSI
];

let copied = 0;
for (const { dir, ext } of picks) {
  let files;
  try {
    files = readdirSync(dir).filter((f) => f.toLowerCase().endsWith(ext));
  } catch {
    continue; // bundle type not built
  }
  for (const f of files) {
    cpSync(join(dir, f), join(outDir, f));
    console.log(`${outName}/${f}  (${(statSync(join(dir, f)).size / 1048576).toFixed(1)} MB)`);
    copied++;
  }
}

// 免安裝版
try {
  const exe = rel("src-tauri", "target", "release", "tokenbar.exe");
  const portable = join(outDir, "TokenBar-portable.exe");
  cpSync(exe, portable);
  console.log(`${outName}/TokenBar-portable.exe  (${(statSync(portable).size / 1048576).toFixed(1)} MB)`);
  copied++;
} catch {
  /* portable exe not built */
}

if (copied === 0) {
  console.error("找不到任何打包產物 — 先跑 npm run tauri build");
  process.exit(1);
}
