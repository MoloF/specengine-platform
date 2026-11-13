import { describe, expect, it } from "vitest";
import type { Ring } from "../api/types";
import { aRing, aRingSector } from "../test/builders";
import {
  angleBetween,
  bandOf,
  CENTRE,
  CHAR_UNITS,
  cutToFit,
  drawnRings,
  LABEL_ARC_MIN,
  LABEL_PAD,
  labelArc,
  labelRoom,
  middleAngle,
  middleOf,
  nearestSector,
  pointAt,
  RINGS_INNER,
  RINGS_OUTER,
  sectorAngles,
  sectorPath,
  showsLabel,
  textUnits,
  WIDE_CHAR_UNITS,
} from "./geometry";

// docs/features/scope-rings.md "Acceptance criteria", SR6: the rings' geometry, pure. Three rings
// fit 460; sector k of n starts at 2*pi*k/n from 12 o'clock whatever the sectors hold; one sector is a
// full band; a 56-unit arc keeps its label and 55 drops it; one answer, one drawing.
// M: angles by title length; a label on a 40-unit arc.

const TURN = 2 * Math.PI;

/** A path's numbers, in order. */
function numbersOf(path: string): number[] {
  return Array.from(path.matchAll(/-?\d+(?:\.\d+)?/g), (match) => Number(match[0]));
}

/** A ring of `count` sectors whose titles grow longer one by one. */
function growing(count: number, title = "Ring"): Ring {
  return aRing(
    title,
    Array.from({ length: count }, (_, at) => aRingSector({ name: `S-${String(at)}`, title: "x".repeat(1 + at * 7) })),
  );
}

describe("the bands (SR6)", () => {
  it("fits three rings from 80 to 460, each band as wide as the next, the first in the middle", () => {
    const bands = [0, 1, 2].map((ring) => bandOf(ring, 3));
    expect([RINGS_INNER, RINGS_OUTER]).toEqual([80, 460]);
    expect(bands[0]?.inner).toBe(80);
    expect(bands[2]?.outer).toBeCloseTo(460, 9);
    expect(bands.map((band) => band.outer - band.inner).map((width) => Math.round(width * 1000) / 1000)).toEqual([126.667, 126.667, 126.667]);
    expect(bands[1]?.inner).toBeCloseTo(bands[0]?.outer ?? 0, 9);
    expect(bands[2]?.inner).toBeCloseTo(bands[1]?.outer ?? 0, 9);
  });

  it("fits one ring and twelve rings within 460 too", () => {
    expect(bandOf(0, 1)).toEqual({ inner: 80, outer: 460 });
    expect(bandOf(11, 12).outer).toBeCloseTo(460, 9);
    expect(bandOf(0, 12).inner).toBe(80);
  });
});

