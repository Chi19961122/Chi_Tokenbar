/**
 * Generation-gated analytics fetch: only the newest request for a logical
 * key may commit cache writes or paint callbacks.
 */

export type CommitDecision = "commit" | "stale";

export interface AnalyticsRequestGate {
  /** Bump and return a new generation for `key`. */
  begin(key: string): number;
  /** True only if `gen` is still the latest for `key`. */
  isCurrent(key: string, gen: number): boolean;
  /**
   * Decide whether a completed fetch may write cache / paint.
   * Stale (superseded by a newer begin) → do not commit.
   */
  decide(key: string, gen: number): CommitDecision;
}

export function createAnalyticsRequestGate(): AnalyticsRequestGate {
  const gens = new Map<string, number>();
  return {
    begin(key: string): number {
      const next = (gens.get(key) ?? 0) + 1;
      gens.set(key, next);
      return next;
    },
    isCurrent(key: string, gen: number): boolean {
      return gens.get(key) === gen;
    },
    decide(key: string, gen: number): CommitDecision {
      return gens.get(key) === gen ? "commit" : "stale";
    },
  };
}
