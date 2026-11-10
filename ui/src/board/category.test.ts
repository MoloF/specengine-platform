import { describe, expect, it } from "vitest";
import type { CardFlow } from "../api/types";
import { aCardFlow } from "../test/builders";
import { enT } from "../test/text";
import { categoryOf, outsideLook, statusLook, type StatusCategory } from "./category";

// DF-5 of docs/features/design-foundation.md, "Status categories": a column's category comes from its
// place in its served flow, the first matching rule winning: stranded, done (`accept.to`), returned
// (`return`), to do (the first column), in progress. Its look: neutral, success, warning, neutral,
// information, each with its icon; the label the column's served title, the raw ID when stranded.

/** The mock's `plates` flow, as the spec's example. */
const PLATES: CardFlow = aCardFlow({
  name: "plates",
  columns: ["queued", "blocked", "checked", "struck"].map((id) => ({ id, title: id.toUpperCase() })),
  moves: [],
  accept: { from: "checked", to: "struck" },
  return: "blocked",
});

describe("status categories (DF-5)", () => {
  it("runs the table over a flow: queued to do, blocked returned, checked in progress, struck done, an unlisted column stranded", () => {
    const shown = ["queued", "blocked", "checked", "struck", "inked"].map((column) => categoryOf(PLATES, column));
    expect(shown).toEqual<StatusCategory[]>(["todo", "returned", "doing", "done", "stranded"]);
  });

  it("gives each category its appearance and icon, and the column's served title; the raw ID when stranded", () => {
    expect(["queued", "blocked", "checked", "struck", "inked"].map((column) => statusLook(PLATES, column))).toEqual([
      { appearance: "neutral", icon: "open", label: "QUEUED" },
      { appearance: "warning", icon: "cardReturned", label: "BLOCKED" },
      { appearance: "information", icon: "taskInProgress", label: "CHECKED" },
      { appearance: "success", icon: "taskAccepted", label: "STRUCK" },
      { appearance: "neutral", icon: "unknown", label: "inked" },
    ]);
  });

  it("strands every column of a flow not served, its raw ID the label", () => {
    expect([categoryOf(undefined, "queued"), statusLook(undefined, "queued")]).toEqual(["stranded", { appearance: "neutral", icon: "unknown", label: "queued" }]);
  });

  it("reads a one-column flow done, and a return that is the first column returned", () => {
    const single = aCardFlow({ columns: [{ id: "only", title: "Only" }], moves: [], accept: { from: "only", to: "only" } });
    expect(categoryOf(single, "only")).toBe("done");
    const backFirst = aCardFlow({ return: "todo" });
    expect(["todo", "doing", "check", "closed"].map((column) => categoryOf(backFirst, column))).toEqual(["returned", "doing", "doing", "done"]);
  });

  it("puts done before returned, and returned before to do, where a flow names one column twice", () => {
    const doneIsReturn = aCardFlow({ return: "closed" });
    expect(categoryOf(doneIsReturn, "closed")).toBe("done");
  });

  it("draws the board's Not in the flow head stranded's way: neutral, the unknown icon, its words", () => {
    expect(outsideLook(enT)).toEqual({ appearance: "neutral", icon: "unknown", label: "Not in the flow" });
  });
});
