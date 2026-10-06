import { vi } from "vitest";

// The daemon as a test sees it (docs/features/daemon-read.md AC-09): a stubbed global `fetch`
// answering by path, and a stubbed `EventSource` a test opens, fails and feeds by hand. Each test
// file that stubs them calls vi.unstubAllGlobals() after each test.

/** A JSON answer as the daemon sends one: compact, `application/json; charset=utf-8`. */
export function jsonAnswer(status: number, body: unknown, statusText = ""): Response {
  return new Response(typeof body === "string" ? body : JSON.stringify(body), {
    status,
    statusText,
    headers: { "Content-Type": "application/json; charset=utf-8", "Cache-Control": "no-store" },
  });
}

/** The daemon's error body: exactly `status` and `message`. */
export function errorAnswer(status: number, message: string): Response {
  return jsonAnswer(status, { status, message });
}

/** How the stub answers one request: by its path and query, as sent. */
export type Answering = (url: string, init: RequestInit | undefined) => Response | Promise<Response>;

/** Stubs the global `fetch`; the returned mock records each call's URL and init. */
export function stubFetch(answering: Answering) {
  const stub = vi.fn((input: RequestInfo | URL, init?: RequestInit) =>
    Promise.resolve(answering(typeof input === "string" ? input : input instanceof URL ? input.href : input.url, init)),
  );
  vi.stubGlobal("fetch", stub);
  return stub;
}

/** The URLs a fetch stub was called with, in order. */
export function urlsOf(stub: ReturnType<typeof stubFetch>): string[] {
  return stub.mock.calls.map(([input]) => (typeof input === "string" ? input : input instanceof URL ? input.href : input.url));
}

/**
 * An EventSource driven by the test: `open()`, `emit()` a named event, `fail()` as the browser
 * reconnecting (CONNECTING) or giving up (CLOSED). Every instance is kept, in order.
 */
export class FakeEventSource {
  static instances: FakeEventSource[] = [];

  readonly url: string;
  readyState = 0;
  onopen: ((event: Event) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  private readonly listeners = new Map<string, ((event: MessageEvent<unknown>) => void)[]>();

  constructor(url: string | URL) {
    this.url = String(url);
    FakeEventSource.instances.push(this);
  }

  addEventListener(type: string, listener: (event: MessageEvent<unknown>) => void): void {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }

  close(): void {
    this.readyState = 2;
  }

  /** The event names listened to. */
  types(): string[] {
    return [...this.listeners.keys()];
  }

  open(): void {
    this.readyState = 1;
    this.onopen?.(new Event("open"));
  }

  /** One SSE event: `id: <id>`, `event: <type>`, `data: <data>`. */
  emit(type: string, data: string, id: string): void {
    const event = new MessageEvent(type, { data, lastEventId: id });
    for (const listener of this.listeners.get(type) ?? []) {
      listener(event);
    }
  }

  /** A broken stream: `closed` false, the browser reconnects itself; true, it has given up. */
  fail(closed: boolean): void {
    this.readyState = closed ? 2 : 0;
    this.onerror?.(new Event("error"));
  }
}

/** Stubs the global `EventSource` with FakeEventSource, its instances forgotten. */
export function stubEventSource(): typeof FakeEventSource {
  FakeEventSource.instances = [];
  vi.stubGlobal("EventSource", FakeEventSource);
  return FakeEventSource;
}

/** The open streams, by URL. */
export function openStreams(): string[] {
  return FakeEventSource.instances.filter((source) => source.readyState !== 2).map((source) => source.url);
}
