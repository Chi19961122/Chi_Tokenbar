// T-901 亮暗雙主題 — theme resolution + application.
//
// The `.dark` class is toggled on <html> (document.documentElement); styles.css
// carries a full `:root.dark` token override set. Kept tiny and mostly pure so
// the resolution rule is unit-testable without a DOM.

/**
 * Resolve the concrete "is dark?" boolean from the stored theme setting and the
 * current OS preference.
 *
 *   dark  ⇔  theme === "dark"  ||  (theme !== "light" && systemDark)
 *
 * i.e. an explicit "dark" always wins, an explicit "light" always wins, and
 * anything else ("system" or junk) follows the OS. Pure — no DOM/matchMedia.
 */
export function resolveThemeDark(theme: string, systemDark: boolean): boolean {
  return theme === "dark" || (theme !== "light" && systemDark);
}

/** Read the OS `prefers-color-scheme` once (false when matchMedia is absent). */
export function systemPrefersDark(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches
  );
}

/** Toggle the `.dark` class on <html> for the given theme setting. Also sets a
 *  matching `color-scheme` so native scrollbars/inputs follow (styles.css sets
 *  it too via the class, this is a belt-and-braces for the earliest paint). */
export function applyTheme(theme: string): void {
  if (typeof document === "undefined") return;
  const dark = resolveThemeDark(theme, systemPrefersDark());
  document.documentElement.classList.toggle("dark", dark);
}

/**
 * Wire a `prefers-color-scheme` change listener that re-applies the theme only
 * while the setting follows the system. `getTheme` is read lazily on each change
 * so the listener always sees the current setting. Returns nothing (the app
 * lives for the process lifetime; no teardown needed).
 */
export function watchSystemTheme(getTheme: () => string): void {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return;
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  const onChange = () => {
    if (getTheme() === "system") applyTheme("system");
  };
  // addEventListener is the modern API; older WebViews only have addListener.
  if (typeof mq.addEventListener === "function") mq.addEventListener("change", onChange);
  else if (typeof mq.addListener === "function") mq.addListener(onChange);
}
