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
    // daemon-read serves the bundle, the projects and one proposal (docs/features/daemon-read.md "Data").
    ["getBundle", "/** GET /api/projects/:p/bundle */"],
    ["getProjects", "/** GET /api/projects */"],
    ["getInbox", "/** GET /api/projects/:p/inbox */"],
    ["getProposal", "/** GET /api/projects/:p/proposals/:id */"],
    ["decideProposal", "/** POST /api/projects/:p/proposals/:id/decision */"],
    // AC-01 of docs/features/ui-graph.md.
    ["getGraph", '/** MISSING ENDPOINT GET /api/projects/:p/graph (07 section 3 lists it; uncut; rust-developer, daemon-read "Out of scope") */'],
    // AC-01 of docs/features/ui-tasks.md.
    [
      "getTasks",
      '/** MISSING ENDPOINT GET /api/projects/:p/tasks (07 section 3 lists it; = spec task list --json; rust-developer, daemon-read "Out of scope") */',
    ],
    ["getTask", "/** MISSING ENDPOINT GET /api/projects/:p/tasks/:id (07 section 3 lacks it; = spec task show T --json, uncut) */"],
  ])("%s says %s", (method, comment) => {
    expect(commentAbove(method)).toBe(comment);
  });

  it("names the options exactly as the MCP tools do", () => {
    for (const option of ["root?: string;", "depth?: number;", "kinds?: string[];", "archive?: boolean;", 'with?: "links"[];', "query: string;", "limit?: number;", "node_ids: string[];", "budget?: number;", "ref: string;", "impact?: boolean;", "types?: string[];"]) {
      expect([option, source.includes(option)]).toEqual([option, true]);
    }
  });
});

describe("the graph's options (AC-01 of ui-graph)", () => {
  it("name `spec graph`'s JSON echo keys", () => {
    const block = /export interface GraphOptions \{([^}]*)\}/.exec(source)?.[1] ?? "";
    expect(block.split("\n").map((line) => line.trim()).filter((line) => line !== "")).toEqual([
      "ref: string;",
      "impact?: boolean;",
      "types?: string[];",
      "depth?: number;",
      "archive?: boolean;",
    ]);
  });
});

describe("the tasks' reads (AC-01 of ui-tasks)", () => {
  /** The member names of the SpecEngineClient interface. */
  function members(): string[] {
    const block = /export interface SpecEngineClient \{([\s\S]*?)\n\}/.exec(source)?.[1] ?? "";
    return [...block.matchAll(/^\s+(?:readonly\s+)?(\w+)\??[(:]/gm)].map((match) => match[1] ?? "");
  }

  it("are two reads and no write: no member transitions, approves, cancels or claims a task", () => {
    const names = members();
    expect(names).toContain("getTasks");
    expect(names).toContain("getTask");
    expect(names).toContain("decideProposal");
    expect(names.filter((name) => /transition|approve|cancel|claim/i.test(name))).toEqual([]);
  });

  it("type a package read as the package or the exit-1 document", () => {
    expect(source).toContain("getTask(project: string, id: string): Promise<TaskPackage | TaskNotFound>;");
    expect(source).toContain("getTasks(project: string): Promise<TaskList>;");
  });
});
