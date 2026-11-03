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
