import { focusManager, onlineManager, QueryClient } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { aCheckReport, aHealthReport, anAmendment } from "../test/builders";
import { errorAnswer, FakeEventSource, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { HttpClient, REOPEN_FIRST_MS } from "./http";
import { ApiProvider, createQueryClient } from "./provider";
import { queryKeys, useHealth } from "./queries";

// AC-U5 of docs/features/health-left.md over HttpClient, the daemon a stubbed `fetch` and
// `EventSource`: the health read is made on entering Health and, while Health is on screen, after
// `proposal.applied`, `proposal.rejected` (UI-3 of docs/features/health-code-ui.md: row 4's
// `rejected`) or a stream gap; never on a stage, another proposal event, a `task.*` event, window
// focus, a reconnect or an interval. A walk in flight is kept: an apply or a rejection during it
// starts no second one. Off screen, either marks it stale and entering Health reads it.

const HEALTH = "/api/projects/alpha/health";
const STREAM = "/api/projects/alpha/events";

/** A health answer the test holds back: while `next` is set, each answer waits for it. */
interface Hold {
  next: Promise<void> | null;
}

/** The daemon: one project, an empty queue and task list, a check, and its health (held by `hold`). */
function daemon(hold: Hold) {
  return stubFetch((url) => {
    if (url === "/api/projects") {
      return jsonAnswer(200, [{ slug: "alpha", name: "Alpha", root: "/work/alpha", branch: "main" }]);
    }
    if (url === "/api/projects/alpha/inbox") {
      return jsonAnswer(200, { proposals: [], notes: [] });
    }
    if (url === "/api/projects/alpha/tasks") {
      return jsonAnswer(200, { tasks: [], notes: [] });
    }
    if (url === "/api/projects/alpha/check") {
      return jsonAnswer(200, aCheckReport());
    }
    if (url === HEALTH) {
      const answer = () => jsonAnswer(200, aHealthReport([anAmendment({ path: "docs/features/a.md" })]));
      return hold.next === null ? answer() : hold.next.then(answer);
    }
    return errorAnswer(404, `no route ${url}`);
  });
}

function stream(): FakeEventSource {
  const source = FakeEventSource.instances.filter((candidate) => candidate.url === STREAM && candidate.readyState !== 2).at(-1);
  if (source === undefined) {
    throw new Error("no open stream");
  }
  return source;
}

/** The health reads made since the stub was last cleared. */
function healthReads(fetchStub: ReturnType<typeof stubFetch>): string[] {
  return urlsOf(fetchStub).filter((url) => url === HEALTH);
}

/** Waits until Health's four regions have answered and nothing is in flight. */
async function healthAnswered() {
  await waitFor(() => {
    expect(document.querySelectorAll('.health-region[aria-busy="false"]')).toHaveLength(4);
    expect(document.querySelector('[data-row="amendments"] [data-amendment]')).not.toBeNull();
  });
}

/** Runs the tasks jsdom queued: a late answer, a refetch scheduled by an event. */
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/** Health of alpha over HttpClient, its live tail open; the fetch stub, cleared after entering. */
async function onHealth(hold: Hold = { next: null }) {
  const fetchStub = daemon(hold);
  window.history.replaceState(null, "", "/#/alpha/health");
  render(<App client={new HttpClient()} scenario={null} />);
  await screen.findByRole("heading", { level: 1, name: "Health" });
  await healthAnswered();
  await waitFor(() => {
    expect(FakeEventSource.instances.map((source) => source.url)).toEqual([STREAM]);
  });
  act(() => {
    stream().open();
  });
  const entered = [...fetchStub.mock.calls];
  fetchStub.mockClear();
  return { fetchStub, entered };
}

function emit(type: string, payload: string, id: string) {
  act(() => {
    stream().emit(type, payload, id);
  });
}

async function go(hash: string, heading: string) {
  act(() => {
    window.location.hash = hash;
  });
  await screen.findByRole("heading", { level: 1, name: heading });
}

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  focusManager.setFocused(undefined);
  onlineManager.setOnline(true);
  vi.unstubAllGlobals();
});

