// Display formatting helpers.

export const nowSecs = () => Math.floor(Date.now() / 1000);

/** Compact duration: "2d 4h", "3h 12m", "25m", "45s". */
export function fmtDur(secs: number): string {
  if (secs < 0) secs = 0;
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h >= 24) {
    const d = Math.floor(h / 24);
    return `${d}d ${h % 24}h`;
  }
  if (h >= 1) return m > 0 ? `${h}h ${m}m` : `${h}h`;
  if (m >= 1) return `${m}m`;
  return `${Math.floor(secs)}s`;
}

/** Absolute clock for "resets by ~3:40 PM". */
export function fmtClock(epochSecs: number): string {
  const d = new Date(epochSecs * 1000);
  return d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

/** 24h "HH:MM" for the header Resets readout. */
export function fmtHM(epochSecs: number): string {
  const d = new Date(epochSecs * 1000);
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
}

/** 1_234_567 -> "1.2M", 12_300 -> "12.3K". */
export function fmtTokens(n: number): string {
  if (n >= 1e9) return `${(n / 1e9).toFixed(2)}B`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)}M`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}K`;
  return `${Math.round(n)}`;
}

export function fmtUsd(n: number): string {
  if (n >= 1000) return `$${(n / 1000).toFixed(2)}K`;
  return `$${n.toFixed(2)}`;
}

export const pctLeft = (util: number) => Math.max(0, Math.round(100 - util));
