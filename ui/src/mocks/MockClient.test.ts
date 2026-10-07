import { afterEach, describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { InboxEntry } from "../api/types";
import { MockClient } from "./MockClient";
import { scenarioFromSearch, SLOW_MS } from "./scenario";

// The mock behind SpecEngineClient (`ui/README.md` "Owner's manual steps"): scenarios by query,
// stages in memory, as the daemon stages (AC-14 of docs/features/decision-staging.md: the mock alike).

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

function ids(proposals: InboxEntry[]): string[] {
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
    ["?scenario=cannot-check", "cannot-check"],
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
      { slug: "harbor-sim", name: "Harbor Sim", root: "/work/harbor-sim", branch: "main" },
      { slug: "ledger-api", name: "Ledger API", root: "/work/ledger-api", branch: "main" },
    ]);
  });

  it("holds every case the slice names in harbor-sim's queue", async () => {
    const client = new MockClient("normal", { now: clock });
    const { proposals } = await client.getInbox("harbor-sim");
    const reviews = await Promise.all(proposals.map((entry) => client.getProposal("harbor-sim", entry.id)));
    const severities = new Set(proposals.map((proposal) => proposal.severity));
    for (const severity of ["high", "normal", "low", "urgent", null]) {
      expect(severities.has(severity)).toBe(true);
    }
    const known = ["discrepancy", "question", "update", "create", "decision", "interpretation", "amendment"];
    expect(proposals.some((proposal) => !known.includes(proposal.kind))).toBe(true);
    const statuses = ["open", "changes_requested", "approved", "applied", "rejected", "deferred", "superseded"];
    expect(proposals.some((proposal) => !statuses.includes(proposal.status))).toBe(true);
    expect(reviews.some((proposal) => proposal.options.length > 0 && proposal.recommendation !== null)).toBe(true);
    expect(reviews.some((proposal) => proposal.options.length === 0)).toBe(true);
    const targets = proposals.flatMap((proposal) => proposal.target_ids);
    expect(targets.some((target, index) => targets.indexOf(target) !== index)).toBe(true);
    expect(reviews.some((proposal) => proposal.diff !== null)).toBe(true);
    const texts = reviews.flatMap((proposal) => [
      proposal.summary ?? "",
      ...proposal.evidence.flatMap((item) => [item.observed, item.documented]),
    ]);
    expect(texts.some((text) => text.split(/\s+/).some((token) => token.length >= 300))).toBe(true);
    expect(texts.some((text) => text !== text.normalize("NFC") && /[^\p{Script=Latin}\p{Script=Common}\p{Script=Inherited}]/u.test(text))).toBe(true);
  });

  it("lists each proposal as spec inbox does and reads its review document by ID (daemon-read)", async () => {
    const client = new MockClient("normal", { now: clock });
    const { proposals } = await client.getInbox("harbor-sim");
    for (const entry of proposals) {
      expect(Object.keys(entry)).toEqual([
        "id",
        "kind",
        "status",
        "target_id",
        "target_ids",
        "branch",
        "created_at",
        "rationale",
        "severity",
        "summary",
        "record_id",
        "staged_at",
      ]);
      const review = await client.getProposal("harbor-sim", entry.id);
      expect([review.id, review.kind, review.status, review.created_at]).toEqual([entry.id, entry.kind, entry.status, entry.created_at]);
      expect(review.target_ids).toEqual(entry.target_ids);
      for (const line of [entry.summary, entry.rationale]) {
        expect(line === null || (!line.includes("\n") && Array.from(line).length <= 80)).toBe(true);
      }
    }
    const missing = await client.getProposal("harbor-sim", "PR-9999");
    expect([missing.id, missing.kind, missing.status]).toEqual([null, null, null]);
    expect(missing.notes.at(-1)).toBe("no proposal `PR-9999` in this project's queue");
  });

  it("serves each review document's task_id after choice, null unbound, as the task canon's Task-bound proposals", async () => {
    const client = new MockClient("normal", { now: clock });
    const { proposals } = await client.getInbox("harbor-sim");
    const reviews = await Promise.all(proposals.map((entry) => client.getProposal("harbor-sim", entry.id)));
    for (const review of reviews) {
      expect([review.id, Object.keys(review).slice(-5)]).toEqual([review.id, ["choice", "task_id", "staged", "staged_at", "notes"]]);
      expect([review.staged, review.staged_at]).toEqual([null, null]);
    }
    expect(Object.keys(proposals[0] ?? {}).slice(-2)).toEqual(["record_id", "staged_at"]);
    expect(reviews.find((review) => review.id === "PR-0041")?.task_id).toBe("T-0107");
    expect(reviews.some((review) => review.task_id === null)).toBe(true);
    expect((await client.getProposal("harbor-sim", "PR-9999")).task_id).toBeNull();
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

  it("stages an approve: the review document keeps it with its time, the proposal stays open and listed", async () => {
    const client = new MockClient("normal", { now: clock });
    const before = await client.getProposal("harbor-sim", "PR-0041");
    const staged = await client.stageDecision("harbor-sim", "PR-0041", { decision: "approve", option: 1, answer: null, canon: null, note: "go" }, before.updated_at ?? "");
    expect([staged.status, staged.staged, staged.staged_at, staged.updated_at]).toEqual([
      "open",
      { decision: "approve", option: 1, answer: null, canon: null, note: "go", span_hash: null },
      "2026-10-05T12:00:00Z",
      "2026-10-05T12:00:00Z",
    ]);
    expect(await client.getProposal("harbor-sim", "PR-0041")).toEqual(staged);
    const entry = (await client.getInbox("harbor-sim")).proposals.find((proposal) => proposal.id === "PR-0041");
    expect([entry?.status, entry?.staged_at]).toEqual(["open", "2026-10-05T12:00:00Z"]);
  });

  it("stages an update's approve with its target's span hash; a reject with its reason; a second stage replaces the first", async () => {
    const client = new MockClient("normal", { now: clock });
    const update = await client.getProposal("harbor-sim", "PR-0042");
    const approved = await client.stageDecision("harbor-sim", "PR-0042", { decision: "approve", option: null, answer: null, canon: null, note: null }, update.updated_at ?? "");
    expect(approved.staged).toEqual({ decision: "approve", option: null, answer: null, canon: null, note: null, span_hash: update.base_hash });
    const rejected = await client.stageDecision("harbor-sim", "PR-0042", { decision: "reject", reason: "Too early" }, approved.updated_at ?? "");
    expect(rejected.staged).toEqual({ decision: "reject", reason: "Too early" });
    expect(ids((await client.getInbox("harbor-sim")).proposals)).toContain("PR-0042");
  });

  it("unstages: both keys null; nothing staged, the document as it was", async () => {
    const client = new MockClient("normal", { now: clock });
    const before = await client.getProposal("ledger-api", "PR-0007");
    expect(await client.unstageDecision("ledger-api", "PR-0007")).toEqual(before);
    await client.stageDecision("ledger-api", "PR-0007", { decision: "reject", reason: "No" }, before.updated_at ?? "");
    const unstaged = await client.unstageDecision("ledger-api", "PR-0007");
    expect([unstaged.staged, unstaged.staged_at, unstaged.status]).toEqual([null, null, "open"]);
  });

  it("refuses in the daemon's words, nothing stored: flag usage 400, not open or changed since read 409, unknown 404", async () => {
    const client = new MockClient("normal", { now: clock });
    const read = (await client.getProposal("harbor-sim", "PR-0041")).updated_at ?? "";
    const approve = (option: number | null) => ({ decision: "approve" as const, option, answer: null, canon: null, note: null });
    const refusals = [
      await rejection(client.stageDecision("harbor-sim", "PR-0041", approve(null), read)),
      await rejection(client.stageDecision("harbor-sim", "PR-0042", approve(0), (await client.getProposal("harbor-sim", "PR-0042")).updated_at ?? "")),
      await rejection(client.stageDecision("harbor-sim", "PR-0041", { decision: "reject", reason: " " }, read)),
      await rejection(client.stageDecision("harbor-sim", "PR-0041", approve(5), read)),
      await rejection(client.stageDecision("harbor-sim", "PR-0041", approve(0), "2026-01-01T00:00:00Z")),
      await rejection(client.stageDecision("harbor-sim", "PR-0046", approve(0), (await client.getProposal("harbor-sim", "PR-0046")).updated_at ?? "")),
      await rejection(client.stageDecision("harbor-sim", "PR-9999", approve(0), read)),
      await rejection(client.unstageDecision("harbor-sim", "PR-0046")),
    ];
    expect(refusals.map((error) => error.status)).toEqual([400, 400, 400, 409, 409, 409, 404, 409]);
    expect(refusals[5]?.message).toBe("`PR-0046` is deferred, not open: only an open proposal takes a staged choice; nothing changed");
    expect((await client.getProposal("harbor-sim", "PR-0041")).staged).toBeNull();
  });

  it("forgets every stage when made again (a reload)", async () => {
    const first = new MockClient("normal", { now: clock });
    await first.stageDecision("harbor-sim", "PR-0041", { decision: "reject", reason: "No" }, (await first.getProposal("harbor-sim", "PR-0041")).updated_at ?? "");
    expect((await new MockClient("normal", { now: clock }).getProposal("harbor-sim", "PR-0041")).staged).toBeNull();
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

  it("conflict: a first stage finds one staged elsewhere meanwhile, 409; the next, on the new updated_at, is taken", async () => {
    const client = new MockClient("conflict", { now: clock });
    const read = (await client.getProposal("harbor-sim", "PR-0041")).updated_at ?? "";
    const error = await rejection(client.stageDecision("harbor-sim", "PR-0041", { decision: "reject", reason: "No" }, read));
    expect(error.status).toBe(409);
    expect(error.message).toContain("`PR-0041` changed since it was read");
    const now = await client.getProposal("harbor-sim", "PR-0041");
    expect([now.status, now.staged?.decision, now.staged_at]).toEqual(["open", "reject", "2026-10-05T12:00:00Z"]);
    expect(ids((await client.getInbox("harbor-sim")).proposals)).toContain("PR-0041");
    const staged = await client.stageDecision("harbor-sim", "PR-0041", { decision: "reject", reason: "No" }, now.updated_at ?? "");
    expect(staged.staged).toEqual({ decision: "reject", reason: "No" });
  });
});
