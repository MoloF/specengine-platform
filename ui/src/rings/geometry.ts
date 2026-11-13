import type { Ring, RingSector } from "../api/types";

// The rings' geometry (docs/features/scope-rings.md "Acceptance criteria", SR6): pure numbers in a
// 1000 x 1000 viewBox, no drawing library, no random and no clock, so one answer is one picture.
// Ring i's band runs from RINGS_INNER + i*W to RINGS_INNER + (i+1)*W, W set by the ring count to fit
// RINGS_OUTER. A ring's n sectors each take 2*pi/n, clockwise from 12 o'clock, in served order,
// whatever they hold; one sector is the whole band; SECTOR_GAP units part two sectors. A sector's
// label stands at the band's middle radius, across the sector, only where that arc is at least
// LABEL_ARC_MIN long, cut with an ellipsis to the room it has (labelRoom): the arc, and the straight
// line the band holds there, since a label is drawn straight; the ring's title runs along its band at
// 12 o'clock.

/** The viewBox's side; the centre is its middle. */
export const VIEW_SIZE = 1000;
export const CENTRE = VIEW_SIZE / 2;

/** The innermost band's inner radius (R0) and the outermost band's outer one. */
export const RINGS_INNER = 80;
export const RINGS_OUTER = 460;

/** The gap between two sectors of a ring, in units. */
export const SECTOR_GAP = 1;

/** The shortest arc, at the band's middle radius, that holds a label. */
export const LABEL_ARC_MIN = 56;

/** Units a character is taken to fill at the label's size: an estimate, the same on every engine. */
export const CHAR_UNITS = 11;

/** A wide character's (Han, kana, Hangul, full-width, an emoji) estimate. */
export const WIDE_CHAR_UNITS = 20;

/** Room kept clear at each end of a label, and around a ring's title. */
export const LABEL_PAD = 6;
const TITLE_PAD = 10;

/** A ring's title takes at most this share of its middle circle. */
const TITLE_SHARE = 0.25;

const WIDE = /[\u1100-\u115f\u2e80-\ua4cf\uac00-\ud7a3\uf900-\ufaff\ufe30-\ufe4f\uff00-\uff60\uffe0-\uffe6\u{1f300}-\u{1faff}\u{20000}-\u{3fffd}]/u;

/** A ring's band: its inner and outer radius. */
export interface Band {
  inner: number;
  outer: number;
}

/** Ring `ring` of `rings`, innermost 0. */
export function bandOf(ring: number, rings: number): Band {
  const width = (RINGS_OUTER - RINGS_INNER) / Math.max(1, rings);
  return { inner: RINGS_INNER + ring * width, outer: RINGS_INNER + (ring + 1) * width };
}

/** The band's middle radius, where its labels and title stand. */
export function middleOf(band: Band): number {
  return (band.inner + band.outer) / 2;
}

/** Sector `index` of `count`: where it starts and ends, radians clockwise from 12 o'clock. */
export function sectorAngles(index: number, count: number): { start: number; end: number } {
  const step = (2 * Math.PI) / Math.max(1, count);
  return { start: index * step, end: (index + 1) * step };
}

/** The angle through the middle of sector `index` of `count`. */
export function middleAngle(index: number, count: number): number {
  const { start, end } = sectorAngles(index, count);
  return (start + end) / 2;
}

/** How far apart two angles are around the circle, at most pi. */
export function angleBetween(a: number, b: number): number {
  const turn = 2 * Math.PI;
  const apart = (((a - b) % turn) + turn) % turn;
  return Math.min(apart, turn - apart);
}

/** The sector of `count` whose middle stands nearest `angle`; the first on a tie. */
export function nearestSector(angle: number, count: number): number {
  let best = 0;
  let bestApart = Number.POSITIVE_INFINITY;
  for (let index = 0; index < count; index += 1) {
    const apart = angleBetween(middleAngle(index, count), angle);
    if (apart < bestApart - 1e-9) {
      best = index;
      bestApart = apart;
    }
  }
  return best;
}

/** The point at `radius` and `angle` (clockwise from 12 o'clock). */
export function pointAt(radius: number, angle: number): { x: number; y: number } {
  return { x: CENTRE + radius * Math.sin(angle), y: CENTRE - radius * Math.cos(angle) };
}

/** A number as the path writes it: two decimals at most, never `-0`. */
function written(value: number): string {
  const rounded = Math.round(value * 100) / 100;
  return String(rounded === 0 ? 0 : rounded);
}

function at(radius: number, angle: number): string {
  const point = pointAt(radius, angle);
  return `${written(point.x)},${written(point.y)}`;
}

/** A circle at `radius` as two half arcs from 12 o'clock, clockwise (`sweep` 1) or back (0). */
function circle(radius: number, sweep: 0 | 1): string {
  const r = written(radius);
  return `M${at(radius, 0)} A${r},${r} 0 1 ${String(sweep)} ${at(radius, Math.PI)} A${r},${r} 0 1 ${String(sweep)} ${at(radius, 0)} Z`;
}

/**
 * Sector `index` of `count` in `band` as an SVG path. One sector is the whole band, its hole left by
 * the inner circle run back. Otherwise each side's edge runs half the gap off the radius through it,
 * so two neighbours' edges stand SECTOR_GAP apart.
 */
