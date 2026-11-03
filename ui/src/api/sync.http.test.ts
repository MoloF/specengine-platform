import { afterEach, describe, expect, it, vi } from "vitest";
import { errorAnswer, jsonAnswer, stubFetch, urlsOf } from "../test/daemonStub";
import { ClientError } from "./client";
import { HttpClient } from "./http";
import type { HubSync } from "./types";

// `docs/features/hub-git.md` "Data" over a stubbed `fetch`: the sync read on its one path, the slug
// encoded once, no query, its document as sent; judged, so a 404, a local daemon's and a hub's
// without `--push`, rejects whatever its body, as does every other refusal; no answer is status 0.

const PATH = "/api/projects/alpha/sync";

/** "Data"'s example, all five keys. */
const SYNC: HubSync = {
  ahead: 2,
  behind: 0,
  state: "held",
  error: "the remote refused `main`: pre-receive hook declined\nremote: frozen",
  at: "2026-11-04T09:00:00Z",
};

async function rejection(promise: Promise<unknown>): Promise<ClientError> {
  const error: unknown = await promise.then(
    () => null,
    (reason: unknown) => reason,
  );
  if (!(error instanceof ClientError)) {
    throw new Error(`expected a ClientError, got ${String(error)}`);
  }
  return error;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("the sync read", () => {
  it("asks the one path, the slug encoded once, no query, and answers the document as sent", async () => {
    const fetched = stubFetch(() => jsonAnswer(200, SYNC));
    expect(await new HttpClient().getSync("alpha")).toEqual(SYNC);
    await new HttpClient().getSync("my project#1");
    expect(urlsOf(fetched)).toEqual([PATH, "/api/projects/my%20project%231/sync"]);
    expect(fetched.mock.calls[0]?.[1]?.method).toBe("GET");
    expect(fetched.mock.calls[0]?.[1]?.body).toBeUndefined();
  });

  it("answers a fresh hub's document, its nulls kept", async () => {
    const fresh: HubSync = { ahead: null, behind: null, state: "none", error: null, at: null };
    stubFetch(() => jsonAnswer(200, fresh));
    expect(await new HttpClient().getSync("alpha")).toEqual(fresh);
  });

  it("takes its query's AbortSignal", async () => {
    const fetched = stubFetch(() => jsonAnswer(200, SYNC));
    const { signal } = new AbortController();
    await new HttpClient().getSync("alpha", signal);
    expect(fetched.mock.calls[0]?.[1]?.signal).toBe(signal);
  });

  it.each([
    ["the unknown route's 404, a local daemon or a hub without `--push`", () => errorAnswer(404, "no route /api/projects/alpha/sync: the routes are /api/projects and the web UI"), 404, "no route /api/projects/alpha/sync: the routes are /api/projects and the web UI"],
    ["a 404 with a JSON body that is no error body: never data", () => jsonAnswer(404, { id: "alpha", reason: "no such project" }), 404, "no such project"],
    ["an error body, 403", () => errorAnswer(403, "refused: no login"), 403, "refused: no login"],
    ["an error body, 503", () => errorAnswer(503, "spec index unavailable"), 503, "spec index unavailable"],
  ])("rejects %s in the daemon's words", async (_case, answer, status, message) => {
    stubFetch(answer);
    const error = await rejection(new HttpClient().getSync("alpha"));
    expect([error.status, error.message]).toEqual([status, message]);
  });

  it("no answer is status 0", async () => {
    vi.stubGlobal("fetch", vi.fn(() => Promise.reject(new TypeError("Failed to fetch"))));
    const error = await rejection(new HttpClient().getSync("alpha"));
    expect([error.status, error.message]).toEqual([0, `GET ${PATH}: no response from the daemon (Failed to fetch)`]);
  });
});
