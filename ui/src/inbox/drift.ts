import type { DriftEvidence, StoredDriftRow } from "../api/types";
import { PROPOSAL_ID } from "./decisions";

// A drift item (`docs/features/drift-queue.md` "Review, inbox, round, stage", "Approve"): `spec
// verify`'s drift of one node, queued for the owner with its evidence. It is decided on a terminal
// only, where it was raised, and never staged: the card offers its commands in place of the
// decisions. The review document's `drift` key says a proposal is one, never its kind (ADR-0031).

export type DriftAction = "approve" | "editorial" | "reject";

export interface DriftCommand {
  action: DriftAction;
  /** What it does, in words. */
  meaning: string;
  /** The command exactly as copied. */
  text: string;
}

/**
 * The terminal commands deciding a drift item: approve (the lock accept), approve `--editorial` (a
 * node's text changed at its locked revision, wording only: the terminal names it when it refuses a
 * plain approve for that), reject with a reason. All three whatever the rows say: they were judged
 * when the item was raised, and the terminal judges `HEAD` again (`docs/canon/proposal-drift.md`
 * "Approve"). Fixed words and the checked ID only; null when the ID is not a proposal ID: no command
 * is offered for it.
 */
export function driftCommands(id: string): DriftCommand[] | null {
  if (!PROPOSAL_ID.test(id)) {
    return null;
  }
  return [
    {
      action: "approve",
      meaning: "Approve: shows the rows, asks [y/N], then locks the node and its code at HEAD as spec lock accept does, in a commit of spec.lock alone",
      text: `spec approve ${id}`,
    },
    {
      action: "editorial",
      meaning:
        "Approve as editorial: only when the node's text changed at its locked revision and keeps its meaning; rows marked Editorial were so when the item was raised, and the terminal judges HEAD again; a plain approve refused for that names this command",
      text: `spec approve ${id} --editorial`,
    },
    {
      action: "reject",
      meaning: "Reject: spec.lock stays as it is, and the same evidence is not queued again; type your reason in place of ...",
      text: `spec reject ${id} --reason "..."`,
    },
  ];
}

/** How many rows the evidence counts, the stored ones (whatever their shape) and those past them. */
export function driftRowsText(drift: DriftEvidence): string {
  const total = drift.rows.length + drift.more;
  const rows = `${String(total)} ${total === 1 ? "row" : "rows"}`;
  return drift.more > 0 ? `${rows}, the first ${String(drift.rows.length)} stored with the item` : rows;
}

/** A row as sent: its compact JSON, for a row not in the shape `storedRowOf` reads. */
export function driftRowText(row: unknown): string {
  return JSON.stringify(row);
}

// A stored row's shape (`docs/canon/proposal-drift-item.md` "The item"; the row:
// `docs/canon/spec-verify-output.md` "JSON"), checked when it is drawn: the store refuses a row
// missing a key core's `DriftRow` reads, yet the card never trusts it to. Each key is checked by
// name from a list, never by a field compared here; a later key is ignored.

/** Whether a stored value fits a key. */
type Fits = (value: unknown) => boolean;

/** A JSON object's fields by key, or null for any other value (an array, null, a scalar). */
function fieldsOf(value: unknown): ReadonlyMap<string, unknown> | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return null;
  }
  return new Map(Object.entries(value as { readonly [key: string]: unknown }));
}

const text: Fits = (value) => typeof value === "string";
const flag: Fits = (value) => typeof value === "boolean";
const count: Fits = (value) => typeof value === "number" && Number.isSafeInteger(value) && value >= 0;

function orNull(fits: Fits): Fits {
  return (value) => value === null || fits(value);
}

function listOf(fits: Fits): Fits {
  return (value) => Array.isArray(value) && value.every((item) => fits(item));
}

/** An object holding every key listed, each fitting; other keys ignored. */
function holding(keys: readonly (readonly [string, Fits])[]): Fits {
  return (value) => {
    const fields = fieldsOf(value);
    return fields !== null && keys.every(([key, fits]) => fields.has(key) && fits(fields.get(key)));
  };
}

/** A binding's place as an anchor names it. */
const place = holding([
  ["file", text],
  ["kind", text],
  ["symbol", text],
  ["item_kind", orNull(text)],
]);

/** The 15 keys core's `drift::DriftRow` reads: always stored, a null value none. */
const GUARANTEED: readonly (readonly [string, Fits])[] = [
  ["node", text],
  ["relation", text],
  ["file", text],
  ["kind", orNull(text)],
  ["symbol", orNull(text)],
  ["item_kind", orNull(text)],
  ["state", text],
  ["category", orNull(text)],
  ["detail", orNull(text)],
  ["class", orNull(text)],
  ["pre_existing", flag],
  [
    "markers",
    listOf(
      holding([
        ["line", count],
        ["rev", orNull(count)],
      ]),
    ),
  ],
  ["node_rev", orNull(count)],
  [
    "lock",
    orNull(
      holding([
        ["rev", count],
        ["commit", text],
        ["verified_at", text],
      ]),
    ),
  ],
  ["compared", orNull(text)],
];

/** `levels` and `anchor`: absent from an item an older build raised; when there, null or their shape. */
const KEPT_AS_STORED: readonly (readonly [string, Fits])[] = [
  [
    "levels",
    orNull(
      holding([
        ["lock", listOf(text)],
        ["markers", listOf(text)],
        ["changed", orNull(listOf(text))],
      ]),
    ),
  ],
  [
    "anchor",
    orNull(
      holding([
        ["state", text],
        ["from", orNull(place)],
        ["candidates", orNull(listOf(place))],
      ]),
    ),
  ],
];

/**
 * A stored row typed, when it has the shape: an object holding the 15 guaranteed keys, each value
 * of its type, and `levels` and `anchor` absent or fitting. Else null: the card shows the row as
 * sent, and it still counts.
 */
export function storedRowOf(row: unknown): StoredDriftRow | null {
  const fields = fieldsOf(row);
  if (fields === null) {
    return null;
  }
  const fits =
    GUARANTEED.every(([key, fitting]) => fields.has(key) && fitting(fields.get(key))) &&
    KEPT_AS_STORED.every(([key, fitting]) => !fields.has(key) || fitting(fields.get(key)));
  return fits ? (row as StoredDriftRow) : null;
}
