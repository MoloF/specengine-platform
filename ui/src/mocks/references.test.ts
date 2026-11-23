import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import { MockClient } from "./MockClient";

// docs/features/work-references.md "UI", The mock: harbor-sim's references in memory, uploads kept up to
// 8 MiB under made-up 64-hex names, each type sniffed from its first bytes; an attach, a delete and the
// file's address answered as the daemon would, refused in its words.

const AT = Date.parse("2026-11-23T12:00:00Z");

/** A PNG's first 24 bytes: its signature and an IHDR of 4032x3024. */
function png(width: number, height: number, padding = 0): Blob {
  const bytes = new Uint8Array(24 + padding);
  bytes.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, 0x49, 0x48, 0x44, 0x52]);
  new DataView(bytes.buffer).setUint32(16, width);
  new DataView(bytes.buffer).setUint32(20, height);
  return new Blob([bytes]);
}

async function refusal(promise: Promise<unknown>): Promise<unknown> {
  try {
    await promise;
  } catch (error) {
    return error instanceof ClientError ? [error.status, error.message] : error;
  }
  throw new Error("the call resolved");
}

const quiet = () => undefined;

describe("the mock's references", () => {
  it("seeds harbor-sim's task with a link, an image, a sound lost with its host and a deleted one; the empty scenario none", async () => {
    const list = await new MockClient("normal", { now: () => AT }).getReferences("harbor-sim", { on: "C-0003" });
    expect(list.references.map((reference) => [reference.id, reference.kind, reference.file, reference.deleted === null])).toEqual([
      ["RF-0001", "link", null, true],
      ["RF-0002", "file", "stored", true],
      ["RF-0003", "file", "lost", true],
      ["RF-0004", "file", null, false],
    ]);
    // No task there: a proposal's target, which the mock does not look up.
    expect((await new MockClient("empty", { now: () => AT }).getReferences("harbor-sim", { on: "PR-0049" })).references).toEqual([]);
  });

  it("sniffs an upload by its bytes, reads a PNG's size, names equal bytes once, and refuses what it does not store", async () => {
    const client = new MockClient("normal", { now: () => AT });
    const signal = new AbortController().signal;
    const first = await client.putBlob("harbor-sim", png(4032, 3024), quiet, signal);
    expect([first.media, first.class, first.inline, first.meta.width, first.meta.height, first.agent_ready, first.stored, /^[0-9a-f]{64}$/.test(first.b3)]).toEqual(["image/png", "image", true, 4032, 3024, false, "new", true]);
    expect((await client.putBlob("harbor-sim", png(4032, 3024), quiet, signal)).stored).toBe("existing");
    expect((await client.putBlob("harbor-sim", png(1200, 800), quiet, signal)).agent_ready).toBe(true);
    const html = new Blob(["<!doctype html><p>harbour</p>"], { type: "image/png" });
    expect(await refusal(client.putBlob("harbor-sim", html, quiet, signal))).toEqual([415, expect.stringContaining("not a type stored here")]);
    expect(await refusal(client.putBlob("harbor-sim", png(10, 10, 9 * 1024 * 1024), quiet, signal))).toEqual([413, expect.stringContaining("8 MiB")]);
  });

  it("attaches a file it holds and a link; refuses a file it does not hold, and a PDF without what matters", async () => {
    const client = new MockClient("normal", { now: () => AT });
    const signal = new AbortController().signal;
    const stored = await client.putBlob("harbor-sim", png(1200, 800), quiet, signal);
    const made = await client.addReference("harbor-sim", { on: "C-0003", b3: stored.b3, agent_b3: null, name: "small.png", title: null, matters: null, as: "lev" });
    expect([made.id, made.actor, made.media, made.file]).toEqual(["RF-0005", "lev", "image/png", "stored"]);
    expect(client.blobUrl("harbor-sim", stored.b3)).toMatch(/^(blob:|data:image\/png;base64,)/);
    const link = await client.addReference("harbor-sim", { on: "C-0003", url: "https://example.org/x", title: "X", matters: null, as: null });
    expect([link.kind, link.actor]).toEqual(["link", "local"]);
    expect(await refusal(client.addReference("harbor-sim", { on: "C-0003", b3: "0".repeat(64), agent_b3: null, name: "gone.png", title: null, matters: null, as: null }))).toEqual([404, `blob ${"0".repeat(64)} is not stored here: upload it again`]);
    const pdf = await client.putBlob("harbor-sim", new Blob(["%PDF-1.7\n"]), quiet, signal);
    expect(pdf.needs_matters).toBe(true);
    expect(await refusal(client.addReference("harbor-sim", { on: "C-0003", b3: pdf.b3, agent_b3: null, name: "tides.pdf", title: null, matters: null, as: null }))).toEqual([400, "spec: matters: required for audio, video, models and PDF: say what matters here"]);
  });

  it("deletes a reference, erasing its texts and collecting a file no live reference names; a hub's login deletes another's as its row allows", async () => {
    const client = new MockClient("normal", { now: () => AT });
    const signal = new AbortController().signal;
    const stored = await client.putBlob("harbor-sim", png(1200, 800), quiet, signal);
    const made = await client.addReference("harbor-sim", { on: "C-0003", b3: stored.b3, agent_b3: null, name: "small.png", title: "Small", matters: null, as: null });
    const gone = await client.deleteReference("harbor-sim", made.id);
    expect([gone.title, gone.name, gone.b3, gone.deleted?.by]).toEqual([null, null, null, "local"]);
    expect(await refusal(client.addReference("harbor-sim", { on: "C-0003", b3: stored.b3, agent_b3: null, name: "small.png", title: null, matters: null, as: null }))).toEqual([404, `blob ${stored.b3} is not stored here: upload it again`]);
    expect(await refusal(client.deleteReference("harbor-sim", made.id))).toEqual([409, `${made.id} is deleted: nothing changed`]);
    const hub = new MockClient("hub", { now: () => AT });
    expect((await hub.deleteReference("harbor-sim", "RF-0001")).deleted?.by).toBe("mira");
  });
});
