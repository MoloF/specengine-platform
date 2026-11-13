import type { Ring, RingSector } from "../api/types";
import { ROLLUP_WORDS } from "../flow/labels";
import { middleAngle, nearestSector } from "./geometry";

// What the Rings view filters, over the served answer alone
// (docs/features/scope-rings.md "Acceptance criteria", SR7): a search dims every sector but the
// matches, a state chip released dims its state's sectors; nothing is hidden from the picture or
// moved in it. A state is compared only for its place in ROLLUP_WORDS (src/flow/labels.ts), never
// by its word here; a state this build does not know comes last, in the order the answer first
// gives it.

/** Text as filters compare it (`ui/README.md` "Screen rules", Text). */
export function folded(text: string): string {
  return text.normalize("NFC").toLowerCase();
}

/** Whether a sector matches the search: its title or its name holds it; an empty search matches all. */
export function matches(sector: RingSector, query: string): boolean {
  const wanted = folded(query.trim());
  return wanted === "" || folded(sector.title ?? "").includes(wanted) || folded(sector.name).includes(wanted);
}

/** One state present in the answer, with how many sectors stand in it. */
export interface StateCount {
  state: string;
  count: number;
}

/** Each state the answer holds, in ROLLUP_WORDS' order, an unknown one after them in the order first served. */
export function statesOf(rings: readonly Ring[]): StateCount[] {
  const counts = new Map<string, number>();
  for (const ring of rings) {
    for (const sector of ring.sectors) {
      counts.set(sector.state, (counts.get(sector.state) ?? 0) + 1);
    }
  }
  const rank = (state: string) => {
    const known = ROLLUP_WORDS.indexOf(state);
    return known < 0 ? ROLLUP_WORDS.length : known;
  };
  return [...counts]
    .map(([state, count]) => ({ state, count }))
    .sort((a, b) => rank(a.state) - rank(b.state));
}

/** Whether a sector is drawn dimmed: the search misses it, or its state's chip is released. */
export function isDimmed(sector: RingSector, query: string, released: ReadonlySet<string>): boolean {
  return released.has(sector.state) || !matches(sector, query);
}

/** A ring's filters as its config writes them, the config's own words: `root <REF> * kinds <a, b>`. */
export function ringWhere(ring: Ring): string {
  const parts: string[] = [];
  if (ring.root !== null) {
    parts.push(`root ${ring.root}`);
  }
  if (ring.kinds !== null) {
    parts.push(`kinds ${ring.kinds.join(", ")}`);
  }
  return parts.join(" \u00b7 ");
}

/** Where a sector stands: its ring and its place in that ring's served order. */
export interface SectorAt {
  ring: number;
  index: number;
}

/** The first sector of the answer: the first of the first ring that holds one; none when no ring does. */
export function firstSector(rings: readonly Ring[]): SectorAt | null {
  const ring = rings.findIndex((candidate) => candidate.sectors.length > 0);
  return ring < 0 ? null : { ring, index: 0 };
}

/** The sector at a place, when the answer has one there. */
export function sectorAt(rings: readonly Ring[], at: SectorAt | null): RingSector | undefined {
  return at === null ? undefined : rings[at.ring]?.sectors[at.index];
}

/** The keys that walk the rings, and only them. */
export type WalkKey = "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" | "Home" | "End";

export function isWalkKey(key: string): key is WalkKey {
  return key === "ArrowLeft" || key === "ArrowRight" || key === "ArrowUp" || key === "ArrowDown" || key === "Home" || key === "End";
}

/**
 * Where a key goes from `at`: Left and Right along the ring's served order, round the circle; Home
 * and End to the ring's first and last; Down to the next ring out and Up to the next ring in that
 * holds a sector, onto its sector nearest in angle. Where there is nowhere to go, `at` itself.
 */
export function walk(rings: readonly Ring[], at: SectorAt, key: WalkKey): SectorAt {
  const count = rings[at.ring]?.sectors.length ?? 0;
  if (count === 0) {
    return at;
  }
  switch (key) {
    case "ArrowRight":
      return { ring: at.ring, index: (at.index + 1) % count };
    case "ArrowLeft":
      return { ring: at.ring, index: (at.index - 1 + count) % count };
    case "Home":
      return { ring: at.ring, index: 0 };
    case "End":
      return { ring: at.ring, index: count - 1 };
    case "ArrowDown":
    case "ArrowUp": {
      const step = key === "ArrowDown" ? 1 : -1;
      for (let ring = at.ring + step; ring >= 0 && ring < rings.length; ring += step) {
        const held = rings[ring]?.sectors.length ?? 0;
        if (held > 0) {
          return { ring, index: nearestSector(middleAngle(at.index, count), held) };
        }
      }
      return at;
    }
  }
}
