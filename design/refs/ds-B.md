> **TokenBar 落地約束（六方向共用，選型時一併考慮）**
> 1. 字體一律本地 subset/bundle（同 Geist 現行做法），**禁止 runtime 外連 Google Fonts**（常駐桌面 app、離線可用）。
> 2. Island pill `--island-*` 固定配色、三發行版外觀一致（CONFIG.md §7）——換皮先只動面板/選單/戰報；若方向要求動 island 需另行拍板。
> 3. 面板 380px 寬、`#analytics` 300px 高度契約不變；狀態色 safe/near/locked/degraded/stale 語意必須保留可辨識；% left 油量隱喻不變。
> 4. 定位：B 液態玻璃 — 深靛藍 + 毛玻璃層 + ambient 光暈。最「高級感」，貼近現行 Live Island 血統的進化版。

## Design System: TokenBar B 液態玻璃

### Design Dials
- **Variance:** 6/10 — Balanced / Modern
- **Motion:** 5/10 — Standard
- **Density:** 6/10 — Standard

### Pattern
- **Name:** Video-First Hero
- **Conversion Focus:** 86% higher engagement with video. Add captions for accessibility. Compress video for performance.
- **CTA Placement:** Overlay on video (center/bottom) + Bottom section
- **Color Strategy:** Dark overlay 60% on video. Brand accent for CTA. White text on dark.
- **Sections:** 1. Hero with video background, 2. Key features overlay, 3. Benefits section, 4. CTA

### Style
- **Name:** Modern Dark (Cinema Mobile)
- **Mode Support:** Light ✓ Light mode only as exception | Dark ✓ Dark Mode Primary
- **Keywords:** dark mode, cinematic, ambient light, glassmorphism, deep black, indigo, glow, blur, atmospheric, reanimated, haptic, premium, layered, frosted glass, linear gradient
- **Best For:** Developer tools, pro productivity apps, fintech/trading dashboards, media/streaming platforms, AI tool interfaces, high-end gaming companion apps
- **Performance:** ⚠ Good (blur effects require native driver) | **Accessibility:** ⚠ WCAG AA (requires careful accent contrast check)

### Colors
| Role | Hex | CSS Variable |
|------|-----|--------------|
| Primary | `#475569` | `--color-primary` |
| On Primary | `#FFFFFF` | `--color-on-primary` |
| Secondary | `#334155` | `--color-secondary` |
| Accent/CTA | `#4338CA` | `--color-accent` |
| Background | `#0F172A` | `--color-background` |
| Foreground | `#FFFFFF` | `--color-foreground` |
| Muted | `#131B2F` | `--color-muted` |
| Border | `rgba(255,255,255,0.08)` | `--color-border` |
| Destructive | `#DC2626` | `--color-destructive` |
| Ring | `#475569` | `--color-ring` |

*Notes: Ambient grey + deep indigo on dark*

### Typography（修正：原自動配對 Varela Round 不合 cinematic 調性，改用資料庫「Modern Dark Cinema (Inter System)」）
- **Heading:** Inter 600（-0.5 tracking）
- **Body:** Inter 400；標籤 Inter 500 uppercase +1.2 tracking
- **Mood:** dark, cinematic, technical, precision, clean, premium, high-end utility
- **Best For:** Developer tools, fintech/trading, AI dashboards, high-end productivity apps
- **落地:** 本地 subset/bundle（同現行 Geist 做法），不外連 Google Fonts

### Key Effects
Expo.out Bezier(0.16,1,0.3,1) easing; spring modals (damping:20 stiffness:90); haptic-linked press (Impact Light/Medium); animated ambient light blobs (Reanimated translateX/Y slow oscillation); BlurView glassmorphism headers/nav (intensity 20); scale press 0.97 → 1.0; avoid pure #000000 (OLED smear)

### Motion
**Page Transition** (Standard) — Trigger: route change | Duration: 400-600ms | Easing: `power2.inOut`
```js
const tl = gsap.timeline(); tl.to('.transition-overlay', { yPercent: 0, duration: 0.4, ease: 'power2.inOut' }).call(navigate).to('.transition-overlay', { yPercent: -100, duration: 0.4, ease: 'power2.inOut', delay: 0.1 });
```
*Framework notes: Keep the overlay element mounted at the layout root (outside the page component) so it survives the route swap*
- ✅ Show a lightweight loading indicator if the destination route's data fetch outlasts the overlay
- ❌ Don't tie the overlay's reveal directly to data-fetch completion without a max-wait timeout; a slow API stalls the whole transition

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

