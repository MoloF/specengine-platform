import { describe, expect, it } from "vitest";
import { parseHash, sectionHash } from "./routes";

describe("hash routes", () => {
  it.each([
    ["", { type: "home" }],
    ["#", { type: "home" }],
    ["#/", { type: "home" }],
    ["#/alpha/inbox", { type: "section", project: "alpha", section: "inbox", id: null }],
    ["#/alpha/inbox/", { type: "section", project: "alpha", section: "inbox", id: null }],
    ["#/alpha/inbox/PR-0042", { type: "section", project: "alpha", section: "inbox", id: "PR-0042" }],
    ["#/alpha/tree/R-12", { type: "section", project: "alpha", section: "tree", id: "R-12" }],
    ["#/alpha", { type: "not_found" }],
    ["#/alpha/nowhere", { type: "not_found" }],
    ["#/alpha/inbox/PR-1/extra", { type: "not_found" }],
    ["#main", { type: "not_found" }],
    ["#/alpha/inbox/%E0%A4%A", { type: "not_found" }],
  ])("reads %j", (hash, route) => {
    expect(parseHash(hash)).toEqual(route);
  });

  it("writes what it reads", () => {
    expect(sectionHash("alpha", "inbox")).toBe("#/alpha/inbox");
    expect(sectionHash("alpha", "inbox", "PR-1")).toBe("#/alpha/inbox/PR-1");
    expect(parseHash(sectionHash("a b", "graph", "X/Y"))).toEqual({
      type: "section",
      project: "a b",
      section: "graph",
      id: "X/Y",
    });
  });
});

// AC-05 of docs/features/ui-tree-node.md: a REF is one segment, encoded by sectionHash, decoded
// once by parseHash: an ID, `slug/ID`, `ID#SECTION` (%23) or a root-relative path (%2F).
describe("the tree's REF in the hash", () => {
  it.each([
    ["MEC-TIDES", "#/harbor-sim/tree/MEC-TIDES"],
    ["MEC-TIDES#RULE-TIDE-WINDOW", "#/harbor-sim/tree/MEC-TIDES%23RULE-TIDE-WINDOW"],
    ["tide-cycle/MEC-TIDES", "#/harbor-sim/tree/tide-cycle%2FMEC-TIDES"],
    ["docs/spec/tides/tide-cycle.md", "#/harbor-sim/tree/docs%2Fspec%2Ftides%2Ftide-cycle.md"],
  ])("writes %s as one segment and reads it back", (ref, hash) => {
    expect(sectionHash("harbor-sim", "tree", ref)).toBe(hash);
    expect(parseHash(hash)).toEqual({ type: "section", project: "harbor-sim", section: "tree", id: ref });
  });

  it("decodes once: %2523 stays %23", () => {
    expect(parseHash("#/p/tree/A%2523B")).toEqual({ type: "section", project: "p", section: "tree", id: "A%23B" });
  });

  it("refuses an unencoded slash as an extra segment", () => {
    expect(parseHash("#/p/tree/docs/spec/a.md")).toEqual({ type: "not_found" });
  });
});
