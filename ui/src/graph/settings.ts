import type { GraphOptions } from "../api/client";
import type { Direction, FollowedType } from "../api/types";
import { WEAK_LINK_TYPE } from "../tree/labels";

// The Graph's options (docs/features/ui-graph.md "Controls"): per project, in memory, never in the
// hash (Back changes only the REF). The type chips are the mode's unfiltered `types`, in the
// order of its latest answer without `types`, then `mentions`; all but `mentions` pressed sends
// no `types`. The chip order also styles each type's line.

export type GraphTab = "canvas" | "list";

export interface GraphSettings {
  impact: boolean;
  /** Chips of the mode's unfiltered types the owner released. */
  released: readonly string[];
  /** Whether the `mentions` chip is pressed. */
  mentions: boolean;
  /** `--depth N`; null: All (no depth sent). */
  depth: number | null;
  archive: boolean;
  tab: GraphTab;
  /** Each mode's unfiltered `types`, from its latest answer without `types`. */
  bases: { readonly outgoing: readonly FollowedType[] | null; readonly impact: readonly FollowedType[] | null };
}

export const DEFAULT_DEPTH = 2;

/** The depths the select offers; All omits `depth`. */
export const DEPTHS = [1, 2, 3, 4, 5, 6] as const;

export const DEFAULT_SETTINGS: GraphSettings = {
  impact: false,
  released: [],
  mentions: false,
  depth: DEFAULT_DEPTH,
  archive: false,
  tab: "canvas",
  bases: { outgoing: null, impact: null },
};

/** The Graph's options per project for this page's life (the shell holds it). */
export interface GraphMemory {
  recall: (project: string) => GraphSettings | undefined;
  remember: (project: string, settings: GraphSettings) => void;
}

/** A memory that forgets: for a view rendered on its own. */
export const NO_MEMORY: GraphMemory = { recall: () => undefined, remember: () => undefined };

/** The current mode's unfiltered types, once an answer gave them. */
export function baseOf(settings: GraphSettings): readonly FollowedType[] | null {
  return settings.impact ? settings.bases.impact : settings.bases.outgoing;
}

/** Records an unfiltered answer's `types` as its mode's chips; the same settings when nothing changed. */
export function withBase(settings: GraphSettings, impact: boolean, types: readonly FollowedType[]): GraphSettings {
  const known = impact ? settings.bases.impact : settings.bases.outgoing;
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
  return { ...settings, bases: impact ? { ...settings.bases, impact: copy } : { ...settings.bases, outgoing: copy } };
}

export interface Chip {
  type: string;
  /** As an answer of this mode followed it; null while none did (`mentions` before it is pressed). */
  direction: Direction | null;
  pressed: boolean;
}

/**
 * The chips in order: the mode's unfiltered types, then `mentions` (unless already among them).
 * `followed` is the current answer's `types`, which names `mentions`' direction once it is sent.
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
export function patternOrder(bases: GraphSettings["bases"], impact: boolean, answered: readonly FollowedType[]): string[] {
  const base = impact ? bases.impact : bases.outgoing;
  const order = (base ?? answered).map((entry) => entry.type);
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
  return chipsOf(settings, []).filter((chip) => chip.pressed).map((chip) => chip.type);
}

/** Presses or releases one chip; the last pressed one stays pressed. */
export function toggleChip(settings: GraphSettings, type: string): GraphSettings {
  const chips = chipsOf(settings, []);
  const chip = chips.find((candidate) => candidate.type === type);
  if (chip === undefined) {
    return settings;
  }
  if (chip.pressed && chips.filter((candidate) => candidate.pressed).length === 1) {
    return settings;
  }
  const base = baseOf(settings) ?? [];
  if (!base.some((entry) => entry.type === type)) {
    return { ...settings, mentions: !settings.mentions };
  }
  const released = chip.pressed ? [...settings.released, type] : settings.released.filter((name) => name !== type);
  return { ...settings, released };
}

/** A mode switch: the chips go back to the mode's default. */
export function withMode(settings: GraphSettings, impact: boolean): GraphSettings {
  return settings.impact === impact ? settings : { ...settings, impact, released: [], mentions: false };
}

/** The read the settings ask for at a REF. */
export function graphOptions(ref: string, settings: GraphSettings): GraphOptions {
  const options: GraphOptions = { ref };
  if (settings.impact) {
    options.impact = true;
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
