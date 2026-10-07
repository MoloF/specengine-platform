import { describe, expect, it } from "vitest";
import source from "./provisional.ts?raw";
import { KNOWN_CHECK_VERDICTS } from "./types";
import type {
  BundleItem,
  BundleLayers,
  BundleVia,
  BundleView,
  CheckCause,
  CheckCounts,
  CheckFinding,
  CheckReport,
  DebtEntry,
  Finding,
  FollowedType,
  GraphEdge,
  GraphNode,
  GraphView,
  OwnerNote,
  Proposal,
  SearchHit,
  SearchResults,
  ShownNode,
  Snippet,
  SnapshotDiff,
  SnapshotNode,
  SnapshotPlace,
  SnippetSegment,
  SpecSnapshot,
  TailEntry,
  TaskAssumption,
  TaskBundle,
  TaskClaim,
  TaskCriterion,
  TaskList,
  TaskListEntry,
  TaskNotFound,
  TaskPackage,
  TaskProposal,
  TaskRun,
  TaskTarget,
  TreeNode,
  TreeView,
  WorkingAnswer,
} from "./types";

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

// AC-02 of docs/features/ui-tree-node.md: each new type's keys are exactly the cited document's.
// A record per type must name every key of the type and no other (`satisfies` fails the build
// otherwise); its keys must equal the list copied from the cited heading.

const KEYS = {
  // docs/canon/spec-cli-graph.md "spec tree": JSON {ref, reason, notes, depth, kinds, archive, left_out, truncated, nodes}.
  TreeView: {
    ref: true,
    reason: true,
    notes: true,
    depth: true,
    kinds: true,
    archive: true,
    left_out: true,
    truncated: true,
    nodes: true,
  } satisfies Record<keyof TreeView, true>,
  // Same heading: a node {id, kind, title, path, line, depth, parent, mark, status, rev, tokens_est, archived}.
  TreeNode: {
    id: true,
    kind: true,
    title: true,
    path: true,
    line: true,
    depth: true,
    parent: true,
    mark: true,
    status: true,
    rev: true,
    tokens_est: true,
    archived: true,
  } satisfies Record<keyof TreeNode, true>,
  // crates/specengine-cli/README.md "Output and the cap": search JSON keys.
  SearchResults: {
    archive: true,
    hits: true,
    kinds: true,
    limit: true,
    notes: true,
    query: true,
    tier3_left_out: true,
    truncated: true,
  } satisfies Record<keyof SearchResults, true>,
  // Same heading: a hit {id, kind, title, path, line, ord, archived, snippet}.
  SearchHit: {
    id: true,
    kind: true,
    title: true,
    path: true,
    line: true,
    ord: true,
    archived: true,
    snippet: true,
  } satisfies Record<keyof SearchHit, true>,
  // docs/features/ui-tree-node.md "Data": Snippet {segments: {text, hit}[], cut_start, cut_end}.
  Snippet: { segments: true, cut_start: true, cut_end: true } satisfies Record<keyof Snippet, true>,
  SnippetSegment: { text: true, hit: true } satisfies Record<keyof SnippetSegment, true>,
  // docs/canon/spec-cli-bundle.md "Output": {refs, reason, notes, task, budget, tokens, chars, bytes, bundle_hash, body, layers, tail, more}.
  BundleView: {
    refs: true,
    reason: true,
    notes: true,
    task: true,
    budget: true,
    tokens: true,
    chars: true,
    bytes: true,
    bundle_hash: true,
    body: true,
    layers: true,
    tail: true,
    more: true,
  } satisfies Record<keyof BundleView, true>,
  // Same heading: an item {name, kind, title, path, line, form, status, via, working_answer, tokens_est, archived}.
  BundleItem: {
    name: true,
    kind: true,
    title: true,
    path: true,
    line: true,
    form: true,
    status: true,
    via: true,
    working_answer: true,
    tokens_est: true,
    archived: true,
  } satisfies Record<keyof BundleItem, true>,
  // Same heading: `via` [{type, direction}].
  BundleVia: { type: true, direction: true } satisfies Record<keyof BundleVia, true>,
  // Same heading: `working_answer` {name, written, path, line, state}.
  WorkingAnswer: { name: true, written: true, path: true, line: true, state: true } satisfies Record<keyof WorkingAnswer, true>,
  // Same heading: `tail` entries {name, title, path, line, tokens_est, layer}.
  TailEntry: { name: true, title: true, path: true, line: true, tokens_est: true, layer: true } satisfies Record<keyof TailEntry, true>,
  // docs/canon/spec-cli-bundle.md "Layers": the nine JSON keys in print order.
  BundleLayers: {
    targets: true,
    open_questions: true,
    ancestors: true,
    criteria: true,
    bindings: true,
    decisions: true,
    neighbours: true,
    terms: true,
    tests: true,
  } satisfies Record<keyof BundleLayers, true>,
  // crates/specengine-cli/README.md "Output and the cap": a shown node, `span_hash` included.
  ShownNode: {
    id: true,
    kind: true,
    title: true,
    path: true,
    line: true,
    end_line: true,
    status: true,
    rev: true,
    tokens_est: true,
    archived: true,
    utf8: true,
    sections: true,
    span_hash: true,
    text: true,
    truncated: true,
    omitted: true,
    links: true,
  } satisfies Record<keyof ShownNode, true>,
};

