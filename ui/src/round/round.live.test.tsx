import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { HttpClient } from "../api/http";
import type { RoundAnswersOutcome, RoundSheet, Stage } from "../api/types";
import { aProposal, aRoundItem, aRoundSheet } from "../test/builders";
import { errorAnswer, FakeEventSource, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { renderApp } from "../test/render";

// U-04 and U-06 of docs/features/ui-round.md over HttpClient, the daemon a stubbed `fetch` and
// `EventSource`: Stage is one POST whose body is exactly W-1 and W-2's (`{"project","answers"}`, each
// stage the decision route's body); the POST's N `proposal.staged` events, which may come before
// its answer, and an Unstage made here raise no alert; a stage made on a terminal does.

const Q_READ = "2026-10-07T17:49:50Z";
const D_READ = "2026-10-07T17:40:02Z";
const STAGED_AT = "2026-10-08T09:12:40Z";

const OWN_ANSWER: Stage = { decision: "approve", option: null, answer: "no, 2 s", canon: null, editorial: null, note: null, span_hash: null };
const REJECT: Stage = { decision: "reject", reason: "dup" };

function sheet(): RoundSheet {
  return aRoundSheet(
    [
      aRoundItem({ id: "PR-0003", working_answer: "yes, 1.5 s", updated_at: Q_READ }),
      aRoundItem({
        id: "PR-0005",
        kind: "discrepancy",
        options: [
          { label: "Code to spec", effect: "Fix the code", price: "One test" },
          { label: "Spec to code", effect: "Edit the rule", price: "A note" },
        ],
        recommendation: 0,
        updated_at: D_READ,
      }),
    ],
    { project: "example" },
  );
}

/** A write's answer, held until the test lets it go. */
interface Held {
  release: (response: Response) => void;
}

/** The daemon: one project, its round sheet from `queue`; the round POST and the decision DELETE answered by `write`. */
function daemon(queue: { sheet: RoundSheet }, write: (method: string, url: string) => Response | Promise<Response>) {
  return stubFetch((url, init) => {
    const method = init?.method ?? "GET";
    if (url === "/api/projects") {
      return jsonAnswer(200, [{ slug: "example", name: "Example", root: "/work/example", branch: "main" }]);
    }
    if (url === "/api/projects/example/round" && method === "GET") {
      return jsonAnswer(200, queue.sheet);
    }
    if (url === "/api/projects/example/round/answers" || url.endsWith("/decision")) {
      return write(method, url);
    }
    return errorAnswer(404, `no route ${url}`);
  });
}

function hold(): { held: Held; answer: Promise<Response> } {
  let release: (response: Response) => void = () => undefined;
  const answer = new Promise<Response>((resolve) => {
    release = resolve;
  });
  return { held: { release }, answer };
}

function stream(): FakeEventSource {
  const source = FakeEventSource.instances.filter((candidate) => candidate.readyState !== 2).at(-1);
  if (source === undefined) {
    throw new Error("no open stream");
  }
  return source;
}

function emit(type: string, payload: unknown, seq: number) {
  act(() => {
    stream().emit(type, JSON.stringify(payload), String(seq));
  });
}

async function openQuestions() {
  renderApp(new HttpClient(), "#/example/questions");
  await screen.findByRole("article", { name: "PR-0005" });
  await waitFor(() => {
    expect(FakeEventSource.instances.map((source) => source.url)).toEqual(["/api/projects/example/events"]);
  });
  act(() => {
    stream().open();
  });
}

function card(id: string): HTMLElement {
  return screen.getByRole("article", { name: id });
}

/** The answers W-1 and W-2 show: PR-0003 an own answer, PR-0005 a reject. */
function answerAsTheSpec() {
  fireEvent.click(within(card("PR-0003")).getByRole("radio", { name: "Your answer" }));
  fireEvent.change(within(card("PR-0003")).getByLabelText("Your answer to PR-0003"), { target: { value: "no, 2 s" } });
  fireEvent.click(within(card("PR-0005")).getByRole("radio", { name: "Reject with a reason" }));
  fireEvent.change(within(card("PR-0005")).getByLabelText("Reason for rejecting PR-0005"), { target: { value: "dup" } });
}

function stagedDocument(): RoundAnswersOutcome {
  return {
    project: "example",
    answers: [
      { id: "PR-0003", outcome: "staged", stage: OWN_ANSWER, staged_at: STAGED_AT, replaced: null, reason: null },
      { id: "PR-0005", outcome: "staged", stage: REJECT, staged_at: STAGED_AT, replaced: null, reason: null },
    ],
    notes: [],
  };
}

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("the round POST over the daemon (U-04)", () => {
  it("sends exactly W-1 and W-2's body: the project, then each answer's id and stage in sheet order", async () => {
    const queue = { sheet: sheet() };
    const fetchStub = daemon(queue, () => jsonAnswer(200, stagedDocument()));
    await openQuestions();
    answerAsTheSpec();
    fireEvent.click(screen.getByRole("button", { name: "Stage 2 answers" }));
    await waitFor(() => {
      expect(fetchStub.mock.calls.filter((call) => call[1]?.method === "POST")).toHaveLength(1);
    });
    const post = fetchStub.mock.calls.find((call) => call[1]?.method === "POST");
    expect(urlsOf(fetchStub).filter((url) => url.endsWith("/answers"))).toEqual(["/api/projects/example/round/answers"]);
    expect(post?.[1]?.body).toBe(
      '{"project":"example","answers":[' +
        `{"id":"PR-0003","stage":{"decision":"approve","option":null,"answer":"no, 2 s","canon":null,"editorial":null,"note":null,"updated_at":"${Q_READ}"}},` +
        `{"id":"PR-0005","stage":{"decision":"reject","reason":"dup","updated_at":"${D_READ}"}}]}`,
    );
    expect(post?.[1]?.headers).toEqual({ Accept: "application/json", "Content-Type": "application/json" });
    expect(post?.[1] !== undefined && "signal" in post[1]).toBe(false);
  });
});

describe("this tab's own writes (U-06)", () => {
  it("raises no alert for the POST's N events, even when they come before its answer", async () => {
    const queue = { sheet: sheet() };
    const { held, answer } = hold();
    const fetchStub = daemon(queue, () => answer);
    await openQuestions();
    answerAsTheSpec();
    fireEvent.click(screen.getByRole("button", { name: "Stage 2 answers" }));
    await waitFor(() => {
      expect(fetchStub.mock.calls.some((call) => call[1]?.method === "POST")).toBe(true);
    });
    // The daemon stores both and logs their events; the tail delivers them before the POST answers.
    emit("proposal.staged", { id: "PR-0003", staged: OWN_ANSWER, staged_at: STAGED_AT }, 41);
    emit("proposal.staged", { id: "PR-0005", staged: REJECT, staged_at: STAGED_AT }, 42);
    await waitFor(() => {
      expect(urlsOf(fetchStub).filter((url) => url === "/api/projects/example/round").length).toBeGreaterThan(1);
    });
    expect(screen.queryByText(/outside this tab/)).toBeNull();
    act(() => {
      held.release(jsonAnswer(200, stagedDocument()));
    });
    await waitFor(() => {
      expect(document.querySelector(".decision-result p")?.textContent).toMatch(/^Staged 2 answers\. Nothing is applied yet\./);
    });
    expect(screen.queryByText(/outside this tab/)).toBeNull();
  });

  it("raises no alert for an Unstage made here, tracked from the stage the sheet shows", async () => {
    const queue = {
      sheet: aRoundSheet([aRoundItem({ id: "PR-0005", staged: REJECT, staged_at: STAGED_AT, updated_at: STAGED_AT })], { project: "example" }),
    };
    const { held, answer } = hold();
    const fetchStub = daemon(queue, (method) => (method === "DELETE" ? answer : errorAnswer(405, "unused")));
    await openQuestions();
    const staged = await within(card("PR-0005")).findByRole("region", { name: "Staged decision" });
    fireEvent.click(within(staged).getByRole("button", { name: "Unstage PR-0005" }));
    await waitFor(() => {
      expect(fetchStub.mock.calls.filter((call) => call[1]?.method === "DELETE").map((call) => call[0])).toEqual([
        "/api/projects/example/proposals/PR-0005/decision",
      ]);
    });
    emit("proposal.unstaged", { id: "PR-0005" }, 51);
    queue.sheet = aRoundSheet([aRoundItem({ id: "PR-0005", updated_at: "2026-10-08T09:13:00Z" })], { project: "example" });
    act(() => {
      held.release(jsonAnswer(200, aProposal({ id: "PR-0005", project: "example", kind: "discrepancy", staged: null, staged_at: null })));
    });
    await waitFor(() => {
      expect(within(card("PR-0005")).queryByRole("region", { name: "Staged decision" })).toBeNull();
    });
    expect(screen.queryByText(/outside this tab/)).toBeNull();
    await waitFor(() => {
      expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe("Unstaged the answer on PR-0005; nothing is staged for it now.");
    });
  });

  it("raises the alert for a stage made on a terminal, and for one this tab did not send after its own", async () => {
    const queue = { sheet: sheet() };
    daemon(queue, () => jsonAnswer(200, stagedDocument()));
    await openQuestions();
    answerAsTheSpec();
    fireEvent.click(screen.getByRole("button", { name: "Stage 2 answers" }));
    await waitFor(() => {
      expect(document.querySelector(".decision-result p")?.textContent).toMatch(/^Staged 2 answers\./);
    });
    emit("proposal.staged", { id: "PR-0003", staged: OWN_ANSWER, staged_at: STAGED_AT }, 61);
    emit("proposal.staged", { id: "PR-0005", staged: REJECT, staged_at: STAGED_AT }, 62);
    expect(screen.queryByText(/outside this tab/)).toBeNull();
    emit("proposal.staged", { id: "PR-0005", staged: { decision: "approve", option: 1, answer: null, canon: null, editorial: null, note: null, span_hash: null }, staged_at: "2026-10-08T09:20:00Z" }, 63);
    const alert = (await screen.findByText("The staged decision on PR-0005 changed outside this tab")).closest('[role="alert"]');
    expect(alert?.textContent).toContain("2026-10-08T09:20:00Z");
  });
});
