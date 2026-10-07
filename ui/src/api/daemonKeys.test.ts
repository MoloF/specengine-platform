import { describe, expect, it } from "vitest";
import source from "./provisional.ts?raw";
import type {
  BundleView,
  CheckCause,
  CheckCounts,
  CheckFinding,
  CheckReport,
  DebtEntry,
  FollowedType,
  GraphEdge,
  GraphNode,
  GraphView,
  InboxEntry,
  NodeView,
  Project,
  Proposal,
  QueueEvent,
  SearchHit,
} from "./types";

// AC-09 of docs/features/daemon-read.md and of docs/features/ui-live.md: the provisional types of
// the daemon's documents. Each record names exactly its type's keys (`satisfies` fails `pnpm
// build` otherwise) and equals the list the cited headings write; the keys the daemon serves (the
// record less the base-only keys of a plain check, NEVER_SERVED) equal `fixtures/daemon-keys.json`,
// which a Rust test regenerates from the daemon on fixture A: daemon-read's six names, ui-live's
// nine. While that file is absent the comparison is skipped, saying so; once it exists it names
// all fifteen.

const KEYS = {
  // docs/features/daemon-read.md "Data": `[{slug, name, root, branch}]`.
  Project: { slug: true, name: true, root: true, branch: true } satisfies Record<keyof Project, true>,
  // Same heading, "Inbox entry" (D1, with decision-apply).
  InboxEntry: {
    id: true,
    kind: true,
    status: true,
    target_id: true,
    target_ids: true,
    branch: true,
    created_at: true,
    rationale: true,
    severity: true,
    summary: true,
    record_id: true,
  } satisfies Record<keyof InboxEntry, true>,
  // docs/canon/proposal-queue.md "Commands" to `updated_at`, docs/canon/agent-intake.md "Review
  // document" the eleven after it, docs/features/decision-apply.md "Data" the five after `linked`.
  Proposal: {
    id: true,
    project: true,
    kind: true,
    status: true,
    target_id: true,
    target_path: true,
    worktree: true,
    branch: true,
    base_commit: true,
    base_hash: true,
    base_text: true,
    new_text: true,
    patch_hash: true,
    rationale: true,
    author: true,
    diagnostics: true,
    diff: true,
    preview: true,
    conflict: true,
    decided_by: true,
    decided_at: true,
    decision_note: true,
    applied_commit: true,
    created_at: true,
    updated_at: true,
    target_ids: true,
    severity: true,
    gap_type: true,
    summary: true,
    working_answer: true,
    price_of_other: true,
    evidence: true,
    options: true,
    recommendation: true,
    distinct_from: true,
    linked: true,
    record_id: true,
    record_path: true,
    record_title: true,
    record_text: true,
    choice: true,
    notes: true,
  } satisfies Record<keyof Proposal, true>,
  // crates/specengine-cli/README.md "Output and the cap": `spec show --json`.
  NodeView: { ref: true, reason: true, notes: true, nodes: true } satisfies Record<keyof NodeView, true>,
  // Same heading: a hit.
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
  // docs/canon/spec-cli-bundle.md "Output".
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
  // docs/canon/spec-cli-graph.md "spec graph": the JSON, a node, an edge, `types`' entries.
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
  GraphNode: { id: true, kind: true, title: true, path: true, line: true, distance: true, archived: true } satisfies Record<keyof GraphNode, true>,
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
  FollowedType: { type: true, direction: true } satisfies Record<keyof FollowedType, true>,
  // docs/canon/spec-check.md "Findings, debt, verdict": the JSON, its counts, a finding.
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
  // docs/features/ui-live.md "Data": a `stale` entry, a `cannot_check` cause.
  DebtEntry: { code: true, path: true, subject: true, reason: true, expires: true, line: true } satisfies Record<keyof DebtEntry, true>,
  CheckCause: { path: true, message: true } satisfies Record<keyof CheckCause, true>,
};

type Named = keyof typeof KEYS;

