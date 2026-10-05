import { afterEach, describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { Proposal } from "../api/types";
import { MockClient } from "./MockClient";
import { scenarioFromSearch, SLOW_MS } from "./scenario";

// The mock behind SpecEngineClient (`ui/README.md` "Owner's manual steps"): scenarios by query,
// decisions in memory.

const NOW = Date.parse("2026-10-05T12:00:00Z");
const clock = () => NOW;

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

function ids(proposals: Proposal[]): string[] {
  return proposals.map((proposal) => proposal.id);
}

afterEach(() => {
  vi.useRealTimers();
});

describe("scenarioFromSearch", () => {
  it.each([
    ["", "normal"],
    ["?scenario=empty", "empty"],
    ["?scenario=error", "error"],
    ["?scenario=slow", "slow"],
    ["?scenario=conflict", "conflict"],
    ["?scenario=large", "large"],
    ["?scenario=bogus", "normal"],
    ["?other=1", "normal"],
  ])("reads %j as %s", (search, scenario) => {
    expect(scenarioFromSearch(search)).toBe(scenario);
  });
});

describe("the normal scenario", () => {
  it("serves two invented projects", async () => {
    const client = new MockClient("normal", { now: clock });
    expect(client.dataSource).toBe("mock");
    expect(await client.getProjects()).toEqual([
      { slug: "harbor-sim", name: "Harbor Sim" },
      { slug: "ledger-api", name: "Ledger API" },
    ]);
  });

  it("holds every case the slice names in harbor-sim's queue", async () => {
    const { proposals } = await new MockClient("normal", { now: clock }).getInbox("harbor-sim");
    const severities = new Set(proposals.map((proposal) => proposal.severity));
    for (const severity of ["high", "normal", "low", "urgent", null]) {
      expect(severities.has(severity)).toBe(true);
    }
    const known = ["discrepancy", "question", "update", "create", "decision", "interpretation", "amendment"];
    expect(proposals.some((proposal) => !known.includes(proposal.kind))).toBe(true);
    const statuses = ["open", "changes_requested", "approved", "applied", "rejected", "deferred", "superseded"];
    expect(proposals.some((proposal) => !statuses.includes(proposal.status))).toBe(true);
    expect(proposals.some((proposal) => proposal.options.length > 0 && proposal.recommendation !== null)).toBe(true);
    expect(proposals.some((proposal) => proposal.options.length === 0)).toBe(true);
    const targets = proposals.flatMap((proposal) => proposal.target_ids);
    expect(targets.some((target, index) => targets.indexOf(target) !== index)).toBe(true);
    expect(proposals.some((proposal) => proposal.diff !== null)).toBe(true);
    const texts = proposals.flatMap((proposal) => [
      proposal.summary ?? "",
      ...proposal.evidence.flatMap((item) => [item.observed, item.documented]),
    ]);
    expect(texts.some((text) => text.split(/\s+/).some((token) => token.length >= 300))).toBe(true);
    expect(texts.some((text) => text !== text.normalize("NFC") && /[^\p{Script=Latin}\p{Script=Common}\p{Script=Inherited}]/u.test(text))).toBe(true);
  });

  it("returns copies the caller cannot change", async () => {
    const client = new MockClient("normal", { now: clock });
    const first = await client.getInbox("harbor-sim");
    first.proposals.length = 0;
    expect((await client.getInbox("harbor-sim")).proposals.length).toBeGreaterThan(0);
  });

  it("reads a node as spec show --json does, or gives the reason", async () => {
    const client = new MockClient("normal", { now: clock });
    const view = await client.getNode("harbor-sim", "RULE-BERTH-DRAFT");
    expect(view.reason).toBeNull();
    expect(view.nodes.map((node) => node.id)).toEqual(["RULE-BERTH-DRAFT"]);
    const missing = await client.getNode("harbor-sim", "R-404");
    expect(missing.nodes).toEqual([]);
    expect(missing.reason).toContain("R-404");
  });

  it("accepts into applied with the commit, and the proposal leaves", async () => {
    const client = new MockClient("normal", { now: clock });
    const result = await client.decideProposal("harbor-sim", "PR-0041", { decision: "accept", option: 0, note: "go" });
    expect(result.proposal.status).toBe("applied");
    expect(result.commit?.subject).toBe("spec: apply PR-0041");
    expect(result.commit?.sha).toMatch(/^[0-9a-f]{40}$/);
    expect(result.proposal.applied_commit).toBe(result.commit?.sha);
    expect(ids((await client.getInbox("harbor-sim")).proposals)).not.toContain("PR-0041");
  });

  it("rejects into rejected, and the proposal leaves", async () => {
    const client = new MockClient("normal", { now: clock });
    const result = await client.decideProposal("harbor-sim", "PR-0042", { decision: "reject", reason: "Too early" });
    expect(result.proposal.status).toBe("rejected");
    expect(result.proposal.decision_note).toBe("Too early");
    expect(result.commit).toBeNull();
    expect(ids((await client.getInbox("harbor-sim")).proposals)).not.toContain("PR-0042");
  });

  it("sends back into changes_requested and defers into deferred; both stay", async () => {
    const client = new MockClient("normal", { now: clock });
    const sentBack = await client.decideProposal("ledger-api", "PR-0007", {
      decision: "needs_clarification",
      note: "Which customers?",
    });
    expect(sentBack.proposal.status).toBe("changes_requested");
    const deferred = await client.decideProposal("ledger-api", "PR-0009", { decision: "defer", note: null });
    expect(deferred.proposal.status).toBe("deferred");
    const inbox = (await client.getInbox("ledger-api")).proposals;
    expect(inbox.find((proposal) => proposal.id === "PR-0007")?.status).toBe("changes_requested");
    expect(inbox.find((proposal) => proposal.id === "PR-0009")?.status).toBe("deferred");
  });

  it("refuses in the daemon's words: an empty reason, a conflicting apply, an unknown proposal", async () => {
    const client = new MockClient("normal", { now: clock });
    expect((await rejection(client.decideProposal("harbor-sim", "PR-0042", { decision: "reject", reason: " " }))).status).toBe(422);
    const conflict = await rejection(client.decideProposal("harbor-sim", "PR-0046", { decision: "accept", option: 0, note: null }));
    expect(conflict.status).toBe(422);
    expect(conflict.message).toContain("conflicts");
    expect(ids((await client.getInbox("harbor-sim")).proposals)).toContain("PR-0046");
    expect((await rejection(client.decideProposal("harbor-sim", "PR-9999", { decision: "defer", note: null }))).status).toBe(404);
  });

  it("forgets every decision when made again (a reload)", async () => {
    await new MockClient("normal", { now: clock }).decideProposal("harbor-sim", "PR-0041", {
      decision: "reject",
      reason: "No",
    });
    expect(ids((await new MockClient("normal", { now: clock }).getInbox("harbor-sim")).proposals)).toContain("PR-0041");
  });
});

describe("the other scenarios", () => {
  it("empty: projects, no proposals", async () => {
    const client = new MockClient("empty", { now: clock });
    expect(await client.getProjects()).toHaveLength(2);
    expect((await client.getInbox("harbor-sim")).proposals).toEqual([]);
    expect((await client.getInbox("ledger-api")).proposals).toEqual([]);
  });

  it("error: every read rejects with 503 and a message", async () => {
    const client = new MockClient("error", { now: clock });
    for (const call of [
      client.getProjects(),
      client.getInbox("harbor-sim"),
      client.getNode("harbor-sim", "MEC-TIDES"),
      client.getTree("harbor-sim"),
      client.search("harbor-sim", { query: "tide" }),
      client.getBundle("harbor-sim", { node_ids: ["MEC-TIDES"] }),
    ]) {
      const error = await rejection(call);
      expect(error.status).toBe(503);
      expect(error.message.length).toBeGreaterThan(0);
    }
  });

  it("slow: every call takes 1.5 s", async () => {
    vi.useFakeTimers();
    const client = new MockClient("slow", { now: clock });
    let done = false;
    const pending = client.getInbox("harbor-sim").then((inbox) => {
      done = true;
      return inbox;
    });
    await vi.advanceTimersByTimeAsync(SLOW_MS - 1);
    expect(done).toBe(false);
    await vi.advanceTimersByTimeAsync(1);
    expect(done).toBe(true);
    expect((await pending).proposals.length).toBeGreaterThan(0);
    expect(SLOW_MS).toBe(1500);
  });

  it("conflict: a decision rejects 409 and the proposal leaves the inbox", async () => {
    const client = new MockClient("conflict", { now: clock });
    const error = await rejection(client.decideProposal("harbor-sim", "PR-0041", { decision: "defer", note: null }));
    expect(error.status).toBe(409);
    expect(error.message).toContain("PR-0041");
    expect(ids((await client.getInbox("harbor-sim")).proposals)).not.toContain("PR-0041");
  });
});
