> **TokenBar 落地約束（六方向共用，選型時一併考慮）**
> 1. 字體一律本地 subset/bundle（同 Geist 現行做法），**禁止 runtime 外連 Google Fonts**（常駐桌面 app、離線可用）。
> 2. Island pill `--island-*` 固定配色、三發行版外觀一致（CONFIG.md §7）——換皮先只動面板/選單/戰報；若方向要求動 island 需另行拍板。
> 3. 面板 380px 寬、`#analytics` 300px 高度契約不變；狀態色 safe/near/locked/degraded/stale 語意必須保留可辨識；% left 油量隱喻不變。
> 4. 定位：F 粗獷復古 — Space Mono、0 圓角、可見框線、即時切換無過渡。反設計，數據誠實感最強。

## Design System: TokenBar F 粗獷復古

### Design Dials
- **Variance:** 9/10 — Bold / Asymmetric
- **Motion:** 3/10 — Subtle
- **Density:** 7/10 — Standard

### Pattern
- **Name:** Hero + Features + CTA
- **Conversion Focus:** Deep CTA placement. Use contrasting color (at least 7:1 contrast ratio). Sticky navbar CTA.
- **CTA Placement:** Hero (sticky) + Bottom
- **Color Strategy:** Hero: Brand primary or vibrant. Features: Card bg #FAFAFA. CTA: Contrasting accent color
- **Sections:** 1. Hero with headline/image, 2. Value prop, 3. Key features (3-5), 4. CTA section, 5. Footer

### Style
- **Name:** Brutalism
- **Mode Support:** Light ✓ Full | Dark ✓ Full
- **Keywords:** Raw, unpolished, stark, high contrast, plain text, default fonts, visible borders, asymmetric, anti-design
- **Best For:** Design portfolios, artistic projects, counter-culture brands, editorial/media sites, tech blogs
- **Performance:** ⚡ Excellent | **Accessibility:** ✓ WCAG AAA

### Colors
| Role | Hex | CSS Variable |
|------|-----|--------------|
| Primary | `#DC2626` | `--color-primary` |
| On Primary | `#FFFFFF` | `--color-on-primary` |
| Secondary | `#2563EB` | `--color-secondary` |
| Accent/CTA | `#22C55E` | `--color-accent` |
| Background | `#0F172A` | `--color-background` |
| Foreground | `#FFFFFF` | `--color-foreground` |
| Muted | `#1F1829` | `--color-muted` |
| Border | `rgba(255,255,255,0.08)` | `--color-border` |
| Destructive | `#DC2626` | `--color-destructive` |
| Ring | `#DC2626` | `--color-ring` |

*Notes: Neon red+blue on dark + score green*

### Typography
- **Heading:** Space Mono
- **Body:** Space Mono
- **Mood:** brutalist, raw, technical, monospace, minimal, stark
- **Best For:** Brutalist designs, developer portfolios, experimental, tech art
- **Google Fonts:** https://fonts.googleapis.com/css2?family=Space+Mono:wght@400;700&display=swap
- **CSS Import:**
```css
@import url('https://fonts.googleapis.com/css2?family=Space+Mono:wght@400;700&display=swap');
```

### Key Effects
No smooth transitions (instant), sharp corners (0px), bold typography (700+), visible grid, large blocks

### Motion
**Scroll Reveal** (Subtle) — Trigger: scroll (viewport enter) | Duration: 300-400ms | Easing: `power1.out`
```js
gsap.from(el, { opacity: 0, y: 12, duration: 0.35, ease: 'power1.out', scrollTrigger: { trigger: el, start: 'top 90%', toggleActions: 'play none none reverse' } });
```
*Framework notes: Requires the ScrollTrigger plugin registered once via gsap.registerPlugin(ScrollTrigger)*
- ✅ Keep the y offset small (8-16px) so it reads as a fade, not a slide
- ❌ Don't reveal below-the-fold content needed for SEO/crawlers as invisible-by-default without a no-JS fallback

### Avoid (Anti-patterns)
- Inconsistent styling
- Poor contrast ratios

### Pre-Delivery Checklist
- [ ] No emojis as icons (use SVG: Heroicons/Lucide)
- [ ] cursor-pointer on all clickable elements
- [ ] Hover states with smooth transitions (150-300ms)
- [ ] Light mode: text contrast 4.5:1 minimum
- [ ] Focus states visible for keyboard nav
- [ ] prefers-reduced-motion respected
- [ ] Responsive: 375px, 768px, 1024px, 1440px