/** The key lists as the cited headings write them, copied verbatim (the review document's joined). */
const CITED: Record<Named, string> = {
  Project: "slug, name, root, branch",
  InboxEntry: "id, kind, status, target_id, target_ids, branch, created_at, rationale, severity, summary, record_id",
  Proposal:
    "id, project, kind, status, target_id, target_path, worktree, branch, base_commit, base_hash, base_text, new_text, patch_hash, rationale, author, diagnostics, diff, preview, conflict, decided_by, decided_at, decision_note, applied_commit, created_at, updated_at, " +
    "target_ids, severity, gap_type, summary, working_answer, price_of_other, evidence, options, recommendation, distinct_from, linked, " +
    "record_id, record_path, record_title, record_text, choice, " +
    "notes",
  NodeView: "ref, reason, notes, nodes",
  SearchHit: "id, kind, title, path, line, ord, archived, snippet",
  BundleView: "refs, reason, notes, task, budget, tokens, chars, bytes, bundle_hash, body, layers, tail, more",
  GraphView: "ref, reason, impact, types, depth, archive, notes, left_out, truncated, nodes, edges",
  GraphNode: "id, kind, title, path, line, distance, archived",
  GraphEdge: "src, type, dst, written, path, line, state, reason",
  FollowedType: "type, direction",
  CheckReport: "mode, verdict, counts, findings, stale, new_debt?, cannot_check",
  CheckCounts: "documents, errors, warnings, debt, expired, stale, introduced?, new_debt?, worst_w_bytes",
  CheckFinding: "code, severity, path, line, subject, message, fix?, debt?, introduced?",
  DebtEntry: "code, path, subject, reason, expires, line",
  CheckCause: "path, message",
};

const NAMES = Object.keys(KEYS) as Named[];

/** daemon-read's six (docs/features/daemon-read.md AC-09). */
const DAEMON_READ: readonly Named[] = ["Project", "InboxEntry", "Proposal", "NodeView", "SearchHit", "BundleView"];

/** ui-live's nine (docs/features/ui-live.md AC-09). */
const UI_LIVE: readonly Named[] = ["GraphView", "GraphNode", "GraphEdge", "FollowedType", "CheckReport", "CheckCounts", "CheckFinding", "DebtEntry", "CheckCause"];

/**
 * The keys only a check against a git base sends: the daemon runs the plain check, so it never
 * serves them (`docs/canon/spec-check.md` "Findings, debt, verdict"; docs/features/ui-live.md "Data").
 */
const NEVER_SERVED: readonly (readonly [Named, string])[] = [
  ["CheckReport", "new_debt"],
  ["CheckCounts", "introduced"],
  ["CheckCounts", "new_debt"],
  ["CheckFinding", "introduced"],
];

/** The keys the daemon sends for a type: its record's, less the never-served ones. */
function served(name: Named): string[] {
  return Object.keys(KEYS[name]).filter((key) => !NEVER_SERVED.some(([type, never]) => type === name && never === key));
}

