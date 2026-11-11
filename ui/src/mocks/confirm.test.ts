import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import { MockClient } from "./MockClient";

// B8 of docs/features/hub-confirm.md: no hub stands behind the mock, so it answers the confirm route
// as a local daemon does, and confirms nothing.

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

describe("the mock's confirm route", () => {
  it("is a local daemon's unknown route: a 404 for the read and the POST", async () => {
    const client = new MockClient("normal", { now: () => 0 });
    expect((await rejection(client.getConfirm())).status).toBe(404);
    expect((await rejection(client.confirmProposal())).status).toBe(404);
  });

  it("the error scenario refuses the read as every read", async () => {
    const client = new MockClient("error", { now: () => 0 });
    expect((await rejection(client.getConfirm())).status).toBe(503);
  });
});

// The sync route of docs/features/hub-git.md ("Status and the screen"): no hub with `--push` behind
// the mock, so its unknown route; the page never asks it, the mock's identity having no login.
describe("the mock's sync route", () => {
  it("is a local daemon's unknown route, a 404, and the mock names no login to ask it", async () => {
    const client = new MockClient("normal", { now: () => 0 });
    expect((await rejection(client.getSync())).status).toBe(404);
    expect((await client.getIdentity()).login).toBeNull();
  });

  it("the error scenario refuses it as every read", async () => {
    expect((await rejection(new MockClient("error", { now: () => 0 }).getSync())).status).toBe(503);
  });
});

// The `hub` scenario (src/mocks/scenario.ts): a hub's login whose `[team]` row allows Confirm, so a
// staged choice is confirmed from the page as a hub does (`docs/canon/hub-confirm.md` "Routes"; the
// card's Answer..., docs/features/card-questions.md "Acceptance criteria", U4 and U6's look).
describe("the mock's hub scenario", () => {
  const AT = Date.parse("2026-11-11T09:00:00Z");
  const working = { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null } as const;

  it("names the login mira and her row, every act and confirm; normal names none", async () => {
    const client = new MockClient("hub", { now: () => AT });
    expect(await client.getIdentity()).toEqual({ login: "mira" });
    expect((await client.getTeam("harbor-sim")).allowed).toEqual(["stage", "accept", "clear", "group_clear", "confirm"]);
    expect((await new MockClient("normal", { now: () => AT }).getIdentity()).login).toBeNull();
  });

  it("reads a staged choice's question and confirms it with that question: applied under the login, the stage gone, its events on the card", async () => {
    const client = new MockClient("hub", { now: () => AT });
    expect((await rejection(client.getConfirm("harbor-sim", "PR-0055"))).status).toBe(409);
    const read = (await client.getProposal("harbor-sim", "PR-0055")).updated_at ?? "";
    await client.stageDecision("harbor-sim", "PR-0055", working, read, "mira");
    const asked = await client.getConfirm("harbor-sim", "PR-0055");
    expect(asked.question).toBe("staged 2026-11-11T09:00:00Z: spec approve PR-0055\napprove PR-0055 (question, staged)? [y/N]");
    expect((await rejection(client.confirmProposal("harbor-sim", "PR-0055", { question: "another text", updated_at: asked.updated_at }))).status).toBe(409);
    const done = await client.confirmProposal("harbor-sim", "PR-0055", { question: asked.question, updated_at: asked.updated_at });
    expect([done.status, done.decided_by, done.staged, done.applied_commit?.length]).toEqual(["applied", "mira <mira@harbor.example.invalid>", null, 40]);
    const card = await client.getCard("harbor-sim", "C-0008");
    expect("history" in card ? card.history?.slice(-3).map((event) => event.type) : null).toEqual(["proposal.staged", "proposal.approved", "proposal.applied"]);
  });
});
