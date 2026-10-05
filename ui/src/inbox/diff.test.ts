import { describe, expect, it } from "vitest";
import { diffLines } from "./diff";

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
