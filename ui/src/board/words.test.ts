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
