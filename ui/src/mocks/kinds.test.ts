import { describe, expect, it } from "vitest";
import { harborSim } from "./harbor-sim/fixtures";
import { nodeKinds as harborKinds } from "./harbor-sim/kinds";
import { ledgerApi } from "./ledger-api/fixtures";
import { nodeKinds as ledgerKinds } from "./ledger-api/kinds";

// AC-19 of docs/features/ui-shell.md: two projects with disjoint node vocabularies, each node's
// kind in its project's set, and no kind quoted in app code; mocks are ASCII (escapes only).

const sources = import.meta.glob<string>("/src/**/*.{ts,tsx}", { query: "?raw", import: "default", eager: true });

function isAppCode(path: string): boolean {
  return !path.startsWith("/src/mocks/") && !path.startsWith("/src/test/") && !/\.test\.tsx?$/.test(path);
}

describe("node kinds", () => {
  it("are disjoint between the two projects", () => {
    expect(harborKinds.filter((kind) => (ledgerKinds as readonly string[]).includes(kind))).toEqual([]);
  });

  it("cover every mock node of each project", () => {
    for (const project of [harborSim(0), ledgerApi(0)]) {
      expect(project.nodes.length).toBeGreaterThan(0);
      for (const node of project.nodes) {
        expect([project.project.slug, project.nodeKinds.includes(node.kind ?? "")]).toEqual([project.project.slug, true]);
      }
    }
    expect(harborSim(0).nodeKinds).toBe(harborKinds);
    expect(ledgerApi(0).nodeKinds).toBe(ledgerKinds);
  });

  it("name every target of the queues", () => {
    for (const project of [harborSim(0), ledgerApi(0)]) {
      const ids = new Set(project.nodes.map((node) => node.id));
      for (const target of project.proposals.flatMap((proposal) => proposal.target_ids)) {
        expect([project.project.slug, target, ids.has(target)]).toEqual([project.project.slug, target, true]);
      }
    }
  });

  it("are never quoted in app code", () => {
    const hits: string[] = [];
    for (const [path, text] of Object.entries(sources)) {
      if (!isAppCode(path)) {
        continue;
      }
      for (const kind of [...harborKinds, ...ledgerKinds]) {
        if (new RegExp(`["'\`]${kind}["'\`]`).test(text)) {
          hits.push(`${path}: ${kind}`);
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
