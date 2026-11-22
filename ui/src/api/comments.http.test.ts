import { afterEach, describe, expect, it, vi } from "vitest";
import { aComment, aCommentList } from "../test/builders";
import { errorAnswer, jsonAnswer, stubFetch, urlsOf } from "../test/daemonStub";
import { ClientError } from "./client";
import { HttpClient } from "./http";

// docs/features/comments.md "HTTP": the comments' two reads and four writes as `HttpClient` sends
// them. The reads GET `.../comments?on=X[&before=CM]` and `.../mentions[?login=L][&before=CM]`, a
// value encoded once, with the query's AbortSignal; judged, so a `{id, reason}` 404 and an older
// daemon's unknown route reject in the daemon's words, never read as a list. The writes POST their
// JSON bodies, the delete is a DELETE with none; a refusal (an error body, a document with notes)
// rejects with its words verbatim.

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

/** Each call's method and body as sent. */
function sent(stub: ReturnType<typeof stubFetch>): [string | undefined, unknown][] {
  return stub.mock.calls.map(([, init]) => [init?.method, typeof init?.body === "string" ? JSON.parse(init.body) : init?.body]);
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("the reads", () => {
  it("asks a thread by `on`, then `before` when paging, encoded once, with the caller's AbortSignal", async () => {
    const list = aCommentList([aComment({ id: "CM-0012" })]);
    const stub = stubFetch(() => jsonAnswer(200, list));
    const client = new HttpClient();
    const signal = new AbortController().signal;
    expect(await client.getComments("alpha", { on: "C-0007" }, signal)).toEqual(list);
    await client.getComments("a b", { on: "docs/spec/rail.md#R", before: "CM-0012" });
    expect(urlsOf(stub)).toEqual(["/api/projects/alpha/comments?on=C-0007", "/api/projects/a%20b/comments?on=docs%2Fspec%2Frail.md%23R&before=CM-0012"]);
    expect(stub.mock.calls[0]?.[1]?.signal).toBe(signal);
  });

  it("asks the mentions with `login` only when given", async () => {
    const stub = stubFetch(() => jsonAnswer(200, { login: null, comments: [], next_before: null, notes: [] }));
    const client = new HttpClient();
    await client.getMentions("alpha", {});
    await client.getMentions("alpha", { login: "lev", before: "CM-0020" });
    expect(urlsOf(stub)).toEqual(["/api/projects/alpha/mentions", "/api/projects/alpha/mentions?login=lev&before=CM-0020"]);
  });

  it("is judged: a 404 `{id, reason}` rejects with its reason, an older daemon's unknown route in its words", async () => {
    stubFetch(() => jsonAnswer(404, { id: "C-0099", reason: "on: no card C-0099" }));
    const missing = await rejection(new HttpClient().getComments("alpha", { on: "C-0099" }));
    expect([missing.status, missing.message]).toEqual([404, "on: no card C-0099"]);
    stubFetch(() => errorAnswer(404, "no route /api/projects/alpha/mentions: the routes are /api/projects and the web UI"));
    const older = await rejection(new HttpClient().getMentions("alpha", {}));
    expect([older.status, older.message]).toEqual([404, "no route /api/projects/alpha/mentions: the routes are /api/projects and the web UI"]);
  });
});

describe("the writes", () => {
  it("posts an add, an edit and a raise with their bodies, deletes with none, each to its path", async () => {
    const comment = aComment({ id: "CM-0012" });
    const stub = stubFetch((url) => jsonAnswer(200, url.endsWith("/items") ? { id: "PR-0031", created: true, hits: [], related: [], linked: null, diagnostics: [], notes: [] } : comment));
    const client = new HttpClient();
    await client.addComment("alpha", { on: "C-0007", body: "Rail?", reply_to: null, as: "mira" });
    await client.editComment("alpha", "CM-0012", { body: "Rail, 1.1 m?", version: 1, as: "mira" });
    await client.raiseFromComment("alpha", "CM-0012", { kind: "discrepancy", node_ids: ["MEC-RAIL"], text: "Rail", gap_type: "contradicts", observed: "1.1", documented: "1.2", as: null });
    await client.deleteComment("alpha", "CM-0012");
    expect(urlsOf(stub)).toEqual(["/api/projects/alpha/comments", "/api/projects/alpha/comments/CM-0012/edit", "/api/projects/alpha/comments/CM-0012/items", "/api/projects/alpha/comments/CM-0012"]);
    expect(sent(stub)).toEqual([
      ["POST", { on: "C-0007", body: "Rail?", reply_to: null, as: "mira" }],
      ["POST", { body: "Rail, 1.1 m?", version: 1, as: "mira" }],
      ["POST", { kind: "discrepancy", node_ids: ["MEC-RAIL"], text: "Rail", gap_type: "contradicts", observed: "1.1", documented: "1.2", as: null }],
      ["DELETE", undefined],
    ]);
    expect(stub.mock.calls.every(([, init]) => init?.signal === undefined)).toBe(true);
  });

  it("rejects a refusal in the daemon's words: a 403 or 400 error body, a 404 or 409 `{id, reason}`, a raise's intake document's last note", async () => {
    stubFetch(() => errorAnswer(403, "refused: CM-0012 is mira's: only its author or an `owner` deletes it"));
    const denied = await rejection(new HttpClient().deleteComment("alpha", "CM-0012"));
    expect([denied.status, denied.message]).toEqual([403, "refused: CM-0012 is mira's: only its author or an `owner` deletes it"]);
    stubFetch(() => errorAnswer(400, "body: 8193 bytes; at most 8192"));
    const capped = await rejection(new HttpClient().addComment("alpha", { on: "C-0007", body: "x", reply_to: null, as: null }));
    expect([capped.status, capped.message]).toEqual([400, "body: 8193 bytes; at most 8192"]);
    stubFetch(() => jsonAnswer(409, { id: "CM-0012", reason: "CM-0012 is at version 3, not 2: nothing changed" }));
    const stale = await rejection(new HttpClient().editComment("alpha", "CM-0012", { body: "x", version: 2, as: null }));
    expect([stale.status, stale.message]).toEqual([409, "CM-0012 is at version 3, not 2: nothing changed"]);
    stubFetch(() => jsonAnswer(404, { id: "CM-0099", reason: "no comment CM-0099" }));
    const gone = await rejection(new HttpClient().deleteComment("alpha", "CM-0099"));
    expect([gone.status, gone.message]).toEqual([404, "no comment CM-0099"]);
    stubFetch(() => jsonAnswer(409, { id: null, created: false, hits: [], related: [], linked: null, diagnostics: [], notes: ["CM-0012 is deleted: nothing changed"] }));
    const raised = await rejection(new HttpClient().raiseFromComment("alpha", "CM-0012", { kind: "question", node_ids: ["MEC-RAIL"], text: "x", working_answer: "y", price_of_other: "z", as: null }));
    expect([raised.status, raised.message]).toEqual([409, "CM-0012 is deleted: nothing changed"]);
  });

  it("answers a delete with the comment's tombstone, as the daemon's 200 sends it", async () => {
    const tombstone = aComment({ id: "CM-0012", body: null, deleted: { by: "mira", at: "2026-11-22T10:09:00Z" } });
    stubFetch(() => jsonAnswer(200, tombstone));
    expect(await new HttpClient().deleteComment("alpha", "CM-0012")).toEqual(tombstone);
  });
});
