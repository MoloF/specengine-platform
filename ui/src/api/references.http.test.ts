import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { aBlobStored, aLinkReference, aProposal, aReference, aReferenceList } from "../test/builders";
import { errorAnswer, jsonAnswer, stubFetch, urlsOf } from "../test/daemonStub";
import { ClientError } from "./client";
import { HttpClient, QUEUE_EVENT_TYPES } from "./http";
import { BROKEN_RETRY_S, OCTET_STREAM, retryAfterOf, UPLOAD_RETRIES } from "./referencesHttp";

// docs/features/work-references.md "HTTP" and RF21: the references' read and two writes as
// `HttpClient` sends them, and the binary door's PUT. The read GETs `.../references?on=X[&before=RF]`,
// judged; the attach POSTs its JSON body, the delete is a DELETE with none; a refusal rejects in the
// daemon's words. The upload is one XMLHttpRequest PUT of the bytes as they are, typed
// `application/octet-stream` (never multipart), its progress the browser's `upload.onprogress`,
// Cancel aborting it; a 413, 415 or 507 rejects with the daemon's reason; a 429 waits its
// `Retry-After` and goes again, at most five times. Each file's address is `.../blobs/<b3>`.

async function rejection(promise: Promise<unknown>): Promise<unknown> {
  try {
    await promise;
  } catch (error) {
    return error;
  }
  throw new Error("the call resolved");
}

/** An XMLHttpRequest the test answers by hand: what was opened, the headers set, the body sent. */
class FakeXhr {
  static made: FakeXhr[] = [];
  method = "";
  url = "";
  headers: Record<string, string> = {};
  body: unknown = undefined;
  aborted = false;
  status = 0;
  statusText = "";
  responseText = "";
  responseType = "";
  answerHeaders: Record<string, string> = {};
  readonly upload: { onprogress: ((event: ProgressEvent) => void) | null } = { onprogress: null };
  onload: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onabort: (() => void) | null = null;

  constructor() {
    FakeXhr.made.push(this);
  }

  open(method: string, url: string): void {
    this.method = method;
    this.url = url;
  }

  setRequestHeader(name: string, value: string): void {
    this.headers[name] = value;
  }

  getResponseHeader(name: string): string | null {
    return this.answerHeaders[name] ?? null;
  }

  send(body: unknown): void {
    this.body = body;
  }

  abort(): void {
    this.aborted = true;
    this.onabort?.();
  }

  /** The browser's count of the body sent. */
  progress(loaded: number, total: number): void {
    this.upload.onprogress?.({ loaded, total, lengthComputable: true } as ProgressEvent);
  }

  /** The daemon's answer. */
  answer(status: number, body: unknown, headers: Record<string, string> = {}): void {
    this.status = status;
    this.responseText = typeof body === "string" ? body : JSON.stringify(body);
    this.answerHeaders = headers;
    this.onload?.();
  }
}

/** The one PUT made last. */
function last(): FakeXhr {
  const made = FakeXhr.made.at(-1);
  if (made === undefined) {
    throw new Error("no upload was made");
  }
  return made;
}

/** Lets the client's chunk load and its request open. */
async function opened(count: number): Promise<void> {
  await vi.waitFor(() => {
    expect(FakeXhr.made).toHaveLength(count);
  });
}

