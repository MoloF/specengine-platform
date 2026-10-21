import { describe, expect, it } from "vitest";
import { aDriftRow } from "../test/builders";
import { rowKeyOf, sideNoteOf } from "./lockDiff";

// docs/features/lock-diff-ui.md "Description and interactions" (Sides): the <why> of a side not
// shown is the rest of the first note starting `lock diff: <key>: no <spec|code> diff: `, raw; a
// note naming the key elsewhere in its text is not one. Rows that share a key (two holders) can
// each have a note: the first one is said for both.

const ROW = aDriftRow({ node: "DOC-A" });
const KEY = "DOC-A | implements | src/lib.rs | item alpha::lib::run function_item";

describe("sideNoteOf (lock-diff-ui \"Description and interactions\")", () => {
  it("names the row by verify's key", () => {
    expect(rowKeyOf(ROW)).toBe(KEY);
  });

  it("takes the first note naming the row and side when several do, its why raw", () => {
    const notes = [`lock diff: ${KEY}: no code diff: other side`, `lock diff: ${KEY}: no spec diff: first\nwhy`, `lock diff: ${KEY}: no spec diff: second why`];
    expect([sideNoteOf(notes, ROW, "spec"), sideNoteOf(notes, ROW, "code")]).toEqual(["first\nwhy", "other side"]);
  });

  it("takes a note only when it starts with the row's prefix", () => {
    expect(sideNoteOf([`note: lock diff: ${KEY}: no spec diff: inside another note`], ROW, "spec")).toBeNull();
  });
});
