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
