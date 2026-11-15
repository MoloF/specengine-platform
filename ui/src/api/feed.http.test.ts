import { afterEach, describe, expect, it, vi } from "vitest";
import { aFeed, aFeedEntry } from "../test/builders";
import { errorAnswer, jsonAnswer, stubFetch, urlsOf } from "../test/daemonStub";
import { ClientError } from "./client";
import { HttpClient } from "./http";

// `docs/features/auto-feed.md` "Data" over a stubbed `fetch`: `GET /api/projects/:p/feed`, each query
// once and only when given (`since`, `needs_me=1`, `task`, `department`), the document as sent;
// judged, so a 400 for a bad value, an older daemon's 404 and a 503 reject in the daemon's words.

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

describe("the feed read", () => {
  it("asks the feed path with each query given, once, in the flags' order, and answers the document as sent", async () => {
    const document = aFeed([aFeedEntry({ seq: 1204, type: "proposal.applied", by: "rule" })]);
    const fetched = stubFetch(() => jsonAnswer(200, document));
    expect(await new HttpClient().getFeed("alpha", {})).toEqual(document);
    await new HttpClient().getFeed("alpha", { since: 1200, needs_me: true, task: "T-0042", department: "maps & charts" });
    await new HttpClient().getFeed("my project#1", { needs_me: false });
    expect(urlsOf(fetched)).toEqual([
      "/api/projects/alpha/feed",
      "/api/projects/alpha/feed?since=1200&needs_me=1&task=T-0042&department=maps%20%26%20charts",
      "/api/projects/my%20project%231/feed",
    ]);
    expect(fetched.mock.calls[0]?.[1]?.method).toBe("GET");
  });

  it("rejects a refusal in the daemon's words, a 400's and an older daemon's 404 alike", async () => {
    stubFetch(() => errorAnswer(400, "`needs_me` is `2`: only 1"));
    const refused = await rejection(new HttpClient().getFeed("alpha", { needs_me: true }));
    expect([refused.status, refused.message]).toEqual([400, "`needs_me` is `2`: only 1"]);
    stubFetch(() => errorAnswer(404, "no route /api/projects/alpha/feed"));
    const older = await rejection(new HttpClient().getFeed("alpha", {}));
    expect([older.status, older.message]).toEqual([404, "no route /api/projects/alpha/feed"]);
  });
});
