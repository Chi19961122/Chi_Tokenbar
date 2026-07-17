# Stitch 出圖提示 — 六方向（每段獨立可貼）

> 用法：一次貼一段進 Stitch（建議每方向開新專案或新 chat，避免風格互相污染）。
> 出圖存檔：`design/refs/direction-A.png` ~ `direction-F.png`（每方向取最好的一張；多張可加 `-2`）。
> 選定方向後：把該方向在 Stitch 的匯出碼（HTML/CSS）放進 `design/stitch/export/`。
> 共同鐵則已寫在每段 prompt 內：沿用現有結構、不發明功能、bars 填「剩餘」not「已用」。

---

## 共同畫面規格（已內嵌每段，這裡僅備查）

380px 寬桌面懸浮面板：頂部 island 膠囊（% left + 剩餘時間）→ Limits 區（Claude Code / Codex 卡片，各有 5h 視窗與週上限兩條 gauge，顯示剩餘%與重置倒數）→ Usage 區（30 日堆疊長條圖、GitHub 式日曆熱力圖、活動類型 donut、專案排名橫條、est. cost / peak day / streak 統計 tiles）→ 底部設定與 share。狀態色：綠=safe、琥珀=near、紅=locked、灰=stale。

---

## A 終端駕駛艙

```
Design a desktop overlay panel UI, 380px wide, for "TokenBar" — a Windows always-on monitor showing how much AI coding quota is LEFT (fuel metaphor: gauges fill with remaining, not used). Keep this exact structure, do not invent new features:
1) Top: compact status pill "62% left · 2h 10m" with tiny app glyph.
2) "Limits" section: two stacked cards (Claude Code, Codex), each with a 5-hour window gauge and a weekly gauge showing % remaining + reset countdown.
3) "Usage" section: 30-day stacked bar chart, GitHub-style calendar heatmap, small activity-type donut, horizontal per-project ranking bars, stat tiles (est. cost, peak day, streak).
4) Bottom row: settings gear, share button.
Status colors: green #22C55E = safe, amber = near limit, red #EF4444 = locked, grey = stale.

STYLE — "Terminal cockpit", exaggerated minimalism, extremely dense dashboard, dark only:
Background #0F172A, surface #1E293B, muted #272F42, border #475569, foreground #F8FAFC, accent (run-green) #22C55E. Everything in JetBrains Mono (headings 700 with tight -0.05em tracking, oversized numerals for the % left values). High contrast, generous negative space around one hero number per card, hairline separators, no rounded blobs (2-4px radius max), no gradients, no glow. Feels like a precision CLI instrument. Subtle 300ms fade reveals only.
```

## B 液態玻璃

```
Design a desktop overlay panel UI, 380px wide, for "TokenBar" — a Windows always-on monitor showing how much AI coding quota is LEFT (fuel metaphor: gauges fill with remaining, not used). Keep this exact structure, do not invent new features:
1) Top: compact status pill "62% left · 2h 10m" with tiny app glyph.
2) "Limits" section: two stacked cards (Claude Code, Codex), each with a 5-hour window gauge and a weekly gauge showing % remaining + reset countdown.
3) "Usage" section: 30-day stacked bar chart, GitHub-style calendar heatmap, small activity-type donut, horizontal per-project ranking bars, stat tiles (est. cost, peak day, streak).
4) Bottom row: settings gear, share button.
Status colors: green = safe, amber = near limit, red #DC2626 = locked, grey = stale.

STYLE — "Liquid glass", modern dark cinema, premium ambient:
Deep navy background #0F172A (never pure black), frosted-glass cards (blur, rgba(255,255,255,0.08) hairline borders), soft ambient indigo light blobs glowing behind the panel, accent deep indigo #4338CA, foreground #FFFFFF, muted surface #131B2F. Typography: Inter precision system — 600 headings with -0.5 tracking, 400 body, 500 uppercase labels with +1.2 tracking; gradient text (white → 70% white) on the hero % number. Layered depth, gentle 400ms transitions, spring-like softness, everything atmospheric and premium like a high-end trading app.
```

## C 儀表硬體

```
Design a desktop overlay panel UI, 380px wide, for "TokenBar" — a Windows always-on monitor showing how much AI coding quota is LEFT (fuel metaphor: gauges fill with remaining, not used). Keep this exact structure, do not invent new features:
1) Top: compact status pill "62% left · 2h 10m" with tiny app glyph.
2) "Limits" section: two stacked cards (Claude Code, Codex), each with a 5-hour window gauge and a weekly gauge showing % remaining + reset countdown.
3) "Usage" section: 30-day stacked bar chart, GitHub-style calendar heatmap, small activity-type donut, horizontal per-project ranking bars, stat tiles (est. cost, peak day, streak).
4) Bottom row: settings gear, share button.
Status colors: green = safe, amber = near limit, red #DC2626 = locked, grey = stale.

STYLE — "Precision instrument", light industrial, soft-UI evolution, dense:
Light background #F8FAFC, white cards with soft realistic shadows (subtle depth, not neumorphism), industrial slate primary #334155, stock-green accent #059669, borders #E6E8EA, foreground #0F172A. Typography: Inter, tabular numerals everywhere. Gauges look like calibrated hardware meters: tick marks, thin needles or notched progress tracks, engraved-looking labels. Feels like a precision multimeter / lab instrument. Restrained 200-300ms motion, WCAG AA+ contrast.
```

