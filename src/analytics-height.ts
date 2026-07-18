export const ANALYTICS_MIN_HEIGHT = 300;
export const ANALYTICS_MAX_HEIGHT = 640;
export const ANALYTICS_SCREEN_FLOOR = 700;

/**
 * Lock the analytics box to a screen-sized height for one Usage-mode visit.
 * Tiny/unknown screens retain the original 300px contract.
 */
export function analyticsHeight(
  availableHeight: number,
  otherPanelHeight: number,
  margin = 40,
): number {
  if (!Number.isFinite(availableHeight) || availableHeight < ANALYTICS_SCREEN_FLOOR) {
    return ANALYTICS_MIN_HEIGHT;
  }

  const budget = availableHeight - otherPanelHeight - margin;
  return Math.min(ANALYTICS_MAX_HEIGHT, Math.max(ANALYTICS_MIN_HEIGHT, budget));
}
