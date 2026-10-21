import { describe, expect, it } from "vitest";
import source from "./provisional.ts?raw";
import { KNOWN_CHECK_VERDICTS } from "./types";
import type {
  BindingAnchor,
  BindingPlace,
  BindingRow,
  BundleBindingRow,
  BundleItem,
  BundleLayers,
  BundleVia,
  BundleView,
  CheckCause,
  CheckCounts,
  CheckFinding,
  CheckReport,
  DebtEntry,
  DriftCounts,
  DriftLevels,
  DriftLock,
  DriftMarker,
  DriftRow,
  Finding,
  FollowedType,
  GraphEdge,
  GraphNode,
  GraphView,
  HealthDrift,
  HealthReport,
  OwnerNote,
  Proposal,
  RevisionChange,
  SearchHit,
  SearchResults,
  ShownBindings,
  ShownNode,
  Snippet,
  SnapshotDiff,
  SnapshotNode,
  SnapshotPlace,
  SnippetSegment,
  SpecSnapshot,
  Stage,
  StageChoice,
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
  TaskRoadmap,
  TaskRun,
  TaskTarget,
  TestedRow,
  TestedState,
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
  // crates/specengine-cli/README.md "Output and the cap": a shown node, `span_hash` included;
  // docs/features/norm-hash.md "Data": `norm_hash` right before it; docs/canon/code-reads.md
  // "spec show --bindings": `bindings`, its 19th key, after `links`.
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
    norm_hash: true,
    span_hash: true,
    text: true,
    truncated: true,
    omitted: true,
    links: true,
    bindings: true,
  } satisfies Record<keyof ShownNode, true>,
  // docs/canon/code-reads.md "spec show --bindings": a node's `bindings` {rows, omitted}, then
  // docs/canon/spec-lock-tested.md "Reads": `tested` after `omitted`.
  ShownBindings: { rows: true, omitted: true, tested: true } satisfies Record<keyof ShownBindings, true>,
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
  // docs/features/norm-hash.md "Data": JSON node {id, ..., sections, norm_hash, span_hash, ...};
  // docs/canon/code-reads.md "spec show --bindings": `bindings` after `links`.
  ShownNode:
    "id, kind, title, path, line, end_line, status, rev, tokens_est, archived, utf8, sections, norm_hash, span_hash, text, truncated, omitted, links, bindings",
  ShownBindings: "rows, omitted, tested",
};

/** An interface's keys in the order `provisional.ts` declares them (its own lines, two-space indented). */
function declaredKeys(type: string): string[] {
  const start = source.indexOf(`export interface ${type} {`);
  const end = source.indexOf("\n}\n", start);
  if (start < 0 || end < 0) {
    return [];
  }
  return Array.from(source.slice(start, end).matchAll(/^ {2}(\w+)\??:/gm), (match) => match[1] ?? "");
}