## D 極簡編輯部

```
Design a desktop overlay panel UI, 380px wide, for "TokenBar" — a Windows always-on monitor showing how much AI coding quota is LEFT (fuel metaphor: gauges fill with remaining, not used). Keep this exact structure, do not invent new features:
1) Top: compact status pill "62% left · 2h 10m" with tiny app glyph.
2) "Limits" section: two stacked cards (Claude Code, Codex), each with a 5-hour window gauge and a weekly gauge showing % remaining + reset countdown.
3) "Usage" section: 30-day stacked bar chart, GitHub-style calendar heatmap, small activity-type donut, horizontal per-project ranking bars, stat tiles (est. cost, peak day, streak).
4) Bottom row: settings gear, share button.
Status colors: green = safe, amber = near limit, red #DC2626 = locked, grey = stale.

STYLE — "Editorial minimal", light, type-as-hero, spacious:
Background #FAFAFA, near-black ink #09090B, editorial black primary #18181B, single pink accent #EC4899 used sparingly (one accent per view), borders #E4E4E7. The remaining-% numbers are typeset like magazine cover headlines: huge Inter 800 numerals, tight tracking, with occasional Playfair Display italic for small editorial labels. Single-column rhythm, massive whitespace, thin rules between sections, charts reduced to elegant minimal marks (no chart chrome). Feels like a beautifully typeset print page that happens to be live data. Motion nearly none — subtle fades only.
```

## E 霓虹 HUD

```
Design a desktop overlay panel UI, 380px wide, for "TokenBar" — a Windows always-on monitor showing how much AI coding quota is LEFT (fuel metaphor: gauges fill with remaining, not used). Keep this exact structure, do not invent new features:
1) Top: compact status pill "62% left · 2h 10m" with tiny app glyph.
2) "Limits" section: two stacked cards (Claude Code, Codex), each with a 5-hour window gauge and a weekly gauge showing % remaining + reset countdown.
3) "Usage" section: 30-day stacked bar chart, GitHub-style calendar heatmap, small activity-type donut, horizontal per-project ranking bars, stat tiles (est. cost, peak day, streak).
4) Bottom row: settings gear, share button.
Status colors: green = safe, amber = near limit, red #EF4444 = locked, grey = stale.

STYLE — "Neon HUD", retro-futurism, gaming cockpit, dark, dense, asymmetric:
Background #0F0F23, neon purple primary #7C3AED with glow (text-shadow + box-shadow), secondary #A78BFA, rose action accent #F43F5E, borders #4C1D95, subtle CRT scanline overlay, chamfered card corners. Typography: Orbitron 700/900 for headings and hero numerals, JetBrains Mono for data. Gauges styled as sci-fi energy bars with segment ticks; heatmap cells glow by intensity. Tactical, synthwave, like a game companion overlay — loud but readable, keep body text clean mono without glow.
```

## F 粗獷復古

```
Design a desktop overlay panel UI, 380px wide, for "TokenBar" — a Windows always-on monitor showing how much AI coding quota is LEFT (fuel metaphor: gauges fill with remaining, not used). Keep this exact structure, do not invent new features:
1) Top: compact status pill "62% left · 2h 10m" with tiny app glyph.
2) "Limits" section: two stacked cards (Claude Code, Codex), each with a 5-hour window gauge and a weekly gauge showing % remaining + reset countdown.
3) "Usage" section: 30-day stacked bar chart, GitHub-style calendar heatmap, small activity-type donut, horizontal per-project ranking bars, stat tiles (est. cost, peak day, streak).
4) Bottom row: settings gear, share button.
Status colors: green #22C55E = safe, amber = near limit, red #DC2626 = locked, grey = stale.

STYLE — "Brutalist utility", raw, anti-design, dark, high contrast:
Background #0F172A, pure white text #FFFFFF, primary red #DC2626 and secondary blue #2563EB used as flat solid blocks, score-green accent #22C55E, visible 1px borders on EVERYTHING, 0px border radius, exposed grid structure, no shadows, no gradients, no transitions (instant state changes). All text Space Mono 400/700, labels in raw uppercase like stamped machine text. Charts drawn as blunt solid rectangles with visible axis lines. Honest, stark, like industrial signage or a punch-card report. WCAG AAA contrast.
```
