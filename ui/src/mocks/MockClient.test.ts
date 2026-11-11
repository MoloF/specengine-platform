import { afterEach, describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { CardFlow, InboxEntry } from "../api/types";
import { mockBoard, splitCardsIn, splitScopeOf } from "./cards";
import { DRAFT_FILE } from "./harbor-sim/fixtures";
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
    ["?scenario=editorial", "editorial"],
    ["?scenario=code-cannot", "code-cannot"],
    ["?scenario=not-configured", "not-configured"],
    ["?scenario=no-gaps", "no-gaps"],
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

  it("is no hub: the identity read answers a local daemon's null (AC-12 of hub-identity)", async () => {
    expect(await new MockClient("normal", { now: clock }).getIdentity()).toEqual({ login: null });
  });

  it("is no hub: the team read answers a local daemon's unknown route (A7 of hub-team)", async () => {
    expect((await rejection(new MockClient("normal", { now: clock }).getTeam())).status).toBe(404);
  });

  it("serves harbor-sim's two workspaces, two rooted modules and a direction, ledger-api the implicit one; config-notes their notes (docs/features/workspaces.md \"The UI\"; DP-11, DP-14 of departments)", async () => {
    const client = new MockClient("normal", { now: clock });
    const harbor = await client.getWorkspaces("harbor-sim");
    expect(harbor.source).toBe("config");
    expect(harbor.workspaces.map(({ name, landing, sections, flows, disciplines, module }) => [name, landing, sections, flows, disciplines, module])).toEqual([
      ["galley", "flow", ["board", "flow", "tree", "inbox"], ["copy"], ["copy", "captions"], "berths"],
      ["foundry", "overview", ["board", "health", "graph", "tree"], ["plates"], ["plates"], null],
    ]);
    expect(harbor.modules.map(({ name, title, root, kinds }) => [name, title, root, kinds])).toEqual([
      ["berths", "Berths", "DOM-BERTHS", null],
      ["waters", "Harbour waters", "DOM-WATER", null],
      ["mechanics", "Mechanics", null, ["mechanic"]],
    ]);
    // Each department's disciplines are what the daemon's rule gives over the mock's flows: each flow's own.
    const flows = (await client.getCards("harbor-sim")).flows;
    expect(flows.map((flow) => [flow.name, flow.title, flow.disciplines])).toEqual([
      ["copy", "Copy desk", ["copy", "captions"]],
      ["plates", "plates", ["plates"]],
    ]);
    expect(harbor.notes).toEqual([]);
    const ledger = await client.getWorkspaces("ledger-api");
    expect([ledger.source, ledger.workspaces.map((workspace) => workspace.name), ledger.modules, ledger.notes]).toEqual(["implicit", ["all"], [], []]);
    const noted = new MockClient("config-notes", { now: clock });
    expect((await noted.getWorkspaces("harbor-sim")).notes).toHaveLength(2);
    expect((await noted.getWorkspaces("ledger-api")).notes).toEqual([expect.stringMatching(/; the implicit workspace is served$/)]);
    expect((await noted.getWorkspaces("ledger-api")).source).toBe("implicit");
    // Every key of the document present, in the spec's order.
    expect(Object.keys(harbor)).toEqual(["source", "notes", "workspaces", "modules"]);
    expect(Object.keys(harbor.workspaces[0] ?? {})).toEqual(["name", "title", "landing", "sections", "flows", "disciplines", "module"]);
    expect(harbor.modules.map((module) => Object.keys(module))).toEqual([0, 1, 2].map(() => ["name", "title", "root", "kinds"]));
    expect(Object.keys((await client.getCards("harbor-sim")).flows[0] ?? {})).toEqual(["name", "title", "disciplines", "columns", "moves", "accept", "return"]);
    expect((await rejection(client.getWorkspaces("nowhere"))).status).toBe(404);
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

  it("serves each review document's task_id after choice, null unbound, as the task canon's Task-bound proposals; the interpretation after the stage, the card and why after it (L1 of card-questions)", async () => {
    const client = new MockClient("normal", { now: clock });
    const { proposals } = await client.getInbox("harbor-sim");
    const reviews = await Promise.all(proposals.map((entry) => client.getProposal("harbor-sim", entry.id)));
    for (const review of reviews) {
      expect([review.id, Object.keys(review).slice(-8)]).toEqual([review.id, ["choice", "task_id", "staged", "staged_at", "interpretation", "card_id", "why", "notes"]]);
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
    const staged = await client.stageDecision("harbor-sim", "PR-0041", { decision: "approve", option: 1, answer: null, canon: null, editorial: null, note: "go" }, before.updated_at ?? "");
    expect([staged.status, staged.staged, staged.staged_at, staged.updated_at]).toEqual([
      "open",
      { decision: "approve", option: 1, answer: null, canon: null, editorial: null, note: "go", span_hash: null },
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
    const approved = await client.stageDecision("harbor-sim", "PR-0042", { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null }, update.updated_at ?? "");
    expect(approved.staged).toEqual({ decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null, span_hash: update.base_hash });
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
    const approve = (option: number | null) => ({ decision: "approve" as const, option, answer: null, canon: null, editorial: null, note: null });
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
      client.getIdentity(),
      client.getTeam(),
      client.getWorkspaces("harbor-sim"),
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
    const now = await client.getProposal("harbor-sim", "PR-0041");
    expect([now.status, now.staged?.decision, now.staged_at]).toEqual(["open", "reject", "2026-10-05T12:00:00Z"]);
    // The daemon's words byte for byte (CLI `stage.rs`, the `updated_at` check).
    expect(now.updated_at).toBe("2026-10-05T12:00:00Z");
    expect(error.message).toBe(
      `\`PR-0041\` changed since it was read: updated at 2026-10-05T12:00:00Z, the choice was made on the one of ${read}; read it again; nothing staged`,
    );
    expect(ids((await client.getInbox("harbor-sim")).proposals)).toContain("PR-0041");
    const staged = await client.stageDecision("harbor-sim", "PR-0041", { decision: "reject", reason: "No" }, now.updated_at ?? "");
    expect(staged.staged).toEqual({ decision: "reject", reason: "No" });
  });
});

// docs/features/card-questions.md "Acceptance criteria", U1 and U2, over the mock: harbor-sim's
// C-0008 names an open question and an applied decision, their events in its
// history; the card door stores a question or a decision into the queue; `conflict` meets one corpus
// hit on a project's first raise, stored once named.
describe("a card's items over the mock (card-questions)", () => {
  const SLUG = "harbor-sim";

  async function cardOf(client: MockClient, id: string) {
    const read = await client.getCard(SLUG, id);
    if (!("items" in read)) {
      throw new Error(`no card ${id}`);
    }
    return read;
  }

  const question = { kind: "question", node_ids: ["MEC-FAIRWAY"], text: "Is the fairway lit at night?", working_answer: "Yes, every buoy.", price_of_other: "The night chart is redrawn.", as: "Ivo Brandt" };

  it("serves C-0008's two items by ID, their events merged with its own by seq, two of them out of time order; the inbox lists the open one alone", async () => {
    const client = new MockClient("normal", { now: clock });
    const card = await cardOf(client, "C-0008");
    expect(card.items.map((item) => [item.id, item.kind, item.status, item.why, item.record_id])).toEqual([
      ["PR-0055", "question", "open", null, null],
      ["PR-0056", "decision", "applied", "Pilots read the diagram for the worst case, and full boom is it.", "DEC-0013"],
    ]);
    expect(Object.keys(card.items[0] ?? {})).toHaveLength(16);
    const history = card.history ?? [];
    expect(history.map((event) => event.type)).toEqual(["card.created", "card.moved", "proposal.created", "proposal.created", "proposal.approved", "proposal.applied"]);
    expect(history.map((event) => event.seq)).toEqual([...history.map((event) => event.seq)].sort((a, b) => a - b));
    const [moved, asked] = [history[1], history[2]];
    expect((asked?.at ?? "") < (moved?.at ?? "")).toBe(true);
    expect(asked?.payload).toEqual({ id: "PR-0055", as: "Noor Ainsley" });
    const listed = ids((await client.getInbox(SLUG)).proposals);
    expect([listed.includes("PR-0055"), listed.includes("PR-0056")]).toEqual([true, false]);
    expect((await client.getProposal(SLUG, "PR-0055")).card_id).toBe("C-0008");
    // Another card names none.
    expect((await cardOf(client, "C-0007")).items).toEqual([]);
  });

  it("stores a raised question into the queue: the next ID, the card named, a person its author, its event under the acting name; the card reads it", async () => {
    const client = new MockClient("normal", { now: clock });
    const raised = await client.raiseOnCard(SLUG, "C-0007", question);
    // PR-0059: the next free number past C-0001's subtasks' decisions PR-0057 and PR-0058 (task-mandate).
    expect(raised).toEqual({ id: "PR-0059", created: true, hits: [], related: [], linked: null, diagnostics: [], notes: [] });
    expect(ids((await client.getInbox(SLUG)).proposals)).toContain("PR-0059");
    const review = await client.getProposal(SLUG, "PR-0059");
    expect([review.kind, review.card_id, review.why, review.author, review.target_ids]).toEqual(["question", "C-0007", null, { type: "human", role: null, model: null, run: null }, ["MEC-FAIRWAY"]]);
    const card = await cardOf(client, "C-0007");
    expect(card.items.map((item) => item.id)).toEqual(["PR-0059"]);
    expect(card.history?.at(-1)).toMatchObject({ type: "proposal.created", payload: { id: "PR-0059", as: "Ivo Brandt" } });
    // The same text again is the first's hit: nothing stored, until it is named.
    const again = await client.raiseOnCard(SLUG, "C-0007", question);
    expect([again.id, again.created, again.hits.map((hit) => hit.id)]).toEqual([null, false, ["PR-0059"]]);
    const named = await client.raiseOnCard(SLUG, "C-0007", { ...question, distinct_from: ["PR-0059"] });
    expect([named.id, named.created]).toEqual(["PR-0060", true]);
  });

  it("stores a decision with its why; refuses an unknown card (404), another kind and a question's why (400), a blank text (409), nothing stored", async () => {
    const client = new MockClient("normal", { now: clock });
    const decision = await client.raiseOnCard(SLUG, "C-0002", { ...question, kind: "decision", node_ids: ["DOM-BERTHS"], text: "Caption in the local name?", why: "The owner's notes use it.", as: null });
    expect((await client.getProposal(SLUG, decision.id ?? "")).why).toBe("The owner's notes use it.");
    expect((await cardOf(client, "C-0002")).history?.at(-1)?.payload).toEqual({ id: decision.id, as: "local" });
    const missing = await rejection(client.raiseOnCard(SLUG, "C-0099", question));
    expect([missing.status, missing.message]).toEqual([404, "no card C-0099"]);
    const update = await rejection(client.raiseOnCard(SLUG, "C-0007", { ...question, kind: "update" }));
    expect([update.status, update.message]).toEqual([400, "`kind`: a card raises a `question` or a `decision`"]);
    const why = await rejection(client.raiseOnCard(SLUG, "C-0007", { ...question, why: "Because." }));
    expect([why.status, why.message]).toEqual([400, "`why` goes with a `decision`"]);
    const blank = await rejection(client.raiseOnCard(SLUG, "C-0007", { ...question, working_answer: "  " }));
    expect([blank.status, blank.message]).toEqual([409, "working_answer: blank: give a value"]);
    expect((await cardOf(client, "C-0007")).items).toEqual([]);
  });

  it("conflict: a project's first raise meets one corpus hit and is not stored; naming it stores it", async () => {
    const client = new MockClient("conflict", { now: clock });
    const first = await client.raiseOnCard(SLUG, "C-0007", question);
    expect([first.id, first.created, first.hits.map((hit) => [hit.id, hit.source, hit.status])]).toEqual([null, false, [["DEC-0012", "corpus", "accepted"]]]);
    const again = await client.raiseOnCard(SLUG, "C-0007", question);
    expect(again.created).toBe(false);
    const named = await client.raiseOnCard(SLUG, "C-0007", { ...question, distinct_from: ["DEC-0012"] });
    expect([named.id, named.created]).toEqual(["PR-0059", true]);
    const next = await client.raiseOnCard(SLUG, "C-0007", { ...question, text: "Another question?" });
    expect(next.hits).toEqual([]);
  });

  it("logs a stage on an item, its acting name trailing when sent, so the card's history reads it", async () => {
    const client = new MockClient("normal", { now: clock });
    const read = (await client.getProposal(SLUG, "PR-0055")).updated_at ?? "";
    await client.stageDecision(SLUG, "PR-0055", { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null }, read, "Mira Holt");
    const card = await cardOf(client, "C-0008");
    expect(card.history?.at(-1)).toMatchObject({ type: "proposal.staged", payload: { id: "PR-0055", as: "Mira Holt" } });
    expect(card.items[0]?.staged_at).toBe("2026-10-05T12:00:00Z");
  });
});

// The mock's split (docs/features/department-split.md "Acceptance criteria", U2): the daemon's plan
// over harbor-sim's two templates, a write of the parent and its subtasks, all or none, a stored
// parent left as it was, and the conflict scenario's one refusal.
describe("a split over the mock (department-split)", () => {
  const SLUG = "harbor-sim";
  const plan = { target: ["MEC-MOORING"], title: "Mooring", goal: null, discipline: "copy", parent: null, child: null, reviewer: null, dry_run: true, as: "Ivo Brandt" };

  it("plans every discipline the flows list: copy and the plates suggested by the mooring's kinds, captions on the task's targets; nothing stored", async () => {
    const client = new MockClient("normal", { now: clock });
    const before = (await client.getCards(SLUG)).cards.length;
    const planned = await client.splitCard(SLUG, plan);
    expect(Object.keys(planned)).toEqual(["dry_run", "parent", "children", "reviewer", "notes"]);
    expect(planned.parent).toEqual({ id: null, created: true, title: "Mooring", discipline: "copy", flow: "copy", flow_title: "Copy desk", column: "draft", targets: ["MEC-MOORING"] });
    expect(planned.children.map(({ id, discipline, flow, column, targets, from, suggested }) => [id, discipline, flow, column, targets, from, suggested])).toEqual([
      [null, "copy", "copy", "draft", ["MEC-MOORING"], "kinds", true],
      [null, "captions", "copy", "draft", ["MEC-MOORING"], "parent", false],
      [null, "plates", "plates", "queued", ["MEC-MOORING", "RULE-MOOR-CREW", "RULE-MOOR-NIGHT"], "kinds", true],
    ]);
    expect([planned.reviewer, planned.notes]).toEqual(["Ivo Brandt", []]);
    expect((await client.getCards(SLUG)).cards.length).toBe(before);
  });

  it("writes the parent first, then each subtask in the order asked, its parent the parent, its reviewer the split's, the parent's title and goal; one card.created each", async () => {
    const client = new MockClient("normal", { now: clock });
    const made = await client.splitCard(SLUG, { ...plan, goal: "Fees and crews", child: ["plates", "copy"], reviewer: "Mira Holt", dry_run: false });
    const [parent, first, second] = [made.parent?.id ?? "", made.children[0]?.id ?? "", made.children[1]?.id ?? ""];
    expect([parent, first, second]).toEqual(["C-0020", "C-0021", "C-0022"]);
    const read = await client.getCard(SLUG, first);
    expect(read).toMatchObject({ parent, reviewer: "Mira Holt", title: "Mooring", goal: "Fees and crews", discipline: "plates", flow: "plates", column: "queued", targets: ["MEC-MOORING", "RULE-MOOR-CREW", "RULE-MOOR-NIGHT"], assignee: null });
    expect((await client.getCards(SLUG, { parent })).cards.map((row) => row.id)).toEqual([second, first]);
  });

  it("under a stored task: only subtasks, the task's row and revision as they were; its reviewer the chain's", async () => {
    const client = new MockClient("normal", { now: clock });
    const before = await client.getCard(SLUG, "C-0001");
    const made = await client.splitCard(SLUG, { target: null, title: null, goal: null, discipline: null, parent: "C-0001", child: ["captions"], reviewer: null, dry_run: false, as: "Mira Holt" });
    expect([made.parent?.id, made.parent?.created, made.reviewer]).toEqual(["C-0001", false, "Ivo Brandt"]);
    expect(await client.getCard(SLUG, "C-0001")).toMatchObject({ revision: "revision" in before ? before.revision : "", updated_at: "updated_at" in before ? before.updated_at : "" });
  });

  it("refuses in the daemon's words, nothing stored: no such parent, a repeat, a reviewer who is no person, a usage line", async () => {
    const client = new MockClient("normal", { now: clock });
    const before = (await client.getCards(SLUG)).cards.length;
    const write = { ...plan, child: ["copy"], reviewer: "Mira Holt", dry_run: false };
    const missing = await rejection(client.splitCard(SLUG, { ...write, target: null, title: null, discipline: null, parent: "C-0099" }));
    expect([missing.status, missing.message]).toEqual([409, "parent: no card C-0099"]);
    const twice = await rejection(client.splitCard(SLUG, { ...write, child: ["copy", "copy"] }));
    expect([twice.status, twice.message]).toEqual([409, "child: `copy` is named twice"]);
    const agent = await rejection(client.splitCard(SLUG, { ...write, reviewer: "agent" }));
    expect([agent.status, agent.message]).toEqual([409, "reviewer: `agent` can be no reviewer: a person accepts a result"]);
    // A plan judges no person.
    expect((await client.splitCard(SLUG, { ...plan, reviewer: "agent" })).reviewer).toBe("agent");
    const usage = await rejection(client.splitCard(SLUG, { ...write, child: null }));
    expect([usage.status, usage.message]).toEqual([400, "spec: name each subtask's discipline with --child, or plan with --dry-run"]);
    expect((await client.getCards(SLUG)).cards.length).toBe(before);
  });

  it("its scope names each node as split_scope does: the root as the target was written, an ID-less document by its path", async () => {
    const client = new MockClient("normal", { now: clock });
    const README = "docs/spec/berths/README.md";
    const rows = (await client.getTree(SLUG, { root: README, depth: 2 })).nodes;
    expect(splitScopeOf(README, rows)).toEqual([
      { target: README, kind: "domain" },
      { target: "RULE-BASIN-NAME", kind: "rule" },
      { target: DRAFT_FILE, kind: null },
      { target: "RULE-BERTH-DRAFT", kind: "rule" },
      { target: "MEC-MOORING", kind: "mechanic" },
      { target: "RULE-MOOR-CREW", kind: "rule" },
    ]);
  });

  it("judges only the disciplines a write names: a plan of every one the flows list is never capped; 33 named, a name off the rule or one named twice is refused in the daemon's words", () => {
    const board = mockBoard(SLUG, "normal", NOW);
    const many = Array.from({ length: 33 }, (_, index) => `d${String(index + 1)}`);
    board.flows = [{ ...(board.flows[0] as CardFlow), disciplines: many }, ...board.flows.slice(1)];
    const at = "2026-10-05T12:00:00Z";
    const split = (child: string[] | null) => () => splitCardsIn(board, { ...plan, discipline: "d1", child }, () => [], at, () => false);
    // 33 on the first flow and the plates' one: all planned, none refused.
    expect(split(null)().children.map((child) => child.discipline)).toEqual([...many, "plates"]);
    const before = board.cards.length;
    expect(split(many)).toThrow("child: 33 named; a split makes 1 to 32");
    expect(split(["d1", "D2"])).toThrow("child: `D2` is not a name: lower-case ASCII letters, digits, `_` and `-`");
    expect(split(["x".repeat(65)])).toThrow(`child: \`${"x".repeat(65)}\` is 65 bytes long; a name has 1 to 64`);
    // In order, as the daemon checks each: a repeat before a later name off the rule.
    expect(split(["d1", "d1", "Bad"])).toThrow("child: `d1` is named twice");
    expect(split(["d1", "Bad", "Bad"])).toThrow("child: `Bad` is not a name");
    expect(board.cards.length).toBe(before);
  });

  it("a new card and a subtask take their flow by one rule, in one wording, the subtask's prefixed child:", async () => {
    const client = new MockClient("normal", { now: clock });
    expect((await client.createCard(SLUG, { title: "x", discipline: "captions", reviewer: "Mira Holt", as: null })).flow).toBe("copy");
    const card = await rejection(client.createCard(SLUG, { title: "x", discipline: "sound", reviewer: "Mira Holt", as: null }));
    expect([card.status, card.message]).toEqual([409, "no flow takes the discipline `sound`: none lists it, and every `[[card.flow]]` lists its disciplines"]);
    const child = await rejection(client.splitCard(SLUG, { ...plan, child: ["sound"] }));
    expect([child.status, child.message]).toEqual([409, `child: ${card.message}`]);
    const bare = await rejection(client.createCard(SLUG, { title: "x", reviewer: "Mira Holt", as: null }));
    expect(bare.message).toBe("no flow takes a card without a discipline: none lists it, and every `[[card.flow]]` lists its disciplines");
  });

  it("the conflict scenario: the first write is refused, nothing stored; the next is taken", async () => {
    const client = new MockClient("conflict", { now: clock });
    const write = { ...plan, child: ["copy", "plates"], reviewer: "Mira Holt", dry_run: false };
    const before = (await client.getCards(SLUG)).cards.length;
    const refused = await rejection(client.splitCard(SLUG, write));
    expect([refused.status, refused.message]).toEqual([409, "child: no flow takes the discipline `copy`: none lists it, and every `[[card.flow]]` lists its disciplines"]);
    expect((await client.getCards(SLUG)).cards.length).toBe(before);
    expect((await client.splitCard(SLUG, write)).children).toHaveLength(2);
    expect((await client.getCards(SLUG)).cards.length).toBe(before + 3);
  });
});

// The mock's mandate (docs/canon/card-mandate.md "The rule"; docs/features/task-mandate.md
// "Acceptance criteria", V1, V3): C-0013 binding and C-0019 open, every other card none; stored on
// create, per child on a split, on an edit by the card's reviewer alone, in the store's words
// otherwise; `card.created` always names it, `card.edited` only when it changes; C-0001 reads its
// subtasks' items in one read.
describe("a mandate over the mock (task-mandate)", () => {
  const SLUG = "harbor-sim";

  async function shown(client: MockClient, id: string) {
    const read = await client.getCard(SLUG, id);
    if (!("items" in read)) {
      throw new Error(`no card ${id}`);
    }
    return read;
  }

  it("serves C-0013 binding and C-0019 open, each with one of C-0001's targets, every other card none; rows hold it after the reviewer", async () => {
    const client = new MockClient("normal", { now: clock });
    const rows = (await client.getCards(SLUG, { with_cancelled: true })).cards;
    expect(rows.filter((row) => row.mandate !== null).map((row) => [row.id, row.mandate, row.targets])).toEqual([
      ["C-0019", "open", ["DOM-WATER"]],
      ["C-0013", "binding", ["MEC-TIDE-TABLES"]],
    ]);
    const parent = await shown(client, "C-0001");
    expect((await shown(client, "C-0013")).targets.every((target) => parent.targets.includes(target))).toBe(true);
    expect((await shown(client, "C-0019")).targets.every((target) => parent.targets.includes(target))).toBe(true);
    // C-0013's plates flow opens its return column to it, so Return stands on its work item.
    expect((await shown(client, "C-0013")).allowed.map((move) => move.to)).toContain("blocked");
    const row = rows[0] ?? {};
    expect(Object.keys(row).slice(Object.keys(row).indexOf("reviewer"), Object.keys(row).indexOf("reviewer") + 2)).toEqual(["reviewer", "mandate"]);
  });

  it("C-0001 reads its subtasks' decisions in subtask_items, by ID, each its card first; a card with no subtask, none", async () => {
    const client = new MockClient("normal", { now: clock });
    const parent = await shown(client, "C-0001");
    expect(parent.subtask_items.map((item) => [item.card_id, item.id, item.kind, item.status, item.why])).toEqual([
      ["C-0019", "PR-0057", "decision", "open", "The glossary serves the tables: a word outside them sends the reader elsewhere."],
      ["C-0013", "PR-0058", "decision", "open", null],
    ]);
    expect(Object.keys(parent.subtask_items[0] ?? {})).toEqual(["card_id", ...Object.keys(parent.items[0] ?? (await shown(client, "C-0008")).items[0] ?? {})]);
    expect((await shown(client, "C-0013")).subtask_items).toEqual([]);
    // A question raised on a subtask joins the parent's read too, the page showing decisions alone.
    const asked = await client.raiseOnCard(SLUG, "C-0019", { kind: "question", node_ids: ["DOM-WATER"], text: "About PR-0057: tide words too?", working_answer: "Yes.", price_of_other: "More pages.", as: "Ivo Brandt" });
    expect((await shown(client, "C-0001")).subtask_items.map((item) => item.id)).toEqual(["PR-0057", "PR-0058", asked.id]);
  });

  it("create: a hand-off stores its mandate, card.created naming it; none, null; a mandate with no reviewer is the store's 409, nothing stored", async () => {
    const client = new MockClient("normal", { now: clock });
    const made = await client.createCard(SLUG, { title: "Keeper's voice", discipline: "plates", parent: "C-0001", reviewer: "Ivo Brandt", mandate: "open", as: "Ivo Brandt" });
    const card = await shown(client, made.id ?? "");
    expect([card.mandate, card.history?.at(-1)?.payload]).toEqual(["open", { id: made.id, flow: "plates", column: "queued", position: 3, assignee: null, reviewer: "Ivo Brandt", mandate: "open", as: "Ivo Brandt" }]);
    const plain = await client.createCard(SLUG, { title: "No mandate", discipline: "copy", as: null });
    expect((await shown(client, plain.id ?? "")).history?.at(-1)?.payload).toMatchObject({ mandate: null });
    const before = (await client.getCards(SLUG)).cards.length;
    const refused = await rejection(client.createCard(SLUG, { title: "Unreviewed", discipline: "copy", mandate: "binding", as: null }));
    expect([refused.status, refused.message]).toEqual([409, "mandate: a mandate goes with a reviewer, who alone changes it: name one with --reviewer"]);
    expect((await client.getCards(SLUG)).cards.length).toBe(before);
  });

  it("edit: the reviewer changes it, one card.edited naming it; another name, or a card with no reviewer, the store's 409 and nothing changed, the title unwritten too; an edit without it keeps {id, fields, as}", async () => {
    const client = new MockClient("normal", { now: clock });
    const answer = await client.editCard(SLUG, "C-0013", { mandate: "open", revision: "1", as: "Mira Holt" });
    expect(answer.revision).toBe("2");
    const card = await shown(client, "C-0013");
    expect([card.mandate, card.history?.at(-1)?.type, card.history?.at(-1)?.payload]).toEqual(["open", "card.edited", { id: "C-0013", fields: ["mandate"], as: "Mira Holt", mandate: "open" }]);
    const other = await rejection(client.editCard(SLUG, "C-0013", { title: "Plates", mandate: "binding", revision: "2", as: "Ben Ode" }));
    expect([other.status, other.message]).toEqual([409, "mandate: only C-0013's reviewer `Mira Holt` changes its mandate, not `Ben Ode`; nothing changed"]);
    const unnamed = await rejection(client.editCard(SLUG, "C-0013", { mandate: "binding", revision: "2", as: null }));
    expect(unnamed.message).toBe("mandate: only C-0013's reviewer `Mira Holt` changes its mandate, not `local`; nothing changed");
    const kept = await shown(client, "C-0013");
    expect([kept.title, kept.mandate, kept.revision]).toEqual(["Tide table plate", "open", "2"]);
    const none = await rejection(client.editCard(SLUG, "C-0008", { mandate: "binding", revision: "1", as: "Noor Ainsley" }));
    expect([none.status, none.message]).toEqual([409, "mandate: C-0008 has no reviewer, who alone changes its mandate: name one with `spec card assign C-0008 --reviewer N` first; nothing changed"]);
    await client.editCard(SLUG, "C-0013", { title: "Plates", revision: "2", as: "Ben Ode" });
    expect((await shown(client, "C-0013")).history?.at(-1)?.payload).toEqual({ id: "C-0013", fields: ["title"], as: "Ben Ode" });
  });

  it("split: each child its own mandate by discipline, never by place, one named by none none; the plan's children too; a new parent none", async () => {
    const client = new MockClient("normal", { now: clock });
    const write = { target: ["MEC-MOORING"], title: "Mooring", goal: null, discipline: "copy", parent: null, child: ["plates", "copy", "captions"], mandate: ["copy=open", "plates=binding"], reviewer: "Mira Holt", dry_run: false, as: "Mira Holt" };
    const planned = await client.splitCard(SLUG, { ...write, dry_run: true });
    expect(planned.children.map((child) => [child.discipline, child.mandate])).toEqual([
      ["plates", "binding"],
      ["copy", "open"],
      ["captions", null],
    ]);
    const made = await client.splitCard(SLUG, write);
    expect(made.children.map((child) => [child.discipline, child.mandate])).toEqual([
      ["plates", "binding"],
      ["copy", "open"],
      ["captions", null],
    ]);
    expect((await shown(client, made.parent?.id ?? "")).mandate).toBeNull();
    for (const child of made.children) {
      const card = await shown(client, child.id ?? "");
      expect([child.discipline, card.mandate, card.history?.at(-1)?.payload]).toEqual([child.discipline, child.mandate, expect.objectContaining({ mandate: child.mandate })]);
    }
  });

  it("split: refuses a mandate off D=M, off the name rule, naming no child or named twice, in the store's words, nothing stored, a plan too; a mandate with no child is a usage line", async () => {
    const client = new MockClient("normal", { now: clock });
    const write = { target: ["MEC-MOORING"], title: "Mooring", goal: null, discipline: "copy", parent: null, child: ["copy", "plates"], reviewer: "Mira Holt", dry_run: false, as: "Mira Holt" };
    const before = (await client.getCards(SLUG)).cards.length;
    for (const [mandate, line] of [
      [["copy"], "mandate: `copy` is not `<discipline>=binding` or `<discipline>=open`"],
      [["copy=maybe"], "mandate: `copy=maybe` is not `<discipline>=binding` or `<discipline>=open`"],
      [["Copy=open"], "mandate: `Copy` is not a name: lower-case ASCII letters, digits, `_` and `-`"],
      [["voice=open"], "mandate: `voice` is no --child of this split"],
      [["copy=open", "copy=binding"], "mandate: `copy` is named twice"],
    ] as const) {
      const refused = await rejection(client.splitCard(SLUG, { ...write, mandate: [...mandate] }));
      expect([mandate, refused.status, refused.message]).toEqual([mandate, 409, line]);
      const plan = await rejection(client.splitCard(SLUG, { ...write, mandate: [...mandate], dry_run: true }));
      expect([mandate, plan.message]).toEqual([mandate, line]);
    }
    const usage = await rejection(client.splitCard(SLUG, { ...write, child: null, mandate: ["copy=open"], dry_run: true }));
    expect([usage.status, usage.message]).toEqual([400, "spec: --mandate names a subtask: give its --child too"]);
    expect((await client.getCards(SLUG)).cards.length).toBe(before);
  });

  it("the conflict scenario stays as it was: the first card write meets a change elsewhere, a mandate edit among them", async () => {
    const client = new MockClient("conflict", { now: clock });
    const first = await rejection(client.editCard(SLUG, "C-0013", { mandate: "open", revision: "1", as: "Mira Holt" }));
    expect(first.message).toMatch(/^`C-0013` changed since this run read it/);
    expect((await client.editCard(SLUG, "C-0013", { mandate: "open", revision: "2", as: "Mira Holt" })).revision).toBe("3");
  });
});