export function sectorPath(band: Band, index: number, count: number): string {
  if (count <= 1) {
    return `${circle(band.outer, 1)} ${circle(band.inner, 0)}`;
  }
  const { start, end } = sectorAngles(index, count);
  const inset = (radius: number) => Math.min(Math.asin(Math.min(1, SECTOR_GAP / 2 / radius)), (end - start) / 2);
  const outer = inset(band.outer);
  const inner = inset(band.inner);
  const large = (span: number) => (span > Math.PI ? 1 : 0);
  const ro = written(band.outer);
  const ri = written(band.inner);
  return [
    `M${at(band.outer, start + outer)}`,
    `A${ro},${ro} 0 ${String(large(end - start - 2 * outer))} 1 ${at(band.outer, end - outer)}`,
    `L${at(band.inner, end - inner)}`,
    `A${ri},${ri} 0 ${String(large(end - start - 2 * inner))} 0 ${at(band.inner, start + inner)}`,
    "Z",
  ].join(" ");
}

/** The arc one sector of `count` spans at the band's middle radius. */
export function labelArc(band: Band, count: number): number {
  return (middleOf(band) * 2 * Math.PI) / Math.max(1, count);
}

/**
 * How long a label of a sector of `count` in `band` may run, in units: a straight line tangent at the
 * middle radius leaves the band at its outer circle 2*sqrt(outer^2 - middle^2) long, so it takes the
 * shorter of that and the arc, less LABEL_PAD at each end. One sector's arc is the whole circle; the
 * band's chord is what holds its label.
 */
export function labelRoom(band: Band, count: number): number {
  const middle = middleOf(band);
  const chord = 2 * Math.sqrt(Math.max(0, band.outer * band.outer - middle * middle));
  return Math.min(labelArc(band, count), chord) - 2 * LABEL_PAD;
}

/** Whether a sector whose middle arc is `arc` long carries a label. */
export function showsLabel(arc: number): boolean {
  return arc >= LABEL_ARC_MIN;
}

const graphemes = new Intl.Segmenter(undefined, { granularity: "grapheme" });

/** A text's characters as a person reads them: a base and its combining marks one. */
function charactersOf(text: string): string[] {
  return Array.from(graphemes.segment(text), (part) => part.segment);
}

function unitsOf(character: string): number {
  return WIDE.test(character) ? WIDE_CHAR_UNITS : CHAR_UNITS;
}

/** How long a text is taken to run at the label's size, in units. */
export function textUnits(text: string): number {
  return charactersOf(text).reduce((sum, character) => sum + unitsOf(character), 0);
}

/** `text` whole when it fits `room` units; else as much as fits, then an ellipsis; at least one character. */
export function cutToFit(text: string, room: number): string {
  if (textUnits(text) <= room) {
    return text;
  }
  let units = CHAR_UNITS;
  let kept = "";
  for (const character of charactersOf(text)) {
    const next = units + unitsOf(character);
    if (next > room && kept !== "") {
      break;
    }
    units = next;
    kept += character;
  }
  return `${kept.trimEnd()}\u2026`;
}

/** A sector as drawn: the served sector, its path, its angles, and its label where one fits. */
export interface DrawnSector {
  sector: RingSector;
  path: string;
  start: number;
  end: number;
  /** The label: its text (cut), its middle point, and its turn in degrees so it reads upright. */
  label: { text: string; x: number; y: number; turn: number } | null;
}

/** A ring as drawn: the served ring, its band, its title (cut) on a circle path through 12 o'clock, its count's place, its sectors. */
export interface DrawnRing {
  ring: Ring;
  band: Band;
  title: string;
  /** The circle the title runs on, from 6 o'clock clockwise: its 50 % is 12 o'clock. */
  titlePath: string;
  /** Where the count of an empty ring stands: 6 o'clock in its band. */
  countAt: { x: number; y: number };
  sectors: DrawnSector[];
}

/**
 * Every ring and sector of an answer, in served order, as drawn: the bands by ring count, the angles
 * by sector count alone. A label too near the ring's title at 12 o'clock is left out; the List and
 * the card hold every title whole.
 */
export function drawnRings(rings: readonly Ring[]): DrawnRing[] {
  return rings.map((ring, index) => {
    const band = bandOf(index, rings.length);
    const radius = middleOf(band);
    const title = cutToFit(ring.title, Math.PI * 2 * radius * TITLE_SHARE);
    const titleReach = (textUnits(title) / 2 + TITLE_PAD) / radius;
    const count = ring.sectors.length;
    const arc = labelArc(band, count);
    const room = labelRoom(band, count);
    const bottom = pointAt(radius, Math.PI);
    const top = pointAt(radius, 0);
    const r = written(radius);
    const titlePath = `M${written(bottom.x)},${written(bottom.y)} A${r},${r} 0 1 1 ${written(top.x)},${written(top.y)} A${r},${r} 0 1 1 ${written(bottom.x)},${written(bottom.y)}`;
    const sectors = ring.sectors.map((sector, at): DrawnSector => {
      const { start, end } = sectorAngles(at, count);
      const middle = (start + end) / 2;
      let label: DrawnSector["label"] = null;
      if (showsLabel(arc)) {
        const text = cutToFit(sector.title ?? sector.name, room);
        const reach = textUnits(text) / 2 / radius;
        if (angleBetween(middle, 0) >= titleReach + reach) {
          const point = pointAt(radius, middle);
          const degrees = (middle * 180) / Math.PI;
          // Along the circle, the lower half turned over and the upper left turned back: within 90 degrees either way.
          label = { text, x: point.x, y: point.y, turn: degrees > 270 ? degrees - 360 : degrees > 90 ? degrees - 180 : degrees };
        }
      }
      return { sector, path: sectorPath(band, at, count), start, end, label };
    });
    return { ring, band, title, titlePath, countAt: bottom, sectors };
  });
}
