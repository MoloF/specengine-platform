import { focusManager, onlineManager, QueryClient } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { aCheckReport, aDriftRow, aHealthReport, anAmendment, anAssumesRow, measuredAssumes, measuredDrift } from "../test/builders";
import { errorAnswer, FakeEventSource, jsonAnswer, openStreams, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { HttpClient, QUEUE_EVENT_TYPES, REOPEN_FIRST_MS } from "./http";
import { ApiProvider, createQueryClient } from "./provider";
import { queryKeys, useHealth, useLiveQueue } from "./queries";
import type { HealthReport } from "./types";

// AC-U5 of docs/features/health-left.md over HttpClient, the daemon a stubbed `fetch` and
// `EventSource`: the health read is made on entering Health and, while Health is on screen, after
// `proposal.applied`, `proposal.rejected` (UI-3 of docs/features/health-code-ui.md: row 4's
// `rejected`), `proposal.approved` and `proposal.apply_failed` (UI-1, UI-2 of
// health-ui-follow-up: row 4's decided states, an item reopened) or a stream gap; never on a
// stage, `proposal.created`, a `task.*` event, window focus, a reconnect or an interval (UI-3). A
// walk in flight is kept, never aborted, and exactly one more read follows it, answered or failed,
// however many events landed during it; that read failing schedules nothing (UI-4). An event with
// no walk in flight reads once (UI-5). Off screen, each marks it stale and entering Health reads
// it once (UI-6). The subscription's cleanup drops a read owed (U2). Row 6 (UI-4 of
// docs/features/health-drift-ui.md) adds no trigger: it rides the same read and its answer redraws it.

const HEALTH = "/api/projects/alpha/health";
const STREAM = "/api/projects/alpha/events";

/** A health answer the test holds back: while `next` is set, each answer waits for it. */
interface Hold {
  next: Promise<void> | null;
  /** The health reads as the daemon saw them, numbered from 1 (entering Health): `sent <n>`, then `answered <n>` or `failed <n>`. */
  log?: string[];
  /** The nth health answer; one unapplied amendment when absent. */
  report?: (n: number) => HealthReport;
  /** The nth health read refused: a 503 with these words; null or absent, answered. */
  failure?: (n: number) => string | null;
}

/** The daemon: one project, an empty queue and task list, a check, and its health (held by `hold`). */
function daemon(hold: Hold) {
  let reads = 0;
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
      reads += 1;
      const n = reads;
      hold.log?.push(`sent ${String(n)}`);
      const answer = () => {
        const refusal = hold.failure?.(n) ?? null;
        if (refusal !== null) {
          hold.log?.push(`failed ${String(n)}`);
          return errorAnswer(503, refusal);
        }
        hold.log?.push(`answered ${String(n)}`);
        return jsonAnswer(200, hold.report?.(n) ?? aHealthReport([anAmendment({ path: "docs/features/a.md" })]));
      };
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

/** Holds every health answer from now on until the returned release is called. */
function holdBack(hold: Hold): () => void {
  let release: () => void = () => undefined;
  hold.next = new Promise<void>((resolve) => {
    release = resolve;
  });
  return release;
}

/** Whether each health read handed `fetch` an AbortSignal. */
function signalled(fetchStub: ReturnType<typeof stubFetch>): boolean[] {
  return fetchStub.mock.calls.filter(([input]) => input === HEALTH).map(([, init]) => init !== undefined && "signal" in init);
}

/** A health answer whose row 4 holds one line, of PR-0001, in `state`. */
function rowFour(state: string): HealthReport {
  return aHealthReport([anAmendment({ path: "docs/features/a.md" })], { assumes: measuredAssumes([anAssumesRow({ id: "PR-0001", state })]) });
}

/** The state row 4's line shows, by its badge's words. */
function rowFourState(): string | null {
  return document.querySelector('[data-row="assumes"] [data-assumes="PR-0001"] .badge-label')?.textContent ?? null;
}

/** The daemon's 503 words for the nth health read: the walk incomplete. */
function incomplete(n: number): string {
  return `spec: the walk is incomplete (read ${String(n)}): the directory \`docs/spec/locks\` cannot be listed: unapplied amendments not measured`;
}

/** Row 3's note of a read failed after an answer was drawn, by its words; null when none is shown. */
function amendmentsNote(): string | null {
  return document.querySelector('[data-row="amendments"] .note[role="alert"]')?.textContent ?? null;
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

  it("never on a stage, proposal.created, a task.* event, window focus or a reconnect (UI-3 of health-ui-follow-up)", async () => {
    const { fetchStub } = await onHealth();
    let seq = 80;
    for (const type of ["proposal.staged", "proposal.unstaged", "proposal.created"]) {
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

  it("row 6 measured: proposal.applied reads /health once more and the answer's row 6 is drawn; the stream listens to no type of its own (UI-4 of health-drift-ui)", async () => {
    const drifting = aHealthReport([anAmendment({ path: "docs/features/a.md" })], { drift: measuredDrift([aDriftRow({ node: "R-1", state: "conflict", class: "fix" })], { ok: 3 }) });
    const settled = aHealthReport([anAmendment({ path: "docs/features/a.md" })], { drift: measuredDrift([], { ok: 4 }) });
    const { fetchStub } = await onHealth({ next: null, report: (n) => (n === 1 ? drifting : settled) });
    const drift = () => document.querySelector('[data-row="drift"]');
    expect(drift()?.querySelector(".health-left-total")?.textContent).toBe("1 of 4 bindings, 0 skipped");
    expect(Array.from(drift()?.querySelectorAll<HTMLElement>("[data-drift]") ?? [], (line) => line.dataset.drift)).toEqual(["R-1"]);
    // Row 6 subscribes to nothing of its own: the stream's types are the queue's (src/api/http.ts).
    expect(stream().types()).toEqual([...QUEUE_EVENT_TYPES]);
    emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "71");
    await waitFor(() => {
      expect(drift()?.querySelector(".health-code-none")?.textContent).toBe("None: every binding spec verify judges is in sync.");
    });
    await settle();
    expect(healthReads(fetchStub)).toEqual([HEALTH]);
    expect(drift()?.querySelectorAll("[data-drift]")).toHaveLength(0);
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

  it.each([
    ["proposal.approved", "UI-1", '{"id":"PR-0001"}'],
    ["proposal.apply_failed", "UI-2", '{"id":"PR-0001","step":8,"reason":"the record moved"}'],
  ])("%s while Health is on screen reads /health once more, never the check or another spec read (%s of health-ui-follow-up)", async (type, _criterion, payload) => {
    const { fetchStub } = await onHealth();
    emit(type, payload, "84");
    await waitFor(() => {
      expect(healthReads(fetchStub)).toEqual([HEALTH]);
    });
    await healthAnswered();
    await settle();
    // The queue part's inbox, as on any proposal event, and the health read: no check, no other spec read.
    expect(urlsOf(fetchStub).sort()).toEqual([HEALTH, "/api/projects/alpha/inbox"]);
  });

  it("an event with no read in flight reads /health once: no second read after it answers (UI-5 of health-ui-follow-up)", async () => {
    const hold: Hold = { next: null, log: [] };
    const { fetchStub } = await onHealth(hold);
    hold.log = [];
    emit("proposal.approved", '{"id":"PR-0001"}', "85");
    await waitFor(() => {
      expect(hold.log).toEqual(["sent 2", "answered 2"]);
    });
    await healthAnswered();
    await settle();
    await settle();
    expect(hold.log).toEqual(["sent 2", "answered 2"]);
    expect(healthReads(fetchStub)).toEqual([HEALTH]);
  });

  it("an approval, then an apply and a rejection while its read is held: two /health, the second sent once the first answered, neither with a signal; the second answer drawn (UI-4 of health-ui-follow-up)", async () => {
    const hold: Hold = { next: null, log: [], report: (n) => rowFour(n <= 2 ? "pending" : "as_assumed") };
    const { fetchStub } = await onHealth(hold);
    expect(rowFourState()).toBe("Pending");
    hold.log = [];
    const release = holdBack(hold);
    emit("proposal.approved", '{"id":"PR-0001"}', "86");
    await waitFor(() => {
      expect(healthReads(fetchStub)).toEqual([HEALTH]);
    });
    emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "87");
    emit("proposal.rejected", '{"id":"PR-0002","reason":"no"}', "88");
    await settle();
    // The walk in flight is kept: nothing aborted, nothing sent beside it.
    expect(hold.log).toEqual(["sent 2"]);
    release();
    await waitFor(() => {
      expect(rowFourState()).toBe("Decided as assumed");
    });
    await healthAnswered();
    await settle();
    await settle();
    expect(hold.log).toEqual(["sent 2", "answered 2", "sent 3", "answered 3"]);
    expect(healthReads(fetchStub)).toEqual([HEALTH, HEALTH]);
    expect(signalled(fetchStub)).toEqual([false, false]);
  });

  it("a rejection while the read is in flight keeps it, then reads exactly one more once it answered; that answer drawn (UI-4 of health-ui-follow-up)", async () => {
    const hold: Hold = { next: null, log: [], report: (n) => rowFour(n <= 2 ? "pending" : "rejected") };
    const { fetchStub } = await onHealth(hold);
    expect(rowFourState()).toBe("Pending");
    hold.log = [];
    const release = holdBack(hold);
    emit("proposal.rejected", '{"id":"PR-0001","reason":"no"}', "86");
    await waitFor(() => {
      expect(healthReads(fetchStub)).toEqual([HEALTH]);
    });
    emit("proposal.rejected", '{"id":"PR-0002","reason":"no"}', "87");
    await settle();
    expect(hold.log).toEqual(["sent 2"]);
    release();
    await waitFor(() => {
      expect(hold.log).toEqual(["sent 2", "answered 2", "sent 3", "answered 3"]);
    });
    await waitFor(() => {
      expect(rowFourState()).toBe("Rejected");
    });
    await healthAnswered();
    await settle();
    expect(healthReads(fetchStub)).toEqual([HEALTH, HEALTH]);
    expect(signalled(fetchStub)).toEqual([false, false]);
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

  it("an apply while the read is in flight keeps it, then reads exactly one more once it answered; that answer drawn (UI-4 of health-ui-follow-up)", async () => {
    const hold: Hold = { next: null, log: [], report: (n) => rowFour(n <= 2 ? "pending" : "otherwise") };
    const { fetchStub } = await onHealth(hold);
    expect(rowFourState()).toBe("Pending");
    hold.log = [];
    const release = holdBack(hold);
    emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "90");
    await waitFor(() => {
      expect(healthReads(fetchStub)).toEqual([HEALTH]);
    });
    emit("proposal.applied", '{"id":"PR-0002","commit":"def5678"}', "91");
    await settle();
    expect(hold.log).toEqual(["sent 2"]);
    release();
    await waitFor(() => {
      expect(hold.log).toEqual(["sent 2", "answered 2", "sent 3", "answered 3"]);
    });
    await waitFor(() => {
      expect(rowFourState()).toBe("Decided otherwise");
    });
    await healthAnswered();
    await settle();
    expect(healthReads(fetchStub)).toEqual([HEALTH, HEALTH]);
    expect(signalled(fetchStub)).toEqual([false, false]);
  });

  it("a walk that fails still owes its read: the trailing read is sent once the walk is refused, and its answer drawn (UI-4 of health-ui-follow-up)", async () => {
    const hold: Hold = {
      next: null,
      log: [],
      report: (n) => rowFour(n <= 2 ? "pending" : "as_assumed"),
      failure: (n) => (n === 2 ? incomplete(n) : null),
    };
    const { fetchStub } = await onHealth(hold);
    expect(rowFourState()).toBe("Pending");
    hold.log = [];
    const release = holdBack(hold);
    emit("proposal.approved", '{"id":"PR-0001"}', "140");
    await waitFor(() => {
      expect(healthReads(fetchStub)).toEqual([HEALTH]);
    });
    emit("proposal.applied", '{"id":"PR-0001","commit":"abc1234"}', "141");
    await settle();
    expect(hold.log).toEqual(["sent 2"]);
    release();
    await waitFor(() => {
      expect(rowFourState()).toBe("Decided as assumed");
    });
    await healthAnswered();
    await settle();
    await settle();
    expect(hold.log).toEqual(["sent 2", "failed 2", "sent 3", "answered 3"]);
    expect(healthReads(fetchStub)).toEqual([HEALTH, HEALTH]);
    expect(signalled(fetchStub)).toEqual([false, false]);
    // The refused walk's note is gone with the answer after it.
    expect(amendmentsNote()).toBeNull();
  });

  it("a trailing read that fails schedules nothing: the walk and the read after it refused, no third /health; the last refusal drawn verbatim (UI-4 of health-ui-follow-up)", async () => {
    const hold: Hold = { next: null, log: [], failure: (n) => (n >= 2 ? incomplete(n) : null) };
    const { fetchStub } = await onHealth(hold);
    hold.log = [];
    const release = holdBack(hold);
    emit("proposal.approved", '{"id":"PR-0001"}', "150");
    await waitFor(() => {
      expect(healthReads(fetchStub)).toEqual([HEALTH]);
    });
    emit("proposal.apply_failed", '{"id":"PR-0001","step":8,"reason":"the record moved"}', "151");
    await settle();
    expect(hold.log).toEqual(["sent 2"]);
    release();
    await waitFor(() => {
      expect(amendmentsNote()).toBe(`The unapplied amendments could not be read again: ${incomplete(3)}`);
    });
    await settle();
    await settle();
    expect(hold.log).toEqual(["sent 2", "failed 2", "sent 3", "failed 3"]);
    expect(healthReads(fetchStub)).toEqual([HEALTH, HEALTH]);
    // The last answer drawn stays up under the note.
    expect(document.querySelector('[data-row="amendments"] [data-amendment]')).not.toBeNull();
  });

  it("Health off screen: an approval and a refused apply read no /health; entering Health reads it once (UI-6 of health-ui-follow-up)", async () => {
    const { fetchStub } = await onHealth();
    await go("#/alpha/inbox", "Inbox");
    emit("proposal.approved", '{"id":"PR-0001"}', "98");
    emit("proposal.apply_failed", '{"id":"PR-0001","step":8,"reason":"the record moved"}', "99");
    await settle();
    await settle();
    expect(healthReads(fetchStub)).toEqual([]);
    await go("#/alpha/health", "Health");
    await healthAnswered();
    await settle();
    expect(healthReads(fetchStub)).toEqual([HEALTH]);
  });

  it("a gap while the read is in flight keeps it, then reads exactly one more once it answered: a gap is a trigger (UI-4 of health-ui-follow-up)", async () => {
    const hold: Hold = { next: null, log: [] };
    const { fetchStub } = await onHealth(hold);
    hold.log = [];
    const release = holdBack(hold);
    emit("proposal.rejected", '{"id":"PR-0001","reason":"no"}', "120");
    await waitFor(() => {
      expect(healthReads(fetchStub)).toEqual([HEALTH]);
    });
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    stream().fail(true);
    vi.advanceTimersByTime(REOPEN_FIRST_MS);
    vi.useRealTimers();
    act(() => {
      stream().open();
    });
    await settle();
    expect(hold.log).toEqual(["sent 2"]);
    release();
    await waitFor(() => {
      expect(hold.log).toEqual(["sent 2", "answered 2", "sent 3", "answered 3"]);
    });
    await healthAnswered();
    await settle();
    expect(healthReads(fetchStub)).toEqual([HEALTH, HEALTH]);
  });

  it("Health left while a read is owed: the walk answering reads no /health off screen; entering Health reads it once (UI-6 of health-ui-follow-up)", async () => {
    const hold: Hold = { next: null, log: [] };
    const { fetchStub } = await onHealth(hold);
    hold.log = [];
    const release = holdBack(hold);
    emit("proposal.approved", '{"id":"PR-0001"}', "130");
    await waitFor(() => {
      expect(healthReads(fetchStub)).toEqual([HEALTH]);
    });
    emit("proposal.apply_failed", '{"id":"PR-0001","step":8,"reason":"the record moved"}', "131");
    await go("#/alpha/inbox", "Inbox");
    release();
    await waitFor(() => {
      expect(hold.log).toEqual(["sent 2", "answered 2"]);
    });
    await settle();
    await settle();
    expect(hold.log).toEqual(["sent 2", "answered 2"]);
    await go("#/alpha/health", "Health");
    await healthAnswered();
    await settle();
    expect(hold.log).toEqual(["sent 2", "answered 2", "sent 3", "answered 3"]);
    expect(healthReads(fetchStub)).toEqual([HEALTH, HEALTH]);
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

/** Alpha's health read kept on screen beside a live tail whose project the test switches. */
function HealthBesideTail({ tail }: { tail: string | null }) {
  useHealth("alpha");
  useLiveQueue(tail);
  return null;
}

describe("the subscription's cleanup (U2 of health-ui-follow-up)", () => {
  it("the tail switched to another project while a read is owed drops it: the walk answering reads no /health, though alpha's read stays on screen", async () => {
    const hold: Hold = { next: null, log: [] };
    daemon(hold);
    const client = new HttpClient();
    const queryClient: QueryClient = createQueryClient();
    const view = render(
      <ApiProvider client={client} queryClient={queryClient}>
        <HealthBesideTail tail="alpha" />
      </ApiProvider>,
    );
    await waitFor(() => {
      expect(hold.log).toEqual(["sent 1", "answered 1"]);
    });
    act(() => {
      stream().open();
    });
    const release = holdBack(hold);
    emit("proposal.approved", '{"id":"PR-0001"}', "160");
    await waitFor(() => {
      expect(hold.log).toEqual(["sent 1", "answered 1", "sent 2"]);
    });
    // A trigger during the walk: one read owed.
    emit("proposal.apply_failed", '{"id":"PR-0001","step":8,"reason":"the record moved"}', "161");
    view.rerender(
      <ApiProvider client={client} queryClient={queryClient}>
        <HealthBesideTail tail="beta" />
      </ApiProvider>,
    );
    expect(openStreams()).toEqual(["/api/projects/beta/events"]);
    release();
    await waitFor(() => {
      expect(hold.log).toEqual(["sent 1", "answered 1", "sent 2", "answered 2"]);
    });
    await settle();
    await settle();
    expect(hold.log).toEqual(["sent 1", "answered 1", "sent 2", "answered 2"]);
    expect(queryClient.getQueryCache().find({ queryKey: queryKeys.health("alpha"), exact: true })?.getObserversCount()).toBe(1);
  });
});

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
