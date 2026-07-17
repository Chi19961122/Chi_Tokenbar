> **TokenBar 落地約束（六方向共用，選型時一併考慮）**
> 1. 字體一律本地 subset/bundle（同 Geist 現行做法），**禁止 runtime 外連 Google Fonts**（常駐桌面 app、離線可用）。
> 2. Island pill `--island-*` 固定配色、三發行版外觀一致（CONFIG.md §7）——換皮先只動面板/選單/戰報；若方向要求動 island 需另行拍板。
> 3. 面板 380px 寬、`#analytics` 300px 高度契約不變；狀態色 safe/near/locked/degraded/stale 語意必須保留可辨識；% left 油量隱喻不變。
> 4. 定位：C 儀表硬體 — 唯二 light mode。工業 slate + 庫存綠，Soft UI 陰影，像一顆精密電表。

## Design System: TokenBar C 儀表硬體

### Design Dials
- **Variance:** 5/10 — Balanced / Modern
- **Motion:** 4/10 — Standard
- **Density:** 8/10 — Dense / Dashboard

### Pattern
- **Name:** Interactive Product Demo + Social Proof
- **CTA Placement:** Above fold
- **Sections:** Hero > Features > CTA

### Style
- **Name:** Soft UI Evolution
- **Mode Support:** Light ✓ Full | Dark ✓ Full
- **Keywords:** Evolved soft UI, better contrast, modern aesthetics, subtle depth, accessibility-focused, improved shadows, hybrid
- **Best For:** Modern enterprise apps, SaaS platforms, health/wellness, modern business tools, professional, hybrid
- **Performance:** ⚡ Excellent | **Accessibility:** ✓ WCAG AA+

### Colors
| Role | Hex | CSS Variable |
|------|-----|--------------|
| Primary | `#334155` | `--color-primary` |
| On Primary | `#FFFFFF` | `--color-on-primary` |
| Secondary | `#475569` | `--color-secondary` |
| Accent/CTA | `#059669` | `--color-accent` |
| Background | `#F8FAFC` | `--color-background` |
| Foreground | `#0F172A` | `--color-foreground` |
| Muted | `#F2F3F4` | `--color-muted` |
| Border | `#E6E8EA` | `--color-border` |
| Destructive | `#DC2626` | `--color-destructive` |
| Ring | `#334155` | `--color-ring` |

*Notes: Industrial slate + stock green（修正：原自動配對誤入「樂器學習」紅色系，改用工業儀表 slate+綠，light mode 精密感）*

### Typography
- **Heading:** Inter
- **Body:** Inter
- **Mood:** dark, cinematic, technical, precision, clean, premium, developer, professional, high-end utility
- **Best For:** Developer tools, fintech/trading, AI dashboards, streaming platforms, high-end productivity apps
- **Google Fonts:** https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&display=swap
- **CSS Import:**
```css
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&display=swap');
```

### Key Effects
Improved shadows (softer than flat, clearer than neumorphism), modern (200-300ms), focus visible, WCAG AA/AAA

### Motion
**Stagger List** (Standard) — Trigger: load or scroll | Duration: 300-450ms | Easing: `back.out(1.4)`
```js
gsap.from('.grid-item', { opacity: 0, scale: 0.92, y: 16, duration: 0.4, stagger: { each: 0.06, from: 'start', grid: 'auto' }, ease: 'back.out(1.4)' });
```
*Framework notes: grid: 'auto' lets GSAP infer rows/columns from a CSS grid layout for a natural wave stagger*
- ✅ Combine with from: 'center' for a bento-grid layout to draw the eye inward first
- ❌ Don't use back.out on dense data tables; the overshoot reads as sloppy on informational UI

### Avoid (Anti-patterns)
- Muted colors
- Low energy

### Pre-Delivery Checklist
- [ ] No emojis as icons (use SVG: Heroicons/Lucide)
- [ ] cursor-pointer on all clickable elements
- [ ] Hover states with smooth transitions (150-300ms)
- [ ] Light mode: text contrast 4.5:1 minimum
- [ ] Focus states visible for keyboard nav
- [ ] prefers-reduced-motion respected
- [ ] Responsive: 375px, 768px, 1024px, 1440px

