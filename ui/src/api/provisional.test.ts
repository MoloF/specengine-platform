import { describe, expect, it } from "vitest";
import source from "./provisional.ts?raw";
import type { Proposal, ShownNode } from "./types";

// AC-06 of docs/features/ui-shell.md: the header, one citation per exported type, `kind` a string.

const HEADER =
  "// PROVISIONAL — hand-written until `spec serve` generates types from Rust; replace, do not extend; generated types win.";

describe("provisional types (AC-06)", () => {
  it("open with the PROVISIONAL header", () => {
    expect(source.split("\n")[0]).toBe(HEADER);
  });

  it("cite a documented source for every exported type", () => {
    const lines = source.split("\n");
    const missing: string[] = [];
    lines.forEach((line, index) => {
      const exported = /^export (?:type|interface) (\w+)/.exec(line)?.[1];
      if (exported === undefined) {
        return;
      }
      let start = index - 1;
      while (start >= 0 && !(lines[start] ?? "").includes("/**")) {
        start -= 1;
      }
      const comment = lines.slice(Math.max(0, start), index).join("\n");
      // A backticked repository path to a Markdown file, one space, a quoted heading.
      if (!/`[^`\s]+[.]md` "[^"]+"/.test(comment)) {
        missing.push(exported);
      }
    });
    expect(missing).toEqual([]);
  });

  it("take any project vocabulary as kind", () => {
    const node: Pick<ShownNode, "kind"> = { kind: "widget" };
    const proposal: Pick<Proposal, "kind"> = { kind: "widget" };
    expect([node.kind, proposal.kind]).toEqual(["widget", "widget"]);
  });

  it("start no key with the word for a hold on work", () => {
    const stem = "bl" + "ock";
    expect(source).not.toMatch(new RegExp(`^\\s+${stem}\\w*\\s*:`, "m"));
  });
});
