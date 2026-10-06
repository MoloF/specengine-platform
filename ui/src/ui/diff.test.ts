import { describe, expect, it } from "vitest";
import { diffLines, removesAll } from "./diff";

describe("diffLines", () => {
  it("styles each line of the daemon's hunks and keeps its text verbatim", () => {
    const diff = [
      "--- base a.md",
      "+++ proposed a.md",
      "@@ -1,3 +1,3 @@",
      " keep",
      "--- a removed list item",
      "+++ an added line",
      "\\ No newline at end of file",
    ].join("\n");
    expect(diffLines(diff)).toEqual([
      { type: "file", text: "--- base a.md" },
      { type: "file", text: "+++ proposed a.md" },
      { type: "hunk", text: "@@ -1,3 +1,3 @@" },
      { type: "context", text: " keep" },
      { type: "removed", text: "--- a removed list item" },
      { type: "added", text: "+++ an added line" },
      { type: "note", text: "\\ No newline at end of file" },
    ]);
  });

  it("drops one final newline: it ends the last line, it is not an empty context line", () => {
    expect(diffLines("@@ -1 +1 @@\n-a\n+b\n")).toEqual([
      { type: "hunk", text: "@@ -1 +1 @@" },
      { type: "removed", text: "-a" },
      { type: "added", text: "+b" },
    ]);
    expect(diffLines("@@ -1 +1 @@\n+b\n\n").map((line) => line.type)).toEqual(["hunk", "added", "context"]);
    expect(diffLines("")).toEqual([{ type: "context", text: "" }]);
  });
});

describe("removesAll", () => {
  it("is true when every hunk's new side is empty, headers or a cut kept", () => {
    expect(removesAll("--- snapshot a.md\n+++ current a.md\n@@ -1,4 +0,0 @@\n-## R-1: Lights\n-\n-Lit at night.\n-Dark closes.\n")).toBe(true);
    expect(removesAll("@@ -1 +0,0 @@\n-only line\n")).toBe(true);
  });

  it("is false for a change, an addition, a deletion inside a kept text, or no hunk at all", () => {
    expect(removesAll("@@ -5,3 +5,4 @@\n keep\n-old\n+new\n+more\n")).toBe(false);
    expect(removesAll("@@ -0,0 +1,2 @@\n+a\n+b\n")).toBe(false);
    expect(removesAll("@@ -1,4 +0,0 @@\n-a\n@@ -9,4 +5,3 @@\n keep\n-b\n")).toBe(false);
    expect(removesAll("@@ -3,5 +3,3 @@\n keep\n-a\n-b\n keep\n")).toBe(false);
    expect(removesAll("")).toBe(false);
    expect(removesAll("-@@ -1,4 +0,0 @@ written in the text\n")).toBe(false);
  });
});