describe("the daemon's document types (AC-09)", () => {
  it.each(NAMES)("%s has exactly the cited keys, in order", (name) => {
    // An optional key is cited with its `?` (W-1: omitted when unset, never null).
    expect(Object.keys(KEYS[name]).join(", ")).toBe(CITED[name].replaceAll("?", ""));
  });

  it("counts 4 project keys, 11 inbox-entry keys, 42 review keys", () => {
    expect([KEYS.Project, KEYS.InboxEntry, KEYS.Proposal].map((keys) => Object.keys(keys).length)).toEqual([4, 11, 42]);
  });

  it("counts the daemon's keys of ui-live's nine: 11, 7, 8, 2; 6, 7, 8, 6, 2 (AC-09 of ui-live)", () => {
    expect(Object.fromEntries(UI_LIVE.map((name) => [name, served(name).length]))).toEqual({
      GraphView: 11,
      GraphNode: 7,
      GraphEdge: 8,
      FollowedType: 2,
      CheckReport: 6,
      CheckCounts: 7,
      CheckFinding: 8,
      DebtEntry: 6,
      CheckCause: 2,
    });
    expect([...DAEMON_READ, ...UI_LIVE].sort()).toEqual([...NAMES].sort());
  });

  it("lists each base-only key once, a key of its type the citation marks optional", () => {
    expect(NEVER_SERVED.map(([name, key]) => `${name}.${key}`)).toEqual([
      "CheckReport.new_debt",
      "CheckCounts.introduced",
      "CheckCounts.new_debt",
      "CheckFinding.introduced",
    ]);
    for (const [name, key] of NEVER_SERVED) {
      expect([name, key, CITED[name].split(", ").includes(`${key}?`)]).toEqual([name, key, true]);
    }
  });

  it("names no task in an inbox entry or a review document", () => {
    expect(Object.keys(KEYS.InboxEntry)).not.toContain("task_id");
    expect(Object.keys(KEYS.Proposal)).not.toContain("task_id");
  });

  it("types a queue event as the stream sends it: {seq, type, payload}", () => {
    const event = { seq: 12, type: "proposal.created", payload: { id: "PR-0005" } } satisfies QueueEvent;
    expect(Object.keys(event)).toEqual(["seq", "type", "payload"]);
  });

  it.each(["Project", "InboxEntry", "Proposal", "QueueEvent"])("%s cites docs/features/daemon-read.md \"Data\"", (type) => {
    const at = source.indexOf(`export interface ${type} `);
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain('`docs/features/daemon-read.md` "Data"');
  });

  it.each([
    ...["GraphView", "GraphNode", "GraphEdge", "FollowedType"].map((type) => [type, '`docs/canon/spec-cli-graph.md` "spec graph"'] as const),
    ...["CheckReport", "CheckCounts", "CheckFinding", "DebtEntry", "CheckCause"].map(
      (type) => [type, '`docs/canon/spec-check.md` "Findings, debt, verdict"'] as const,
    ),
  ])("%s cites %s", (type, citation) => {
    const at = Math.max(source.indexOf(`export interface ${type} `), source.indexOf(`export type ${type} =`));
    expect(at).toBeGreaterThan(0);
    expect(source.slice(source.lastIndexOf("/**", at), at)).toContain(citation);
  });
});

/** `fixtures/daemon-keys.json` as parsed JSON, when a Rust test has written it (absent: undefined). */
const written = Object.values(import.meta.glob<unknown>("../../../fixtures/daemon-keys.json", { import: "default", eager: true }))[0];

const ABSENT =
  "fixtures/daemon-keys.json is absent: the daemon's key-set test (crates/specengine-http/tests) writes it; " +
  "the comparison with the daemon is skipped until then";

/** The file's lists by type name; a malformed file fails here, naming what is wrong. */
function keySets(parsed: unknown): Record<string, string[]> {
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw new Error("fixtures/daemon-keys.json is not a JSON object of type name to key list");
  }
  const sets: Record<string, string[]> = {};
  for (const [name, keys] of Object.entries(parsed)) {
    if (!Array.isArray(keys) || !keys.every((key): key is string => typeof key === "string")) {
      throw new Error(`fixtures/daemon-keys.json: ${name} is not a list of key names`);
    }
    sets[name] = keys;
  }
  return sets;
}

describe.skipIf(written === undefined)(written === undefined ? ABSENT : "the key sets equal fixtures/daemon-keys.json (AC-09)", () => {
  const sets = written === undefined ? {} : keySets(written);

  it("names exactly the fifteen types: daemon-read's six, ui-live's nine", () => {
    expect(Object.keys(sets).sort()).toEqual([...DAEMON_READ, ...UI_LIVE].sort());
  });

  it.each(NAMES)("%s: the daemon's keys are the type's", (name) => {
    expect([...(sets[name] ?? [])].sort()).toEqual(served(name).sort());
  });
});
