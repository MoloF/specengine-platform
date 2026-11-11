import { describe, expect, it } from "vitest";
import { ru } from "../i18n/ru";
import { enT, ruT } from "../test/text";
import * as board from "./labels";

// LG-3 of docs/features/ui-languages.md: under `ruT` every word board/labels.ts exports is ru.ts's
// value at its key, so none is left a literal (M9); an unknown value stays raw, English unchanged.

describe("the Board's words in Russian (LG-3)", () => {
  it("labels the card states and the plain words from ru.ts", () => {
    expect([board.acceptedLook(ruT), board.cancelledLook(ruT), board.overdueLook(ruT), board.openLook(ruT)].map((look) => look.label)).toEqual([
      ru.board.accepted,
      ru.board.cancelled,
      ru.board.overdue,
      ru.board.open,
    ]);
    expect([board.unassigned(ruT), board.byHand(ruT), board.notInTheFlow(ruT), board.noCards(ruT), board.nameIsTyped(ruT), board.nameIsLogin(ruT)]).toEqual([
      ru.board.unassigned,
      ru.board.byHand,
      ru.board.notInTheFlow,
      ru.board.noCards,
      ru.board.nameIsTyped,
      ru.board.nameIsLogin,
    ]);
    for (const act of board.TEAM_ACTS) {
      expect([act, board.actOff(act, ruT)]).toEqual([act, ru.board.actOff[act]]);
    }
  });

  it("words each event, mark and mark line from ru.ts, an unknown one raw", () => {
    expect(board.EVENT_TYPES).toHaveLength(9);
    for (const type of board.EVENT_TYPES) {
      const word = type.replace("card.", "") as keyof typeof ru.board.event;
      expect([type, board.eventWords(type, ruT)]).toEqual([type, ru.board.event[word]]);
    }
    expect(board.eventWords("card.archived", ruT)).toBe("card.archived");
    for (const mark of board.MARK_WORDS) {
      expect([mark, board.markLook({ mark, on: null, discrepancy: null, correction: null }, ruT)?.label]).toEqual([mark, ru.board.mark[mark]]);
    }
    expect(board.cardStateLook({ mark: "accepted", on: null, discrepancy: null, correction: null }, ruT).label).toBe(ru.board.accepted);
    const line = (mark: string, on: string | null, other: string | null) => board.markLine({ mark, on, discrepancy: other, correction: other }, ruT);
    expect(line("suspect", null, null)).toBe(ru.board.markLine.suspect);
    expect(line("suspect", "REQ-1", null)).toBe(ru.board.markLine.suspectAt.replace("{{on}}", "REQ-1"));
    expect(line("departed", null, null)).toBe(ru.board.markLine.departed.replace("{{on}}", ru.board.markLine.itsBrief).replace("{{discrepancy}}", ru.board.markLine.itsDiscrepancy));
    expect(line("correcting", null, null)).toBe(ru.board.markLine.correcting.replace("{{correction}}", ru.board.markLine.aCorrection));
    expect(line("correcting", "REQ-1", "PR-0002")).toBe(ru.board.markLine.correctingAt.replace("{{on}}", "REQ-1").replace("{{correction}}", "PR-0002"));
    expect(board.markLook({ mark: "wobbly", on: null, discrepancy: null, correction: null }, ruT)?.label).toBe("wobbly");
  });

  it("words the gap types, the evidence kinds and a move's needs from ru.ts", () => {
    expect(board.gapTypes(ruT).map(({ word, label }) => [word, label])).toEqual([
      ["contradicts", ru.board.gapType.contradicts],
      ["partial", ru.board.gapType.partial],
      ["missing", ru.board.gapType.missing],
      ["unrequested", ru.board.gapType.unrequested],
    ]);
    expect(board.evidenceKinds(ruT).map(({ word, label, hint }) => [word, label, hint])).toEqual([
      ["url", ru.board.evidence.url.label, ru.board.evidence.url.hint],
      ["path", ru.board.evidence.path.label, ru.board.evidence.path.hint],
      ["build", ru.board.evidence.build.label, ru.board.evidence.build.hint],
      ["note", ru.board.evidence.note.label, ru.board.evidence.note.hint],
    ]);
    expect(board.needsLine(["lead", "qa"], ruT)).toBe(ru.board.needs.replace("{{names}}", `lead${ru.board.or}qa`));
    expect([board.needsLine([], ruT), board.needsLine(["lead", "qa"], enT)]).toEqual([null, "needs lead or qa"]);
  });

  it("keeps the English of the module it replaced", () => {
    expect(board.markLine({ mark: "departed", on: null, discrepancy: null, correction: null }, enT)).toBe("Accepted as departing from its brief; its discrepancy is not settled yet.");
    expect(board.markLine({ mark: "correcting", on: "REQ-1", discrepancy: null, correction: null }, enT)).toBe("The brief changed at REQ-1; a correction is putting it right.");
    expect(board.eventWords("card.reestimated", enT)).toBe("Re-estimated");
  });
});

// U2 of docs/features/department-split.md "Acceptance criteria": where a subtask's targets come from, each served word's line, in both languages; a word this build does not know raw.
describe("a split's target lines", () => {
  it("says by kind with its count, the task's targets, and an unknown word raw and served", () => {
    expect([board.splitFromWords("kinds", 1, enT), board.splitFromWords("kinds", 3, enT), board.splitFromWords("parent", 2, enT)]).toEqual([
      { text: "By kind: 1 document", served: false },
      { text: "By kind: 3 documents", served: false },
      { text: "The task's targets", served: false },
    ]);
    expect([board.splitFromWords("kinds", 3, ruT).text, board.splitFromWords("kinds", 5, ruT).text, board.splitFromWords("parent", 1, ruT).text]).toEqual([
      ru.board.split.byKind_few.replace("{{count}}", "3"),
      ru.board.split.byKind_many.replace("{{count}}", "5"),
      ru.board.split.fromTask,
    ]);
    expect(board.splitFromWords("rows", 4, enT)).toEqual({ text: "rows", served: true });
  });
});
