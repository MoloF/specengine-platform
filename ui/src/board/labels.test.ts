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

// docs/features/task-mandate.md "Acceptance criteria", V1 and V2: a mandate is looked up here alone.
// Its two stored words, their screen words in both catalogs, the acts each puts after Accept (binding
// Return and Ask, open Record, none none), and the frames of a return and a decision; a word this build
// does not know reads raw, under lang="en", and frames nothing.
describe("a task's mandate (V1, V2 of task-mandate)", () => {
  it("names the two stored words, binding first, each with its screen word in English and Russian; none, none", () => {
    expect(board.MANDATE_WORDS).toEqual(["binding", "open"]);
    expect(board.mandateChoices(enT)).toEqual([
      { value: "binding", label: "Binding" },
      { value: "open", label: "Department decides" },
    ]);
    expect(board.mandateChoices(ruT).map((choice) => choice.label)).toEqual([ru.board.mandate.binding, ru.board.mandate.open]);
    expect([board.mandateWord("binding", enT), board.mandateWord("open", enT), board.mandateWord(null, enT)]).toEqual(["Binding", "Department decides", null]);
    expect(board.mandateMark("binding", enT)).toEqual({ icon: "mandateBinding", label: "Binding" });
    expect(board.mandateMark("open", ruT)).toEqual({ icon: "mandateOpen", label: ru.board.mandate.open });
    expect(board.mandateMark(null, enT)).toBeNull();
  });

  it("puts binding's Return and Ask, open's Record, after Accept; none, nothing (M: binding leading with Record)", () => {
    expect([board.leadActs("binding"), board.leadActs("open"), board.leadActs(null)]).toEqual([["return", "ask"], ["decide"], []]);
    expect([board.asksOnReturn("binding"), board.asksOnReturn("open"), board.asksOnReturn(null)]).toEqual([true, false, false]);
  });

  it("frames Record a decision: binding departs, open asks a why; none neither", () => {
    expect([board.decisionDeparts("binding"), board.decisionDeparts("open"), board.decisionDeparts(null)]).toEqual([true, false, false]);
    expect([board.decisionNeedsWhy("binding"), board.decisionNeedsWhy("open"), board.decisionNeedsWhy(null)]).toEqual([false, true, false]);
    expect([board.mandateAbout("binding", enT), board.mandateAbout("open", enT), board.mandateAbout(null, enT)]).toEqual([
      "Binding: the brief stands. To depart from it, return the task with a question.",
      "Department decides: record each decision and why.",
      null,
    ]);
  });

  it("marks a binding subtask's decision as a departure that warns, an open one's as the department's, neutral; each with its icon", () => {
    expect(board.subtaskMandateLook("binding", enT)).toEqual({ appearance: "warning", icon: "mandateBinding", label: "Departure from a binding task" });
    expect(board.subtaskMandateLook("open", enT)).toEqual({ appearance: "neutral", icon: "mandateOpen", label: "Department decides" });
    expect(board.subtaskMandateLook(null, enT)).toBeNull();
    expect(board.subtaskMandateLook("binding", ruT)?.label).toBe(ru.board.fromSubtasks.departure);
  });

  it("reads a chosen value as a mandate, Choose… and any other word none; a served word it does not know raw, served", () => {
    expect([board.asMandate("binding"), board.asMandate("open"), board.asMandate(""), board.asMandate("Binding"), board.asMandate(" open")]).toEqual(["binding", "open", null, null, null]);
    expect(board.mandateWords("open", enT)).toEqual({ text: "Department decides", served: false });
    expect(board.mandateWords("advisory", enT)).toEqual({ text: "advisory", served: true });
    expect([board.isDecision({ kind: "decision" }), board.isDecision({ kind: "question" }), board.isDecision({ kind: "discrepancy" })]).toEqual([true, false, false]);
  });
});

describe("what changed above a subtask (US-13 of upstream-signal)", () => {
  /** A slot as fill.ts writes it into a sentence. */
  const at = (name: string) => `\u0000${name}\u0000`;

  it("badges a flagged row as information, its own tone and icon, its label from the catalog in both languages", () => {
    expect(board.upstreamLook(enT)).toEqual({ label: "Changed above", tone: "card-upstream", icon: "cardUpstream" });
    expect(board.upstreamLook(ruT).label).toBe(ru.board.upstream.label);
  });

  it("words the parent's two marks with their slots, none for null, an unknown word raw and served", () => {
    expect(board.upstreamMarkLine("suspect", enT)).toEqual({ text: `The parent's document changed at ${at("on")}.`, served: false });
    expect(board.upstreamMarkLine("correcting", enT)).toEqual({ text: `The parent's document changed at ${at("on")}; ${at("correction")} is putting it right.`, served: false });
    expect(board.upstreamMarkLine(null, enT)).toBeNull();
    expect(board.upstreamMarkLine("departed", enT)).toEqual({ text: "departed", served: true });
    expect(board.upstreamMarkLine("suspect", ruT)?.text).toBe(ru.board.upstream.mark.replace("{{on}}", at("on")));
  });

  it("words the three fields, an unknown one raw and served", () => {
    expect(["goal", "targets", "criteria", "title"].map((field) => board.upstreamFieldWords(field, enT))).toEqual([
      { text: "goal", served: false },
      { text: "source", served: false },
      { text: "criteria", served: false },
      { text: "title", served: true },
    ]);
    expect(board.upstreamFieldWords("targets", ruT).text).toBe(ru.board.upstream.field.targets);
  });

  it("words an edit and the three kinds of item with their slots; an unknown source, an unknown kind or none raw and served", () => {
    const line = (source: string, kind: string | null) => board.upstreamChangeLine({ source, kind }, enT);
    expect(line("edit", null)).toEqual({ text: `The parent's ${at("fields")} changed`, served: false });
    expect([line("item", "question").text, line("item", "discrepancy").text, line("item", "decision").text]).toEqual([
      `Question ${at("id")} answered on ${at("nodes")}`,
      `Discrepancy ${at("id")} settled on ${at("nodes")}`,
      `Decision ${at("id")} applied on ${at("nodes")}`,
    ]);
    expect([line("item", "amendment"), line("item", null), line("rename", "decision")]).toEqual([
      { text: "amendment", served: true },
      { text: "item", served: true },
      { text: "rename", served: true },
    ]);
    expect(board.upstreamChangeLine({ source: "item", kind: "decision" }, ruT).text).toBe(ru.board.upstream.decision.replace("{{id}}", at("id")).replace("{{nodes}}", at("nodes")));
  });
});
