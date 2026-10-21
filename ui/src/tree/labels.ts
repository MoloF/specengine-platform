import type { BundleForm, BundleLayers, Direction, KnownSyncState, LinkState, SyncState, TreeMark } from "../api/types";
import { oneLine } from "../health/oneLine";
import type { Look } from "../ui/Badge";

// The closed tables the tree and node views label: link states, tree marks, bundle forms, layer
// keys, directions, a binding row's sync. Known literals get a label (and for states and marks a
// colour role and an icon); any other string is shown raw, never mapped to a known value. Kinds, spec statuses and
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

/** The layers of code binding rows, not nodes (`docs/features/bundle-bindings.md` "Layers 5 and 9"). */
const CODE_LAYERS = ["bindings", "tests"] as const satisfies readonly (keyof BundleLayers)[];

/** Whether a layer key names a layer of code rows: its entries are drawn as lines and never linked as tree nodes. */
export function isCodeLayer(key: string): key is (typeof CODE_LAYERS)[number] {
  return (CODE_LAYERS as readonly string[]).includes(key);
}

const SYNC: Record<KnownSyncState, Look> = {
  cannot_verify: { label: "Cannot verify", tone: "sync-cannot-verify", icon: "cannotCheck" },
  predated: { label: "Predated", tone: "sync-predated", icon: "deferred" },
  bad_marker: { label: "Bad marker", tone: "sync-bad-marker", icon: "findingWarning" },
  broken_binding: { label: "Broken binding", tone: "sync-broken-binding", icon: "linkDangling" },
  unlocked: { label: "Unlocked", tone: "sync-unlocked", icon: "open" },
  spec_ahead: { label: "Spec ahead", tone: "sync-spec-ahead", icon: "specChanged" },
  code_ahead: { label: "Code ahead", tone: "sync-code-ahead", icon: "changes" },
  conflict: { label: "Conflict", tone: "sync-conflict", icon: "checkFails" },
  ok: { label: "In sync", tone: "sync-ok", icon: "checkClean" },
};

/** A row the read listed but could not judge (`docs/canon/code-reads.md` "Degrade"): never ok's look. */
const NOT_JUDGED: Look = { label: "Not judged", tone: "sync-not-judged", icon: "stub" };

/** A binding row's sync: verify's state as a label, colour role and icon; null not judged; any other word raw (on one line, `oneLine`), neutral. */
export function syncLook(sync: SyncState | null): Look {
  if (sync === null) {
    return NOT_JUDGED;
  }
  return lookup(SYNC, sync) ?? { label: oneLine(sync), tone: "sync-unknown", icon: "unknown" };
}

/** The weak link type (`docs/canon/spec-cli-graph.md` "Link types"): listed after the strong ones. */
export const WEAK_LINK_TYPE = "mentions";