describe("the health read on the live tail (AC-U5 of health-left)", () => {
  it("entering Health reads /health once, with no AbortSignal: a walk is never aborted", async () => {
    const { entered } = await onHealth();
    const reads = entered.filter(([input]) => input === HEALTH);
    expect(reads).toHaveLength(1);
    expect(reads.map(([, init]) => init !== undefined && "signal" in init)).toEqual([false]);
  });

  it("proposal.applied while Health is on screen reads /health once more", async () => {
    const { fetchStub } = await onHealth();
    emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "70");
    await waitFor(() => {
      expect(healthReads(fetchStub)).toEqual([HEALTH]);
    });
    await healthAnswered();
    await settle();
    expect(healthReads(fetchStub)).toEqual([HEALTH]);
  });

  it("a stream this client opened again (a gap) reads /health once", async () => {
    const { fetchStub } = await onHealth();
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    stream().fail(true);
    vi.advanceTimersByTime(REOPEN_FIRST_MS);
    vi.useRealTimers();
    expect(healthReads(fetchStub)).toEqual([]);
    act(() => {
      stream().open();
    });
    await waitFor(() => {
      expect(healthReads(fetchStub)).toEqual([HEALTH]);
    });
    await healthAnswered();
    await settle();
    expect(healthReads(fetchStub)).toEqual([HEALTH]);
  });

  it("never on a stage, another proposal event, a task.* event, window focus or a reconnect", async () => {
    const { fetchStub } = await onHealth();
    let seq = 80;
    for (const type of ["proposal.staged", "proposal.unstaged", "proposal.created", "proposal.approved", "proposal.apply_failed"]) {
      seq += 1;
      emit(type, '{"id":"PR-0001","staged_at":"2026-10-09T10:00:00Z"}', String(seq));
    }
    for (const type of ["task.created", "task.approved", "task.completed", "task.refreshed"]) {
      seq += 1;
      emit(type, '{"id":"T-0001"}', String(seq));
    }
    act(() => {
      focusManager.setFocused(false);
      focusManager.setFocused(true);
      window.dispatchEvent(new Event("focus"));
      document.dispatchEvent(new Event("visibilitychange"));
      onlineManager.setOnline(false);
      onlineManager.setOnline(true);
    });
    await settle();
    await settle();
    expect(healthReads(fetchStub)).toEqual([]);
  });

  it("proposal.rejected while Health is on screen reads /health once more, never the check or another spec read (UI-3 of health-code-ui)", async () => {
    const { fetchStub } = await onHealth();
    emit("proposal.rejected", '{"id":"PR-0001","reason":"no"}', "85");
    await waitFor(() => {
      expect(healthReads(fetchStub)).toEqual([HEALTH]);
    });
    await healthAnswered();
    await settle();
    // The queue part's inbox, as on any proposal event, and the health read: no check, no other spec read.
    expect(urlsOf(fetchStub).sort()).toEqual([HEALTH, "/api/projects/alpha/inbox"]);
  });

  it("a rejection while the read is in flight starts no second walk: one /health, its answer taken", async () => {
    const hold: Hold = { next: null };
    const { fetchStub } = await onHealth(hold);
    let release: () => void = () => undefined;
    hold.next = new Promise<void>((resolve) => {
      release = resolve;
    });
    emit("proposal.rejected", '{"id":"PR-0001","reason":"no"}', "86");
    await waitFor(() => {
      expect(healthReads(fetchStub)).toEqual([HEALTH]);
    });
    emit("proposal.rejected", '{"id":"PR-0002","reason":"no"}', "87");
    release();
    await healthAnswered();
    await settle();
    expect(healthReads(fetchStub)).toEqual([HEALTH]);
  });

  it("Health off screen: a rejection reads no /health; entering Health reads it once", async () => {
    const { fetchStub } = await onHealth();
    await go("#/alpha/inbox", "Inbox");
    emit("proposal.rejected", '{"id":"PR-0001","reason":"no"}', "88");
    await settle();
    await settle();
    expect(healthReads(fetchStub)).toEqual([]);
    await go("#/alpha/health", "Health");
    await healthAnswered();
    await settle();
    expect(healthReads(fetchStub)).toEqual([HEALTH]);
  });

  it("an apply while the read is in flight starts no second walk: one /health, its answer taken", async () => {
    const hold: Hold = { next: null };
    const { fetchStub } = await onHealth(hold);
    let release: () => void = () => undefined;
    hold.next = new Promise<void>((resolve) => {
      release = resolve;
    });
    emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "90");
    await waitFor(() => {
      expect(healthReads(fetchStub)).toEqual([HEALTH]);
    });
    emit("proposal.applied", '{"id":"PR-0002","commit":"def5678"}', "91");
    release();
    await healthAnswered();
    await settle();
    expect(healthReads(fetchStub)).toEqual([HEALTH]);
  });

  it("Health off screen: an apply reads no /health; entering Health reads it once", async () => {
    const { fetchStub } = await onHealth();
    await go("#/alpha/inbox", "Inbox");
    emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "100");
    await settle();
    await settle();
    expect(healthReads(fetchStub)).toEqual([]);
    await go("#/alpha/health", "Health");
    await healthAnswered();
    await settle();
    expect(healthReads(fetchStub)).toEqual([HEALTH]);
  });
});

function HealthRead() {
  useHealth("alpha");
  return null;
}

describe("the health query's options (AC-U5 of health-left)", () => {
  it("turn off the refetch on window focus, on a reconnect and on an interval", async () => {
    daemon({ next: null });
    const queryClient: QueryClient = createQueryClient();
    render(
      <ApiProvider client={new HttpClient()} queryClient={queryClient}>
        <HealthRead />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(queryClient.getQueryData(queryKeys.health("alpha"))).toBeDefined();
    });
    const options = queryClient.getQueryCache().find({ queryKey: queryKeys.health("alpha") })?.observers[0]?.options;
    expect([options?.refetchOnWindowFocus, options?.refetchOnReconnect, options?.refetchInterval]).toEqual([false, false, false]);
  });
});
