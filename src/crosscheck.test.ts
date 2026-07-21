// T-test-001 — TS side of the shared Rust<->TS crosscheck.
//
// Loads the SAME neutral fixture the Rust suite reads
// (fixtures/crosscheck-v1.json at the repo root) and drives each scenario
// through the *real* frontend decision functions — pickIslandLimit, islandText,
// fmtResetRel, fmtResetClock — asserting against expect.frontend. The backend
// loads the same cases and asserts expect.backend, so a lone edit to either
// end's logic that drifts from the golden fixture turns at least one suite red.
//
// Time is relative: the fixture stores resets_in_secs / sample offsets as
// seconds from "now". This end pins its own fixed fake now anchored to a LOCAL
// wall-clock instant (so fmtResetClock's day markers are deterministic across
// the runner's timezone, exactly like format.test.ts); the Rust end uses a
// different, arbitrary epoch. No absolute timestamps live in the file.
//
// vitest.config.ts is untouched: the fixture sits outside src/, but tsconfig has
// resolveJsonModule, so it is imported directly (the same single file the Rust
// suite reads from disk) rather than pulling in @types/node for fs.

import { describe, expect, it } from "vitest";
import fixtureJson from "../fixtures/crosscheck-v1.json";
import { islandText, pickIslandLimit } from "./island";
import { fmtResetClock, fmtResetRel } from "./format";
import type { Limit } from "./types";
import type { Locale } from "./i18n";

// This end's fixed fake now: Thu 15 Jan 2026, 10:00 local (Jan 18 2026 is a
// Sunday). Built with the local Date constructor so getHours()/getDay() inside
// fmtResetClock match the golden strings regardless of the runner's timezone.
const NOW = Math.floor(new Date(2026, 0, 15, 10, 0, 0).getTime() / 1000);

interface Fixture {
  version: number;
  cases: Case[];
}
interface Case {
  name: string;
  note?: string;
  input: {
    subject: { resets_in_secs: number };
    island: { provider: "anthropic" | "codex"; pin: string; limits: IslandLimitSpec[] };
  };
  expect: {
    frontend: {
      pick: string | null;
      islandText: string | null;
      resetRel: string | null;
      resetClock: { en: string; "zh-TW": string } | null;
    };
  };
}
interface IslandLimitSpec {
  id: string;
  provider: "anthropic" | "codex";
  label: string;
  util: number;
  status: Limit["status"];
}

/** Flesh a fixture island spec out into the full Limit shape the frontend
 *  functions consume. Fields the island decisions don't read (resets_at, pace,
 *  …) get inert placeholders — the reset formatting is driven separately from
 *  subject.resets_in_secs, mirroring how the real app renders resets from the
 *  same window it picked. */
function toLimit(spec: IslandLimitSpec): Limit {
  return {
    id: spec.id,
    provider: spec.provider,
    label: spec.label,
    util: spec.util,
    resets_at: 0,
    window_secs: 0,
    status: spec.status,
    absolute: null,
    pace: null,
    runway_secs: null,
  };
}

const fixture = fixtureJson as unknown as Fixture;

describe("crosscheck fixture — 前端與後端共用同一份 fixture", () => {
  it("fixture 版本與案例數符合契約(>=12 案)", () => {
    expect(fixture.version).toBe(1);
    expect(fixture.cases.length).toBeGreaterThanOrEqual(12);
  });

  for (const c of fixture.cases) {
    it(`[${c.name}] 前端決策與格式化對上 golden`, () => {
      const fe = c.expect.frontend;
      const limits = c.input.island.limits.map(toLimit);

      // ── pickIslandLimit: which limit (or an honest blank) ──────────────
      const picked = pickIslandLimit(limits, c.input.island.provider, c.input.island.pin);
      if (fe.pick === null) {
        // v0.3.0 iron rule: a pin with no matching data resolves to null (the
        // caller renders "—"), never a silent fallback to auto.
        expect(picked, `[${c.name}] pin 無資料必須回 null(絕不退 auto)`).toBeNull();
      } else {
        expect(picked, `[${c.name}] pickIslandLimit 回 null,期望 ${fe.pick}`).not.toBeNull();
        expect(picked!.id, `[${c.name}] pickIslandLimit 選錯限額`).toBe(fe.pick);

        // ── islandText: verbatim pill text for the picked limit ──────────
        if (fe.islandText !== null) {
          expect(islandText(picked!), `[${c.name}] islandText 逐字不符`).toBe(fe.islandText);
        }
      }

      // ── fmtResetRel: countdown (locale-independent) ────────────────────
      const resetsAt = NOW + c.input.subject.resets_in_secs;
      if (fe.resetRel !== null) {
        expect(fmtResetRel(resetsAt, NOW), `[${c.name}] fmtResetRel 逐字不符`).toBe(fe.resetRel);
      }

      // ── fmtResetClock: wall-clock + day marker, zh-TW 與 en 各一 ────────
      if (fe.resetClock !== null) {
        for (const locale of ["en", "zh-TW"] as Locale[]) {
          expect(
            fmtResetClock(resetsAt, NOW, locale),
            `[${c.name}] fmtResetClock(${locale}) 逐字不符`,
          ).toBe(fe.resetClock[locale]);
        }
      }
    });
  }
});
