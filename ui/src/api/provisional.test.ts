import { describe, expect, it } from "vitest";
import source from "./provisional.ts?raw";
import type {
  BundleItem,
  BundleLayers,
  BundleVia,
  BundleView,
  Proposal,
  SearchHit,
  SearchResults,
  ShownNode,
  Snippet,
  SnippetSegment,
  TailEntry,
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
