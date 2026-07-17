> **TokenBar 落地約束（六方向共用，選型時一併考慮）**
> 1. 字體一律本地 subset/bundle（同 Geist 現行做法），**禁止 runtime 外連 Google Fonts**（常駐桌面 app、離線可用）。
> 2. Island pill `--island-*` 固定配色、三發行版外觀一致（CONFIG.md §7）——換皮先只動面板/選單/戰報；若方向要求動 island 需另行拍板。
> 3. 面板 380px 寬、`#analytics` 300px 高度契約不變；狀態色 safe/near/locked/degraded/stale 語意必須保留可辨識；% left 油量隱喻不變。
> 4. 定位：E 霓虹 HUD — 紫霓虹 + CRT 掃描線 + Orbitron。電競儀表，戰報 share 最搶眼，日常監控最吵。

## Design System: TokenBar E 霓虹 HUD

### Design Dials
- **Variance:** 8/10 — Bold / Asymmetric
- **Motion:** 7/10 — Standard
- **Density:** 8/10 — Dense / Dashboard

### Pattern
- **Name:** Trust & Authority + Conversion
- **Conversion Focus:** Security badges. Case studies. Transparent pricing. Low-friction form.
- **CTA Placement:** Contact Sales / Get Quote (primary) + Nav
- **Color Strategy:** Navy/Grey corporate. Trust blue. Accent for CTA only.
- **Sections:** 1. Hero (mission/credibility), 2. Proof (logos, certs, stats), 3. Solution overview, 4. Clear CTA path

### Style
- **Name:** Retro-Futurism
- **Mode Support:** Light ✓ Full | Dark ✓ Dark focused
- **Keywords:** Vintage sci-fi, 80s aesthetic, neon glow, geometric patterns, CRT scanlines, pixel art, cyberpunk, synthwave
- **Best For:** Gaming, entertainment, music platforms, tech brands, artistic projects, nostalgic, cyberpunk
- **Performance:** ⚠ Moderate | **Accessibility:** ⚠ High contrast/strain

### Colors
| Role | Hex | CSS Variable |
|------|-----|--------------|
| Primary | `#7C3AED` | `--color-primary` |
| On Primary | `#FFFFFF` | `--color-on-primary` |
| Secondary | `#A78BFA` | `--color-secondary` |
| Accent/CTA | `#F43F5E` | `--color-accent` |
| Background | `#0F0F23` | `--color-background` |
| Foreground | `#E2E8F0` | `--color-foreground` |
| Muted | `#27273B` | `--color-muted` |
| Border | `#4C1D95` | `--color-border` |
| Destructive | `#EF4444` | `--color-destructive` |
| Ring | `#7C3AED` | `--color-ring` |

*Notes: Neon purple + rose action*

### Typography
- **Heading:** Orbitron
- **Body:** JetBrains Mono
- **Mood:** cyberpunk, neon, glitch, hud, sci-fi, dark, matrix green, magenta, chamfered, tactical
- **Best For:** Gaming companion apps, fintech/crypto, data visualization, dark brand apps, cyberpunk narrative games
- **Google Fonts:** https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500&family=Orbitron:wght@700;900&display=swap
- **CSS Import:**
```css
@import url('https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500&family=Orbitron:wght@700;900&display=swap');
```

### Key Effects
CRT scanlines (::before overlay), neon glow (text-shadow+box-shadow), glitch effects (skew/offset keyframes)

### Motion
**Stagger List** (Standard) — Trigger: load or scroll | Duration: 300-450ms | Easing: `back.out(1.4)`
```js
gsap.from('.grid-item', { opacity: 0, scale: 0.92, y: 16, duration: 0.4, stagger: { each: 0.06, from: 'start', grid: 'auto' }, ease: 'back.out(1.4)' });
```
*Framework notes: grid: 'auto' lets GSAP infer rows/columns from a CSS grid layout for a natural wave stagger*
- ✅ Combine with from: 'center' for a bento-grid layout to draw the eye inward first
- ❌ Don't use back.out on dense data tables; the overshoot reads as sloppy on informational UI

### Avoid (Anti-patterns)
- Minimalist design
- Static assets

### Pre-Delivery Checklist
- [ ] No emojis as icons (use SVG: Heroicons/Lucide)
- [ ] cursor-pointer on all clickable elements
- [ ] Hover states with smooth transitions (150-300ms)
- [ ] Light mode: text contrast 4.5:1 minimum
- [ ] Focus states visible for keyboard nav
- [ ] prefers-reduced-motion respected
- [ ] Responsive: 375px, 768px, 1024px, 1440px

