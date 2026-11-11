import { describe, expect, it } from "vitest";
import { en } from "../i18n/en";
import { ru } from "../i18n/ru";

// BH-18 of board-handoff: "Accept" stays the asking side's word and act (docs/canon/card-flows.md
// "The rule"), so no word of a hand-off, a take, a return or the rollup is built on it: in English no
// "accept", in Russian not the stem the Russian Accept is built on, written here with escapes alone.

/** The Russian stem of "accept": the verb the Russian catalog's Accept is built on. */
const RUSSIAN_STEM = "\u043f\u0440\u0438\u043d\u044f";

/** Every leaf under a group, by its dotted path. */
function leaves(group: unknown, path: string): [string, string][] {
  if (typeof group === "string") {
    return [[path, group]];
  }
  if (typeof group !== "object" || group === null) {
    return [];
  }
  return Object.entries(group).flatMap(([key, value]) => leaves(value, `${path}.${key}`));
}

/** The words of a hand-off, a return and the rollup's three in one catalog. */
function words(catalog: typeof en | typeof ru): [string, string][] {
  return [
    ...leaves(catalog.board.handoff, "board.handoff"),
    ...leaves(catalog.board.return, "board.return"),
    ["board.rollup.handedOff", catalog.board.rollup.handedOff],
    ["board.rollup.taken", catalog.board.rollup.taken],
    ["board.rollup.returned", catalog.board.rollup.returned],
  ];
}

describe("the hand-off's words (BH-18)", () => {
  it("cover every hand-off and return key and the rollup's three, in both languages", () => {
    expect(words(en).map(([path]) => path)).toEqual(words(ru).map(([path]) => path));
    expect(words(en).length).toBeGreaterThan(15);
  });

  it("hold no `accept` in English, case-folded", () => {
    expect(words(en).filter(([, value]) => value.toLowerCase().includes("accept"))).toEqual([]);
  });

  it("hold not the Russian stem of accept in Russian, case-folded (M21)", () => {
    expect(RUSSIAN_STEM).toHaveLength(5);
    expect(ru.board.panel.accept.toLowerCase()).toContain(RUSSIAN_STEM);
    expect(words(ru).filter(([, value]) => value.toLowerCase().includes(RUSSIAN_STEM))).toEqual([]);
  });
});

// docs/features/handoff-progress.md "Acceptance criteria", HP-4: "Ready to accept" is the owner's
// word (v5 Q3), a status beside Accept; it sits under `board.progress`, outside the scan above.
describe("Ready to accept, outside the guard's scan (HP-4)", () => {
  it("says Ready to accept, and no board.progress key is among the scanned words", () => {
    expect(en.board.progress.ready).toBe("Ready to accept");
    expect(words(en).filter(([path]) => path.startsWith("board.progress"))).toEqual([]);
    expect(words(ru).filter(([path]) => path.startsWith("board.progress"))).toEqual([]);
  });
});