describe("the read types' keys (AC-02 of ui-tree-node)", () => {
  it.each(Object.keys(KEYS) as (keyof typeof KEYS)[])("%s has exactly the cited keys, in order", (name) => {
    expect(Object.keys(KEYS[name]).join(", ")).toBe(CITED[name]);
  });

  it("gives a shown node its span hash", () => {
    expect(Object.keys(KEYS.ShownNode)).toContain("span_hash");
  });

  it("declares a shown node's keys in the cited order: norm_hash, nullable, right before span_hash (UI-1 of norm-hash)", () => {
    // The type itself, not the record above: a key missing from it or moved after span_hash is red here.
    expect(declaredKeys("ShownNode").join(", ")).toBe(CITED.ShownNode);
    const at = source.indexOf("\n  norm_hash: string | null;\n");
    expect(at).toBeGreaterThan(source.indexOf("export interface ShownNode {"));
    const comment = source.slice(source.lastIndexOf("/**", at), at);
    expect(comment).toContain('Source: `docs/features/norm-hash.md` "Data".');
    const none: Pick<ShownNode, "norm_hash"> = { norm_hash: null };
    expect(none.norm_hash).toBeNull();
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
    tested: true,
    spec_snapshot: true,
    snapshot_diff: true,
    claim: true,
    runs: true,
    bundle: true,
    roadmap: true,
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
  TaskRoadmap: {
    item: true,
    group: true,
    release: true,
    position: true,
    of: true,
    priority: true,
    waits_on: true,
  } satisfies Record<keyof TaskRoadmap, true>,
};

/** The key lists as the canon's JSON writes them, copied verbatim. */
const TASK_CITED: Record<keyof typeof TASK_KEYS, string> = {
  TaskList: "tasks, notes",
  TaskListEntry: "id, status, title, targets, stale, updated_at",
  TaskNotFound: "id, reason",
  TaskPackage:
    "schema_version, id, project, status, title, goal, profile, stale, targets, criteria, affected_nodes, plan, assumptions, open_proposals, owner_notes, bindings, tested, spec_snapshot, snapshot_diff, claim, runs, bundle, roadmap, author, created_at, updated_at, notes",
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
  TaskRoadmap: "item, group, release, position, of, priority, waits_on",
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

  it("count 27 package keys, 6 list keys, 7 run keys, 2 note keys, 2 keys of the exit-1 document, 7 roadmap keys", () => {
    expect(
      [TASK_KEYS.TaskPackage, TASK_KEYS.TaskListEntry, TASK_KEYS.TaskRun, TASK_KEYS.OwnerNote, TASK_KEYS.TaskNotFound, TASK_KEYS.TaskRoadmap].map(
        (keys) => Object.keys(keys).length,
      ),
    ).toEqual([27, 6, 7, 2, 2, 7]);
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

  it("type the package's roadmap as the task's place on the plan, or null (UI-1 of roadmap-surfaces)", () => {
    const placed: TaskRoadmap = { item: "RM-0011", group: "release", release: "beta", position: 5, of: 8, priority: "should", waits_on: ["RM-0010"] };
    const pooled: TaskRoadmap = { item: "RM-0020", group: "pool", release: null, position: null, of: 3, priority: null, waits_on: [] };
    const unplaced: TaskRoadmap = { item: "RM-0030", group: "unplaced", release: "someday", position: null, of: 1, priority: "any-priority", waits_on: [] };
    const none: Pick<TaskPackage, "roadmap"> = { roadmap: null };
    expect([placed, pooled, unplaced].map(({ group, position }) => [group, position])).toEqual([
      ["release", 5],
      ["pool", null],
      ["unplaced", null],
    ]);
    expect(none.roadmap).toBeNull();
    expect(source).toMatch(/^  roadmap: TaskRoadmap \| null;$/m);
    expect(source).toMatch(/^export type RoadmapGroupKind = "release" \| "pool" \| "unplaced" \| Unlisted;$/m);
    expect(commentOf("RoadmapGroupKind")).toContain('`docs/canon/spec-roadmap.md` "Command"');
    expect(commentOf("TaskRoadmap")).toContain("never holds the task (ADR-0012)");
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

/** The staging canon (AC-14 of docs/features/decision-staging.md). */
const STAGING = "`docs/canon/decision-staging.md`";

describe("the stage's types (AC-14 of decision-staging)", () => {
  it.each([
    ["StagedApprove", "The stage"],
    ["StagedReject", "The stage"],
    ["Stage", "The stage"],
    ["StageChoice", "Daemon"],
    ["Proposal", "Documents"],
    ["InboxEntry", "Documents"],
  ])("%s cites the staging canon's %s", (type, heading) => {
    expect(commentOf(type)).toContain(`${STAGING} "${heading}"`);
  });

  it("type a staged choice as one of the canon's two shapes, an approve's absent flags null", () => {
    const approve: Stage = { decision: "approve", option: 1, answer: null, canon: null, editorial: null, note: "keep the cap", span_hash: null };
    const reject: Stage = { decision: "reject", reason: "duplicate of PR-0003" };
    expect([Object.keys(approve), Object.keys(reject)]).toEqual([
      ["decision", "option", "answer", "canon", "editorial", "note", "span_hash"],
      ["decision", "reason"],
    ]);
    // What the UI sends: the stage less span_hash, which the daemon reads itself.
    const sent: StageChoice = { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null };
    expect(Object.keys(sent)).toEqual(["decision", "option", "answer", "canon", "editorial", "note"]);
  });

  it("keep no decision, commit or apply result: the UI never decides (ADR-0035)", () => {
    expect(source).not.toMatch(/export (type|interface) (Decision|DecisionResult|Commit)\b/);
  });
});

/** This slice's spec (UI-4 of docs/features/rev-discipline.md). */
const REV_DISCIPLINE = '`docs/features/rev-discipline.md` "Review document, stage, queue"';

describe("the revision types (UI-4 of rev-discipline)", () => {
  it.each(["StagedApprove", "Proposal", "RevisionChange"])("%s cites the slice's Review document, stage, queue", (type) => {
    expect(commentOf(type)).toContain(REV_DISCIPLINE);
  });

  it("gives a revisions entry exactly the spec's five keys, in its order: id, rev, to, lock_entries, refused", () => {
    const keys = { id: true, rev: true, to: true, lock_entries: true, refused: true } satisfies Record<keyof RevisionChange, true>;
    const entry: RevisionChange = { id: "R-12", rev: 3, to: 4, lock_entries: 2, refused: null };
    expect([Object.keys(keys), Object.keys(entry)]).toEqual([
      ["id", "rev", "to", "lock_entries", "refused"],
      ["id", "rev", "to", "lock_entries", "refused"],
    ]);
    // An unreadable lock: the count null, a note saying why.
    const unread: RevisionChange = { id: "R-12", rev: 3, to: 4, lock_entries: null, refused: null };
    expect(unread.lock_entries).toBeNull();
  });

  it("types a refused raise: to null, refused the daemon's reason (Q15 of rev-discipline)", () => {
    const refused: RevisionChange = {
      id: "ADR-0007",
      rev: 1,
      to: null,
      lock_entries: 0,
      refused: "`ADR-0007`: class `decision` is closed and does not admit `rev`: admit it in [classes], or approve with --editorial",
    };
    expect([refused.to, refused.refused]).toEqual([null, expect.stringContaining("--editorial")]);
    expect(source).toMatch(/^  to: number \| null;\n  lock_entries: number \| null;\n  refused: string \| null;\n\}$/m);
  });

  it("types the review's revisions as a list or null, and the stage's editorial as true or null, never false", () => {
    const none: Pick<Proposal, "revisions"> = { revisions: null };
    const unchanged: Pick<Proposal, "revisions"> = { revisions: [] };
    expect([none.revisions, unchanged.revisions]).toEqual([null, []]);
    const editorial: Extract<StageChoice, { decision: "approve" }>["editorial"][] = [true, null];
    expect(editorial).toEqual([true, null]);
    expect(source).toMatch(/^  editorial: true \| null;$/m);
  });
});

/**
 * UI-1 of docs/features/bundle-bindings.md: the row of
 * `docs/canon/code-reads.md` "The binding row" (14 keys, verify's order), its anchor and place; a
 * bundle row is it plus `tokens_est` (15 keys) in layers 5 and 9; a shown node's `bindings`
 * {rows, omitted, tested} after `links` (`tested` since tested-reads); the package's `bindings` a list of rows.
 */
const ROW_KEYS = {
  BindingRow: {
    node: true,
    relation: true,
    file: true,
    kind: true,
    symbol: true,
    item_kind: true,
    line: true,
    end_line: true,
    signature: true,
    layer: true,
    sync: true,
    category: true,
    detail: true,
    anchor: true,
  } satisfies Record<keyof BindingRow, true>,
  BindingAnchor: { state: true, from: true, candidates: true } satisfies Record<keyof BindingAnchor, true>,
  BindingPlace: { file: true, kind: true, symbol: true, item_kind: true } satisfies Record<keyof BindingPlace, true>,
};

/** The key lists as the canon writes them: the table's rows joined, the anchor as verify's JSON gives it. */
const ROW_CITED: Record<keyof typeof ROW_KEYS, string> = {
  BindingRow: "node, relation, file, kind, symbol, item_kind, line, end_line, signature, layer, sync, category, detail, anchor",
  BindingAnchor: "state, from, candidates",
  BindingPlace: "file, kind, symbol, item_kind",
};

const BINDING_ROW = '`docs/canon/code-reads.md` "The binding row"';

describe("the binding row types (UI-1 of bundle-bindings)", () => {
  it.each(Object.keys(ROW_KEYS) as (keyof typeof ROW_KEYS)[])("%s has exactly the canon's keys, declared in its order", (name) => {
    expect(Object.keys(ROW_KEYS[name]).join(", ")).toBe(ROW_CITED[name]);
    expect(declaredKeys(name).join(", ")).toBe(ROW_CITED[name]);
  });

  it("counts a row's 14 keys, a bundle row's 15: the row, then tokens_est", () => {
    expect(Object.keys(ROW_KEYS.BindingRow)).toHaveLength(14);
    const placed: BundleBindingRow = {
      node: "MEC-TIDES",
      relation: "implements",
      file: "src/sim/tide.rs",
      kind: "item",
      symbol: "harbor_sim::tide::TideClock",
      item_kind: "struct_item",
      line: 5,
      end_line: 14,
      signature: "pub struct TideClock",
      layer: "a",
      sync: "ok",
      category: null,
      detail: null,
      anchor: { state: "exact", from: null, candidates: null },
      tokens_est: 30,
    };
    expect(Object.keys(placed)).toEqual([...ROW_CITED.BindingRow.split(", "), "tokens_est"]);
    expect(source).toMatch(/^export type BundleBindingRow = BindingRow & \{ tokens_est: number \};$/m);
  });

  it.each(["BindingRow", "BindingAnchor", "BindingPlace"])("%s cites the canon's binding row", (type) => {
    expect(commentOf(type)).toContain(BINDING_ROW);
  });

  it("cites the slice's layers for the bundle row, the layers and the tail; the show heading for a node's bindings", () => {
    for (const type of ["BundleBindingRow", "BundleLayers", "TailEntry"]) {
      expect([type, commentOf(type).includes('`docs/features/bundle-bindings.md` "Layers 5 and 9"')]).toEqual([type, true]);
    }
    expect(commentOf("ShownBindings")).toContain('`docs/canon/code-reads.md` "spec show --bindings"');
    expect(commentOf("KnownSyncState")).toContain('`docs/canon/spec-verify.md` "States"');
  });

  it("types layers 5 and 9 as bundle rows and every other layer as nodes", () => {
    const layers = source.slice(source.indexOf("export interface BundleLayers {"), source.indexOf("\n}\n", source.indexOf("export interface BundleLayers {")));
    expect(Array.from(layers.matchAll(/^ {2}(\w+): (\w+)\[\];$/gm), (match) => `${match[1] ?? ""} ${match[2] ?? ""}`)).toEqual([
      "targets BundleItem",
      "open_questions BundleItem",
      "ancestors BundleItem",
      "criteria BundleItem",
      "bindings BundleBindingRow",
      "decisions BundleItem",
      "neighbours BundleItem",
      "terms BundleItem",
      "tests BundleBindingRow",
    ]);
  });

  it("gives a shown node its bindings after links, null or {rows, omitted, tested}; the package its rows, never unknown", () => {
    expect(source).toMatch(/^ {2}links: ShownLinks \| null;\n(?: {2}\/\*\*[\s\S]*?\*\/\n)? {2}bindings: ShownBindings \| null;\n\}$/m);
    const none: Pick<ShownNode, "bindings"> = { bindings: null };
    const some: Pick<ShownNode, "bindings"> = { bindings: { rows: [], omitted: 3, tested: null } };
    expect([none.bindings, some.bindings?.omitted]).toEqual([null, 3]);
    expect(source).toMatch(/^ {2}bindings: BindingRow\[\];$/m);
    expect(source).not.toMatch(/^ {2}bindings: unknown\[\];$/m);
    const pkg: Pick<TaskPackage, "bindings"> = { bindings: [] };
    expect(pkg.bindings).toEqual([]);
  });

  it("keeps the judgement nullable: a row not judged has sync, category, detail and anchor null, never ok", () => {
    const listed: Pick<BindingRow, "sync" | "category" | "detail" | "anchor"> = { sync: null, category: null, detail: null, anchor: null };
    expect(Object.values(listed)).toEqual([null, null, null, null]);
    expect(source).toMatch(/^ {2}sync: SyncState \| null;$/m);
  });

  it("lists verify's nine states in its table's order and still takes any other word", () => {
    const declared = /export type KnownSyncState =([^;]+);/.exec(source)?.[1] ?? "";
    expect(Array.from(declared.matchAll(/"(\w+)"/g), (match) => match[1])).toEqual([
      "cannot_verify",
      "predated",
      "bad_marker",
      "broken_binding",
      "unlocked",
      "spec_ahead",
      "code_ahead",
      "conflict",
      "ok",
    ]);
    const other: BindingRow["sync"] = "drifting";
    expect(other).toBe("drifting");
  });

  it("keeps a row's words plain strings: relation, kind, item kind, layer, category, detail", () => {
    const row: Pick<BindingRow, "relation" | "kind" | "item_kind" | "layer" | "category" | "detail"> = {
      relation: "any-relation",
      kind: "any-kind",
      item_kind: "any_item",
      layer: "z",
      category: "any_category",
      detail: "any detail",
    };
    expect(Object.values(row)).toHaveLength(6);
  });
});

/**
 * UI of docs/features/tested-reads.md (types only, no display: Q-J): plain verify's entry, as
 * `docs/canon/spec-lock-tested.md` "Reads" puts it in the reads, 7 keys in verify's order; its four
 * states; a shown node's `bindings.tested` after `omitted`; the package's `tested` after `bindings`;
 * each null when not judged, [] with no block.
 */
const TESTED_KEYS = {
  TestedRow: {
    node: true,
    state: true,
    category: true,
    rev: true,
    node_rev: true,
    commit: true,
    verified_at: true,
  } satisfies Record<keyof TestedRow, true>,
};

/** The key list as the canon writes the entry, copied verbatim. */
const TESTED_CITED: Record<keyof typeof TESTED_KEYS, string> = {
  TestedRow: "node, state, category, rev, node_rev, commit, verified_at",
};

const TESTED_READS = '`docs/canon/spec-lock-tested.md` "Reads"';

describe("the tested types (UI of tested-reads)", () => {
  it.each(Object.keys(TESTED_KEYS) as (keyof typeof TESTED_KEYS)[])("%s has exactly the canon's 7 keys, declared in its order", (name) => {
    expect(Object.keys(TESTED_KEYS[name]).join(", ")).toBe(TESTED_CITED[name]);
    expect(declaredKeys(name).join(", ")).toBe(TESTED_CITED[name]);
    expect(Object.keys(TESTED_KEYS[name])).toHaveLength(7);
  });

  it.each(["TestedState", "TestedRow", "ShownBindings"])("%s cites the canon's Reads", (type) => {
    expect(commentOf(type)).toContain(TESTED_READS);
  });

  it("cites the canon's Reads right above the package's tested", () => {
    const at = source.indexOf("\n  tested: TestedRow[] | null;\n", source.indexOf("export interface TaskPackage {"));
    expect(at).toBeGreaterThan(source.indexOf("export interface TaskPackage {"));
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain(TESTED_READS);
  });

  it("lists 8b-1's four states in its mirror's order and still takes any other word", () => {
    const declared = /export type TestedState =([^;]+);/.exec(source)?.[1] ?? "";
    expect(Array.from(declared.matchAll(/"(\w+)"/g), (match) => match[1])).toEqual(["verified", "stale", "unverifiable", "node_gone"]);
    expect(declared.trim().endsWith("| Unlisted")).toBe(true);
    const other: TestedState = "pending";
    const state: TestedRow["state"] = other;
    expect(state).toBe("pending");
  });

  it("gives a shown node's bindings tested after omitted, the package tested after bindings: null not judged, [] none", () => {
    expect(declaredKeys("ShownBindings")).toEqual(["rows", "omitted", "tested"]);
    const pkg = declaredKeys("TaskPackage");
    expect(pkg.slice(pkg.indexOf("bindings"), pkg.indexOf("bindings") + 3)).toEqual(["bindings", "tested", "spec_snapshot"]);
    // The third is Health's row 6 (UI-1 of health-drift): verify's entries but `verified`.
    expect(source.match(/^ {2}tested: TestedRow\[\] \| null;$/gm)).toHaveLength(3);
    const entry: TestedRow = {
      node: "R-12",
      state: "stale",
      category: "code_changed",
      rev: 3,
      node_rev: 3,
      commit: "a0cea48".padEnd(40, "0"),
      verified_at: "2026-10-21",
    };
    const shown: ShownBindings = { rows: [], omitted: 0, tested: [entry] };
    const unjudged: Pick<TaskPackage, "tested"> = { tested: null };
    const none: Pick<TaskPackage, "tested"> = { tested: [] };
    expect([shown.tested?.map((row) => row.state), unjudged.tested, none.tested]).toEqual([["stale"], null, []]);
  });

  it("keeps the node's revision now nullable and the category a plain word, null but for stale and unverifiable", () => {
    const gone: TestedRow = { node: "R-9", state: "node_gone", category: null, rev: 2, node_rev: null, commit: "c".repeat(40), verified_at: "2026-10-21" };
    const unverifiable: Pick<TestedRow, "state" | "category"> = { state: "unverifiable", category: "spec_ahead" };
    expect([gone.node_rev, gone.category, unverifiable.category]).toEqual([null, null, "spec_ahead"]);
    expect(source).toMatch(/^ {2}category: string \| null;\n {2}rev: number;\n {2}node_rev: number \| null;\n {2}commit: string;\n {2}verified_at: string;\n\}$/m);
  });
});

/**
 * UI-1 of docs/features/health-drift.md (types only, nothing drawn): Health's row 6, the health
 * document's `drift` {state, causes, counts, rows, tested, notes}; verify's seven counts and its
 * 17-key row with its marker, lock and levels, as `docs/canon/spec-verify-output.md` "JSON" writes
 * them; the row's anchor and places the binding row's, its `tested` entries the reads'.
 */
const DRIFT_KEYS = {
  HealthDrift: { state: true, causes: true, counts: true, rows: true, tested: true, notes: true } satisfies Record<keyof HealthDrift, true>,
  DriftCounts: {
    rows: true,
    ok: true,
    fix: true,
    check: true,
    cannot: true,
    pre_existing: true,
    skipped: true,
  } satisfies Record<keyof DriftCounts, true>,
  DriftRow: {
    node: true,
    relation: true,
    file: true,
    kind: true,
    symbol: true,
    item_kind: true,
    state: true,
    category: true,
    detail: true,
    class: true,
    pre_existing: true,
    markers: true,
    node_rev: true,
    lock: true,
    compared: true,
    levels: true,
    anchor: true,
  } satisfies Record<keyof DriftRow, true>,
  DriftMarker: { line: true, rev: true } satisfies Record<keyof DriftMarker, true>,
  DriftLock: { rev: true, commit: true, verified_at: true } satisfies Record<keyof DriftLock, true>,
  DriftLevels: { lock: true, markers: true, changed: true } satisfies Record<keyof DriftLevels, true>,
};

/** The key lists as the spec's "Data" and verify's JSON write them, copied verbatim. */
const DRIFT_CITED: Record<keyof typeof DRIFT_KEYS, string> = {
  HealthDrift: "state, causes, counts, rows, tested, notes",
  DriftCounts: "rows, ok, fix, check, cannot, pre_existing, skipped",
  DriftRow: "node, relation, file, kind, symbol, item_kind, state, category, detail, class, pre_existing, markers, node_rev, lock, compared, levels, anchor",
  DriftMarker: "line, rev",
  DriftLock: "rev, commit, verified_at",
  DriftLevels: "lock, markers, changed",
};

const DRIFT_NAMES = Object.keys(DRIFT_KEYS) as (keyof typeof DRIFT_KEYS)[];

const HEALTH_DRIFT_DATA = '`docs/features/health-drift.md` "Data"';

const VERIFY_JSON = '`docs/canon/spec-verify-output.md` "JSON"';

/** The canon's example row (`docs/canon/spec-verify-output.md` "JSON"), a `conflict` on R-12, keys in its order. */
function aConflictRow(): DriftRow {
  return {
    node: "R-12",
    relation: "implements",
    file: "crates/game/src/stamina.rs",
    kind: "item",
    symbol: "game::stamina::regen_system",
    item_kind: "function_item",
    state: "conflict",
    category: null,
    detail: null,
    class: "fix",
    pre_existing: false,
    markers: [{ line: 41, rev: 2 }],
    node_rev: 3,
    lock: { rev: 2, commit: "a0cea48".padEnd(40, "0"), verified_at: "2026-10-10" },
    compared: "ast",
    levels: { lock: ["sig", "body"], markers: ["sig", "body"], changed: ["body"] },
    anchor: { state: "exact", from: null, candidates: null },
  };
}

describe("the drift types (UI-1 of health-drift)", () => {
  it.each(DRIFT_NAMES)("%s has exactly the cited keys, declared in their order", (name) => {
    expect(Object.keys(DRIFT_KEYS[name]).join(", ")).toBe(DRIFT_CITED[name]);
    expect(declaredKeys(name).join(", ")).toBe(DRIFT_CITED[name]);
  });

  it("counts row 6's 6 keys, verify's 7 counts, a row's 17, a marker's 2, a lock's 3, the levels' 3", () => {
    expect(DRIFT_NAMES.map((name) => Object.keys(DRIFT_KEYS[name]).length)).toEqual([6, 7, 17, 2, 3, 3]);
    expect(Object.keys(aConflictRow())).toEqual(DRIFT_CITED.DriftRow.split(", "));
  });

  it.each(DRIFT_NAMES)("%s cites the spec's Data, the path and heading on one line", (name) => {
    expect(commentOf(name).split("\n").some((line) => line.includes(HEALTH_DRIFT_DATA))).toBe(true);
  });

  it.each(DRIFT_NAMES.filter((name) => name !== "HealthDrift"))("%s cites verify's JSON: its shape is plain verify's, verbatim", (name) => {
    expect(commentOf(name)).toContain(VERIFY_JSON);
  });

  it("gives the health document its drift after unbound, the seventh and last key, never null; cites the spec's Data on one line", () => {
    expect(declaredKeys("HealthReport")).toEqual(["documents", "amendments", "notes", "code", "assumes", "unbound", "drift"]);
    expect(source).toMatch(/^ {2}unbound: HealthUnbound;\n {2}drift: HealthDrift;\n\}$/m);
    expect(commentOf("HealthReport").split("\n").some((line) => line.includes(HEALTH_DRIFT_DATA))).toBe(true);
  });

  it("reuses the binding row's anchor and places and the reads' tested entry: no drift copy of them", () => {
    expect(source).toMatch(/^ {2}levels: DriftLevels \| null;\n {2}anchor: BindingAnchor \| null;\n\}$/m);
    expect(source).toMatch(/^ {2}rows: DriftRow\[\] \| null;\n {2}tested: TestedRow\[\] \| null;\n {2}notes: string\[\];\n\}$/m);
    expect(source).not.toMatch(/^export (?:interface|type) Drift(?:Anchor|Place|Tested)\w*/m);
    const moved: NonNullable<DriftRow["anchor"]> = {
      state: "moved",
      from: { file: "crates/game/src/old.rs", kind: "item", symbol: "game::old::regen_system", item_kind: "function_item" },
      candidates: null,
    };
    const row: DriftRow = { ...aConflictRow(), anchor: moved };
    expect(row.anchor?.from?.file).toBe("crates/game/src/old.rs");
  });

  it("types a row's state as verify's states, any other word passing; its class, words and levels plain strings", () => {
    expect(source).toMatch(/^ {2}item_kind: string \| null;\n {2}state: SyncState;\n/m);
    const other: DriftRow["state"] = "drifting";
    const row: Pick<DriftRow, "relation" | "kind" | "item_kind" | "category" | "detail" | "class" | "compared" | "levels"> = {
      relation: "any-relation",
      kind: "any-kind",
      item_kind: "any_item",
      category: "any_category",
      detail: "any detail",
      class: "any-class",
      compared: "any-compare",
      levels: { lock: ["any"], markers: [], changed: null },
    };
    expect([other, Object.values(row).length]).toEqual(["drifting", 8]);
  });

  it("keeps counts, rows and tested null unless measured, never 0; causes and notes lists", () => {
    const off: HealthDrift = {
      state: "off",
      causes: ["specengine.toml has no [code] table: the code layer is off (add [code] with roots)"],
      counts: null,
      rows: null,
      tested: null,
      notes: [],
    };
    const measured: HealthDrift = {
      state: "measured",
      causes: [],
      counts: { rows: 9, ok: 8, fix: 1, check: 0, cannot: 0, pre_existing: 0, skipped: 1 },
      rows: [aConflictRow()],
      tested: [],
      notes: [],
    };
    const report: Pick<HealthReport, "drift"> = { drift: off };
    expect([report.drift.counts, report.drift.rows, report.drift.tested, measured.rows?.[0]?.class]).toEqual([null, null, null, "fix"]);
  });

  it("keeps a row's keyless place, markers' revision, node revision, lock, compared, levels and anchor nullable, as verify sends them", () => {
    const keyless: DriftRow = {
      ...aConflictRow(),
      kind: null,
      symbol: null,
      item_kind: null,
      state: "cannot_verify",
      category: "unanchored",
      class: "cannot",
      markers: [{ line: 7, rev: null }],
      node_rev: null,
      lock: null,
      compared: null,
      levels: null,
      anchor: null,
    };
    expect(Object.keys(keyless)).toEqual(DRIFT_CITED.DriftRow.split(", "));
    expect(source).toMatch(/^ {2}line: number;\n {2}rev: number \| null;\n\}$/m);
    expect(source).toMatch(/^ {2}lock: string\[\];\n {2}markers: string\[\];\n {2}changed: string\[\] \| null;\n\}$/m);
  });
});
