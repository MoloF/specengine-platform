import type { HistoryEntry, KnownHistoryChange } from "../api/types";
import type { Translate } from "../i18n";
import { slot } from "../i18n/fill";

// A node's history in words (docs/features/node-history.md "Acceptance criteria", NH13): the one
// map of a served `change` word, here alone. "Version N" is said openly; a wording fix keeps its
// version; a commit with no apply trailers is an edit outside a proposal; a task accepted on the node
// and an item applied to it name their ID and their record, an item with none no record. A word
// this build has no sentence for is shown as served, under `lang="en"`. Who is said by `by_name` as
// served, core's name of the identity: `by` (`Name <email>`) and a commit's OID are never drawn.

/** The element a sentence holds in its slot: the task's ID (a Board link) or the item's record (a document link). */
export type WhatSlot = { name: "id"; id: string } | { name: "record"; record: string };

/** What an entry did as the chrome says it: a sentence, its slot filled by the part; or the served word, raw. */
export type HistoryWhat = { raw: false; text: string; slot: WhatSlot | null } | { raw: true; text: string };

/** The served word as it came, for a change this build has no sentence for, or a revision the read could not tell. */
function served(entry: HistoryEntry): HistoryWhat {
  return { raw: true, text: entry.change };
}

/** One sentence, no slot. */
function said(text: string): HistoryWhat {
  return { raw: false, text, slot: null };
}

/** An item's sentence: in its record when it has one, else without (`with` and `without` its two keys). */
function item(entry: HistoryEntry, withRecord: string, without: string): HistoryWhat {
  return entry.record === null ? said(without) : { raw: false, text: withRecord, slot: { name: "record", record: entry.record } };
}

/** A known change's sentence, its revision in it where it says one. */
type Sentence = (entry: HistoryEntry, t: Translate) => HistoryWhat;

/** The one table of the change words this build has words for: a version, a wording fix, an edit outside a proposal, a task, an item. */
const SENTENCES: Readonly<Record<KnownHistoryChange, Sentence>> = {
  raised: (entry, t) => (entry.rev === null ? served(entry) : said(t("doc.history.version", { rev: entry.rev }))),
  applied: (entry, t) => (entry.rev === null ? served(entry) : said(t("doc.history.version", { rev: entry.rev }))),
  editorial: (entry, t) => (entry.rev === null ? served(entry) : said(t("doc.history.wording", { rev: entry.rev }))),
  hand: (_entry, t) => said(t("doc.history.outside")),
  accepted: (entry, t) => ({ raw: false, text: t("doc.history.task", { id: slot("id") }), slot: { name: "id", id: entry.id } }),
  question: (entry, t) => item(entry, t("doc.history.question", { record: slot("record") }), t("doc.history.questionPlain")),
  discrepancy: (entry, t) => item(entry, t("doc.history.discrepancy", { record: slot("record") }), t("doc.history.discrepancyPlain")),
  decision: (entry, t) => item(entry, t("doc.history.decision", { record: slot("record") }), t("doc.history.decisionPlain")),
};

/** A change word this build has a sentence for. */
function isKnownChange(change: string): change is KnownHistoryChange {
  return Object.hasOwn(SENTENCES, change);
}

/** The sentence an entry's `change` takes; a word the table does not hold, served raw. */
export function historyWhat(entry: HistoryEntry, t: Translate): HistoryWhat {
  return isKnownChange(entry.change) ? SENTENCES[entry.change](entry, t) : served(entry);
}

/** The line under an entry: an apply's rationale, else a hand commit's subject or the task's title. */
export function historyText(entry: HistoryEntry): string | null {
  return entry.rationale ?? entry.title;
}

/** The read's notes on the history: `history: <path>: not read: <cause>`, or the queue's, as the CLI's `note: history:` lines. */
export function historyNotes(notes: readonly string[]): string[] {
  return notes.filter((note) => note.startsWith("history: "));
}
