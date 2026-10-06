import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { errorAnswer, FakeEventSource, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { apiErrorOf, ClientError, isNotServed, NOT_SERVED, type SpecEngineClient } from "./client";
import { errorBodyOf, HttpClient, QUEUE_EVENT_TYPES, REOPEN_FIRST_MS, REOPEN_MAX_MS } from "./http";
import type { QueueEvent } from "./types";

// AC-09 of docs/features/daemon-read.md: the browser side of the daemon, over a stubbed `fetch`
// and `EventSource`. Each method hits its URL, a REF encoded once; a 404 document resolves as data;
// any other non-2xx rejects with the daemon's message verbatim; no response is status 0.

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
  ])("%s", async (_name, call, url) => {
    await call(new HttpClient());
    expect(urlsOf(fetchStub)).toEqual([url]);
    expect(fetchStub.mock.calls[0]?.[1]?.method).toBe("GET");
  });

  it("posts a decision to its URL with the decision as JSON", async () => {
    await new HttpClient().decideProposal("alpha", "PR-0004", { decision: "reject", reason: "No" });
    expect(urlsOf(fetchStub)).toEqual(["/api/projects/alpha/proposals/PR-0004/decision"]);
    const init = fetchStub.mock.calls[0]?.[1];
    expect(init?.method).toBe("POST");
    expect(init?.body).toBe('{"decision":"reject","reason":"No"}');
  });

  it("is the daemon: dataSource `daemon`", () => {
    expect(new HttpClient().dataSource).toBe("daemon");
  });

  it("requests nothing the daemon does not serve: graph, tasks, a task (not served, 501, never 0; the gap named)", async () => {
    const client = new HttpClient();
    for (const call of [(client as SpecEngineClient).getGraph("alpha", { ref: "MEC-TIDES" }), client.getTasks("alpha"), client.getTask("alpha", "T-0001")]) {
      const error = await rejection(call);
      // R-n7: status 0 says no response came (the daemon down); this read is not built.
      expect(error.notServed).toBe(true);
      expect(isNotServed(error)).toBe(true);
      expect(error.status).toBe(NOT_SERVED);
      expect(NOT_SERVED).toBe(501);
      expect(error.message).toMatch(/^Not served by the daemon yet: GET \/api\/projects\/alpha\/(graph|tasks|tasks\/T-0001) is a missing endpoint/);
    }
    expect(fetchStub).not.toHaveBeenCalled();
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

  it("rejects a 404 carrying the error body (an unknown project or route) with its message", async () => {
    stubFetch(() => errorAnswer(404, "no project `zeta`: this daemon serves alpha, beta"));
    const error = await rejection(new HttpClient().getInbox("zeta"));
    expect([error.status, error.message]).toEqual([404, "no project `zeta`: this daemon serves alpha, beta"]);
  });

  it.each([
    [503, "spec: the index cannot be read: database is locked\nsecond line, verbatim"],
    [400, "unknown query `kind` for /search: query, kinds, limit, archive"],
    [403, "decisions are made on a terminal: `spec approve PR-0004` or `spec reject PR-0004 --reason …` in /work/alpha; nothing changed"],
    [405, "method PUT not allowed"],
  ])("rejects a %i with the error body's message verbatim", async (status, message) => {
    stubFetch(() => errorAnswer(status, message));
    const error = await rejection(new HttpClient().getInbox("alpha"));
    expect(error).toBeInstanceOf(ClientError);
    expect(apiErrorOf(error)).toEqual({ status, message });
  });

  it("rejects the decision with the daemon's 403 in its own words, the terminal command in it", async () => {
    const message = "decisions are made on a terminal: `spec approve PR-0004` or `spec reject PR-0004 --reason …` in /work/alpha; nothing changed";
    stubFetch(() => errorAnswer(403, message));
    const error = await rejection(new HttpClient().decideProposal("alpha", "PR-0004", { decision: "accept", option: null, note: null }));
    expect([error.status, error.message]).toEqual([403, message]);
  });

  it("gives another error body's text verbatim, and says when there is none", async () => {
    stubFetch(() => new Response("upstream timed out", { status: 504, statusText: "Gateway Timeout" }));
    expect(apiErrorOf(await rejection(new HttpClient().getProjects()))).toEqual({ status: 504, message: "upstream timed out" });
    stubFetch(() => new Response("", { status: 502, statusText: "Bad Gateway" }));
    const empty = await rejection(new HttpClient().getProjects());
    expect(empty.status).toBe(502);
    expect(empty.message).toBe("GET /api/projects answered 502 Bad Gateway with an empty body: the dev server's proxy reached no daemon; is specengine-http running?");
  });

  it("never takes a POST's 404 for a document", async () => {
    stubFetch(() => jsonAnswer(404, { proposal: null }));
    expect((await rejection(new HttpClient().decideProposal("alpha", "PR-1", { decision: "defer", note: null }))).status).toBe(404);
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

  it("opens the project's stream, listens to the queue's five event types and hands each over parsed", () => {
    const events: QueueEvent[] = [];
    const stop = new HttpClient().subscribe("alpha", (event) => events.push(event));
    const source = only();
    expect(source.url).toBe("/api/projects/alpha/events");
    // docs/canon/proposal-queue.md "States and events": the five types, each a listener of its own.
    expect(source.types()).toEqual(["proposal.created", "proposal.approved", "proposal.applied", "proposal.rejected", "proposal.apply_failed"]);
    expect([...QUEUE_EVENT_TYPES]).toEqual(source.types());
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
