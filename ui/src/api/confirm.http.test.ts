import { afterEach, describe, expect, it, vi } from "vitest";
import { errorAnswer, jsonAnswer, stubFetch, urlsOf } from "../test/daemonStub";
import { aProposal } from "../test/builders";
import { ClientError } from "./client";
import { HttpClient } from "./http";
import type { ConfirmBody } from "./types";

// `docs/features/hub-confirm.md` "Data" over a stubbed `fetch`: the read and the POST on the one path,
// the POST's body exactly `{question, updated_at}`, closed keys, so a choice cannot travel; the read
// judged as the POST is, so each refusal rejects in the daemon's words whatever its body; no answer,
// the login door's redirect among them, is status 0.

const PATH = "/api/projects/alpha/proposals/PR-0007/confirm";

const READ = { id: "PR-0007", question: "apply PR-0007 to a.md on main in /w (applies, staged)? [y/N]", updated_at: "2026-11-02T09:00:00Z" };

const BODY: ConfirmBody = { question: READ.question, updated_at: READ.updated_at };

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

describe("the confirm read", () => {
  it("asks the one path, the ID encoded once, and answers the document as sent", async () => {
    const fetched = stubFetch(() => jsonAnswer(200, READ));
    expect(await new HttpClient().getConfirm("alpha", "PR-0007")).toEqual(READ);
    await new HttpClient().getConfirm("my project", "PR/7#x");
    expect(urlsOf(fetched)).toEqual([PATH, "/api/projects/my%20project/proposals/PR%2F7%23x/confirm"]);
    expect(fetched.mock.calls[0]?.[1]?.method).toBe("GET");
    expect(fetched.mock.calls[0]?.[1]?.body).toBeUndefined();
  });

  it("takes its query's AbortSignal: a dialog closed before the answer drops it", async () => {
    const fetched = stubFetch(() => jsonAnswer(200, READ));
    const { signal } = new AbortController();
    await new HttpClient().getConfirm("alpha", "PR-0007", signal);
    expect(fetched.mock.calls[0]?.[1]?.signal).toBe(signal);
  });

  it.each([
    ["an error body, 403", () => errorAnswer(403, "refused: mira is not in this project's `[team]`: reads and plain writes"), 403, "refused: mira is not in this project's `[team]`: reads and plain writes"],
    ["an error body, 503", () => errorAnswer(503, "spec index unavailable"), 503, "spec index unavailable"],
    ["a refused review document, 409: its last note", () => jsonAnswer(409, aProposal({ id: "PR-0007", notes: ["a note", "`PR-0007` is staged editorial: confirm it on a terminal"] })), 409, "`PR-0007` is staged editorial: confirm it on a terminal"],
    ["a document, 403: its last note", () => jsonAnswer(403, aProposal({ id: "PR-0007", notes: ["refused: mira leads maps; a.md lies outside them"] })), 403, "refused: mira leads maps; a.md lies outside them"],
    ["an exit-1 document, 404: resolved as no data, its last note", () => jsonAnswer(404, aProposal({ id: "PR-0007", notes: ["no proposal `PR-0007`"] })), 404, "no proposal `PR-0007`"],
    ["no such proposal's reason, 404", () => jsonAnswer(404, { id: "PR-0007", reason: "no proposal `PR-0007`" }), 404, "no proposal `PR-0007`"],
    ["an older hub's unknown route", () => errorAnswer(404, "no route /api/projects/alpha/proposals/PR-0007/confirm"), 404, "no route /api/projects/alpha/proposals/PR-0007/confirm"],
  ])("rejects %s in the daemon's words", async (_case, answer, status, message) => {
    stubFetch(answer);
    const error = await rejection(new HttpClient().getConfirm("alpha", "PR-0007"));
    expect([error.status, error.message]).toEqual([status, message]);
  });

  it("a `fetch` meeting the login door's redirect is status 0", async () => {
    vi.stubGlobal("fetch", vi.fn(() => Promise.reject(new TypeError("Failed to fetch"))));
    const error = await rejection(new HttpClient().getConfirm("alpha", "PR-0007"));
    expect([error.status, error.message]).toEqual([0, `GET ${PATH}: no response from the daemon (Failed to fetch)`]);
  });
});

describe("the confirm POST", () => {
  it("sends exactly the read's two strings, as JSON, and answers the review document", async () => {
    const applied = aProposal({ id: "PR-0007", status: "applied", applied_commit: "1a2b3c4" });
    const fetched = stubFetch(() => jsonAnswer(200, applied));
    expect(await new HttpClient().confirmProposal("alpha", "PR-0007", BODY)).toEqual(applied);
    const [url, init] = fetched.mock.calls[0] ?? [];
    expect(url).toBe(PATH);
    expect(init?.method).toBe("POST");
    expect(init?.headers).toMatchObject({ "Content-Type": "application/json" });
    expect(init?.body).toBe(JSON.stringify(BODY));
    expect(init?.signal).toBeUndefined();
  });

  it("carries nothing else, whatever it is handed: the keys are closed", async () => {
    const fetched = stubFetch(() => jsonAnswer(200, aProposal({ id: "PR-0007" })));
    const widened = { ...BODY, id: "PR-0007", choice: { option: 1 }, decision: "approve" };
    await new HttpClient().confirmProposal("alpha", "PR-0007", widened);
    const body = fetched.mock.calls[0]?.[1]?.body;
    const sent: unknown = typeof body === "string" ? JSON.parse(body) : null;
    expect(sent).toEqual(BODY);
    expect(Object.keys(sent as object)).toEqual(["question", "updated_at"]);
  });

  it.each([
    ["a refused document, 409: its last note", () => jsonAnswer(409, aProposal({ id: "PR-0007", notes: ["`PR-0007` changed since the question; nothing changed"] })), 409, "`PR-0007` changed since the question; nothing changed"],
    ["an error body, 403", () => errorAnswer(403, "refused: mira leads maps; a.md lies outside them"), 403, "refused: mira leads maps; a.md lies outside them"],
    ["an error body, 400: a closed key", () => errorAnswer(400, "unknown field `choice`"), 400, "unknown field `choice`"],
    ["unknown, 404", () => jsonAnswer(404, aProposal({ id: "PR-0007", notes: ["no proposal `PR-0007`"] })), 404, "no proposal `PR-0007`"],
  ])("rejects %s in the daemon's words", async (_case, answer, status, message) => {
    stubFetch(answer);
    const error = await rejection(new HttpClient().confirmProposal("alpha", "PR-0007", BODY));
    expect([error.status, error.message]).toEqual([status, message]);
  });

  it("a redirect from the login door is status 0, in words a dialog can show", async () => {
    vi.stubGlobal("fetch", vi.fn(() => Promise.reject(new TypeError("Failed to fetch"))));
    const error = await rejection(new HttpClient().confirmProposal("alpha", "PR-0007", BODY));
    expect([error.status, error.message]).toEqual([0, `POST ${PATH}: no response from the daemon (Failed to fetch)`]);
  });
});
