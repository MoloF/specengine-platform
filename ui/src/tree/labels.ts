import type { BundleForm, BundleLayers, Direction, KnownSyncState, LinkState, SyncState, TreeMark } from "../api/types";
import { oneLine } from "../health/oneLine";
import type { Translate } from "../i18n";
import type { Look } from "../ui/Badge";

// The closed tables the tree and node views label: link states, tree marks, bundle forms, layer
// keys, directions, a binding row's sync. Known literals get a label from the catalog's `tree` group
// (and for states and marks a colour role and an icon); any other string is shown raw, never mapped
// to a known value. Kinds, spec statuses and link types are project or corpus vocabulary: shown as
// written, never quoted here.

type Mark = Omit<Look, "label">;

function lookup<K extends string, V>(table: Record<K, V>, value: string): V | null {
  return Object.hasOwn(table, value) ? table[value as K] : null;
}

type KnownLinkState = "resolved" | "dangling" | "skipped" | "unchecked";

const LINK_STATE: Record<KnownLinkState, Mark> = {
  resolved: { tone: "link-resolved", icon: "linkResolved" },
  dangling: { tone: "link-dangling", icon: "linkDangling" },
  skipped: { tone: "link-skipped", icon: "linkSkipped" },
  unchecked: { tone: "link-unchecked", icon: "linkUnchecked" },
};

/** The four link states, in the table's order. */
export const KNOWN_LINK_STATES = Object.keys(LINK_STATE) as readonly KnownLinkState[];

/** A link's state: label, colour role and icon; the raw text in a neutral badge when unknown. */
export function linkStateLook(state: LinkState, t: Translate): Look {
  const mark = lookup(LINK_STATE, state);
  return mark === null ? { label: state, tone: "link-unknown", icon: "unknown", served: true } : { label: t(`tree.linkState.${state as KnownLinkState}`), ...mark };
}

type KnownMark = "dangling-parent" | "parent-cycle";

const MARK: Record<KnownMark, Mark & { key: "danglingParent" | "parentCycle" }> = {
  "dangling-parent": { key: "danglingParent", tone: "mark-dangling-parent", icon: "parentDangling" },
  "parent-cycle": { key: "parentCycle", tone: "mark-parent-cycle", icon: "parentCycle" },
};

/** The two tree marks, as the daemon writes them. */
export const KNOWN_TREE_MARKS = Object.keys(MARK) as readonly KnownMark[];

/** Why a row is a root although its document names a parent. */
export function markLook(mark: TreeMark, t: Translate): Look {
  const known = lookup(MARK, mark);
  return known === null ? { label: mark, tone: "mark-unknown", icon: "unknown", served: true } : { label: t(`tree.mark.${known.key}`), tone: known.tone, icon: known.icon };
}

type KnownForm = "text" | "outline" | "header" | "summary";

const FORM: Record<KnownForm, true> = { text: true, outline: true, header: true, summary: true };

/** The four bundle forms, in the table's order. */
export const KNOWN_FORMS = Object.keys(FORM) as readonly KnownForm[];

/** The form a bundle item takes; the raw text when unknown. */
export function formLabel(form: BundleForm, t: Translate): string {
  return Object.hasOwn(FORM, form) ? t(`tree.form.${form as KnownForm}`) : form;
}

type KnownDirection = "in" | "out";

const DIRECTION: Record<KnownDirection, true> = { in: true, out: true };

/** A link's direction from the target; the raw text when unknown. */
export function directionLabel(direction: Direction, t: Translate): string {
  return Object.hasOwn(DIRECTION, direction) ? t(`tree.direction.${direction as KnownDirection}`) : direction;
}

/** The bundle's layers in print order (`docs/canon/spec-cli-bundle.md` "Layers"). */
const LAYERS = [
  "targets",
  "open_questions",
  "ancestors",
  "criteria",
  "bindings",
  "decisions",
  "neighbours",
  "terms",
  "tests",
] as const satisfies readonly (keyof BundleLayers)[];

/** The bundle's layers in print order with the CLI's headings, in the catalog's words. */
export function layerHeadings(t: Translate): readonly (readonly [keyof BundleLayers, string])[] {
  return LAYERS.map((key) => [key, t(`tree.layer.${key}`)] as const);
}

/** A tail entry's layer key as its heading; the raw key when unknown. */
export function layerHeading(key: string, t: Translate): string {
  return (LAYERS as readonly string[]).includes(key) ? t(`tree.layer.${key as (typeof LAYERS)[number]}`) : key;
}

/** The layers of code binding rows, not nodes (`docs/features/bundle-bindings.md` "Layers 5 and 9"). */
const CODE_LAYERS = ["bindings", "tests"] as const satisfies readonly (keyof BundleLayers)[];

/** Whether a layer key names a layer of code rows: its entries are drawn as lines and never linked as tree nodes. */
export function isCodeLayer(key: string): key is (typeof CODE_LAYERS)[number] {
  return (CODE_LAYERS as readonly string[]).includes(key);
}

const SYNC: Record<KnownSyncState, Mark> = {
  cannot_verify: { tone: "sync-cannot-verify", icon: "cannotCheck" },
  predated: { tone: "sync-predated", icon: "deferred" },
  bad_marker: { tone: "sync-bad-marker", icon: "findingWarning" },
  broken_binding: { tone: "sync-broken-binding", icon: "linkDangling" },
  unlocked: { tone: "sync-unlocked", icon: "open" },
  spec_ahead: { tone: "sync-spec-ahead", icon: "specChanged" },
  code_ahead: { tone: "sync-code-ahead", icon: "changes" },
  conflict: { tone: "sync-conflict", icon: "checkFails" },
  ok: { tone: "sync-ok", icon: "checkClean" },
};

/** verify's nine states, in its table's order. */
export const KNOWN_SYNC_STATES = Object.keys(SYNC) as readonly KnownSyncState[];

/** A binding row's sync: verify's state as a label, colour role and icon; null not judged (`docs/canon/code-reads.md` "Degrade", never ok's look); any other word raw (on one line, `oneLine`), neutral. */
export function syncLook(sync: SyncState | null, t: Translate): Look {
  if (sync === null) {
    return { label: t("tree.notJudged"), tone: "sync-not-judged", icon: "stub" };
  }
  const mark = lookup(SYNC, sync);
  return mark === null ? { label: oneLine(sync), tone: "sync-unknown", icon: "unknown", served: true } : { label: t(`tree.sync.${sync as KnownSyncState}`), ...mark };
}

/** The weak link type (`docs/canon/spec-cli-graph.md` "Link types"): listed after the strong ones. */
export const WEAK_LINK_TYPE = "mentions";