beforeEach(() => {
  FakeXhr.made = [];
  vi.stubGlobal("XMLHttpRequest", FakeXhr);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("the read and the writes", () => {
  it("asks a target's references by `on`, then `before` when paging, encoded once, with the caller's AbortSignal; judged, a 404 rejects with its reason", async () => {
    const list = aReferenceList([aReference({ id: "RF-0012" })]);
    const stub = stubFetch(() => jsonAnswer(200, list));
    const client = new HttpClient();
    const signal = new AbortController().signal;
    expect(await client.getReferences("alpha", { on: "C-0007" }, signal)).toEqual(list);
    await client.getReferences("a b", { on: "docs/spec/rail.md#R", before: "RF-0012" });
    expect(urlsOf(stub)).toEqual(["/api/projects/alpha/references?on=C-0007", "/api/projects/a%20b/references?on=docs%2Fspec%2Frail.md%23R&before=RF-0012"]);
    expect(stub.mock.calls[0]?.[1]?.signal).toBe(signal);
    stubFetch(() => jsonAnswer(404, { id: "C-0099", reason: "on: no card C-0099" }));
    const missing = await rejection(client.getReferences("alpha", { on: "C-0099" }));
    expect(missing instanceof ClientError ? [missing.status, missing.message] : missing).toEqual([404, "on: no card C-0099"]);
  });

  it("posts a file's attach and a link's with their bodies, deletes with none, each to its path; a refusal in the daemon's words", async () => {
    const stub = stubFetch(() => jsonAnswer(200, aLinkReference({ id: "RF-0012" })));
    const client = new HttpClient();
    await client.addReference("alpha", { on: "C-0007", b3: "f".repeat(64), agent_b3: "c".repeat(64), name: "harbour.png", title: "Harbour", matters: null, as: "mira" });
    await client.addReference("alpha", { on: "C-0007", url: "https://example.org/board", title: null, matters: "the palette", as: null });
    await client.deleteReference("alpha", "RF-0012");
    expect(urlsOf(stub)).toEqual(["/api/projects/alpha/references", "/api/projects/alpha/references", "/api/projects/alpha/references/RF-0012"]);
    const bodyOf = (body: BodyInit | null | undefined): unknown => (typeof body === "string" ? (JSON.parse(body) as unknown) : body);
    expect(stub.mock.calls.map(([, init]) => [init?.method, bodyOf(init?.body)])).toEqual([
      ["POST", { on: "C-0007", b3: "f".repeat(64), agent_b3: "c".repeat(64), name: "harbour.png", title: "Harbour", matters: null, as: "mira" }],
      ["POST", { on: "C-0007", url: "https://example.org/board", title: null, matters: "the palette", as: null }],
      ["DELETE", undefined],
    ]);
    stubFetch(() => errorAnswer(400, "spec: matters: required for audio, video, models and PDF: say what matters here"));
    const refused = await rejection(client.addReference("alpha", { on: "C-0007", b3: "f".repeat(64), agent_b3: null, name: "sound.wav", title: null, matters: null, as: null }));
    expect(refused instanceof ClientError ? [refused.status, refused.message] : refused).toEqual([400, "spec: matters: required for audio, video, models and PDF: say what matters here"]);
  });

  it("posts a promotion's body to the reference's own route, one segment each; 200 the review document; a 409 and a 503 in the daemon's words (docs/canon/work-references-promotion-surfaces.md \"HTTP\"; PR16 of reference-promotion)", async () => {
    const review = aProposal({ id: "PR-0042", kind: "reference", target_id: "LOC-HARBOUR", ref_id: "RF-0012", notes: ["adds docs/spec/world/media/harbour.png: RF-0012, image/png 2.1 MiB, LFS"] });
    const stub = stubFetch(() => jsonAnswer(200, review));
    const client = new HttpClient();
    const body = { to: "LOC-HARBOUR", name: null, matters: "the light at dusk, not the boats", rationale: "Promote RF-0012 beside LOC-HARBOUR", as: "mira" };
    expect(await client.promoteReference("a b", "RF-0012", body)).toEqual(review);
    expect(urlsOf(stub)).toEqual(["/api/projects/a%20b/references/RF-0012/promote"]);
    const [, init] = stub.mock.calls[0] ?? [];
    expect([init?.method, typeof init?.body === "string" ? (JSON.parse(init.body) as unknown) : init?.body]).toEqual(["POST", body]);
    stubFetch(() => jsonAnswer(409, { id: "RF-0012", reason: "RF-0012 is already proposed for `LOC-HARBOUR` by PR-0042 (open)" }));
    const twin = await rejection(client.promoteReference("alpha", "RF-0012", body));
    expect(twin instanceof ClientError ? [twin.status, twin.message] : twin).toEqual([409, "RF-0012 is already proposed for `LOC-HARBOUR` by PR-0042 (open)"]);
    stubFetch(() => errorAnswer(503, "spec: no [references] home takes kind `mechanic` (`MEC-TIDE`): add one to [[references.homes]] in specengine.toml; nothing stored"));
    const home = await rejection(client.promoteReference("alpha", "RF-0012", { ...body, to: "MEC-TIDE" }));
    expect(home instanceof ClientError ? [home.status, home.message] : home).toEqual([503, "spec: no [references] home takes kind `mechanic` (`MEC-TIDE`): add one to [[references.homes]] in specengine.toml; nothing stored"]);
  });

  it("gives each file's address under the project, its hash one segment", () => {
    expect(new HttpClient().blobUrl("a b", "5f".repeat(32))).toBe(`/api/projects/a%20b/blobs/${"5f".repeat(32)}`);
  });

  it("listens to the references' two events, no more (docs/features/work-references.md \"Events and feed\")", () => {
    expect(QUEUE_EVENT_TYPES.filter((type) => type.startsWith("reference."))).toEqual(["reference.created", "reference.deleted"]);
  });
});

describe("the upload, the binary door's PUT (RF21)", () => {
  it("PUTs the Blob itself as application/octet-stream, never a form: M: a multipart body or type", async () => {
    const file = new File([new Uint8Array([0x89, 0x50, 0x4e, 0x47])], "harbour.png", { type: "image/png" });
    const put = new HttpClient().putBlob("alpha", file, () => undefined, new AbortController().signal);
    await opened(1);
    const xhr = last();
    expect([xhr.method, xhr.url]).toEqual(["PUT", "/api/projects/alpha/blobs"]);
    // Set by hand over the File's own `image/png`.
    expect(xhr.headers["Content-Type"]).toBe(OCTET_STREAM);
    expect(OCTET_STREAM).toBe("application/octet-stream");
    expect(Object.values(xhr.headers).some((value) => /multipart|form-data/i.test(value))).toBe(false);
    expect(xhr.body).toBe(file);
    expect(xhr.body instanceof FormData).toBe(false);
    xhr.answer(200, aBlobStored({ b3: "b".repeat(64) }));
    expect((await put).b3).toBe("b".repeat(64));
  });

  it("reports the browser's count of the body sent, as it goes", async () => {
    const seen: [number, number][] = [];
    const put = new HttpClient().putBlob(
      "alpha",
      new Blob([new Uint8Array(200)]),
      (sent, total) => {
        seen.push([sent, total]);
      },
      new AbortController().signal,
    );
    await opened(1);
    last().progress(64, 200);
    last().progress(200, 200);
    last().answer(200, aBlobStored());
    await put;
    expect(seen).toEqual([
      [64, 200],
      [200, 200],
    ]);
  });

  it("Cancel aborts the request: an AbortError, never the daemon's refusal", async () => {
    const cancel = new AbortController();
    const put = new HttpClient().putBlob("alpha", new Blob([new Uint8Array(8)]), () => undefined, cancel.signal);
    await opened(1);
    cancel.abort();
    const error = await rejection(put);
    expect(last().aborted).toBe(true);
    expect(error instanceof DOMException ? error.name : error).toBe("AbortError");
    expect(error instanceof ClientError).toBe(false);
  });

  it.each([
    [413, "refused: the upload is 300.0 MiB; a file here is at most 256 MiB"],
    [415, "refused: an upload is application/octet-stream, never multipart"],
    [411, "refused: an upload needs its Content-Length"],
    [507, "refused: the store keeps 5 GiB free; 4.8 GiB would remain"],
    [503, "the blob store /data/specengine/blobs/alpha is not writable: on a hub, chown 10001:10001 /data/specengine/blobs"],
  ])("rejects a %s with the daemon's reason verbatim", async (status, message) => {
    const put = new HttpClient().putBlob("alpha", new Blob([new Uint8Array(8)]), () => undefined, new AbortController().signal);
    await opened(1);
    last().answer(status, { status, message });
    const error = await rejection(put);
    expect(error instanceof ClientError ? [error.status, error.message] : error).toEqual([status, message]);
  });

  it("rejects a proxy's plain answer with its text", async () => {
    const plain = new HttpClient().putBlob("alpha", new Blob([new Uint8Array(8)]), () => undefined, new AbortController().signal);
    await opened(1);
    last().answer(413, "413 Request Entity Too Large");
    const proxied = await rejection(plain);
    expect(proxied instanceof ClientError ? [proxied.status, proxied.message] : proxied).toEqual([413, "413 Request Entity Too Large"]);
  });

  it("goes again once, after 5 s, when an upload is cut with no answer (a daemon refusing early resets it); cut twice, it broke off (M: no second try)", async () => {
    vi.useFakeTimers();
    const client = new HttpClient();
    const once = client.putBlob("alpha", new Blob([new Uint8Array(8)]), () => undefined, new AbortController().signal);
    await vi.waitFor(() => {
      expect(FakeXhr.made).toHaveLength(1);
    });
    expect(BROKEN_RETRY_S).toBe(5);
    last().onerror?.();
    await vi.advanceTimersByTimeAsync(4_999);
    expect(FakeXhr.made).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(FakeXhr.made).toHaveLength(2);
    last().answer(200, aBlobStored({ b3: "d".repeat(64) }));
    expect((await once).b3).toBe("d".repeat(64));
    const twice = rejection(client.putBlob("alpha", new Blob([new Uint8Array(8)]), () => undefined, new AbortController().signal));
    await vi.waitFor(() => {
      expect(FakeXhr.made).toHaveLength(3);
    });
    last().onerror?.();
    await vi.advanceTimersByTimeAsync(5_000);
    expect(FakeXhr.made).toHaveLength(4);
    last().onerror?.();
    const none = await twice;
    expect(none instanceof ClientError ? [none.status, none.message] : none).toEqual([0, "PUT /api/projects/alpha/blobs: the upload broke off, no answer from the daemon"]);
    await vi.advanceTimersByTimeAsync(10_000);
    expect(FakeXhr.made).toHaveLength(4);
  });

  it("waits a 429's Retry-After and goes again, at most five times, then rejects with its words (M: a 429 taken as final)", async () => {
    vi.useFakeTimers();
    const seen: number[] = [];
    const put = new HttpClient().putBlob(
      "alpha",
      new Blob([new Uint8Array(8)]),
      (sent) => {
        seen.push(sent);
      },
      new AbortController().signal,
    );
    const caught = rejection(put);
    await vi.waitFor(() => {
      expect(FakeXhr.made).toHaveLength(1);
    });
    for (let attempt = 1; attempt <= UPLOAD_RETRIES; attempt += 1) {
      last().answer(429, { status: 429, message: "refused: mira runs 2 uploads; one must end first" }, { "Retry-After": "5" });
      await vi.advanceTimersByTimeAsync(4_999);
      expect(FakeXhr.made).toHaveLength(attempt);
      await vi.advanceTimersByTimeAsync(1);
      expect(FakeXhr.made).toHaveLength(attempt + 1);
    }
    last().answer(429, { status: 429, message: "refused: mira runs 2 uploads; one must end first" }, { "Retry-After": "5" });
    const error = await caught;
    expect(error instanceof ClientError ? [error.status, error.message] : error).toEqual([429, "refused: mira runs 2 uploads; one must end first"]);
    expect(FakeXhr.made).toHaveLength(1 + UPLOAD_RETRIES);
    // Each try starts its progress over.
    expect(seen.filter((sent) => sent === 0)).toHaveLength(UPLOAD_RETRIES);
  });

  it("a 429 then a 200 resolves, and Cancel during the wait aborts it", async () => {
    vi.useFakeTimers();
    const client = new HttpClient();
    const put = client.putBlob("alpha", new Blob([new Uint8Array(8)]), () => undefined, new AbortController().signal);
    await vi.waitFor(() => {
      expect(FakeXhr.made).toHaveLength(1);
    });
    last().answer(429, { status: 429, message: "refused: this hub runs 8 uploads; try again in a moment" }, { "Retry-After": "2" });
    await vi.advanceTimersByTimeAsync(2_000);
    expect(FakeXhr.made).toHaveLength(2);
    last().answer(200, aBlobStored({ b3: "e".repeat(64) }));
    expect((await put).b3).toBe("e".repeat(64));
    const cancel = new AbortController();
    const waiting = rejection(client.putBlob("alpha", new Blob([new Uint8Array(8)]), () => undefined, cancel.signal));
    await vi.waitFor(() => {
      expect(FakeXhr.made).toHaveLength(3);
    });
    last().answer(429, { status: 429, message: "refused: this hub runs 8 uploads; try again in a moment" }, { "Retry-After": "5" });
    await vi.advanceTimersByTimeAsync(1_000);
    cancel.abort();
    const error = await waiting;
    expect(error instanceof DOMException ? error.name : error).toBe("AbortError");
    await vi.advanceTimersByTimeAsync(10_000);
    expect(FakeXhr.made).toHaveLength(3);
  });

  it("reads Retry-After as whole seconds, else the daemon's 5", () => {
    expect([retryAfterOf("2"), retryAfterOf(" 7 "), retryAfterOf(null), retryAfterOf("soon"), retryAfterOf("1.5"), retryAfterOf("600")]).toEqual([2, 7, 5, 5, 5, 60]);
  });
});
