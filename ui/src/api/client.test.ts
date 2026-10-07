import { describe, expect, it } from "vitest";
import source from "./client.ts?raw";

// AC-01 of docs/features/ui-tree-node.md: each read names its endpoint in the comment above it;
// one the daemon does not serve yet would be flagged MISSING ENDPOINT (named for rust-developer):
// none since ui-live-tasks. The graph's, the check's and the tasks' name the daemon's README (AC-10
// of docs/features/ui-live.md, AC-08 of docs/features/ui-live-tasks.md, amending AC-01 of
// ui-graph, ui-health and ui-tasks).

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
    // AC-14 of docs/features/decision-staging.md: the stage's two writes on one path.
    ["stageDecision", '/** POST /api/projects/:p/proposals/:id/decision (docs/canon/decision-staging.md "Daemon"; stages, nothing applied) */'],
    ["unstageDecision", '/** DELETE /api/projects/:p/proposals/:id/decision (docs/canon/decision-staging.md "Daemon"; nothing staged: no event) */'],
    // AC-10 of docs/features/ui-live.md (AC-01 of ui-graph, amended).
    ["getGraph", '/** GET /api/projects/:p/graph (crates/specengine-http/README.md "Endpoints"; the browser view, uncut) */'],
    // AC-08 of docs/features/ui-live-tasks.md (AC-01 of ui-tasks, amended): the two comments of its "Data".
    ["getTasks", '/** GET /api/projects/:p/tasks (crates/specengine-http/README.md "Endpoints"; = spec task list --json) */'],
    [
      "getTask",
      '/** GET /api/projects/:p/tasks/:id (crates/specengine-http/README.md "Endpoints"; = spec task show T --json, uncut; 404 the exit-1 document) */',
    ],
    // AC-10 of docs/features/ui-live.md (AC-01 of ui-health, amended).
    ["getCheck", '/** GET /api/projects/:p/check (crates/specengine-http/README.md "Endpoints"; every verdict a 200 document) */'],
  ])("%s says %s", (method, comment) => {
    expect(commentAbove(method)).toBe(comment);
  });

  it("flags no read MISSING ENDPOINT: the daemon serves every one (AC-08 of ui-live-tasks)", () => {
    expect(source).not.toContain("MISSING ENDPOINT");
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
    expect(names).toContain("stageDecision");
    expect(names.filter((name) => /transition|approve|cancel|claim/i.test(name))).toEqual([]);
  });

  it("write only a stage: stageDecision and unstageDecision, no decide, apply or reject member, neither taking an AbortSignal (AC-14 of decision-staging)", () => {
    const names = members();
    expect(names.filter((name) => /stage|decide|decision|apply|reject/i.test(name))).toEqual(["stageDecision", "unstageDecision"]);
    expect(source).toContain("stageDecision(project: string, id: string, stage: StageChoice, updatedAt: string): Promise<Proposal>;");
    expect(source).toContain("unstageDecision(project: string, id: string): Promise<Proposal>;");
  });

  it("type a package read as the package or the exit-1 document, each read taking its query's AbortSignal", () => {
    expect(source).toContain("getTask(project: string, id: string, signal?: AbortSignal): Promise<TaskPackage | TaskNotFound>;");
    expect(source).toContain("getTasks(project: string, signal?: AbortSignal): Promise<TaskList>;");
  });
});

describe("the check's read (AC-01 of ui-health)", () => {
  it("is one read typed as the report, taking no AbortSignal (a walk in flight is never aborted), and no member applies a fix", () => {
    expect(source).toContain("getCheck(project: string): Promise<CheckReport>;");
    const block = /export interface SpecEngineClient \{([\s\S]*?)\n\}/.exec(source)?.[1] ?? "";
    const names = [...block.matchAll(/^\s+(?:readonly\s+)?(\w+)\??[(:]/gm)].map((match) => match[1] ?? "");
    expect(names.filter((name) => /fix|apply|check/i.test(name))).toEqual(["getCheck"]);
  });
});
