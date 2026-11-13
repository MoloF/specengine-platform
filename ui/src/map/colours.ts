import type { WorkspaceModule } from "../api/types";
import { isDirection } from "../workspace/choice";

// A node's colour on the map (docs/features/corpus-map.md "Acceptance criteria", CM7, CM13): a fixed
// colour per kind, named in the legend. The node set's kinds are ranked by how many of its nodes hold
// each, most first, ties in code-point order of the kind; the first MAP_SLOTS take slots 1 to 8 in
// that order, any other kind and no kind the grey, slot 0. So a corpus with no direction is coloured
// all the same, and one answer and Sections choice give one colouring. No colour is keyed on what a
// kind says: only on how many nodes hold it. The colours themselves are tokens.css roles, read
// through hidden swatches (MapCanvas) and again on a theme change; a colour always comes with words
// (the legend, the card, the List). A direction is no colour: it names a kind's group in the card and
// the List, and the Direction select presses its kinds' chips.

/** How many kinds take a colour of their own. */
export const MAP_SLOTS = 8;

/** A served direction: its title and the kinds it lists, in served order. */
export interface MapDirection {
  name: string;
  title: string;
  kinds: readonly string[];
}

/** The served directions among a project's modules, in served order: a module with kinds and no root. */
export function directionsOf(modules: readonly WorkspaceModule[]): MapDirection[] {
  return modules.filter(isDirection).map((module) => ({ name: module.name, title: module.title, kinds: module.kinds }));
}

/** The first served direction listing a kind, for its title; none for no kind or an unlisted one. */
export function directionOf(kind: string | null, directions: readonly MapDirection[]): MapDirection | undefined {
  return kind === null ? undefined : directions.find((direction) => direction.kinds.includes(kind));
}

/** Code-point order, never a locale's. */
export function byCodePoint(a: string, b: string): number {
  const left = Array.from(a);
  const right = Array.from(b);
  for (let at = 0; at < Math.min(left.length, right.length); at += 1) {
    const difference = (left[at]?.codePointAt(0) ?? 0) - (right[at]?.codePointAt(0) ?? 0);
    if (difference !== 0) {
      return difference;
    }
  }
  return left.length - right.length;
}

/** A kind that takes a colour: its slot, 1 on, and how many nodes of the set hold it. */
export interface ColouredKind {
  kind: string;
  slot: number;
  count: number;
}

/** A node set's colouring: the coloured kinds in slot order, each kind's slot, and how many nodes take the grey. */
export interface KindColours {
  coloured: readonly ColouredKind[];
  slots: ReadonlyMap<string, number>;
  /** The node set's nodes of any other kind or of none. */
  other: number;
}

/** The colouring of a node set by its nodes' kinds: most held first, ties by code point, MAP_SLOTS of them. */
export function kindColours(kinds: Iterable<string | null>): KindColours {
  const counts = new Map<string, number>();
  let none = 0;
  for (const kind of kinds) {
    if (kind === null) {
      none += 1;
    } else {
      counts.set(kind, (counts.get(kind) ?? 0) + 1);
    }
  }
  const ranked = [...counts].sort(([a, countA], [b, countB]) => countB - countA || byCodePoint(a, b));
  const coloured = ranked.slice(0, MAP_SLOTS).map(([kind, count], at) => ({ kind, slot: at + 1, count }));
  const other = none + ranked.slice(MAP_SLOTS).reduce((sum, [, count]) => sum + count, 0);
  return { coloured, slots: new Map(coloured.map((each) => [each.kind, each.slot])), other };
}

/** A kind's slot in a colouring: 1 to 8 for a coloured kind; 0, the grey, for any other and for no kind. */
export function slotOf(kind: string | null, colours: KindColours): number {
  return kind === null ? 0 : (colours.slots.get(kind) ?? 0);
}

/** The roles each slot fills with: the grey at 0, then slots 1 to 8. */
export const SLOT_ROLES: readonly string[] = ["--map-kind-other", ...Array.from({ length: MAP_SLOTS }, (_, at) => `--map-kind-${String(at + 1)}`)];

/** The roles the drawing reads besides the slots. */
export const PEN_ROLES = {
  surface: "--map-surface",
  edge: "--map-edge",
  weak: "--map-edge-weak",
  ghost: "--map-ghost",
  label: "--map-label",
  focus: "--focus-ring",
} as const;

/** The colours a drawing uses, each a role's value as the browser resolved it in the current theme. */
export interface MapPalette {
  /** By slot: the grey at 0, then 1 to 8. */
  slots: readonly string[];
  surface: string;
  edge: string;
  weak: string;
  ghost: string;
  label: string;
  focus: string;
  /** The text font, as the page's `--font-sans` resolves. */
  font: string;
}

/** A palette from a reader of roles: a role's resolved colour, else its name (a test's DOM resolves none). */
export function paletteOf(read: (role: string) => string, font: string): MapPalette {
  return {
    slots: SLOT_ROLES.map(read),
    surface: read(PEN_ROLES.surface),
    edge: read(PEN_ROLES.edge),
    weak: read(PEN_ROLES.weak),
    ghost: read(PEN_ROLES.ghost),
    label: read(PEN_ROLES.label),
    focus: read(PEN_ROLES.focus),
    font,
  };
}
