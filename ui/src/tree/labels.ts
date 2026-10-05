import type { BundleForm, BundleLayers, Direction, LinkState, TreeMark } from "../api/types";
import type { Look } from "../ui/Badge";

// The closed tables the tree and node views label: link states, tree marks, bundle forms, layer
// keys, directions. Known literals get a label (and for states and marks a colour role and an
// icon); any other string is shown raw, never mapped to a known value. Kinds, spec statuses and
// link types are project or corpus vocabulary: shown as written, never quoted here.

function lookup<K extends string, V>(table: Record<K, V>, value: string): V | null {
  return Object.hasOwn(table, value) ? table[value as K] : null;
}

type KnownLinkState = "resolved" | "dangling" | "skipped" | "unchecked";

const LINK_STATE: Record<KnownLinkState, Look> = {
  resolved: { label: "Resolved", tone: "link-resolved", icon: "linkResolved" },
  dangling: { label: "Dangling", tone: "link-dangling", icon: "linkDangling" },
  skipped: { label: "Skipped: another project", tone: "link-skipped", icon: "linkSkipped" },
  unchecked: { label: "Not checked", tone: "link-unchecked", icon: "linkUnchecked" },
};

/** A link's state: label, colour role and icon; the raw text in a neutral badge when unknown. */
export function linkStateLook(state: LinkState): Look {
  return lookup(LINK_STATE, state) ?? { label: state, tone: "link-unknown", icon: "unknown" };
}

type KnownMark = "dangling-parent" | "parent-cycle";

const MARK: Record<KnownMark, Look> = {
  "dangling-parent": { label: "Parent dangling", tone: "mark-dangling-parent", icon: "parentDangling" },
  "parent-cycle": { label: "Parent cycle", tone: "mark-parent-cycle", icon: "parentCycle" },
};

/** Why a row is a root although its document names a parent. */
export function markLook(mark: TreeMark): Look {
  return lookup(MARK, mark) ?? { label: mark, tone: "mark-unknown", icon: "unknown" };
}

type KnownForm = "text" | "outline" | "header" | "summary";

const FORM: Record<KnownForm, string> = {
  text: "Full text",
  outline: "Outline: header and summary",
  header: "Header only",
  summary: "Header and summary",
};

/** The form a bundle item takes; the raw text when unknown. */
export function formLabel(form: BundleForm): string {
  return lookup(FORM, form) ?? form;
}

type KnownDirection = "in" | "out";

const DIRECTION: Record<KnownDirection, string> = { in: "incoming", out: "outgoing" };

/** A link's direction from the target; the raw text when unknown. */
export function directionLabel(direction: Direction): string {
  return lookup(DIRECTION, direction) ?? direction;
}

/** The bundle's layers in print order with the CLI's headings (`docs/canon/spec-cli-bundle.md` "Layers"). */
export const LAYER_HEADINGS: readonly (readonly [keyof BundleLayers, string])[] = [
  ["targets", "Targets"],
  ["open_questions", "Open questions"],
  ["ancestors", "Ancestors"],
  ["criteria", "Criteria"],
  ["bindings", "Bindings"],
  ["decisions", "Decisions"],
  ["neighbours", "Neighbours"],
  ["terms", "Terms"],
  ["tests", "Tests"],
];

/** A tail entry's layer key as its heading; the raw key when unknown. */
export function layerHeading(key: string): string {
  return LAYER_HEADINGS.find(([known]) => known === key)?.[1] ?? key;
}

/** The weak link type (`docs/canon/spec-cli-graph.md` "Link types"): listed after the strong ones. */
export const WEAK_LINK_TYPE = "mentions";
