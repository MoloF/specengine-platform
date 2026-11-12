import { describe, expect, it } from "vitest";
import { en } from "../i18n/en";
import { ru } from "../i18n/ru";
import { aHistoryEntry } from "../test/builders";
import { enT } from "../test/text";
import { historyNotes, historyText, historyWhat } from "./history";

// NH13 of docs/features/node-history.md "Acceptance criteria": the one map of a served change word,
// the line under an entry, the read's history notes; the part's keys in both catalogs.

/** The sentence as drawn, a slot shown by its name, and what fills it. */
function words(fields: Parameters<typeof aHistoryEntry>[0]): [string, boolean, string | null] {
  const what = historyWhat(aHistoryEntry(fields), enT);
  const slot = what.raw || what.slot === null ? null : what.slot.name === "id" ? `id ${what.slot.id}` : `record ${what.slot.record}`;
  return [what.text.replaceAll("\u0000", ""), what.raw, slot];
}

describe("a change in words (NH13 of node-history)", () => {
  it("says a version openly, a wording fix keeping its version, an edit outside a proposal; an apply with no revision of its own as its version", () => {
    expect(words({ id: "a", change: "raised", rev: 2, rev_before: 1 })).toEqual(["Version 2", false, null]);
    expect(words({ id: "a", change: "applied", rev: 3, rev_before: 3 })).toEqual(["Version 3", false, null]);
    expect(words({ id: "a", change: "editorial", rev: 2, rev_before: 2 })).toEqual(["Wording fixed, version 2 kept", false, null]);
    expect(words({ id: "a", change: "hand" })).toEqual(["Edited outside a proposal", false, null]);
  });

  it("names a task by its ID and an item by its record, each a slot", () => {
    expect(words({ id: "C-0001", source: "card", change: "accepted" })).toEqual(["Task id accepted", false, "id C-0001"]);
    expect(words({ id: "PR-0040", source: "item", change: "question", record: "DEC-0031" })).toEqual(["Question answered in record", false, "record DEC-0031"]);
    expect(words({ id: "PR-0041", source: "item", change: "discrepancy", record: "DEC-0032" })).toEqual(["Discrepancy settled in record", false, "record DEC-0032"]);
    expect(words({ id: "PR-0042", source: "item", change: "decision", record: "DEC-0033" })).toEqual(["Decision recorded in record", false, "record DEC-0033"]);
  });

  it("says an item with no record without one, never its own ID in the record's place (M: the ID in the slot)", () => {
    expect(words({ id: "PR-0040", source: "item", change: "question", record: null })).toEqual(["Question answered", false, null]);
    expect(words({ id: "PR-0041", source: "item", change: "discrepancy", record: null })).toEqual(["Discrepancy settled", false, null]);
    expect(words({ id: "PR-0042", source: "item", change: "decision", record: null })).toEqual(["Decision recorded", false, null]);
  });

  it("keeps a word it does not know as served, and a version the read could not tell, raw", () => {
    expect(words({ id: "a", change: "squashed" })).toEqual(["squashed", true, null]);
    expect(words({ id: "a", change: "raised", rev: null })).toEqual(["raised", true, null]);
    // A word inherited from Object is no sentence of the table's.
    expect(words({ id: "a", change: "toString" })).toEqual(["toString", true, null]);
  });
});

describe("the line, the notes (NH13 of node-history)", () => {
  it("puts an apply's rationale under it, else a hand commit's subject or a task's title, else nothing", () => {
    expect(historyText(aHistoryEntry({ id: "a", rationale: "Tighten the window.", title: null }))).toBe("Tighten the window.");
    expect(historyText(aHistoryEntry({ id: "a", title: "Add the rules" }))).toBe("Add the rules");
    expect(historyText(aHistoryEntry({ id: "a" }))).toBeNull();
  });

  it("keeps the read's `history:` notes alone, in order", () => {
    expect(historyNotes(["bindings: off", "history: docs/a.md: not read: a shallow clone", "history: the queue not read: locked; commits only", "historyless"])).toEqual([
      "history: docs/a.md: not read: a shallow clone",
      "history: the queue not read: locked; commits only",
    ]);
  });
});

describe("the part's keys in both catalogs (NH13 of node-history)", () => {
  const KEYS = ["title", "version", "wording", "outside", "created", "task", "question", "discrepancy", "decision", "by", "proposedBy", "none", "truncated"] as const;

  it("holds the thirteen in English and in Russian, each with the same placeholders, plus the read's two lines and an item's three with no record (M: a key missing in ru.ts)", () => {
    const placeholders = (value: string) => [...value.matchAll(/\{\{(\w+)\}\}/g)].map((match) => match[1]).sort();
    for (const key of KEYS) {
      expect([key, typeof en.doc.history[key], typeof ru.doc.history[key]]).toEqual([key, "string", "string"]);
      expect([key, placeholders(ru.doc.history[key])]).toEqual([key, placeholders(en.doc.history[key])]);
    }
    expect(Object.keys(en.doc.history).sort()).toEqual([...KEYS, "reading", "failed", "questionPlain", "discrepancyPlain", "decisionPlain"].sort());
    expect(Object.keys(ru.doc.history).sort()).toEqual(Object.keys(en.doc.history).sort());
  });

  it("says no writer, card or workspace in English", () => {
    for (const value of Object.values(en.doc.history)) {
      expect([value, /\b(writers?|cards?|workspaces?)\b/i.test(value)]).toEqual([value, false]);
    }
  });
});
