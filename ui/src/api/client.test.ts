import { describe, expect, it } from "vitest";
import source from "./client.ts?raw";

// AC-01 of docs/features/ui-tree-node.md: each read names its endpoint in the comment above it;
// the bundle's is flagged MISSING ENDPOINT until the daemon serves it (named for rust-developer).

function commentAbove(method: string): string {
  const lines = source.split("\n");
  const at = lines.findIndex((line) => new RegExp(`^\\s+${method}\\(`).test(line));
  if (at < 1) {
    throw new Error(`no method ${method}`);
  }
  return (lines[at - 1] ?? "").trim();
}

describe("SpecEngineClient's endpoint comments (AC-01)", () => {
  it.each([
    ["getTree", "/** GET /api/projects/:p/tree */"],
    ["getNode", "/** GET /api/projects/:p/nodes/:ref */"],
    ["search", "/** GET /api/projects/:p/search */"],
    ["getBundle", "/** MISSING ENDPOINT GET /api/projects/:p/bundle (07 §3 lacks it; rust-developer, daemon-read) */"],
  ])("%s says %s", (method, comment) => {
    expect(commentAbove(method)).toBe(comment);
  });

  it("names the options exactly as the MCP tools do", () => {
    for (const option of ["root?: string;", "depth?: number;", "kinds?: string[];", "archive?: boolean;", 'with?: "links"[];', "query: string;", "limit?: number;", "node_ids: string[];", "budget?: number;"]) {
      expect([option, source.includes(option)]).toEqual([option, true]);
    }
  });
});
