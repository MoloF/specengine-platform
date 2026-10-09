import { describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { HealthReport } from "../api/types";
import { checkReportOf } from "./check";
import { nodeViewOf } from "./corpus";
import { harborSim } from "./harbor-sim/fixtures";
import { healthReportOf, LARGE_AMENDMENTS, walkGapMessage } from "./health";
import { MockClient } from "./MockClient";
import { SCENARIOS } from "./scenario";

// The mock's `health` (docs/features/health-left.md "Data", Mocks, W13): one builder, no clock, the
// core's order; normal, empty, large and cannot-check as the spec lists them, `error` and `slow`
// as every read.

const PROJECTS = ["harbor-sim", "ledger-api"] as const;

async function rejection(promise: Promise<unknown>): Promise<ClientError> {
  try {
    await promise;
  } catch (error) {
    if (error instanceof ClientError) {
      return error;
    }
    throw error;
  }
  throw new Error("the call resolved");
}

/** Byte order, as the core sorts paths. */
function byBytes(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

describe("the mock's health read", () => {
  it("normal: harbor-sim's two, the spec's decision with a dangling link and a draft spec amending RULE-BERTH-DRAFT, resolved", () => {
    const report = healthReportOf("harbor-sim", "normal");
    expect(Object.keys(report)).toEqual(["documents", "amendments", "notes"]);
    expect(report.amendments.map((amendment) => Object.keys(amendment))).toEqual([
      ["path", "id", "title", "class", "status", "links"],
      ["path", "id", "title", "class", "status", "links"],
    ]);
    expect(report.amendments.map(({ path, id, class: kind, status }) => [path, id, kind, status])).toEqual([
      ["docs/decisions/DEC-0014.md", "DEC-0014", "decision", "accepted"],
      ["docs/features/deep-berth.md", null, "spec", "draft"],
    ]);
    expect(report.amendments.flatMap((amendment) => amendment.links.map(({ type, written, name, state }) => [type, written, name, state]))).toEqual([
      ["amends", "RULE-QUAY-LIGHTS", null, "dangling"],
      ["amends", "RULE-BERTH-DRAFT", "RULE-BERTH-DRAFT", "resolved"],
    ]);
    expect(report.notes).toEqual([]);
    expect(healthReportOf("ledger-api", "normal").amendments).toEqual([]);
  });

  it("normal: each of harbor-sim's two is a document of its corpus, listed as its node and `show --links` give it (AC-R2's rule, mocked)", () => {
    const { corpus } = harborSim(Date.parse("2026-10-09T12:00:00Z"));
    for (const amendment of healthReportOf("harbor-sim", "normal").amendments) {
      const view = nodeViewOf(corpus, amendment.path, { with: ["links"] });
      const node = view.nodes[0];
      expect([amendment.path, view.reason, view.nodes.length]).toEqual([amendment.path, null, 1]);
      expect([node?.id, node?.title, node?.status]).toEqual([amendment.id, amendment.title, amendment.status]);
      expect(node?.text).toContain(`class: ${amendment.class ?? "?"}`);
      expect(node?.links?.outgoing.filter((link) => link.type === "amends")).toEqual(amendment.links);
    }
  });

  it("empty: none for either project", () => {
    for (const project of PROJECTS) {
      expect(healthReportOf(project, "empty").amendments).toEqual([]);
    }
  });

  it("large: harbor-sim's 3 000 by path, each with one link, never empty", () => {
    const { amendments } = healthReportOf("harbor-sim", "large");
    expect(amendments).toHaveLength(LARGE_AMENDMENTS);
    const paths = amendments.map((amendment) => amendment.path);
    expect(paths).toEqual([...paths].sort(byBytes));
    expect(new Set(paths).size).toBe(LARGE_AMENDMENTS);
    expect(amendments.every((amendment) => amendment.links.length === 1)).toBe(true);
    expect(new Set(amendments.flatMap((amendment) => amendment.links.map((link) => link.state)))).toEqual(new Set(["resolved", "dangling"]));
  });

  it.each(SCENARIOS.filter((scenario) => scenario !== "cannot-check"))("%s: documents are the check's counts.documents for the same project", (scenario) => {
    for (const project of PROJECTS) {
      expect([project, healthReportOf(project, scenario).documents]).toEqual([project, checkReportOf(project, scenario).counts.documents]);
    }
  });

  it("returns copies the caller cannot change", async () => {
    const client = new MockClient("normal");
    const first: HealthReport = await client.getHealth("harbor-sim");
    first.amendments.pop();
    expect((await client.getHealth("harbor-sim")).amendments).toHaveLength(2);
  });

  it("cannot-check: a 503 naming the walk's first gap, as the CLI words it; the check's causes hold it", async () => {
    const client = new MockClient("cannot-check");
    for (const project of PROJECTS) {
      const error = await rejection(client.getHealth(project));
      expect([error.status, error.message]).toEqual([503, walkGapMessage(project)]);
      expect(error.message).toMatch(/^spec: the walk is incomplete: .+: unapplied amendments not measured$/);
      const first = checkReportOf(project, "cannot-check").cannot_check[0];
      expect(error.message).toContain(`\`${first?.path ?? "?"}\``);
    }
  });

  it("error: a 503 as every read; an unknown project its 404", async () => {
    const error = await rejection(new MockClient("error").getHealth("harbor-sim"));
    expect(error.status).toBe(503);
    expect((await rejection(new MockClient("normal").getHealth("zeta"))).status).toBe(404);
  });

  it("slow: answers after the scenario's delay", async () => {
    vi.useFakeTimers();
    try {
      const client = new MockClient("slow");
      let answered = false;
      void client.getHealth("harbor-sim").then(() => {
        answered = true;
      });
      await vi.advanceTimersByTimeAsync(1_000);
      expect(answered).toBe(false);
      await vi.advanceTimersByTimeAsync(600);
      expect(answered).toBe(true);
    } finally {
      vi.useRealTimers();
    }
  });
});
