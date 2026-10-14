import { afterEach, describe, expect, it, vi } from "vitest";
import { aRoundItem, aRoundSheet } from "../test/builders";
import { errorAnswer, jsonAnswer, stubFetch, urlsOf } from "../test/daemonStub";
import { ClientError } from "./client";
import { HttpClient } from "./http";
import type { RoundAnswersOutcome } from "./types";

// docs/features/ui-round.md "Data", the browser side: the sheet is `GET .../round`, `limit` its one
// query, left out when absent, the query's AbortSignal handed over; the answers are one
// `POST .../round/answers` whose body is exactly `{"project","answers"}`, no AbortSignal; a 200 is
// the answers document whatever became of each answer; a refusal is the daemon's error body's
// message verbatim.

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

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("the round's sheet", () => {
  it("reads GET /api/projects/:p/round, no query by default, the slug encoded once", async () => {
    const fetchStub = stubFetch(() => jsonAnswer(200, aRoundSheet([])));
    await new HttpClient().getRound("a b");
    await new HttpClient().getRound("alpha", {});
    expect(urlsOf(fetchStub)).toEqual(["/api/projects/a%20b/round", "/api/projects/alpha/round"]);
    expect(fetchStub.mock.calls.map((call) => call[1]?.method)).toEqual(["GET", "GET"]);
  });

  it("sends `limit` alone as its query, and the query's AbortSignal", async () => {
    const fetchStub = stubFetch(() => jsonAnswer(200, aRoundSheet([])));
    const controller = new AbortController();
    await new HttpClient().getRound("alpha", { limit: 200 }, controller.signal);
    expect(urlsOf(fetchStub)).toEqual(["/api/projects/alpha/round?limit=200"]);
    expect(fetchStub.mock.calls[0]?.[1]?.signal).toBe(controller.signal);
  });

  it("resolves the sheet as sent, uncut, its items in the daemon's order", async () => {
    const sheet = aRoundSheet([aRoundItem({ id: "PR-0009", severity: "low" }), aRoundItem({ id: "PR-0002", severity: "high" })], { more: 3, notes: ["n"] });
    stubFetch(() => jsonAnswer(200, sheet));
    expect(await new HttpClient().getRound("alpha")).toEqual(sheet);
  });

  it("rejects a refused limit, an unknown project or a sheet that cannot be read in the daemon's words", async () => {
    for (const [status, message] of [
      [400, "--limit 0: the limit is 1 to 200"],
      [404, "no project `zeta`: the daemon serves alpha"],
      [503, "spec: the queue is locked"],
    ] as const) {
      stubFetch(() => errorAnswer(status, message));
      const error = await rejection(new HttpClient().getRound("alpha", { limit: 0 }));
      expect([error.status, error.message]).toEqual([status, message]);
    }
  });
});

describe("the round's answers", () => {
  it("POSTs exactly {project, answers} as JSON to /api/projects/:p/round/answers, with no AbortSignal", async () => {
    const fetchStub = stubFetch(() => jsonAnswer(200, { project: "alpha", answers: [], notes: [] }));
    await new HttpClient().stageRoundAnswers("alpha", [
      { id: "PR-0003", stage: { decision: "approve", option: 1, answer: null, canon: null, editorial: null, note: null, updated_at: "2026-10-07T17:49:50Z" } },
      { id: "PR-0005", stage: { decision: "reject", reason: "dup", updated_at: "2026-10-07T17:40:02Z" } },
    ]);
    expect(urlsOf(fetchStub)).toEqual(["/api/projects/alpha/round/answers"]);
    const init = fetchStub.mock.calls[0]?.[1];
    expect(init?.method).toBe("POST");
    expect(init?.body).toBe(
      '{"project":"alpha","answers":[{"id":"PR-0003","stage":{"decision":"approve","option":1,"answer":null,"canon":null,"editorial":null,"note":null,"updated_at":"2026-10-07T17:49:50Z"}},' +
        '{"id":"PR-0005","stage":{"decision":"reject","reason":"dup","updated_at":"2026-10-07T17:40:02Z"}}]}',
    );
    expect(init?.headers).toEqual({ Accept: "application/json", "Content-Type": "application/json" });
    expect(init !== undefined && "signal" in init).toBe(false);
  });

  it("resolves a 200 to the answers document whatever each answer became", async () => {
    const document: RoundAnswersOutcome = {
      project: "alpha",
      answers: [
        {
          id: "PR-0003",
          outcome: "staged",
          stage: { decision: "approve", option: null, answer: "no, 2 s", canon: null, editorial: null, note: null, span_hash: null },
          staged_at: "2026-10-08T09:12:40Z",
          replaced: null,
          reason: null,
        },
        { id: "PR-0005", outcome: "refused", stage: null, staged_at: null, replaced: null, reason: "`PR-0005` is rejected" },
      ],
      notes: [],
    };
    stubFetch(() => jsonAnswer(200, document));
    expect(await new HttpClient().stageRoundAnswers("alpha", [])).toEqual(document);
  });

  it("rejects the fence's, the type's, the size's and a body's refusal with the daemon's message verbatim; no answer is status 0", async () => {
    for (const [status, message] of [
      [403, "staging needs a same-origin page (not authentication: ADR-0034)"],
      [413, "the body is over 4194304 bytes"],
      [415, "the body is JSON: send it with Content-Type: application/json"],
      [400, "the answers are for `beta`, this is `alpha`; nothing changed"],
      [503, "spec: the queue is locked"],
    ] as const) {
      stubFetch(() => errorAnswer(status, message));
      const error = await rejection(new HttpClient().stageRoundAnswers("alpha", []));
      expect([error.status, error.message]).toEqual([status, message]);
    }
    stubFetch(() => {
      throw new TypeError("Failed to fetch");
    });
    const down = await rejection(new HttpClient().stageRoundAnswers("alpha", []));
    expect([down.status, down.message]).toEqual([0, "POST /api/projects/alpha/round/answers: no response from the daemon (Failed to fetch)"]);
  });
});
