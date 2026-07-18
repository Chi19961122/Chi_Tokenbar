// Copy build artifacts from Tauri's deep bundle path into a sibling folder
// OUTSIDE the repo (../Atoll-release), so build output never clutters the
// project tree or git. Run via `npm run build:release` (or standalone after
// any `npm run tauri build`).

import { cpSync, mkdirSync, readdirSync, readFileSync, renameSync, statSync } from "node:fs";
import { join } from "node:path";

const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const rel = (...p) => join(root, ...p);

const bundleDir = rel("src-tauri", "target", "release", "bundle");
// Sibling of the repo folder (e.g. C:\Coding\TokenBar\Atoll-release, next to
// TokenBar-Src); namespaced so it won't collide with other projects.
const outDir = join(root, "..", "Atoll-release");
const outName = "Atoll-release";
mkdirSync(outDir, { recursive: true });

const version = JSON.parse(readFileSync(rel("package.json"), "utf8")).version;

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
  const exe = rel("src-tauri", "target", "release", "atoll.exe");
  const portable = join(outDir, "Atoll-portable.exe");
  cpSync(exe, portable);
  console.log(`${outName}/Atoll-portable.exe  (${(statSync(portable).size / 1048576).toFixed(1)} MB)`);
  copied++;
} catch {
  /* portable exe not built */
}

if (copied === 0) {
  console.error("找不到任何打包產物 — 先跑 npm run tauri build");
  process.exit(1);
}

// Keep only the current version's installers in the release folder; sweep any
// older versioned installer (Atoll_<x.y.z>_...) into archive/ as a backup.
// Runs AFTER copying because Tauri's bundle dir accumulates past builds, so the
// copy step above may re-introduce stale versions.
const archiveDir = join(outDir, "archive");
const versioned = /^Atoll_(\d+\.\d+\.\d+)_/;
let archived = 0;
for (const f of readdirSync(outDir)) {
  const m = f.match(versioned);
  if (m && m[1] !== version) {
    mkdirSync(archiveDir, { recursive: true });
    renameSync(join(outDir, f), join(archiveDir, f));
    console.log(`archived  ${outName}/archive/${f}`);
    archived++;
  }
}
if (archived) console.log(`(${archived} 個舊版安裝檔移入 archive/,主資料夾只留 v${version})`);