describe("the angles (SR6)", () => {
  it("starts sector k of n at 2*pi*k/n from 12 o'clock and ends it at 2*pi*(k+1)/n", () => {
    for (const count of [2, 3, 5, 12]) {
      for (let k = 0; k < count; k += 1) {
        const { start, end } = sectorAngles(k, count);
        expect(start).toBeCloseTo((TURN * k) / count, 12);
        expect(end).toBeCloseTo((TURN * (k + 1)) / count, 12);
      }
    }
    // 12 o'clock is straight up from the centre, and the turn runs clockwise.
    expect(pointAt(100, 0)).toEqual({ x: CENTRE, y: CENTRE - 100 });
    const right = pointAt(100, Math.PI / 2);
    expect([Math.round(right.x), Math.round(right.y)]).toEqual([CENTRE + 100, CENTRE]);
  });

  it("gives every sector of a drawn ring the same angle, whatever its title's length (M: angles by title length)", () => {
    const [ring] = drawnRings([growing(5)]);
    expect(ring?.sectors.map((sector) => [Math.round(sector.start * 1e9) / 1e9, Math.round(sector.end * 1e9) / 1e9])).toEqual(
      [0, 1, 2, 3, 4].map((k) => [Math.round(((TURN * k) / 5) * 1e9) / 1e9, Math.round(((TURN * (k + 1)) / 5) * 1e9) / 1e9]),
    );
    // The drawn path is the angles' too: the same as a ring of equal titles.
    const [plain] = drawnRings([aRing("Ring", [0, 1, 2, 3, 4].map((at) => aRingSector({ name: `S-${String(at)}`, title: "x" })))]);
    expect(ring?.sectors.map((sector) => sector.path)).toEqual(plain?.sectors.map((sector) => sector.path));
  });

  it("starts each sector's path at its start angle on the outer edge, half the gap in", () => {
    const band = bandOf(0, 1);
    for (const [k, count] of [
      [0, 4],
      [1, 4],
      [3, 7],
    ] as const) {
      const [x, y] = numbersOf(sectorPath(band, k, count));
      const start = sectorAngles(k, count).start;
      const at = pointAt(band.outer, start + Math.asin(0.5 / band.outer));
      expect([x, y]).toEqual([Math.round(at.x * 100) / 100, Math.round(at.y * 100) / 100]);
    }
  });

  it("parts two neighbours by one unit at either radius", () => {
    const band = bandOf(1, 3);
    for (const radius of [band.inner, band.outer]) {
      const left = pointAt(radius, sectorAngles(0, 6).end - Math.asin(0.5 / radius));
      const right = pointAt(radius, sectorAngles(1, 6).start + Math.asin(0.5 / radius));
      expect(Math.hypot(left.x - right.x, left.y - right.y)).toBeCloseTo(1, 6);
    }
  });

  it("draws one sector as the whole band: the outer circle and the inner one, nothing cut out but the hole", () => {
    const band = bandOf(1, 3);
    const path = sectorPath(band, 0, 1);
    expect(path.match(/M/g)).toHaveLength(2);
    expect(path).toContain(`M${String(CENTRE)},${String(Math.round((CENTRE - band.outer) * 100) / 100)} `);
    expect(path).toContain(`M${String(CENTRE)},${String(Math.round((CENTRE - band.inner) * 100) / 100)} `);
    expect(path).not.toContain("L");
    const [ring] = drawnRings([aRing("One", [aRingSector({ name: "ONLY" })])]);
    expect([ring?.sectors[0]?.start, ring?.sectors[0]?.end]).toEqual([0, TURN]);
    expect(ring?.sectors[0]?.path).toBe(sectorPath(bandOf(0, 1), 0, 1));
  });

  it("finds the nearest sector by its middle's angle, the first on a tie", () => {
    expect(middleAngle(0, 4)).toBeCloseTo(Math.PI / 4, 12);
    expect(nearestSector(middleAngle(0, 12), 5)).toBe(0);
    expect(nearestSector(middleAngle(6, 12), 5)).toBe(2);
    expect(nearestSector(middleAngle(11, 12), 5)).toBe(4);
    expect(nearestSector(0, 2)).toBe(0);
    expect(nearestSector(Math.PI, 1)).toBe(0);
    expect(angleBetween(0.1, TURN - 0.1)).toBeCloseTo(0.2, 12);
  });
});

