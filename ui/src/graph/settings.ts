import type { GraphOptions } from "../api/client";
import type { Direction, FollowedType } from "../api/types";
import { WEAK_LINK_TYPE } from "../tree/labels";

// The Graph's options (docs/features/ui-graph.md "Implementation", Controls): per project, in
// memory, never in the hash (Back changes only the REF). The type chips are the mode's unfiltered
// `types`, in the order of its latest answer without `types`, then `mentions`; all but `mentions`
// pressed sends no `types`. The chip order also styles each type's line.

export type GraphTab = "canvas" | "list";

/**
 * What the walk follows: each type outgoing (Links), each both ways (Both ways), or the impact table
 * (Impact) (docs/features/corpus-map.md "Acceptance criteria", CM10).
 */
export type GraphMode = "out" | "both" | "impact";

export const GRAPH_MODES: readonly GraphMode[] = ["out", "both", "impact"];

/** The section's home view with no REF (docs/features/corpus-map.md "Acceptance criteria", CM8). */
export type GraphHomeView = "map" | "rings";

export interface GraphSettings {
  mode: GraphMode;
  /** Chips of the mode's unfiltered types the owner released. */
  released: readonly string[];
  /** Whether the `mentions` chip is pressed. */
  mentions: boolean;
  /** `--depth N`; null: All (no depth sent). */
  depth: number | null;
  archive: boolean;
  tab: GraphTab;
  /** Each mode's unfiltered `types`, from its latest answer without `types`. */
  bases: Readonly<Record<GraphMode, readonly FollowedType[] | null>>;
  /** The home's view, kept per project for the page's life. */
  home: GraphHomeView;
}

export const DEFAULT_DEPTH = 2;

/** The depths the select offers; All omits `depth`. */
export const DEPTHS = [1, 2, 3, 4, 5, 6] as const;

export const DEFAULT_SETTINGS: GraphSettings = {
  mode: "out",
  released: [],
  mentions: false,
  depth: DEFAULT_DEPTH,
  archive: false,
  tab: "canvas",
  bases: { out: null, both: null, impact: null },
  home: "map",
};

/** The mode an answer was read in. */
export function modeOf(answer: { impact: boolean; both: boolean }): GraphMode {
  return answer.both ? "both" : answer.impact ? "impact" : "out";
}

/** The Graph's options per project for this page's life (the shell holds it). */
export interface GraphMemory {
  recall: (project: string) => GraphSettings | undefined;
  remember: (project: string, settings: GraphSettings) => void;
}

/** A memory that forgets: for a view rendered on its own. */
export const NO_MEMORY: GraphMemory = { recall: () => undefined, remember: () => undefined };

/** The current mode's unfiltered types, once an answer gave them. */
export function baseOf(settings: GraphSettings): readonly FollowedType[] | null {
  return settings.bases[settings.mode];
}

/** Records an unfiltered answer's `types` as its mode's chips; the same settings when nothing changed. */
export function withBase(settings: GraphSettings, mode: GraphMode, types: readonly FollowedType[]): GraphSettings {
  const known = settings.bases[mode];
  const same =
    known !== null &&
    known.length === types.length &&
    known.every((followed, index) => {
      const given = types[index];
      return given !== undefined && followed.type === given.type && followed.direction === given.direction;
    });
  if (same) {
    return settings;
  }
  const copy = types.map((followed) => ({ type: followed.type, direction: followed.direction }));
  return { ...settings, bases: { ...settings.bases, [mode]: copy } };
}

export interface Chip {
  type: string;
  /** As an answer of this mode followed it; null while none did (`mentions` before it is pressed). */
  direction: Direction | null;
  pressed: boolean;
}

/**
 * The chips in order: the mode's unfiltered (type, direction) pairs, as the answer gives them (Both
 * ways: each type out, then in), then `mentions` (unless already among them). A chip presses and
 * releases its type, every pair of it at once. `followed` is the current answer's `types`, which
 * names `mentions`' direction once it is sent.
 */
export function chipsOf(settings: GraphSettings, followed: readonly FollowedType[]): Chip[] {
  const base = baseOf(settings);
  if (base === null) {
    return [];
  }
  const chips: Chip[] = base.map((entry) => ({
    type: entry.type,
    direction: entry.direction,
    pressed: !settings.released.includes(entry.type),
  }));
  if (!base.some((entry) => entry.type === WEAK_LINK_TYPE)) {
    chips.push({
      type: WEAK_LINK_TYPE,
      direction: followed.find((entry) => entry.type === WEAK_LINK_TYPE)?.direction ?? null,
      pressed: settings.mentions,
    });
  }
  return chips;
}

/**
 * The order that styles a mode's types: its chips' order (the unfiltered types, then `mentions`),
 * so pressing or releasing a chip never restyles another type. A type the chips do not hold (no
 * unfiltered answer of the mode yet) follows in the answer's order.
 */
export function patternOrder(bases: GraphSettings["bases"], mode: GraphMode, answered: readonly FollowedType[]): string[] {
  const order = [...new Set((bases[mode] ?? answered).map((entry) => entry.type))];
  const known = new Set(order);
  for (const type of [WEAK_LINK_TYPE, ...answered.map((entry) => entry.type)]) {
    if (!known.has(type)) {
      known.add(type);
      order.push(type);
    }
  }
  return order;
}

/** The `types` to send: none while every chip but `mentions` is pressed, else the pressed ones in chip order. */
export function typesToSend(settings: GraphSettings): string[] | undefined {
  if (settings.released.length === 0 && !settings.mentions) {
    return undefined;
  }
  return pressedTypes(chipsOf(settings, []));
}

/** The pressed chips' types, each once, in chip order. */
export function pressedTypes(chips: readonly Chip[]): string[] {
  return [...new Set(chips.filter((chip) => chip.pressed).map((chip) => chip.type))];
}

/** Presses or releases one type's chips; the last pressed type stays pressed. */
export function toggleChip(settings: GraphSettings, type: string): GraphSettings {
  const chips = chipsOf(settings, []);
  const chip = chips.find((candidate) => candidate.type === type);
  if (chip === undefined) {
    return settings;
  }
  if (chip.pressed && pressedTypes(chips).length === 1) {
    return settings;
  }
  const base = baseOf(settings) ?? [];
  if (!base.some((entry) => entry.type === type)) {
    return { ...settings, mentions: !settings.mentions };
  }
  const released = chip.pressed ? [...settings.released, type] : settings.released.filter((name) => name !== type);
  return { ...settings, released };
}

/** A mode switch: the chips go back to the mode's default; each mode keeps its own base. */
export function withMode(settings: GraphSettings, mode: GraphMode): GraphSettings {
  return settings.mode === mode ? settings : { ...settings, mode, released: [], mentions: false };
}

/** The read the settings ask for at a REF: Impact sends `impact`, Both ways `both` (CM10). */
export function graphOptions(ref: string, settings: GraphSettings): GraphOptions {
  const options: GraphOptions = { ref };
  if (settings.mode === "impact") {
    options.impact = true;
  }
  if (settings.mode === "both") {
    options.both = true;
  }
  const types = typesToSend(settings);
  if (types !== undefined) {
    options.types = types;
  }
  if (settings.depth !== null) {
    options.depth = settings.depth;
  }
  if (settings.archive) {
    options.archive = true;
  }
  return options;
}
