import { describe, expect, it } from "vitest";
import source from "./provisional.ts?raw";
import type { BundleView, InboxEntry, NodeView, Project, Proposal, QueueEvent, SearchHit } from "./types";

// AC-09 of docs/features/daemon-read.md: the provisional types of the daemon's documents. Each
// record names exactly its type's keys (`satisfies` fails `pnpm build` otherwise) and equals the
// list the cited headings write; the six AC-09 names equal `fixtures/daemon-keys.json`, which a
// Rust test regenerates from the daemon on fixture A. While that file is absent the comparison is
// skipped, saying so.

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
};

const NAMES = Object.keys(KEYS) as Named[];

describe("the daemon's document types (AC-09)", () => {
  it.each(NAMES)("%s has exactly the cited keys, in order", (name) => {
    expect(Object.keys(KEYS[name]).join(", ")).toBe(CITED[name]);
  });

  it("counts 4 project keys, 11 inbox-entry keys, 42 review keys", () => {
    expect([KEYS.Project, KEYS.InboxEntry, KEYS.Proposal].map((keys) => Object.keys(keys).length)).toEqual([4, 11, 42]);
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

  it("names exactly the six types", () => {
    expect(Object.keys(sets).sort()).toEqual([...NAMES].sort());
  });

  it.each(NAMES)("%s: the daemon's keys are the type's", (name) => {
    expect([...(sets[name] ?? [])].sort()).toEqual(Object.keys(KEYS[name]).sort());
  });
});
