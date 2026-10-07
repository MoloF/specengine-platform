import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { errorAnswer, FakeEventSource, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { aCheckFinding, aCheckReport, aGraphEdge, aGraphNode, aGraphView, aProposal, aTaskEntry, aTaskPackage } from "../test/builders";
import { apiErrorOf, ClientError, isNotServed } from "./client";
import { errorBodyOf, HttpClient, QUEUE_EVENT_TYPES, refusedReasonOf, REOPEN_FIRST_MS, REOPEN_MAX_MS } from "./http";
import httpSource from "./http.ts?raw";
import type { QueueEvent } from "./types";

// AC-09 of docs/features/daemon-read.md: the browser side of the daemon, over a stubbed `fetch`
// and `EventSource`. Each method hits its URL, a REF encoded once; a 404 document resolves as data;
// any other non-2xx rejects with the daemon's message verbatim; no response is status 0. AC-10 and
// AC-11 of docs/features/ui-live.md: the graph and the check are fetched, every check verdict a
// 200 report resolved as data. AC-08 and AC-09 of docs/features/ui-live-tasks.md: the tasks are
// fetched too (the list with no query, a task's ID one path segment), a 404 `{id, reason}` resolved
// as data; the live tail listens to the nine `task.*` types after the five `proposal.*`. AC-13 and
// AC-14 of docs/features/decision-staging.md: the stage is a POST of exactly the stage plus
// `updated_at`, the unstage a DELETE, neither with an AbortSignal; a refusal carrying the review
// document rejects with its last note; the tail listens to the seven `proposal.*` types, the
// stage's two after `apply_failed`, sixteen in all.

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

/** The exit-1 document of `spec show REF --json`. */
function nodeNotFound(ref: string) {
  return { ref, reason: `\`${ref}\` resolves to no ID and no alias`, notes: [], nodes: [] };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("each method hits its URL (AC-09)", () => {
  let fetchStub: ReturnType<typeof stubFetch>;

  beforeEach(() => {
    fetchStub = stubFetch(() => jsonAnswer(200, {}));
  });

  it.each<[string, (client: HttpClient) => Promise<unknown>, string]>([
    ["getProjects", (client) => client.getProjects(), "/api/projects"],
    ["getInbox", (client) => client.getInbox("alpha"), "/api/projects/alpha/inbox"],
    ["getProposal", (client) => client.getProposal("alpha", "PR-0004"), "/api/projects/alpha/proposals/PR-0004"],
    ["getTree, no option", (client) => client.getTree("alpha"), "/api/projects/alpha/tree"],
    [
      "getTree, every option, an array repeating its key",
      (client) => client.getTree("alpha", { root: "MEC-TIDES", depth: 2, kinds: ["rule", "mechanic"], archive: true }),
      "/api/projects/alpha/tree?root=MEC-TIDES&depth=2&kinds=rule&kinds=mechanic&archive=true",
    ],
    [
      "getNode, `#` as %23 (ui-tree-node's example)",
      (client) => client.getNode("harbor-sim", "MEC-TIDES#RULE-TIDE-WINDOW", { with: ["links"], archive: true }),
      "/api/projects/harbor-sim/nodes/MEC-TIDES%23RULE-TIDE-WINDOW?with=links&archive=true",
    ],
    [
      "getNode, a path REF's `/` as %2F",
      (client) => client.getNode("alpha", "docs/spec/movement/stamina.md"),
      "/api/projects/alpha/nodes/docs%2Fspec%2Fmovement%2Fstamina.md",
    ],
    ["getNode, slug/ID", (client) => client.getNode("alpha", "stamina/MEC-STAMINA"), "/api/projects/alpha/nodes/stamina%2FMEC-STAMINA"],
    [
      "getNode, a REF holding %23 encoded once, never decoded first",
      (client) => client.getNode("alpha", "MEC-STAMINA%23X"),
      "/api/projects/alpha/nodes/MEC-STAMINA%2523X",
    ],
    [
      "getNode, non-Latin text as UTF-8",
      (client) => client.getNode("alpha", `docs/${String.fromCodePoint(0x417, 0x435)}.md`),
      "/api/projects/alpha/nodes/docs%2F%D0%97%D0%B5.md",
    ],
    [
      "search, the query as typed",
      (client) => client.search("alpha", { query: "tide window & co", kinds: ["rule"], limit: 5, archive: true }),
      "/api/projects/alpha/search?query=tide%20window%20%26%20co&kinds=rule&limit=5&archive=true",
    ],
    ["search, the query alone", (client) => client.search("alpha", { query: "tide" }), "/api/projects/alpha/search?query=tide"],
    [
      "getBundle, node_ids repeated",
      (client) => client.getBundle("alpha", { node_ids: ["MEC-TIDES", "RULE-X#A"], budget: 2000 }),
      "/api/projects/alpha/bundle?node_ids=MEC-TIDES&node_ids=RULE-X%23A&budget=2000",
    ],
    ["a project slug, encoded too", (client) => client.getInbox("a b"), "/api/projects/a%20b/inbox"],
    // AC-10 of docs/features/ui-live.md: ui-graph's example, exactly.
    [
      "getGraph, ui-graph's example: `#` as %23, types repeated in order",
      (client) => client.getGraph("harbor-sim", { ref: "MEC-TIDES#RULE-TIDE-WINDOW", impact: true, types: ["depends_on", "constrains"], depth: 3 }),
      "/api/projects/harbor-sim/graph?ref=MEC-TIDES%23RULE-TIDE-WINDOW&impact=true&types=depends_on&types=constrains&depth=3",
    ],
    ["getGraph, the REF alone", (client) => client.getGraph("alpha", { ref: "MEC-SPRINT" }), "/api/projects/alpha/graph?ref=MEC-SPRINT"],
    [
      "getGraph, impact and archive sent only as true, no empty types",
      (client) => client.getGraph("alpha", { ref: "MEC-SPRINT", impact: false, types: [], archive: false }),
      "/api/projects/alpha/graph?ref=MEC-SPRINT",
    ],
    [
      "getGraph, depth 0 and the archive, in the five names' order",
      (client) => client.getGraph("alpha", { archive: true, depth: 0, types: ["mentions"], ref: "MEC-SPRINT" }),
      "/api/projects/alpha/graph?ref=MEC-SPRINT&types=mentions&depth=0&archive=true",
    ],
    [
      "getGraph, a path REF's `/` as %2F, any link type sent as given",
      (client) => client.getGraph("alpha", { ref: "docs/spec/movement/sprint.md", types: ["no such type"] }),
      "/api/projects/alpha/graph?ref=docs%2Fspec%2Fmovement%2Fsprint.md&types=no%20such%20type",
    ],
    [
      "getGraph, non-Latin text as UTF-8, a REF holding %23 encoded once",
      (client) => client.getGraph("alpha", { ref: `${String.fromCodePoint(0x417, 0x435)}%23X` }),
      "/api/projects/alpha/graph?ref=%D0%97%D0%B5%2523X",
    ],
    ["getCheck: no query", (client) => client.getCheck("harbor-sim"), "/api/projects/harbor-sim/check"],
    // AC-08 of docs/features/ui-live-tasks.md: exactly these two URLs; no `status` is sent.
    ["getTasks: no query", (client) => client.getTasks("harbor-sim"), "/api/projects/harbor-sim/tasks"],
    ["getTask: the ID one path segment", (client) => client.getTask("harbor-sim", "T-0107"), "/api/projects/harbor-sim/tasks/T-0107"],
    [
      "getTask, a look-alike ID sent as typed (the daemon names its Latin form), `/` as %2F",
      (client) => client.getTask("alpha", `${String.fromCodePoint(0x422)}-0001/x`),
      "/api/projects/alpha/tasks/%D0%A2-0001%2Fx",
    ],
  ])("%s", async (_name, call, url) => {
    await call(new HttpClient());
    expect(urlsOf(fetchStub)).toEqual([url]);
    expect(fetchStub.mock.calls[0]?.[1]?.method).toBe("GET");
  });

  it("stages an approve: one POST to the decision path, its body exactly the stage plus updated_at (AC-14 of decision-staging)", async () => {
    await new HttpClient().stageDecision(
      "alpha",
      "PR-0004",
      { decision: "approve", option: 2, answer: null, canon: null, note: "keep the cap" },
      "2026-10-06T09:14:02Z",
    );
    expect(urlsOf(fetchStub)).toEqual(["/api/projects/alpha/proposals/PR-0004/decision"]);
    const init = fetchStub.mock.calls[0]?.[1];
    expect(init?.method).toBe("POST");
    expect(init?.body).toBe('{"decision":"approve","option":2,"answer":null,"canon":null,"note":"keep the cap","updated_at":"2026-10-06T09:14:02Z"}');
    expect(init?.headers).toEqual({ Accept: "application/json", "Content-Type": "application/json" });
  });

  it("stages a reject: its reason, then updated_at, nothing else; the ID one path segment", async () => {
    await new HttpClient().stageDecision("a b", "PR-0004/x", { decision: "reject", reason: "duplicate of PR-0003" }, "2026-10-06T09:14:02Z");
    expect(urlsOf(fetchStub)).toEqual(["/api/projects/a%20b/proposals/PR-0004%2Fx/decision"]);
    expect(fetchStub.mock.calls[0]?.[1]?.body).toBe('{"decision":"reject","reason":"duplicate of PR-0003","updated_at":"2026-10-06T09:14:02Z"}');
  });

  it("unstages with a DELETE on the same path, no body (AC-14 of decision-staging)", async () => {
    await new HttpClient().unstageDecision("alpha", "PR-0004");
    expect(urlsOf(fetchStub)).toEqual(["/api/projects/alpha/proposals/PR-0004/decision"]);
    const init = fetchStub.mock.calls[0]?.[1];
    expect(init?.method).toBe("DELETE");
    expect(init?.body).toBeUndefined();
    expect(init?.headers).toEqual({ Accept: "application/json" });
  });

  it("is the daemon: dataSource `daemon`", () => {
    expect(new HttpClient().dataSource).toBe("daemon");
  });

  it("requests the tasks the daemon serves: no read is refused unsent any more (AC-08 of ui-live-tasks)", async () => {
    const client = new HttpClient();
    await client.getTasks("alpha");
    await client.getTask("alpha", "T-0001");
    expect(urlsOf(fetchStub)).toEqual(["/api/projects/alpha/tasks", "/api/projects/alpha/tasks/T-0001"]);
    expect(fetchStub.mock.calls.map((call) => call[1]?.method)).toEqual(["GET", "GET"]);
    // WA-5: the not-served mark stays in client.ts for the next missing endpoint; HttpClient has no helper for it.
    expect(httpSource).not.toContain("notServed(");
    expect(httpSource).not.toContain("DAEMON_READ_GAP");
  });

  it.each<[string, (client: HttpClient, signal: AbortSignal) => Promise<unknown>]>([
    ["getProjects", (client, signal) => client.getProjects(signal)],
    ["getInbox", (client, signal) => client.getInbox("alpha", signal)],
    ["getProposal", (client, signal) => client.getProposal("alpha", "PR-0004", signal)],
    ["getTree", (client, signal) => client.getTree("alpha", undefined, signal)],
    ["getNode", (client, signal) => client.getNode("alpha", "R-1", undefined, signal)],
    ["search", (client, signal) => client.search("alpha", { query: "tide" }, signal)],
    ["getBundle", (client, signal) => client.getBundle("alpha", { node_ids: ["R-1"] }, signal)],
    ["getGraph", (client, signal) => client.getGraph("alpha", { ref: "R-1" }, signal)],
    ["getTasks", (client, signal) => client.getTasks("alpha", signal)],
    ["getTask", (client, signal) => client.getTask("alpha", "T-0001", signal)],
  ])("%s hands the caller's AbortSignal to fetch (ui-live-tasks: a superseded read is aborted)", async (_name, call) => {
    const controller = new AbortController();
    await call(new HttpClient(), controller.signal);
    expect(fetchStub.mock.calls[0]?.[1]?.signal).toBe(controller.signal);
  });

  it("sends the check, a stage and an unstage with no AbortSignal: a walk in flight is never aborted, a write's answer always taken", async () => {
    const client = new HttpClient();
    await client.getCheck("alpha");
    await client.stageDecision("alpha", "PR-0004", { decision: "reject", reason: "No" }, "2026-10-06T09:14:02Z");
    await client.unstageDecision("alpha", "PR-0004");
    await client.getTasks("alpha");
    expect(fetchStub.mock.calls.map((call) => call[1] !== undefined && "signal" in call[1])).toEqual([false, false, false, false]);
  });

  it("rejects an aborted read with the abort error as fetch gave it, never a ClientError of the daemon", async () => {
    const controller = new AbortController();
    const aborted = new DOMException("The operation was aborted.", "AbortError");
    stubFetch((_url, init) => {
      if (init?.signal?.aborted === true) {
        throw aborted;
      }
      return jsonAnswer(200, {});
    });
    controller.abort();
    const error: unknown = await new HttpClient().getTask("alpha", "T-0001", controller.signal).then(
      () => null,
      (reason: unknown) => reason,
    );
    expect(error).toBe(aborted);
    expect(error instanceof ClientError).toBe(false);
  });

  it("tells a read not built from the daemon down or refusing (R-n7)", async () => {
    stubFetch(() => {
      throw new TypeError("Failed to fetch");
    });
    const down = await rejection(new HttpClient().getInbox("alpha"));
    expect([down.status, down.notServed, isNotServed(down)]).toEqual([0, false, false]);
    stubFetch(() => errorAnswer(501, "the daemon's own 501"));
    const refused = await rejection(new HttpClient().getInbox("alpha"));
    expect([refused.status, refused.notServed, isNotServed(refused)]).toEqual([501, false, false]);
    stubFetch(() => new Response("", { status: 502, statusText: "Bad Gateway" }));
    expect(isNotServed(await rejection(new HttpClient().getProjects()))).toBe(false);
    expect(isNotServed(new Error("Not served by the daemon yet: a plain Error"))).toBe(false);
  });
});

describe("answers (AC-09)", () => {
  it("resolves a 200 to its document as sent", async () => {
    const projects = [{ slug: "alpha", name: null, root: "/work/alpha", branch: null }];
    stubFetch(() => jsonAnswer(200, projects));
    expect(await new HttpClient().getProjects()).toEqual(projects);
  });

  it("resolves a 404 exit-1 document as data: a node, a tree, a bundle, a proposal", async () => {
    const node = nodeNotFound("R-404");
    const tree = { ref: "R-404", reason: "`R-404` resolves to no ID", notes: [], depth: null, kinds: [], archive: false, left_out: { generated: 0, tier3: 0 }, truncated: false, nodes: [] };
    const bundle = { refs: ["R-404"], reason: "`R-404` resolves to no ID", notes: [], task: null, budget: null, tokens: null, chars: null, bytes: null, bundle_hash: null, body: null, layers: null, tail: null, more: null };
    const review = { id: null, kind: null, target_ids: [], notes: ["no proposal `PR-9999` in this project's queue"] };
    stubFetch((url) => {
      if (url.includes("/nodes/")) {
        return jsonAnswer(404, node);
      }
      if (url.includes("/tree")) {
        return jsonAnswer(404, tree);
      }
      return jsonAnswer(404, url.includes("/bundle") ? bundle : review);
    });
    const client = new HttpClient();
    expect(await client.getNode("alpha", "R-404")).toEqual(node);
    expect(await client.getTree("alpha", { root: "R-404" })).toEqual(tree);
    expect(await client.getBundle("alpha", { node_ids: ["R-404"] })).toEqual(bundle);
    expect(await client.getProposal("alpha", "PR-9999")).toEqual(review);
  });

  it("resolves the task list and a package as sent: `bundle_hash` null, a diff null with `cut` false kept (AC-08 of ui-live-tasks)", async () => {
    const list = { tasks: [aTaskEntry({ id: "T-0108", status: "ready", stale: true })], notes: ["T-0106: a corrupt row skipped"] };
    const pkg = aTaskPackage({
      id: "T-0107",
      bundle: { node_ids: ["MEC-TIDES"], budget: 10000, bundle_hash: null },
      snapshot_diff: [{ id: "MEC-TIDES", path: "docs/spec/tides.md", span_hash: "b3:0", diff: null, cut: false }],
      notes: ["bundle: `MEC-TIDES` resolves to no ID", "snapshot_diff: no diff of `MEC-TIDES`: git failed"],
    });
    stubFetch((url) => jsonAnswer(200, url.endsWith("/tasks") ? list : pkg));
    const client = new HttpClient();
    expect(await client.getTasks("harbor-sim")).toEqual(list);
    expect(await client.getTask("harbor-sim", "T-0107")).toEqual(pkg);
  });

  it.each([
    ["no such task", "T-0099", { id: "T-0099", reason: "no task T-0099 in this repository" }],
    ["not a task ID: `id` null", "foo", { id: null, reason: "no task `foo`: a task ID is `T-` and 4 or more digits, as `spec task list` lists it" }],
  ])("resolves a task's 404 exit-1 document as data, never a refusal: %s (AC-08 of ui-live-tasks)", async (_name, id, document) => {
    const fetchStub = stubFetch(() => jsonAnswer(404, document));
    expect(await new HttpClient().getTask("harbor-sim", id)).toEqual(document);
    expect(urlsOf(fetchStub)).toEqual([`/api/projects/harbor-sim/tasks/${id}`]);
  });

  it.each<[string, (client: HttpClient) => Promise<unknown>, number, string]>([
    ["a task's 404 error body (an unknown project)", (client) => client.getTask("zeta", "T-0001"), 404, "no project `zeta`: this daemon serves alpha, beta"],
    [
      "a task's 503 (a look-alike ID, its Latin form named)",
      (client) => client.getTask("alpha", `${String.fromCodePoint(0x422)}-0001`),
      503,
      `spec: \`${String.fromCodePoint(0x422)}-0001\` is not a Latin task ID: write \`T-0001\``,
    ],
    ["the list's 503 (the root in no git worktree)", (client) => client.getTasks("alpha"), 503, "spec: /work/alpha is in no git worktree\nsecond line, verbatim"],
    ["the list's 404 error body (an unknown route)", (client) => client.getTasks("alpha"), 404, "no route GET /api/projects/alpha/tasks/: try projects, tasks, tasks/:id"],
  ])("rejects %s with a ClientError, the daemon's message verbatim (AC-08 of ui-live-tasks)", async (_name, call, status, message) => {
    stubFetch(() => errorAnswer(status, message));
    const error = await rejection(call(new HttpClient()));
    expect(error).toBeInstanceOf(ClientError);
    expect([error.status, error.message, error.notServed, isNotServed(error)]).toEqual([status, message, false, false]);
  });

  it("resolves the graph's 404 exit-1 document as data, its reason set (AC-03 of ui-live)", async () => {
    const unknown = aGraphView([], [], { ref: "R-404", reason: "`R-404` resolves to no ID and no alias", types: [], depth: null });
    stubFetch(() => jsonAnswer(404, unknown));
    expect(await new HttpClient().getGraph("alpha", { ref: "R-404" })).toEqual(unknown);
  });

  it("resolves a graph as sent, uncut: every node and edge, `truncated` as the daemon says", async () => {
    const nodes = Array.from({ length: 300 }, (_, index) => aGraphNode({ id: `R-${String(index)}`, distance: index === 0 ? 0 : 1 }));
    const edges = nodes.slice(1).map((node) => aGraphEdge({ src: "R-0", type: "depends_on", dst: node.id }));
    const whole = aGraphView(nodes, edges);
    stubFetch(() => jsonAnswer(200, whole));
    const answer = await new HttpClient().getGraph("alpha", { ref: "R-0" });
    expect([answer.nodes.length, answer.edges.length, answer.truncated]).toEqual([300, 299, false]);
    expect(answer).toEqual(whole);
  });

  it.each([
    ["clean", aCheckReport()],
    ["observed", aCheckReport({ mode: "observe", verdict: "observed", findings: [aCheckFinding({ code: "key-missing" })], counts: { errors: 1 } })],
    [
      "blocked",
      aCheckReport({
        verdict: "blocked",
        counts: { documents: 9, errors: 1, worst_w_bytes: 24576 },
        findings: [aCheckFinding({ code: "key-missing", path: "docs/spec/movement/sprint.md", subject: "status", message: "..." })],
      }),
    ],
    [
      "cannot-check",
      aCheckReport({ verdict: "cannot-check", counts: { documents: 0, worst_w_bytes: 0 }, cannot_check: [{ path: ".spec-debt.toml", message: "line 3: `expires` is not a YYYY-MM-DD date" }] }),
    ],
  ])("resolves the check's 200 `%s` report as data, never a refusal (AC-10 of ui-live)", async (_verdict, report) => {
    const fetchStub = stubFetch(() => jsonAnswer(200, report));
    expect(await new HttpClient().getCheck("harbor-sim")).toEqual(report);
    expect(urlsOf(fetchStub)).toEqual(["/api/projects/harbor-sim/check"]);
  });

  it.each<[string, (client: HttpClient) => Promise<unknown>, number, string]>([
    ["the check's 503", (client) => client.getCheck("alpha"), 503, "spec: `[check] mode` must be one of observe, enforce-introduced, enforce\nsecond line, verbatim"],
    ["the graph's 503 (depth below 0)", (client) => client.getGraph("alpha", { ref: "R", depth: -1 }), 503, "spec: --depth: `-1` is not a number of 0 or more"],
    ["the graph's 400", (client) => client.getGraph("alpha", { ref: "R" }), 400, "unknown query `x` for /graph: it takes ref, impact, types, depth, archive"],
  ])("rejects %s with a ClientError, the daemon's message verbatim (AC-10 of ui-live)", async (_name, call, status, message) => {
    stubFetch(() => errorAnswer(status, message));
    const error = await rejection(call(new HttpClient()));
    expect(error).toBeInstanceOf(ClientError);
    expect([error.status, error.message, error.notServed]).toEqual([status, message, false]);
  });

  it("rejects a 404 carrying the error body (an unknown project or route) with its message", async () => {
    stubFetch(() => errorAnswer(404, "no project `zeta`: this daemon serves alpha, beta"));
    const error = await rejection(new HttpClient().getInbox("zeta"));
    expect([error.status, error.message]).toEqual([404, "no project `zeta`: this daemon serves alpha, beta"]);
  });

  it.each([
    [503, "spec: the index cannot be read: database is locked\nsecond line, verbatim"],
    [400, "unknown query `kind` for /search: query, kinds, limit, archive"],
    [403, "refused: Origin http://evil.example is not this daemon's"],
    [405, "method PUT not allowed"],
  ])("rejects a %i with the error body's message verbatim", async (status, message) => {
    stubFetch(() => errorAnswer(status, message));
    const error = await rejection(new HttpClient().getInbox("alpha"));
    expect(error).toBeInstanceOf(ClientError);
    expect(apiErrorOf(error)).toEqual({ status, message });
  });

  it("resolves a stage's and an unstage's 200 to the review document as sent", async () => {
    const staged = aProposal({
      id: "PR-0004",
      staged: { decision: "approve", option: 1, answer: null, canon: null, note: "n", span_hash: null },
      staged_at: "2026-10-06T09:14:02Z",
      updated_at: "2026-10-06T09:14:02Z",
    });
    stubFetch((_url, init) => jsonAnswer(200, init?.method === "DELETE" ? { ...staged, staged: null, staged_at: null } : staged));
    const client = new HttpClient();
    expect(await client.stageDecision("alpha", "PR-0004", { decision: "approve", option: 1, answer: null, canon: null, note: "n" }, "2026-10-06T09:00:00Z")).toEqual(staged);
    expect(await client.unstageDecision("alpha", "PR-0004")).toEqual({ ...staged, staged: null, staged_at: null });
  });

  it.each<[string, "POST" | "DELETE", number]>([
    ["a stage's 409 (changed since read)", "POST", 409],
    ["an unstage's 409 (not open)", "DELETE", 409],
    ["a stage's 404 (the exit-1 document)", "POST", 404],
  ])("rejects %s carrying the review document with its last note, never the raw JSON (AC-14 of decision-staging)", async (_name, method, status) => {
    const reason = "`PR-0004` is approved, not open: nothing changed";
    stubFetch(() => jsonAnswer(status, aProposal({ id: "PR-0004", status: "approved", notes: ["rebases: RULE-X changed", reason] })));
    const client = new HttpClient();
    const call = method === "POST" ? client.stageDecision("alpha", "PR-0004", { decision: "reject", reason: "No" }, "2026-10-06T09:14:02Z") : client.unstageDecision("alpha", "PR-0004");
    const error = await rejection(call);
    expect(error).toBeInstanceOf(ClientError);
    expect([error.status, error.message, error.notServed]).toEqual([status, reason, false]);
  });

  it.each([
    [400, "spec: `PR-0004` is a question: --option is for a discrepancy (exit 2)"],
    [403, "staging needs a same-origin page (not authentication: ADR-0034)"],
    [413, "the body is over 16384 bytes"],
    [415, "the body must be application/json"],
    [503, "spec: the queue cannot be read: database is locked\nsecond line, verbatim"],
  ])("rejects a stage's %i error body with its message verbatim", async (status, message) => {
    stubFetch(() => errorAnswer(status, message));
    const error = await rejection(new HttpClient().stageDecision("alpha", "PR-0004", { decision: "reject", reason: "No" }, "2026-10-06T09:14:02Z"));
    expect(apiErrorOf(error)).toEqual({ status, message });
  });

  it("reads a refused write's reason only from a review document's last note", () => {
    expect(refusedReasonOf({ id: "PR-1", notes: ["a", "the reason"] })).toBe("the reason");
    expect(refusedReasonOf({ notes: [] })).toBeNull();
    expect(refusedReasonOf({ notes: ["a", 7] })).toBeNull();
    expect(refusedReasonOf({ notes: "the reason" })).toBeNull();
    expect(refusedReasonOf({ status: 409, message: "x" })).toBeNull();
    expect(refusedReasonOf(["the reason"])).toBeNull();
    expect(refusedReasonOf(null)).toBeNull();
  });

  it("never takes a GET's error answer for a refused write: a read's 409 with notes is its text", async () => {
    const body = { notes: ["not a write"] };
    stubFetch(() => jsonAnswer(409, body));
    const error = await rejection(new HttpClient().getInbox("alpha"));
    expect([error.status, error.message]).toEqual([409, JSON.stringify(body)]);
  });

  it("gives another error body's text verbatim, and says when there is none", async () => {
    stubFetch(() => new Response("upstream timed out", { status: 504, statusText: "Gateway Timeout" }));
    expect(apiErrorOf(await rejection(new HttpClient().getProjects()))).toEqual({ status: 504, message: "upstream timed out" });
    stubFetch(() => new Response("", { status: 502, statusText: "Bad Gateway" }));
    const empty = await rejection(new HttpClient().getProjects());
    expect(empty.status).toBe(502);
    expect(empty.message).toBe("GET /api/projects answered 502 Bad Gateway with an empty body: the dev server's proxy reached no daemon; is specengine-http running?");
  });

  it("never takes a POST's or a DELETE's 404 for a document", async () => {
    stubFetch(() => jsonAnswer(404, { proposal: null }));
    const client = new HttpClient();
    const staged = await rejection(client.stageDecision("alpha", "PR-1", { decision: "reject", reason: "No" }, "2026-10-06T09:14:02Z"));
    const unstaged = await rejection(client.unstageDecision("alpha", "PR-1"));
    expect([staged.status, staged.message, unstaged.status, unstaged.message]).toEqual([404, '{"proposal":null}', 404, '{"proposal":null}']);
  });

  it("rejects with status 0 when no response came", async () => {
    stubFetch(() => {
      throw new TypeError("Failed to fetch");
    });
    const error = await rejection(new HttpClient().getProjects());
    expect(apiErrorOf(error)).toEqual({ status: 0, message: "GET /api/projects: no response from the daemon (Failed to fetch)" });
  });

  it("knows the error body by its two keys only", () => {
    expect(errorBodyOf({ status: 503, message: "x" })).toEqual({ status: 503, message: "x" });
    expect(errorBodyOf({ status: 404, message: "x", reason: "y" })).toBeNull();
    expect(errorBodyOf({ ref: "R", reason: "x" })).toBeNull();
    expect(errorBodyOf({ status: "404", message: "x" })).toBeNull();
    expect(errorBodyOf([404, "x"])).toBeNull();
  });
});

describe("the live tail (AC-09)", () => {
  beforeEach(() => {
    stubEventSource();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function only(): FakeEventSource {
    const sources = FakeEventSource.instances;
    expect(sources).toHaveLength(1);
    const [source] = sources;
    if (source === undefined) {
      throw new Error("no stream");
    }
    return source;
  }

  it("opens the project's stream, listens to the queue's sixteen event types and hands each over parsed", () => {
    const events: QueueEvent[] = [];
    const stop = new HttpClient().subscribe("alpha", (event) => events.push(event));
    const source = only();
    expect(source.url).toBe("/api/projects/alpha/events");
    // docs/canon/proposal-queue.md "States and events": the seven types, the stage's two after
    // `apply_failed` (AC-13 of decision-staging); then docs/canon/tasks.md "Store": the nine (AC-09
    // of ui-live-tasks). Each a listener of its own, in this order.
    expect(source.types()).toEqual([
      "proposal.created",
      "proposal.approved",
      "proposal.applied",
      "proposal.rejected",
      "proposal.apply_failed",
      "proposal.staged",
      "proposal.unstaged",
      "task.created",
      "task.planned",
      "task.approved",
      "task.changes_requested",
      "task.claimed",
      "task.run_reported",
      "task.completed",
      "task.cancelled",
      "task.refreshed",
    ]);
    expect([...QUEUE_EVENT_TYPES]).toEqual(source.types());
    expect(QUEUE_EVENT_TYPES).toHaveLength(16);
    source.open();
    source.emit("proposal.created", '{"id":"PR-0005"}', "12");
    source.emit("proposal.rejected", '{"id":"PR-0002","reason":"no"}', "13");
    expect(events).toEqual([
      { seq: 12, type: "proposal.created", payload: { id: "PR-0005" } },
      { seq: 13, type: "proposal.rejected", payload: { id: "PR-0002", reason: "no" } },
    ]);
    stop();
    expect(source.readyState).toBe(2);
    source.emit("proposal.created", '{"id":"PR-0006"}', "14");
    expect(events).toHaveLength(2);
  });

  it("hands each of the nine task events over as {seq, type, payload}, the stored payload parsed (AC-09 of ui-live-tasks)", () => {
    const events: QueueEvent[] = [];
    new HttpClient().subscribe("alpha", (event) => events.push(event));
    const source = only();
    source.open();
    const sent: [string, string][] = [
      ["task.created", '{"id":"T-0001"}'],
      ["task.planned", '{"id":"T-0001"}'],
      ["task.approved", '{"id":"T-0001"}'],
      ["task.changes_requested", '{"id":"T-0001"}'],
      ["task.claimed", '{"id":"T-0001","run":1}'],
      ["task.run_reported", '{"id":"T-0001","run":1}'],
      ["task.completed", '{"id":"T-0001"}'],
      ["task.cancelled", '{"id":"T-0002"}'],
      ["task.refreshed", '{"id":"T-0003","proposal":"PR-0007","node":"MEC-TIDES"}'],
    ];
    sent.forEach(([type, data], index) => {
      source.emit(type, data, String(70 + index));
    });
    expect(events).toEqual(sent.map(([type, data], index) => ({ seq: 70 + index, type, payload: JSON.parse(data) as unknown })));
  });

  it("hands the stage's two events over as {seq, type, payload}, the stored payload parsed (AC-13 of decision-staging)", () => {
    const events: QueueEvent[] = [];
    new HttpClient().subscribe("alpha", (event) => events.push(event));
    const source = only();
    source.open();
    const staged = '{"id":"PR-0004","staged":{"decision":"approve","option":1,"answer":null,"canon":null,"note":"keep the cap","span_hash":null},"staged_at":"2026-10-06T09:14:02Z"}';
    source.emit("proposal.staged", staged, "90");
    source.emit("proposal.unstaged", '{"id":"PR-0004"}', "91");
    expect(events).toEqual([
      { seq: 90, type: "proposal.staged", payload: JSON.parse(staged) as unknown },
      { seq: 91, type: "proposal.unstaged", payload: { id: "PR-0004" } },
    ]);
  });

  it("leaves a dropped stream to the browser: it resumes with Last-Event-ID, so no gap is said (R-n6)", () => {
    vi.useFakeTimers();
    const gaps = vi.fn();
    new HttpClient().subscribe("alpha", () => undefined, gaps);
    const source = only();
    source.open();
    expect(gaps).not.toHaveBeenCalled();
    source.fail(false);
    vi.advanceTimersByTime(REOPEN_MAX_MS * 2);
    expect(FakeEventSource.instances).toHaveLength(1);
    expect(source.readyState).toBe(0);
    source.open();
    // Twice more, the browser reconnecting each time: still the same stream, still no gap.
    source.fail(false);
    source.fail(false);
    source.open();
    expect(FakeEventSource.instances).toHaveLength(1);
    expect(gaps).not.toHaveBeenCalled();
  });

  it("says a gap only when it opened the stream again itself, once per such opening (R-n6)", () => {
    vi.useFakeTimers();
    const gaps = vi.fn();
    new HttpClient().subscribe("alpha", () => undefined, gaps);
    const first = only();
    first.open();
    // The browser retries on its own, then gives up: this client opens a new stream.
    first.fail(false);
    first.fail(true);
    vi.advanceTimersByTime(REOPEN_FIRST_MS);
    const second = FakeEventSource.instances.at(-1);
    expect(FakeEventSource.instances).toHaveLength(2);
    second?.open();
    expect(gaps).toHaveBeenCalledTimes(1);
    // The new stream drops and the browser resumes it: no second gap.
    second?.fail(false);
    second?.open();
    expect(gaps).toHaveBeenCalledTimes(1);
  });

  it("opens a given-up stream again after 1 s, 2 s, 4 s … at most 30 s, a gap said on each opening", () => {
    vi.useFakeTimers();
    const gaps = vi.fn();
    const stop = new HttpClient().subscribe("alpha", () => undefined, gaps);
    let waits = 0;
    for (const wait of [REOPEN_FIRST_MS, 2_000, 4_000, 8_000, 16_000, REOPEN_MAX_MS, REOPEN_MAX_MS]) {
      const current = FakeEventSource.instances.at(-1);
      current?.fail(true);
      vi.advanceTimersByTime(wait - 1);
      expect(FakeEventSource.instances).toHaveLength(waits + 1);
      vi.advanceTimersByTime(1);
      waits += 1;
      expect(FakeEventSource.instances).toHaveLength(waits + 1);
      expect(FakeEventSource.instances.at(-1)?.url).toBe("/api/projects/alpha/events");
    }
    FakeEventSource.instances.at(-1)?.open();
    expect(gaps).toHaveBeenCalledTimes(1);
    // An opening resets the wait.
    FakeEventSource.instances.at(-1)?.fail(true);
    vi.advanceTimersByTime(REOPEN_FIRST_MS);
    expect(FakeEventSource.instances).toHaveLength(waits + 2);
    // Stopped while waiting: nothing opens again.
    FakeEventSource.instances.at(-1)?.fail(true);
    stop();
    vi.advanceTimersByTime(REOPEN_MAX_MS);
    expect(FakeEventSource.instances).toHaveLength(waits + 2);
  });
});
