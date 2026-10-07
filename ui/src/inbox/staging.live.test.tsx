import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { HttpClient } from "../api/http";
import type { Proposal, Stage } from "../api/types";
import { aProposal, entryOf } from "../test/builders";
import { errorAnswer, FakeEventSource, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { renderApp } from "../test/render";

// AC-13 and AC-14 of docs/features/decision-staging.md over HttpClient, the daemon a stubbed
// `fetch` and `EventSource`: Accept stages with one POST whose body is exactly the stage plus the
// `updated_at` read, the card shows the staged choice, its command and Unstage (a DELETE), the
// proposal stays listed; a 409 carrying the review document shows its last note in the dialog; a
// stage event reads only that project's inbox and that proposal again; a stage made outside this
// tab is an alert, this tab's own is not.

const READ_AT = "2026-10-01T10:00:00Z";
const STAGED_AT = "2026-10-06T09:14:02Z";

/** The queue as the stubbed daemon holds it now: a test or a write changes it. */
interface Queue {
  proposal: Proposal;
}

const PR_0004 = aProposal({
  id: "PR-0004",
  severity: "high",
  kind: "discrepancy",
  summary: "Stamina cap",
  target_ids: ["R-1"],
  options: [
    { label: "Code to spec", effect: "Fix the code", price: "One test" },
    { label: "Spec to code", effect: "Edit the rule", price: "Balance changes" },
    { label: "Keep the cap", effect: "Both stay", price: "A note" },
  ],
  recommendation: 0,
});

/** The daemon: alpha's inbox and PR-0004 from `queue`; the decision POST and DELETE as `write` answers them. */
function daemon(queue: Queue, write: (method: string, body: unknown) => Response) {
  return stubFetch((url, init) => {
    if (url === "/api/projects") {
      return jsonAnswer(200, [{ slug: "alpha", name: "Alpha", root: "/work/alpha", branch: "main" }]);
    }
    if (url === "/api/projects/alpha/inbox") {
      return jsonAnswer(200, { proposals: [entryOf(queue.proposal)], notes: [] });
    }
    if (url === "/api/projects/alpha/proposals/PR-0004") {
      return jsonAnswer(200, queue.proposal);
    }
    if (url === "/api/projects/alpha/proposals/PR-0004/decision") {
      const body: unknown = typeof init?.body === "string" ? JSON.parse(init.body) : null;
      return write(init?.method ?? "GET", body);
    }
    if (url === "/api/projects/alpha/nodes/R-1") {
      return jsonAnswer(200, { ref: "R-1", reason: null, notes: [], nodes: [] });
    }
    return errorAnswer(404, `no route ${url}`);
  });
}

/** The stage a POST's body holds: the body less `updated_at`, plus the `span_hash` the daemon reads (none here). */
function stageOf(body: unknown): Stage {
  const stage = new Map(Object.entries(body as Record<string, unknown>));
  stage.delete("updated_at");
  return { ...Object.fromEntries(stage), span_hash: null } as unknown as Stage;
}

/** A write the daemon stores: the stage (or none), its time, the new `updated_at`. */
function store(queue: Queue, staged: Stage | null): Response {
  queue.proposal = { ...queue.proposal, staged, staged_at: staged === null ? null : STAGED_AT, updated_at: STAGED_AT };
  return jsonAnswer(200, queue.proposal);
}

function stream(): FakeEventSource {
  const source = FakeEventSource.instances.filter((candidate) => candidate.readyState !== 2).at(-1);
  if (source === undefined) {
    throw new Error("no open stream");
  }
  return source;
}

/** The Inbox on PR-0004 over the daemon, its review read, alpha's stream open; the fetch stub. */
async function openInbox(queue: Queue, write: (method: string, body: unknown) => Response) {
  const fetchStub = daemon(queue, write);
  renderApp(new HttpClient(), "#/alpha/inbox/PR-0004");
  const card = await screen.findByRole("article");
  await within(card).findByRole("heading", { level: 3, name: "Provenance" });
  await waitFor(() => {
    expect(FakeEventSource.instances.map((source) => source.url)).toEqual(["/api/projects/alpha/events"]);
  });
  act(() => {
    stream().open();
  });
  return { fetchStub, card };
}

/** The requests made since `from`, as `METHOD url`. */
function requests(fetchStub: ReturnType<typeof stubFetch>, from = 0): string[] {
  const urls = urlsOf(fetchStub);
  return fetchStub.mock.calls.slice(from).map((call, index) => `${call[1]?.method ?? "GET"} ${urls[from + index] ?? ""}`);
}

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("staging over the daemon (AC-14 of decision-staging)", () => {
  it("accept, option [2], a note: one POST, its body exactly the stage plus updated_at; then only the inbox and PR-0004 are read again", async () => {
    const queue: Queue = { proposal: PR_0004 };
    const { fetchStub, card } = await openInbox(queue, (method, body) => store(queue, method === "POST" ? stageOf(body) : null));
    const before = fetchStub.mock.calls.length;
    fireEvent.click(within(card).getByRole("button", { name: "Accept" }));
    const dialog = await screen.findByRole("dialog", { name: "Accept PR-0004" });
    fireEvent.click(within(dialog).getByRole("radio", { name: /^\[2\] Keep the cap/ }));
    fireEvent.change(within(dialog).getByLabelText("Note for the record (optional)"), { target: { value: "keep the cap" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage accept" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    const posts = fetchStub.mock.calls.filter((call) => call[1]?.method === "POST");
    expect(posts).toHaveLength(1);
    expect(posts[0]?.[1]?.body).toBe(`{"decision":"approve","option":2,"answer":null,"canon":null,"note":"keep the cap","updated_at":"${READ_AT}"}`);
    expect(posts[0]?.[1] !== undefined && "signal" in posts[0][1]).toBe(false);
    expect(queue.proposal.staged).toEqual({ decision: "approve", option: 2, answer: null, canon: null, note: "keep the cap", span_hash: null });
    const staged = await within(screen.getByRole("article")).findByRole("region", { name: "Staged decision" });
    expect(staged.querySelector(".staged-line")?.textContent).toBe(`Staged ${STAGED_AT}. Confirm on a terminal: spec approve PR-0004`);
    expect(within(screen.getByRole("listbox")).getAllByRole("option").map((option) => option.dataset.proposal)).toEqual(["PR-0004"]);
    await waitFor(() => {
      expect(requests(fetchStub, before).sort()).toEqual([
        "GET /api/projects/alpha/inbox",
        "GET /api/projects/alpha/proposals/PR-0004",
        "POST /api/projects/alpha/proposals/PR-0004/decision",
      ]);
    });

    // Its event on the live tail: this tab's own, no alert; the inbox and PR-0004 read once more.
    const afterStage = fetchStub.mock.calls.length;
    act(() => {
      stream().emit("proposal.staged", JSON.stringify({ id: "PR-0004", staged: queue.proposal.staged, staged_at: STAGED_AT }), "70");
    });
    await waitFor(() => {
      expect(requests(fetchStub, afterStage).sort()).toEqual(["GET /api/projects/alpha/inbox", "GET /api/projects/alpha/proposals/PR-0004"]);
    });
    expect(screen.queryByText(/outside this tab/)).toBeNull();

    // Unstage: one DELETE, no body.
    const afterEvent = fetchStub.mock.calls.length;
    fireEvent.click(within(staged).getByRole("button", { name: "Unstage PR-0004" }));
    await waitFor(() => {
      expect(within(screen.getByRole("article")).queryByRole("region", { name: "Staged decision" })).toBeNull();
    });
    const deletes = fetchStub.mock.calls.filter((call) => call[1]?.method === "DELETE");
    expect(deletes.map((call) => call[1]?.body)).toEqual([undefined]);
    await waitFor(() => {
      expect(requests(fetchStub, afterEvent).sort()).toEqual([
        "DELETE /api/projects/alpha/proposals/PR-0004/decision",
        "GET /api/projects/alpha/inbox",
        "GET /api/projects/alpha/proposals/PR-0004",
      ]);
    });
  });

  it("a 409 carrying the review document: the dialog shows its last note, never the JSON; the inbox and PR-0004 read again", async () => {
    const queue: Queue = { proposal: PR_0004 };
    const reason = "`PR-0004` changed since it was read: read it again; nothing changed";
    const { fetchStub, card } = await openInbox(queue, () => {
      queue.proposal = { ...queue.proposal, staged: { decision: "reject", reason: "elsewhere" }, staged_at: "2026-10-06T09:00:00Z", updated_at: "2026-10-06T09:00:00Z" };
      return jsonAnswer(409, { ...queue.proposal, notes: [reason] });
    });
    fireEvent.click(within(card).getByRole("button", { name: "Reject" }));
    const dialog = await screen.findByRole("dialog", { name: "Reject PR-0004" });
    fireEvent.change(within(dialog).getByLabelText("Reason (required)"), { target: { value: "dup" } });
    const before = fetchStub.mock.calls.length;
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage reject" }));
    const refusal = (await within(dialog).findByText(reason)).closest('[role="alert"]');
    expect(refusal?.textContent).toBe(`The daemon refused to stage it; nothing changed.${reason}`);
    expect(dialog.textContent).not.toContain('"notes"');
    expect(await within(dialog).findByText("This proposal changed since you opened it")).toBeTruthy();
    await waitFor(() => {
      expect(requests(fetchStub, before).sort()).toEqual([
        "GET /api/projects/alpha/inbox",
        "GET /api/projects/alpha/proposals/PR-0004",
        "POST /api/projects/alpha/proposals/PR-0004/decision",
      ]);
    });
  });

  it("a stage made outside this tab: its event reads the inbox and PR-0004 again, and an alert says so", async () => {
    const queue: Queue = { proposal: PR_0004 };
    const { fetchStub } = await openInbox(queue, () => errorAnswer(503, "unused"));
    const before = fetchStub.mock.calls.length;
    queue.proposal = { ...queue.proposal, staged: { decision: "reject", reason: "injected" }, staged_at: STAGED_AT, updated_at: STAGED_AT };
    act(() => {
      stream().emit("proposal.staged", JSON.stringify({ id: "PR-0004", staged: queue.proposal.staged, staged_at: STAGED_AT }), "80");
    });
    const alert = (await screen.findByText("The staged decision on PR-0004 changed outside this tab")).closest('[role="alert"]');
    expect(alert?.textContent).toContain(STAGED_AT);
    const staged = await within(screen.getByRole("article")).findByRole("region", { name: "Staged decision" });
    expect(within(staged).getByText("injected")).toBeTruthy();
    await waitFor(() => {
      expect(requests(fetchStub, before).sort()).toEqual(["GET /api/projects/alpha/inbox", "GET /api/projects/alpha/proposals/PR-0004"]);
    });
  });
});
