import { describe, expect, it } from "vitest";
import { aTrace, aTraceNode } from "../test/builders";
import { holderRow } from "./holderRow";

// docs/features/release-hardening.md "Acceptance criteria", RH-U5: the row of the resolved holder, by
// its canonical ID, an ID-less holder by its path as the row names it, and by its file; never by the
// REF as written.

const PLAIN = "docs/notes/plain.md";

describe("holderRow (RH-U5)", () => {
  const trace = aTrace([aTraceNode("DOM-MOVEMENT", []), aTraceNode("MEC-STAMINA", [], { rollup: "suspect" }), aTraceNode(PLAIN, [], { path: PLAIN, rollup: "no card" })]);

  it("finds a holder's row by its canonical ID and its file, whichever row comes first", () => {
    expect(holderRow(trace, { id: "MEC-STAMINA", path: "docs/spec/MEC-STAMINA.md" })?.rollup).toBe("suspect");
  });

  // An ambiguous bare ID: one read holds two rows of the same ID in two files. M: the path check dropped.
  it("tells two rows of one ID apart by the holder's file", () => {
    const twins = aTrace([
      aTraceNode("R-01", [], { path: "docs/spec/first.md", rollup: "confirmed" }),
      aTraceNode("R-01", [], { path: "docs/spec/second.md", rollup: "suspect" }),
    ]);
    expect(holderRow(twins, { id: "R-01", path: "docs/spec/second.md" })?.rollup).toBe("suspect");
    expect(holderRow(twins, { id: "R-01", path: "docs/spec/first.md" })?.rollup).toBe("confirmed");
    expect(holderRow(twins, { id: "R-01", path: "docs/spec/third.md" })).toBeUndefined();
  });

  it("finds an ID-less holder's row by its path", () => {
    expect(holderRow(trace, { id: null, path: PLAIN })?.rollup).toBe("no card");
  });

  it("finds no row for a holder the read does not hold, never another's, and never matches an ID'd holder by its path", () => {
    expect(holderRow(trace, { id: "MEC-ENDURANCE", path: "docs/spec/MEC-STAMINA.md" })).toBeUndefined();
    expect(holderRow(trace, { id: "MEC-OTHER", path: PLAIN })).toBeUndefined();
  });
});