describe("the labels (SR6)", () => {
  it("keeps a label on a 56-unit arc and drops it on a 55-unit one (M: a label on a 40-unit arc)", () => {
    expect(LABEL_ARC_MIN).toBe(56);
    expect([showsLabel(56), showsLabel(55.99), showsLabel(55), showsLabel(40)]).toEqual([true, false, false, false]);
    // One ring whose middle radius is 270: 30 sectors make 56.5-unit arcs, 31 make 54.7.
    const band = bandOf(0, 1);
    expect(middleOf(band)).toBe(270);
    expect(labelArc(band, 30)).toBeGreaterThanOrEqual(56);
    expect(labelArc(band, 31)).toBeLessThan(56);
    const labelled = (count: number) => drawnRings([growing(count)])[0]?.sectors.filter((sector) => sector.label !== null).length ?? 0;
    expect(labelled(30)).toBeGreaterThan(20);
    expect(labelled(31)).toBe(0);
  });

  it("cuts a long label with an ellipsis to its arc and keeps a short one whole", () => {
    expect(cutToFit("Mooring", 200)).toBe("Mooring");
    const cut = cutToFit("Night-time berthing of deep-draught vessels", 100);
    expect(cut.endsWith("\u2026")).toBe(true);
    expect(textUnits(cut)).toBeLessThanOrEqual(100);
    expect(cut.length).toBeGreaterThan(1);
    // At least one character, however little room.
    expect(cutToFit("Mooring", 1)).toBe("M\u2026");
  });

  it("cuts a one-sector ring's long label to the straight line its band holds, never to the whole circle (M: the arc alone)", () => {
    const long = "Night-time berthing of deep-draught vessels at the outer quay under a falling tide, pilots aboard, tugs standing by";
    for (const rings of [1, 3, 12]) {
      const band = bandOf(0, rings);
      const middle = middleOf(band);
      const chord = 2 * Math.sqrt(band.outer ** 2 - middle ** 2);
      expect(labelRoom(band, 1)).toBeCloseTo(Math.min(labelArc(band, 1), chord) - 2 * LABEL_PAD, 9);
      // One sector's arc is the whole circle, far longer than its band's chord.
      expect(labelArc(band, 1)).toBeGreaterThan(chord);
      const answer = Array.from({ length: rings }, (_, at) => (at === 0 ? aRing("R", [aRingSector({ name: "S-0", title: long })]) : aRing(`R${String(at)}`, [])));
      const label = drawnRings(answer)[0]?.sectors[0]?.label;
      expect(label?.text.endsWith("\u2026")).toBe(true);
      expect(textUnits(label?.text ?? "")).toBeLessThanOrEqual(chord - 2 * LABEL_PAD);
    }
    // Many sectors: the arc is the shorter, as before.
    const band = bandOf(0, 1);
    expect(labelRoom(band, 12)).toBeCloseTo(labelArc(band, 12) - 2 * LABEL_PAD, 9);
  });

  it("counts a base and its combining mark as one character, and a wide character as wide", () => {
    // The basin's name, its yo written as ie and U+0308.
    const decomposed = "\u0417\u0435\u043b\u0435\u0308\u043d\u044b\u0439";
    expect(textUnits(decomposed)).toBe(7 * CHAR_UNITS);
    expect(cutToFit(decomposed, 4 * CHAR_UNITS)).toBe("\u0417\u0435\u043b\u2026");
    expect(cutToFit(decomposed, 5 * CHAR_UNITS)).toBe("\u0417\u0435\u043b\u0435\u0308\u2026");
    expect(textUnits("\u6e2f\u306e")).toBe(2 * WIDE_CHAR_UNITS);
  });

  it("stands a label at the band's middle radius and middle angle, turned to read upright", () => {
    const [ring] = drawnRings([growing(4)]);
    const radius = middleOf(bandOf(0, 1));
    for (const [index, sector] of (ring?.sectors ?? []).entries()) {
      if (sector.label === null) {
        continue;
      }
      const middle = middleAngle(index, 4);
      const at = pointAt(radius, middle);
      expect([sector.label.x, sector.label.y]).toEqual([at.x, at.y]);
      expect(Math.abs(sector.label.turn)).toBeLessThanOrEqual(90);
    }
    // The ring's title stands at 12 o'clock: no label there.
    expect(ring?.sectors.filter((sector) => sector.label !== null).length).toBeGreaterThan(0);
  });

  it("leaves out a label too near the ring's title at 12 o'clock, and keeps the title", () => {
    const [ring] = drawnRings([aRing("A ring title long enough to reach the first sectors", Array.from({ length: 24 }, (_, at) => aRingSector({ name: `S-${String(at)}`, title: "Abc" })))]);
    expect(ring?.title.startsWith("A ring title")).toBe(true);
    expect(ring?.sectors[0]?.label).toBeNull();
    expect(ring?.sectors[23]?.label).toBeNull();
    expect(ring?.sectors[12]?.label?.text).toBe("Abc");
  });
});

describe("one answer, one drawing (SR6)", () => {
  it("draws the same answer the same way twice, no random and no clock", () => {
    const answer = [growing(12, "Inner"), growing(5, "Middle"), growing(1, "Outer"), aRing("Empty", [])];
    expect(drawnRings(answer)).toEqual(drawnRings(structuredClone(answer)));
    const drawn = drawnRings(answer);
    expect(drawn.map((ring) => ring.sectors.length)).toEqual([12, 5, 1, 0]);
    // An empty ring keeps its band, its title and the place of its 0, at 6 o'clock.
    expect(drawn[3]?.countAt).toEqual(pointAt(middleOf(bandOf(3, 4)), Math.PI));
    expect(drawn[3]?.title).toBe("Empty");
  });
});
