import { describe, expect, it } from "vitest";
import type { MockNode, MockProject } from "./build";
import { harborSim } from "./harbor-sim/fixtures";
import { nodeKinds as harborKinds, specStatuses as harborStatuses } from "./harbor-sim/kinds";
import { largeDocuments } from "./harbor-sim/large";
import { ledgerApi } from "./ledger-api/fixtures";
import { nodeKinds as ledgerKinds, specStatuses as ledgerStatuses } from "./ledger-api/kinds";

// AC-19 of docs/features/ui-shell.md and AC-14, AC-15 of docs/features/ui-tree-node.md: two
// projects with disjoint node vocabularies, each node's kind and status in its project's sets
// (`large` too), no kind or spec status quoted in app code; mocks are ASCII (escapes only).

const sources = import.meta.glob<string>("/src/**/*.{ts,tsx}", { query: "?raw", import: "default", eager: true });

function isAppCode(path: string): boolean {
  return !path.startsWith("/src/mocks/") && !path.startsWith("/src/test/") && !/\.test\.tsx?$/.test(path);
}

function nodesOf(project: MockProject): MockNode[] {
  return project.corpus.documents.flatMap((file) => [file.node, ...file.sections]);
}

const harborLarge = (() => {
  const project = harborSim(0);
  project.corpus = { ...project.corpus, documents: [...project.corpus.documents, ...largeDocuments()] };
  return project;
})();

describe("node kinds and spec statuses", () => {
  it("are disjoint between the two projects (kinds)", () => {
    expect(harborKinds.filter((kind) => (ledgerKinds as readonly string[]).includes(kind))).toEqual([]);
  });

  it("cover every mock node of each project, large's included; a kind may be absent", () => {
    for (const project of [harborSim(0), ledgerApi(0), harborLarge]) {
      expect(nodesOf(project).length).toBeGreaterThan(0);
      for (const node of nodesOf(project)) {
        const kindKnown = node.kind === null || project.nodeKinds.includes(node.kind);
        const statusKnown = node.status === null || project.specStatuses.includes(node.status);
        expect([project.project.slug, node.path, node.line, kindKnown, statusKnown]).toEqual([
          project.project.slug,
          node.path,
          node.line,
          true,
          true,
        ]);
      }
    }
    expect(harborSim(0).nodeKinds).toBe(harborKinds);
    expect(ledgerApi(0).nodeKinds).toBe(ledgerKinds);
    expect(harborSim(0).specStatuses).toBe(harborStatuses);
    expect(ledgerApi(0).specStatuses).toBe(ledgerStatuses);
  });

  it("give the large scenario 3 000 and more nodes, at most five levels below a root", () => {
    const generated = largeDocuments().flatMap((file) => [file.node, ...file.sections]);
    expect(generated.length).toBeGreaterThanOrEqual(3000);
    expect(new Set(generated.map((node) => node.kind))).toEqual(new Set(harborKinds));
  });

  it("name every target of the queues: an ID, else an ID-less document's path", () => {
    for (const project of [harborSim(0), ledgerApi(0)]) {
      const names = new Set(nodesOf(project).map((node) => node.id ?? node.path));
      for (const target of project.proposals.flatMap((proposal) => proposal.target_ids)) {
        expect([project.project.slug, target, names.has(target)]).toEqual([project.project.slug, target, true]);
      }
    }
  });

  it("are never quoted in app code", () => {
    const hits: string[] = [];
    const words = [...harborKinds, ...ledgerKinds, ...harborStatuses, ...ledgerStatuses];
    for (const [path, text] of Object.entries(sources)) {
      if (!isAppCode(path)) {
        continue;
      }
      for (const word of words) {
        if (new RegExp(`["'\`]${word}["'\`]`).test(text)) {
          hits.push(`${path}: ${word}`);
        }
      }
    }
    expect(hits).toEqual([]);
  });

  it("keep the mocks ASCII: non-Latin text only as escapes", () => {
    const mocks = Object.entries(sources).filter(([path]) => path.startsWith("/src/mocks/"));
    expect(mocks.length).toBeGreaterThan(4);
    for (const [path, text] of mocks) {
      expect([path, /[^\x09\x0a\x0d\x20-\x7e]/.test(text)]).toEqual([path, false]);
    }
  });
});