// docs/features/task-mandate.md "Acceptance criteria", V1 and V2: a mandate's words are no status. "Binding" and "Department decides" match no state the page labels
// (a task's, an item's, a hand-off's, a progress word), so "Approved" and "Open" are out; and no word
// of the mandate, the split's choice, the Activity's or the parent's part is built on accept, in
// English or on the Russian stem, as the hand-off's are not.
describe("the mandate's words (V1, V2 of task-mandate)", () => {
  /** The mandate's words in one catalog: its group, the split's two, the Activity's and the parent's part. */
  function mandateWords(catalog: typeof en | typeof ru): [string, string][] {
    return [
      ...leaves(catalog.board.mandate, "board.mandate"),
      ["board.split.mandateFor", catalog.board.split.mandateFor],
      ["board.split.mandateForDiscipline", catalog.board.split.mandateForDiscipline],
      ["board.split.mandateNeeded", catalog.board.split.mandateNeeded],
      ["board.history.mandate", catalog.board.history.mandate],
      ...leaves(catalog.board.fromSubtasks, "board.fromSubtasks"),
    ];
  }

  /** Every status label the page shows, case-folded: a task's, an item's, a hand-off's and a progress word's. */
  function statusLabels(catalog: typeof en | typeof ru): string[] {
    return [
      catalog.board.open,
      catalog.board.accepted,
      catalog.board.cancelled,
      catalog.board.overdue,
      catalog.board.return.badge,
      catalog.board.rollup.handedOff,
      catalog.board.rollup.taken,
      catalog.board.rollup.returned,
      catalog.board.progress.ready,
      ...leaves(catalog.inbox.status, "inbox.status").map(([, value]) => value),
      ...leaves(catalog.tasks.status, "tasks.status").map(([, value]) => value),
    ].map((value) => value.toLowerCase());
  }

  it("cover the same keys in both languages", () => {
    expect(mandateWords(en).map(([path]) => path)).toEqual(mandateWords(ru).map(([path]) => path));
    expect(mandateWords(en).length).toBeGreaterThan(15);
  });

  it("match no status label, in either language: never Approved or Open (M: binding labelled Approved)", () => {
    for (const catalog of [en, ru]) {
      const labels = statusLabels(catalog);
      expect(labels).toContain(catalog.inbox.status.approved.toLowerCase());
      expect([catalog.board.mandate.binding, catalog.board.mandate.open, catalog.board.fromSubtasks.departure].filter((word) => labels.includes(word.toLowerCase()))).toEqual([]);
    }
    expect([en.board.mandate.binding, en.board.mandate.open]).toEqual(["Binding", "Department decides"]);
    for (const word of [en.board.mandate.binding, en.board.mandate.open]) {
      expect(["approved", "open"]).not.toContain(word.toLowerCase());
    }
  });

  it("hold no `accept` in English, nor the Russian stem in Russian", () => {
    expect(mandateWords(en).filter(([, value]) => value.toLowerCase().includes("accept"))).toEqual([]);
    expect(mandateWords(ru).filter(([, value]) => value.toLowerCase().includes(RUSSIAN_STEM))).toEqual([]);
  });

  /** Acceptance's two verdicts and the three marks a task wears, case-folded: As briefed, Departs from it, Suspect, Departed, Correcting. */
  function verdictWords(catalog: typeof en | typeof ru): string[] {
    return [catalog.board.dialog.asBriefed, catalog.board.dialog.departsFromIt, ...leaves(catalog.board.mark, "board.mark").map(([, value]) => value)].map((value) => value.toLowerCase());
  }

  it("match no acceptance verdict or mark, in either language, none holding another (M: binding worded as As briefed, the departure as Departed)", () => {
    for (const catalog of [en, ru]) {
      const verdicts = verdictWords(catalog);
      expect(verdicts).toHaveLength(5);
      const shown = [catalog.board.mandate.label, catalog.board.mandate.binding, catalog.board.mandate.open, catalog.board.fromSubtasks.departure].map((word) => word.toLowerCase());
      expect(shown.filter((word) => verdicts.some((verdict) => word.includes(verdict) || verdict.includes(word)))).toEqual([]);
    }
  });

  it("name the field apart from its values: no value repeats a word of the field's name, in either language", () => {
    for (const catalog of [en, ru]) {
      const named = catalog.board.mandate.label.toLowerCase().split(" ").filter((word) => word.length > 3);
      expect(named.length).toBeGreaterThan(0);
      const values = [catalog.board.mandate.binding, catalog.board.mandate.open].map((word) => word.toLowerCase());
      expect(values.filter((value) => named.some((word) => value.includes(word)))).toEqual([]);
    }
  });

  // The Russian words the orchestrator chose for the mandate in task-mandate's second UI pass: the field
  // "Who decides", binding "Task-setter", open "Department", written in escapes; no mandate key keeps a
  // word built on the first pass's "Authority" or "Binding" stems.
  it("in Russian: the field, binding and open as chosen; no key keeps an earlier stem", () => {
    expect([ru.board.mandate.label, ru.board.mandate.binding, ru.board.mandate.open]).toEqual([
      "\u041a\u0442\u043e \u0440\u0435\u0448\u0430\u0435\u0442",
      "\u041f\u043e\u0441\u0442\u0430\u043d\u043e\u0432\u0449\u0438\u043a",
      "\u041e\u0442\u0434\u0435\u043b",
    ]);
    const earlier = ["\u043f\u043e\u043b\u043d\u043e\u043c\u043e\u0447", "\u043e\u0431\u044f\u0437\u044b\u0432\u0430"];
    expect(mandateWords(ru).filter(([, value]) => earlier.some((stem) => value.toLowerCase().includes(stem)))).toEqual([]);
  });
});