/**
 * AC-02 of docs/features/ui-graph.md: `docs/canon/spec-cli-graph.md` "spec graph": JSON {ref,
 * reason, impact, types, depth, archive, notes, left_out, truncated, nodes, edges}; `types`
 * [{type, direction}]; a node {id, kind, title, path, line, distance, archived}; an edge {src,
 * type, dst, written, path, line, state, reason}.
 */
const GRAPH_KEYS = {
  GraphView: {
    ref: true,
    reason: true,
    impact: true,
    types: true,
    depth: true,
    archive: true,
    notes: true,
    left_out: true,
    truncated: true,
    nodes: true,
    edges: true,
  } satisfies Record<keyof GraphView, true>,
  FollowedType: { type: true, direction: true } satisfies Record<keyof FollowedType, true>,
  GraphNode: { id: true, kind: true, title: true, path: true, line: true, distance: true, archived: true } satisfies Record<
    keyof GraphNode,
    true
  >,
  GraphEdge: {
    src: true,
    type: true,
    dst: true,
    written: true,
    path: true,
    line: true,
    state: true,
    reason: true,
  } satisfies Record<keyof GraphEdge, true>,
};

const GRAPH_CITED: Record<keyof typeof GRAPH_KEYS, string> = {
  GraphView: "ref, reason, impact, types, depth, archive, notes, left_out, truncated, nodes, edges",
  FollowedType: "type, direction",
  GraphNode: "id, kind, title, path, line, distance, archived",
  GraphEdge: "src, type, dst, written, path, line, state, reason",
};

describe("the graph's types (AC-02 of ui-graph)", () => {
  it.each(Object.keys(GRAPH_KEYS) as (keyof typeof GRAPH_KEYS)[])("%s has exactly the canon's keys, in order", (name) => {
    expect(Object.keys(GRAPH_KEYS[name]).join(", ")).toBe(GRAPH_CITED[name]);
  });

  it.each(Object.keys(GRAPH_KEYS))("%s cites `docs/canon/spec-cli-graph.md` \"spec graph\"", (type) => {
    const at = source.indexOf(`export interface ${type} `);
    expect(at).toBeGreaterThan(0);
    const comment = source.slice(source.lastIndexOf("/**", at), at);
    expect(comment).toContain('`docs/canon/spec-cli-graph.md` "spec graph"');
  });

  it("keeps a link type a plain string", () => {
    const edge: Pick<GraphEdge, "type"> = { type: "any_project_type" };
    const followed: Pick<FollowedType, "type"> = { type: "any_project_type" };
    expect([edge.type, followed.type]).toEqual(["any_project_type", "any_project_type"]);
    expect(source).not.toMatch(/closed table with the graph slice/);
  });
});

