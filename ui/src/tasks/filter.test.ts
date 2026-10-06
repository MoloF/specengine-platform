import { describe, expect, it } from "vitest";
import { aTaskEntry } from "../test/builders";
import { SOME_TASKS } from "../test/taskStub";
import { chipKeyOf, chipsOf, DEFAULT_FILTERS, folded, isPressed, matchesFilters, toggleChip } from "./filter";

// AC-05 of docs/features/ui-tasks.md: a chip per state present (done and cancelled one Closed),
// counts over the whole answer, all pressed but Closed; "Spec changed"; a text filter, NFC.

function visible(filters = DEFAULT_FILTERS, entries = SOME_TASKS): string[] {
  return entries.filter((entry) => matchesFilters(entry, filters)).map((entry) => entry.id);
}

describe("chips (AC-05)", () => {
  it("one per state present in the groups' order, Closed for done and cancelled, the unknown raw, counts over the whole answer", () => {
    expect(chipsOf(SOME_TASKS).map((chip) => [chip.label, chip.count])).toEqual([
      ["Plan review", 1],
      ["Draft", 1],
      ["Changes requested", 1],
      ["Ready", 2],
      ["In progress", 1],
      ["Closed", 2],
      ["triage", 1],
    ]);
  });

  it("are all pressed but Closed by default", () => {
    expect(chipsOf(SOME_TASKS).map((chip) => isPressed(DEFAULT_FILTERS, chip.key))).toEqual([true, true, true, true, true, false, true]);
    expect(visible()).toEqual(["T-0002", "T-0003", "T-0004", "T-0005", "T-0006", "T-0007", "T-0009"]);
  });

  it("keep a state named closed apart from the Closed chip", () => {
    expect(chipKeyOf("closed")).not.toBe(chipKeyOf("done"));
    expect(chipKeyOf("done")).toBe(chipKeyOf("cancelled"));
  });

  it("toggle one state at a time", () => {
    const withClosed = toggleChip(DEFAULT_FILTERS, chipKeyOf("done"));
    expect(visible(withClosed)).toEqual(["T-0001", "T-0002", "T-0003", "T-0004", "T-0005", "T-0006", "T-0007", "T-0008", "T-0009"]);
    const noReady = toggleChip(withClosed, chipKeyOf("ready"));
    expect(visible(noReady)).toEqual(["T-0001", "T-0002", "T-0005", "T-0006", "T-0007", "T-0008", "T-0009"]);
  });
});

describe("Spec changed and the text (AC-05)", () => {
  it("Spec changed keeps only tasks whose stale is true", () => {
    expect(visible({ ...DEFAULT_FILTERS, changedOnly: true })).toEqual(["T-0003", "T-0006"]);
  });

  it("matches ID, title and targets, case-folded", () => {
    expect(visible({ ...DEFAULT_FILTERS, query: "t-0004" })).toEqual(["T-0004"]);
    expect(visible({ ...DEFAULT_FILTERS, query: "PLAN TO" })).toEqual(["T-0002"]);
    expect(visible({ ...DEFAULT_FILTERS, query: "r-5" })).toEqual(["T-0002"]);
    expect(visible({ ...DEFAULT_FILTERS, query: "  " })).toHaveLength(7);
  });

  it("finds a precomposed title with a decomposed query, and the other way round", () => {
    const precomposed = "Caf\u00e9 \u0417\u0435\u043b\u0451\u043d\u044b\u0439";
    const decomposed = "cafe\u0301 \u0437\u0435\u043b\u0435\u0308\u043d";
    const entries = [aTaskEntry({ id: "T-0100", status: "draft", title: precomposed }), aTaskEntry({ id: "T-0101", status: "draft", title: "Other" })];
    expect(visible({ ...DEFAULT_FILTERS, query: decomposed }, entries)).toEqual(["T-0100"]);
    const reversed = [aTaskEntry({ id: "T-0102", status: "draft", title: decomposed })];
    expect(visible({ ...DEFAULT_FILTERS, query: precomposed.slice(0, 4) }, reversed)).toEqual(["T-0102"]);
    expect(folded(decomposed)).toBe(folded(decomposed.normalize("NFC")));
  });
});