/** The key lists as the cited headings write them, copied verbatim. */
const CITED: Record<keyof typeof KEYS, string> = {
  TreeView: "ref, reason, notes, depth, kinds, archive, left_out, truncated, nodes",
  TreeNode: "id, kind, title, path, line, depth, parent, mark, status, rev, tokens_est, archived",
  SearchResults: "archive, hits, kinds, limit, notes, query, tier3_left_out, truncated",
  SearchHit: "id, kind, title, path, line, ord, archived, snippet",
  Snippet: "segments, cut_start, cut_end",
  SnippetSegment: "text, hit",
  BundleView: "refs, reason, notes, task, budget, tokens, chars, bytes, bundle_hash, body, layers, tail, more",
  BundleItem: "name, kind, title, path, line, form, status, via, working_answer, tokens_est, archived",
  BundleVia: "type, direction",
  WorkingAnswer: "name, written, path, line, state",
  TailEntry: "name, title, path, line, tokens_est, layer",
  BundleLayers: "targets, open_questions, ancestors, criteria, bindings, decisions, neighbours, terms, tests",
  ShownNode: "id, kind, title, path, line, end_line, status, rev, tokens_est, archived, utf8, sections, span_hash, text, truncated, omitted, links",
};

describe("the read types' keys (AC-02 of ui-tree-node)", () => {
  it.each(Object.keys(KEYS) as (keyof typeof KEYS)[])("%s has exactly the cited keys, in order", (name) => {
    expect(Object.keys(KEYS[name]).join(", ")).toBe(CITED[name]);
  });

  it("gives a shown node its span hash", () => {
    expect(Object.keys(KEYS.ShownNode)).toContain("span_hash");
  });

  it("cites a source for each new type: the canon, the CLI README or this slice's spec", () => {
    const cited = (type: string) => {
      const at = source.indexOf(`export interface ${type} `) >= 0 ? source.indexOf(`export interface ${type} `) : source.indexOf(`export type ${type} `);
      const before = source.slice(Math.max(0, source.lastIndexOf("/**", at)), at);
      return /`[^`\s]+[.]md` "[^"]+"/.test(before);
    };
    for (const type of [...Object.keys(KEYS), "TreeMark", "BundleForm", "Direction"]) {
      expect([type, cited(type)]).toEqual([type, true]);
    }
  });
});

/**
 * AC-02 of docs/features/ui-tasks.md and AC-13 of docs/features/ui-live-tasks.md: the task types'
 * keys equal the lists the canon writes (`docs/canon/tasks.md` "Commands" for the list and the
 * exit-1 document, `docs/canon/task-package.md` "Package" for the package and its parts); each type
 * cites its heading as ui-live-tasks "Citations" re-points it.
 */
const TASK_KEYS = {
  TaskList: { tasks: true, notes: true } satisfies Record<keyof TaskList, true>,
  TaskListEntry: { id: true, status: true, title: true, targets: true, stale: true, updated_at: true } satisfies Record<keyof TaskListEntry, true>,
  TaskNotFound: { id: true, reason: true } satisfies Record<keyof TaskNotFound, true>,
  TaskPackage: {
    schema_version: true,
    id: true,
    project: true,
    status: true,
    title: true,
    goal: true,
    profile: true,
    stale: true,
    targets: true,
    criteria: true,
    affected_nodes: true,
    plan: true,
    assumptions: true,
    open_proposals: true,
    owner_notes: true,
    bindings: true,
    spec_snapshot: true,
    snapshot_diff: true,
    claim: true,
    runs: true,
    bundle: true,
    author: true,
    created_at: true,
    updated_at: true,
    notes: true,
  } satisfies Record<keyof TaskPackage, true>,
  TaskTarget: { id: true, path: true, kind: true, title: true } satisfies Record<keyof TaskTarget, true>,
  TaskCriterion: { ref: true, text: true } satisfies Record<keyof TaskCriterion, true>,
  TaskAssumption: { proposal: true, text: true } satisfies Record<keyof TaskAssumption, true>,
  TaskProposal: { id: true, kind: true, status: true, target_ids: true, task_id: true, summary: true } satisfies Record<keyof TaskProposal, true>,
  OwnerNote: { at: true, note: true } satisfies Record<keyof OwnerNote, true>,
  SpecSnapshot: { at: true, place: true, nodes: true } satisfies Record<keyof SpecSnapshot, true>,
  SnapshotPlace: { worktree: true, root_rel: true, branch: true, commit: true } satisfies Record<keyof SnapshotPlace, true>,
  SnapshotNode: { id: true, path: true, span_hash: true } satisfies Record<keyof SnapshotNode, true>,
  SnapshotDiff: { id: true, path: true, span_hash: true, diff: true, cut: true } satisfies Record<keyof SnapshotDiff, true>,
  TaskClaim: { at: true, role: true, worktree: true, branch: true } satisfies Record<keyof TaskClaim, true>,
  TaskRun: {
    run: true,
    role: true,
    started_at: true,
    ended_at: true,
    outcome: true,
    summary: true,
    changed_files: true,
  } satisfies Record<keyof TaskRun, true>,
  TaskBundle: { node_ids: true, budget: true, bundle_hash: true } satisfies Record<keyof TaskBundle, true>,
};

/** The key lists as the canon's JSON writes them, copied verbatim. */
const TASK_CITED: Record<keyof typeof TASK_KEYS, string> = {
  TaskList: "tasks, notes",
  TaskListEntry: "id, status, title, targets, stale, updated_at",
  TaskNotFound: "id, reason",
  TaskPackage:
    "schema_version, id, project, status, title, goal, profile, stale, targets, criteria, affected_nodes, plan, assumptions, open_proposals, owner_notes, bindings, spec_snapshot, snapshot_diff, claim, runs, bundle, author, created_at, updated_at, notes",
  TaskTarget: "id, path, kind, title",
  TaskCriterion: "ref, text",
  TaskAssumption: "proposal, text",
  TaskProposal: "id, kind, status, target_ids, task_id, summary",
  OwnerNote: "at, note",
  SpecSnapshot: "at, place, nodes",
  SnapshotPlace: "worktree, root_rel, branch, commit",
  SnapshotNode: "id, path, span_hash",
  SnapshotDiff: "id, path, span_hash, diff, cut",
  TaskClaim: "at, role, worktree, branch",
  TaskRun: "run, role, started_at, ended_at, outcome, summary, changed_files",
  TaskBundle: "node_ids, budget, bundle_hash",
};

/** The canon of the task commands, and of the package (ui-live-tasks "Citations"). */
const TASKS = "`docs/canon/tasks.md`";
const PACKAGE = "`docs/canon/task-package.md`";

/** The types `docs/canon/tasks.md` "Commands" writes: the list, a row, the exit-1 document. */
const LISTED = new Set(["TaskList", "TaskListEntry", "TaskNotFound"]);

/** Each task type's citation per ui-live-tasks "Citations": the commands, the diffs' two headings, else the package. */
function taskCitation(type: string): string {
  if (LISTED.has(type)) {
    return `${TASKS} "Commands"`;
  }
  return type === "SnapshotDiff" ? `${PACKAGE} "Staleness", "Caps"` : `${PACKAGE} "Package"`;
}

function commentOf(type: string): string {
  const at = source.search(new RegExp(`export (interface|type) ${type}\\b`));
  expect([type, at > 0]).toEqual([type, true]);
  return source.slice(source.lastIndexOf("/**", at), at);
}

describe("the task types (AC-02 of ui-tasks, AC-13 of ui-live-tasks)", () => {
  it.each(Object.keys(TASK_KEYS) as (keyof typeof TASK_KEYS)[])("%s has exactly the canon's keys, in order", (name) => {
    expect(Object.keys(TASK_KEYS[name]).join(", ")).toBe(TASK_CITED[name]);
  });

  it("count 25 package keys, 6 list keys, 7 run keys, 2 note keys, 2 keys of the exit-1 document", () => {
    expect([TASK_KEYS.TaskPackage, TASK_KEYS.TaskListEntry, TASK_KEYS.TaskRun, TASK_KEYS.OwnerNote, TASK_KEYS.TaskNotFound].map((keys) => Object.keys(keys).length)).toEqual([
      25, 6, 7, 2, 2,
    ]);
  });

  it.each(Object.keys(TASK_KEYS))("%s cites the canon's heading", (type) => {
    expect(commentOf(type)).toContain(taskCitation(type));
  });

  it("cite 05 for the ten states, the canon's Transitions for a state as sent, its Caps for the outcomes", () => {
    expect(commentOf("KnownTaskStatus")).toContain('`docs/specs/specengine-platform/05-architecture.md` "3.3. Index schema (SQLite)"');
    expect(commentOf("TaskStatus")).toContain(`${TASKS} "Transitions"`);
    for (const type of ["KnownRunOutcome", "RunOutcome"]) {
      expect(commentOf(type)).toContain(`${PACKAGE} "Caps"`);
    }
  });

  it("cite the package's Versioning for its schema_version, and the task-bound proposals for a review's task_id", () => {
    expect(commentOf("TaskPackage")).toContain(`${PACKAGE} "Package", "Versioning"`);
    expect(commentOf("Proposal")).toContain(`${TASKS} "Task-bound proposals"`);
  });

  it("cite no task-package feature spec: its blocks go at shipping (WA-9)", () => {
    expect(source).not.toContain("docs/features/" + "task-package.md");
  });

  it("type a bundle hash as the package sends it: null when no bundle can be made", () => {
    const unmade: TaskBundle = { node_ids: ["MEC-TIDES"], budget: 10000, bundle_hash: null };
    const made: TaskBundle = { node_ids: ["MEC-TIDES"], budget: 10000, bundle_hash: "b3:0" };
    expect([unmade.bundle_hash, made.bundle_hash]).toEqual([null, "b3:0"]);
    expect(commentOf("TaskBundle")).toContain("`bundle_hash` null");
  });

  it("say a proposal's summary and a diff's two nulls as the canon does", () => {
    const summary = source.slice(source.indexOf("export interface TaskProposal "), source.indexOf("  summary: string | null;", source.indexOf("export interface TaskProposal ")));
    expect(summary).toContain("/** A question's or discrepancy's summary, else the rationale's first line, null for an empty rationale. */");
    // The comment as one line of words, its `*` margins dropped.
    const diff = commentOf("SnapshotDiff").replace(/\s*\n\s*\*?\s*/g, " ");
    expect(diff).toContain("`diff` null has two meanings: `cut` true, left out past 262 144 B in all");
    expect(diff).toContain("`cut` false, a diff git cannot make, a note says why");
  });

  it("keep kind, role and profile plain strings", () => {
    const target: Pick<TaskTarget, "kind"> = { kind: "any-project-kind" };
    const run: Pick<TaskRun, "role"> = { role: "any-project-role" };
    const pkg: Pick<TaskPackage, "profile"> = { profile: "any-profile" };
    expect([target.kind, run.role, pkg.profile]).toEqual(["any-project-kind", "any-project-role", "any-profile"]);
  });
});

/**
 * AC-02 of docs/features/ui-health.md: the check's types copy the JSON of `docs/canon/spec-check.md`
 * "Findings, debt, verdict": `{mode, verdict, counts: {documents, errors, warnings, debt, expired,
 * stale, introduced?, new_debt?, worst_w_bytes}, findings, stale, new_debt?, cannot_check}`; a
 * finding `{code, severity, path, line, subject, message, fix?: {span, text}, debt?: {reason,
 * expires, expired}, introduced?}`; a baseline entry the TOML's five keys plus its `line` (core
 * `check/baseline.rs` `DebtEntry`); a cause `{path, message}`.
 */
const CHECK_KEYS = {
  CheckReport: {
    mode: true,
    verdict: true,
    counts: true,
    findings: true,
    stale: true,
    new_debt: true,
    cannot_check: true,
  } satisfies Record<keyof CheckReport, true>,
  CheckCounts: {
    documents: true,
    errors: true,
    warnings: true,
    debt: true,
    expired: true,
    stale: true,
    introduced: true,
    new_debt: true,
    worst_w_bytes: true,
  } satisfies Record<keyof CheckCounts, true>,
  CheckFinding: {
    code: true,
    severity: true,
    path: true,
    line: true,
    subject: true,
    message: true,
    fix: true,
    debt: true,
    introduced: true,
  } satisfies Record<keyof CheckFinding, true>,
  DebtEntry: { code: true, path: true, subject: true, reason: true, expires: true, line: true } satisfies Record<keyof DebtEntry, true>,
  CheckCause: { path: true, message: true } satisfies Record<keyof CheckCause, true>,
};

/** The key lists as the cited heading writes them (and, for `line`, the core's DebtEntry), copied verbatim. */
const CHECK_CITED: Record<keyof typeof CHECK_KEYS, string> = {
  CheckReport: "mode, verdict, counts, findings, stale, new_debt, cannot_check",
  CheckCounts: "documents, errors, warnings, debt, expired, stale, introduced, new_debt, worst_w_bytes",
  CheckFinding: "code, severity, path, line, subject, message, fix, debt, introduced",
  DebtEntry: "code, path, subject, reason, expires, line",
  CheckCause: "path, message",
};

const CHECK_HEADING = '`docs/canon/spec-check.md` "Findings, debt, verdict"';

describe("the check's types (AC-02 of ui-health)", () => {
  it.each(Object.keys(CHECK_KEYS) as (keyof typeof CHECK_KEYS)[])("%s has exactly the cited keys, in order", (name) => {
    expect(Object.keys(CHECK_KEYS[name]).join(", ")).toBe(CHECK_CITED[name]);
  });

  it("count 7 report keys, 9 counts, 9 finding keys, 6 baseline keys, 2 cause keys", () => {
    expect(Object.values(CHECK_KEYS).map((keys) => Object.keys(keys).length)).toEqual([7, 9, 9, 6, 2]);
  });

  it.each([...Object.keys(CHECK_KEYS), "Finding", "KnownCheckVerdict", "CheckVerdict", "NewDebtEntry", "KnownFindingSeverity", "FindingSeverity"])(
    "%s cites the canon's heading",
    (type) => {
      expect(commentOf(type)).toContain(CHECK_HEADING);
    },
  );

  it("cite the canon's Configuration for the modes", () => {
    for (const type of ["KnownCheckMode", "CheckMode"]) {
      expect(commentOf(type)).toContain('`docs/canon/spec-check.md` "Configuration"');
    }
  });

  it("keep Finding the six keys a proposal's diagnostics carry; the report's finding adds three optional ones (R5)", () => {
    const six: Record<keyof Finding, true> = { code: true, severity: true, path: true, line: true, subject: true, message: true };
    expect(Object.keys(six)).toEqual(CHECK_CITED.CheckFinding.split(", ").slice(0, 6));
    const bare: CheckFinding = { code: "c", severity: "warning", path: "", line: 1, subject: "", message: "m" };
    expect(Object.keys(bare)).toHaveLength(6);
  });

  it("list the four verdicts on the one line that spells them (AC-11)", () => {
    const lines = source.split("\n").filter((line) => line.includes("KNOWN_CHECK_VERDICTS = ["));
    expect(lines).toHaveLength(1);
    expect(lines[0]).toMatch(/^export type KnownCheckVerdict = /);
    expect(KNOWN_CHECK_VERDICTS).toHaveLength(4);
    expect(KNOWN_CHECK_VERDICTS[0]).toBe("clean");
    expect(KNOWN_CHECK_VERDICTS[3]).toBe("cannot-check");
  });
});
