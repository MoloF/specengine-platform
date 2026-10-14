import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import appCss from "../styles/app.css?raw";
import { ClientError } from "../api/client";
import type { QueueEvent, RoundAnswerBody, RoundAnswersOutcome, RoundItem, RoundSheet } from "../api/types";
import { aRoundItem, aRoundSheet } from "../test/builders";
import { renderApp } from "../test/render";
import { STAGED_AT, stubClient, type StubClient } from "../test/stubClient";

// docs/features/ui-round.md U-01 to U-10 on the Questions screen over a stub client whose live tail
// a test drives by hand: the states, the sheet as sent, controls by structure with nothing picked,
// one POST of the decision route's bodies, drafts by ID across re-reads and sections, the round's
// terminal command, UTF-8 byte counters, agent text escaped and isolated, the sheet read again on
// every proposal or task event (a superseded read aborted). Iteration 2 of the review: a pick the
// revised item no longer offers is never sent; a POST on its way when the owner leaves and comes
// back is neither sent again nor lost; the sticky Stage bar stays short with the page's scroll
// padding reserving it; a staged option's label is escaped. Non-Latin text is built from code
// points, so this file stays ASCII. Own writes over the daemon: round.live.test.tsx.

const T0 = "2026-10-01T10:00:00Z";
const T1 = "2026-10-02T08:00:00Z";
const RLO = String.fromCodePoint(0x202e);
const ZHE = String.fromCodePoint(0x416);

const OPTIONS = [
  { label: "Code to spec", effect: "Fix the code", price: "One test" },
  { label: "Spec to code", effect: "Edit the rule", price: "Balance changes" },
  { label: "Keep both", effect: "Both stay", price: "A note" },
];

/** The stub's live tail, driven by the test: `emit` a queue event, `gap` a re-opened stream. */
function liveTail(client: StubClient) {
  let deliver: ((event: QueueEvent) => void) | null = null;
  let reopened: (() => void) | null = null;
  let seq = 0;
  client.subscribe = (_project, onEvent, onGap) => {
    deliver = onEvent;
    reopened = onGap ?? null;
    return () => {
      deliver = null;
      reopened = null;
    };
  };
  return {
    emit(type: string, payload: unknown) {
      seq += 1;
      const event = { seq, type, payload };
      act(() => {
        deliver?.(event);
      });
    },
    gap() {
      act(() => {
        reopened?.();
      });
    },
    open: () => deliver !== null,
  };
}

/** A stub client serving `sheet`, its live tail driven by hand. */
function serving(sheet: RoundSheet) {
  const client = stubClient();
  client.state.round = sheet;
  const tail = liveTail(client);
  return { client, tail };
}

async function openQuestions(client: StubClient) {
  renderApp(client, "#/alpha/questions");
  await screen.findByRole("heading", { level: 1, name: "Questions" });
}

function card(id: string): HTMLElement {
  return screen.getByRole("article", { name: id });
}

function cardIds(): string[] {
  return screen.getAllByRole("article").map((article) => article.querySelector("h2")?.textContent ?? "");
}

function stageButton(): HTMLElement {
  return screen.getByRole("button", { name: /^(Stage \d+ answers?|Staging)$/ });
}

function checkedIn(id: string): string[] {
  return within(card(id))
    .getAllByRole("radio")
    .filter((radio) => (radio as HTMLInputElement).checked)
    .map((radio) => radio.closest("label")?.textContent ?? "");
}

/** The answers of the stub's round POSTs, as sent. */
function posted(client: StubClient): RoundAnswerBody[][] {
  return client.stageRoundAnswers.mock.calls.map(([, answers]) => answers);
}

const clipboard = Object.getOwnPropertyDescriptor(navigator, "clipboard");

afterEach(() => {
  if (clipboard === undefined) {
    Reflect.deleteProperty(navigator, "clipboard");
  } else {
    Object.defineProperty(navigator, "clipboard", clipboard);
  }
});

describe("loading, empty, error (U-01)", () => {
  it("loading: a skeleton, busy, while the sheet is read", async () => {
    const client = stubClient();
    client.getRound.mockImplementation(() => new Promise<RoundSheet>(() => undefined));
    await openQuestions(client);
    const skeleton = await screen.findByRole("status", { name: "Loading the question round of alpha" });
    expect(skeleton.getAttribute("aria-busy")).toBe("true");
    expect(screen.getByRole("heading", { level: 1, name: "Questions" }).closest("section")?.getAttribute("aria-busy")).toBe("true");
  });

  it("empty: what it means, the daemon's notes verbatim, the next step", async () => {
    const note = "1 open change proposal(s) not on this sheet: PR-0002; spec inbox lists them";
    const { client } = serving(aRoundSheet([], { notes: [note] }));
    await openQuestions(client);
    const empty = (await screen.findByRole("heading", { level: 2, name: "No open questions or discrepancies." })).closest(".empty-state");
    expect(empty).not.toBeNull();
    const notes = within(empty as HTMLElement).getByRole("list", { name: "Notes from the daemon" });
    expect(within(notes).getAllByRole("listitem").map((item) => item.textContent)).toEqual([note]);
    expect(within(empty as HTMLElement).getByRole("link", { name: "review the change proposals in the Inbox" }).getAttribute("href")).toBe("#/alpha/inbox");
    expect(screen.queryByRole("button", { name: /^Stage/ })).toBeNull();
  });

  it("error: the daemon's message verbatim in an alert with Retry; Retry reads the sheet again", async () => {
    const { client } = serving(aRoundSheet([aRoundItem({ id: "PR-0003" })]));
    const message = "spec: the queue is locked by another process; try again";
    client.getRound.mockRejectedValueOnce(new ClientError({ status: 503, message }));
    await openQuestions(client);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(`The question round could not be loaded${message}`);
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("article", { name: "PR-0003" })).toBeTruthy();
    expect(client.getRound).toHaveBeenCalledTimes(2);
  });
});

describe("the sheet as sent (U-02)", () => {
  it("keeps the daemon's order, never sorting again: low before high, no severity, then normal", async () => {
    const items = [
      aRoundItem({ id: "PR-0009", severity: "low", created_at: "2026-09-01T00:00:00Z" }),
      aRoundItem({ id: "PR-0002", severity: "high" }),
      aRoundItem({ id: "PR-0005", severity: null }),
      aRoundItem({ id: "PR-0001", severity: "normal", created_at: "2026-10-05T00:00:00Z" }),
    ];
    const { client } = serving(aRoundSheet(items, { minutes: 2 }));
    await openQuestions(client);
    await screen.findByRole("article", { name: "PR-0009" });
    expect(cardIds()).toEqual(["PR-0009", "PR-0002", "PR-0005", "PR-0001"]);
    expect(screen.getByText("4 items, about 2 min of reading")).toBeTruthy();
  });

  it("shows the text sheet's fields and links; no worktree, branch, gap type, author or evidence", async () => {
    const item = aRoundItem({
      id: "PR-0003",
      kind: "question",
      severity: "high",
      gap_type: "contradicts",
      summary: "Does regeneration wait for rest?\nIt resumes after 1.5 s.",
      working_answer: "yes, 1.5 s",
      price_of_other: "A second timer",
      evidence: [{ file: "src/stamina.rs", qpath: null, lines: "4", observed: "OBSERVED-X", documented: "DOCUMENTED-X" }],
      targets: [
        { id: "MEC-STAMINA", path: "docs/spec/stamina.md", title: "Stamina" },
        { id: null, path: "docs/spec/regen.md", title: null },
      ],
      tasks: [{ id: "T-0004", status: "ready", title: "Tune stamina" }],
      worktree: "/work/alpha/WORKTREE-X",
      branch: "BRANCH-X",
      author: { type: "agent", role: "ROLE-X", model: "MODEL-X", run: "RUN-X" },
    });
    const notes = ["2 held item(s) not on this sheet (approved: an apply in progress or stopped): PR-0010 and PR-0011; spec approve <ID> completes each"];
    const { client } = serving(aRoundSheet([item], { minutes: 1, notes }));
    await openQuestions(client);
    const article = await screen.findByRole("article", { name: "PR-0003" });
    expect(screen.getByText("1 item, about 1 min of reading")).toBeTruthy();
    expect(within(screen.getByRole("list", { name: "Notes from the daemon" })).getByRole("listitem").textContent).toBe(notes[0]);
    expect(article.querySelector(".kind-tag")?.textContent).toBe("question");
    expect(within(article).getByText("High")).toBeTruthy();
    expect(within(article).getByText(/^Does regeneration/).textContent).toBe("Does regeneration wait for rest?\nIt resumes after 1.5 s.");
    expect(within(article).getByText("Working answer").nextElementSibling?.textContent).toBe("yes, 1.5 s");
    expect(within(article).getByText("Another answer costs").nextElementSibling?.textContent).toBe("A second timer");
    expect(within(article).getByRole("link", { name: "Stamina (MEC-STAMINA)" }).getAttribute("href")).toBe("#/alpha/tree/MEC-STAMINA");
    expect(within(article).getByRole("link", { name: "docs/spec/regen.md" }).getAttribute("href")).toBe("#/alpha/tree/docs%2Fspec%2Fregen.md");
    expect(within(article).getByRole("link", { name: "Tune stamina (T-0004)" }).getAttribute("href")).toBe("#/alpha/tasks/T-0004");
    expect(within(article).getByRole("link", { name: "Open in Inbox: PR-0003" }).getAttribute("href")).toBe("#/alpha/inbox/PR-0003");
    for (const hidden of ["WORKTREE-X", "BRANCH-X", "contradicts", "Contradicts", "ROLE-X", "MODEL-X", "OBSERVED-X", "DOCUMENTED-X", "src/stamina.rs"]) {
      expect([hidden, article.textContent.includes(hidden)]).toEqual([hidden, false]);
    }
  });

  it("Show all reads the items past the limit: limit = items + more, at most 200", async () => {
    const { client } = serving(aRoundSheet([aRoundItem({ id: "PR-0001" })], { more: 4 }));
    await openQuestions(client);
    fireEvent.click(await screen.findByRole("button", { name: "Show all 5" }));
    await waitFor(() => {
      expect(client.getRound.mock.calls.map((call) => call[1])).toEqual([undefined, { limit: 5 }]);
    });
  });

  it("caps Show all at the 200 one sheet holds", async () => {
    const { client } = serving(aRoundSheet([aRoundItem({ id: "PR-0001" })], { more: 300 }));
    await openQuestions(client);
    fireEvent.click(await screen.findByRole("button", { name: "Show 200, the most one sheet holds" }));
    await waitFor(() => {
      expect(client.getRound.mock.calls.map((call) => call[1])).toEqual([undefined, { limit: 200 }]);
    });
  });
});

describe("controls by structure, nothing picked (U-03)", () => {
  it("gives an unlisted kind with options its options, Reject and Leave open; none picked, the recommendation only marked", async () => {
    const riddle = aRoundItem({ id: "PR-0005", kind: "riddle", options: OPTIONS, recommendation: 2, working_answer: "Both stay for now" });
    const { client } = serving(aRoundSheet([riddle]));
    await openQuestions(client);
    const article = await screen.findByRole("article", { name: "PR-0005" });
    const radios = within(article).getAllByRole("radio");
    expect(radios.map((radio) => radio.closest("label")?.textContent)).toEqual([
      "[0] Code to specFix the codePrice: One test",
      "[1] Spec to codeEdit the rulePrice: Balance changes",
      "[2] Keep bothRecommendedBoth stayPrice: A note",
      "Reject with a reason",
      "Leave open",
    ]);
    expect(checkedIn("PR-0005")).toEqual(["Leave open"]);
    expect(within(article).queryByRole("radio", { name: "Take the working answer" })).toBeNull();
    expect(stageButton().getAttribute("aria-disabled")).toBe("true");
    expect(stageButton().textContent).toBe("Stage 0 answers");
  });

  it("gives an item without options the working answer (when given), Your answer, Reject and Leave open, whatever its kind", async () => {
    const items = [
      aRoundItem({ id: "PR-0003", kind: "discrepancy", working_answer: "yes, 1.5 s" }),
      aRoundItem({ id: "PR-0004", kind: "question", working_answer: null }),
    ];
    const { client } = serving(aRoundSheet(items));
    await openQuestions(client);
    const first = await screen.findByRole("article", { name: "PR-0003" });
    expect(within(first).getAllByRole("radio").map((radio) => radio.closest("label")?.textContent)).toEqual([
      "Take the working answer",
      "Your answer",
      "Reject with a reason",
      "Leave open",
    ]);
    expect(within(card("PR-0004")).getAllByRole("radio").map((radio) => radio.closest("label")?.textContent)).toEqual([
      "Your answer",
      "Reject with a reason",
      "Leave open",
    ]);
    expect([...checkedIn("PR-0003"), ...checkedIn("PR-0004")]).toEqual(["Leave open", "Leave open"]);
  });

  it("stages option N on one click of it: the recommended one too is staged only when clicked", async () => {
    const riddle = aRoundItem({ id: "PR-0005", kind: "riddle", options: OPTIONS, recommendation: 2 });
    const { client } = serving(aRoundSheet([riddle]));
    await openQuestions(client);
    const article = await screen.findByRole("article", { name: "PR-0005" });
    fireEvent.click(within(article).getByRole("radio", { name: /^\[2\] Keep both/ }));
    expect(stageButton().textContent).toBe("Stage 1 answer");
    fireEvent.click(stageButton());
    await waitFor(() => {
      expect(posted(client)).toEqual([[{ id: "PR-0005", stage: { decision: "approve", option: 2, answer: null, canon: null, editorial: null, note: null, updated_at: T0 } }]]);
    });
  });
});

describe("one POST; refused drafts stay, staged ones clear (U-04)", () => {
  const sheet = () =>
    aRoundSheet([
      aRoundItem({ id: "PR-0003", working_answer: "yes, 1.5 s" }),
      aRoundItem({ id: "PR-0005", kind: "discrepancy", options: OPTIONS, recommendation: 0 }),
      aRoundItem({ id: "PR-0007" }),
      aRoundItem({ id: "PR-0008" }),
    ]);

  it("sends every ready answer in one call, in sheet order, as the decision route's bodies; left-open items are not sent", async () => {
    const { client } = serving(sheet());
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0007" })).getByRole("radio", { name: "Your answer" }));
    fireEvent.change(within(card("PR-0007")).getByLabelText("Your answer to PR-0007"), { target: { value: "no,\n2 s" } });
    fireEvent.click(within(card("PR-0005")).getByRole("radio", { name: "Reject with a reason" }));
    fireEvent.change(within(card("PR-0005")).getByLabelText("Reason for rejecting PR-0005"), { target: { value: "dup" } });
    fireEvent.click(within(card("PR-0003")).getByRole("radio", { name: "Take the working answer" }));
    expect(stageButton().textContent).toBe("Stage 3 answers");
    fireEvent.click(stageButton());
    fireEvent.click(stageButton());
    await waitFor(() => {
      expect(client.stageRoundAnswers).toHaveBeenCalledTimes(1);
    });
    expect(client.stageRoundAnswers.mock.calls[0]?.[0]).toBe("alpha");
    expect(JSON.stringify(posted(client)[0])).toBe(
      `[{"id":"PR-0003","stage":{"decision":"approve","option":null,"answer":null,"canon":null,"editorial":null,"note":null,"updated_at":"${T0}"}},` +
        `{"id":"PR-0005","stage":{"decision":"reject","reason":"dup","updated_at":"${T0}"}},` +
        `{"id":"PR-0007","stage":{"decision":"approve","option":null,"answer":"no,\\n2 s","canon":null,"editorial":null,"note":null,"updated_at":"${T0}"}}]`,
    );
  });

  it("keeps a refused draft with the daemon's reason verbatim and clears the staged ones; a polite summary says which", async () => {
    const { client } = serving(sheet());
    const reason = "`PR-0005` is rejected";
    client.stageRoundAnswers.mockImplementation((project: string, answers: RoundAnswerBody[]): Promise<RoundAnswersOutcome> => {
      client.state.round = {
        ...client.state.round,
        items: client.state.round.items.map((item) =>
          item.id === "PR-0003" ? { ...item, staged: { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null, span_hash: null }, staged_at: STAGED_AT, updated_at: STAGED_AT } : item,
        ),
      };
      return Promise.resolve({
        project,
        answers: answers.map(({ id }) =>
          id === "PR-0005"
            ? { id, outcome: "refused", stage: null, staged_at: null, replaced: null, reason }
            : { id, outcome: "staged", stage: { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null, span_hash: null }, staged_at: STAGED_AT, replaced: null, reason: null },
        ),
        notes: ["PR-0001: a corrupt row (the stage's shape); not listed"],
      });
    });
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0003" })).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(within(card("PR-0005")).getByRole("radio", { name: "Reject with a reason" }));
    fireEvent.change(within(card("PR-0005")).getByLabelText("Reason for rejecting PR-0005"), { target: { value: "dup" } });
    const reads = client.getRound.mock.calls.length;
    fireEvent.click(stageButton());
    const refusal = (await within(card("PR-0005")).findByText(reason)).closest(".refusal");
    expect(refusal?.textContent).toBe(`The daemon refused this answer; nothing was staged for it.${reason}`);
    expect(checkedIn("PR-0005")).toEqual(["Reject with a reason"]);
    expect(within(card("PR-0005")).getByLabelText("Reason for rejecting PR-0005")).toHaveProperty("value", "dup");
    const summary = "Staged 1 answer; 1 answer not staged: PR-0005. Nothing is applied yet. Confirm on a terminal: spec round answer --staged; each answer is asked again there, one at a time.";
    await waitFor(() => {
      expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe(summary);
    });
    expect(document.querySelector(".decision-result p")?.textContent).toBe(summary);
    expect(screen.getByRole("list", { name: "Notes from the daemon on the answers" }).textContent).toBe("PR-0001: a corrupt row (the stage's shape); not listed");
    // The sheet is read again: PR-0003 staged, its draft gone; PR-0005's kept and still ready.
    await waitFor(() => {
      expect(client.getRound.mock.calls.length).toBeGreaterThan(reads);
    });
    await within(card("PR-0003")).findByRole("region", { name: "Staged decision" });
    expect(checkedIn("PR-0003")).toEqual(["Leave open"]);
    expect(stageButton().textContent).toBe("Stage 1 answer");
  });

  it("names the picks that replace a staged choice", async () => {
    const staged = aRoundItem({
      id: "PR-0003",
      staged: { decision: "reject", reason: "earlier" },
      staged_at: "2026-10-07T17:49:50Z",
      updated_at: "2026-10-07T17:49:50Z",
    });
    const { client } = serving(aRoundSheet([staged, aRoundItem({ id: "PR-0004" })]));
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0003" })).getByRole("radio", { name: "Your answer" }));
    fireEvent.change(within(card("PR-0003")).getByLabelText("Your answer to PR-0003"), { target: { value: "no" } });
    expect(screen.getByText(/^Replaces the choice staged on/).textContent).toBe("Replaces the choice staged on PR-0003 (2026-10-07T17:49:50Z).");
  });

  it("a refusal of the whole POST: its message verbatim in an alert, every draft kept, nothing read again", async () => {
    const { client } = serving(sheet());
    const message = "spec: the answers are not `{\"project\",\"answers\":[{\"id\",\"stage\"}]}` as JSON: missing field `stage`; nothing changed";
    client.stageRoundAnswers.mockRejectedValue(new ClientError({ status: 400, message }));
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0008" })).getByRole("radio", { name: "Reject with a reason" }));
    fireEvent.change(within(card("PR-0008")).getByLabelText("Reason for rejecting PR-0008"), { target: { value: "dup" } });
    const reads = client.getRound.mock.calls.length;
    fireEvent.click(stageButton());
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(`The daemon refused the answers; nothing was staged. Your answers are kept.${message}`);
    expect(checkedIn("PR-0008")).toEqual(["Reject with a reason"]);
    expect(client.getRound.mock.calls.length).toBe(reads);
  });

  it("no answer at all (status 0): the sheet read again, every draft kept", async () => {
    const { client } = serving(sheet());
    client.stageRoundAnswers.mockRejectedValue(new ClientError({ status: 0, message: "POST /api/projects/alpha/round/answers: no response from the daemon (Failed to fetch)" }));
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0003" })).getByRole("radio", { name: "Take the working answer" }));
    const reads = client.getRound.mock.calls.length;
    fireEvent.click(stageButton());
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("No answer from the daemon: some answers may have been staged. The sheet is read again; your answers are kept.");
    expect(alert.textContent).toContain("no response from the daemon (Failed to fetch)");
    await waitFor(() => {
      expect(client.getRound.mock.calls.length).toBe(reads + 1);
    });
    expect(checkedIn("PR-0003")).toEqual(["Take the working answer"]);
  });
});

describe("drafts by ID (U-05)", () => {
  it("survive a re-read that reorders the sheet: each answer stays on its item", async () => {
    const a = aRoundItem({ id: "PR-0003", options: OPTIONS });
    const b = aRoundItem({ id: "PR-0005", options: OPTIONS });
    const { client, tail } = serving(aRoundSheet([a, b]));
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0005" })).getByRole("radio", { name: /^\[1\]/ }));
    client.state.round = aRoundSheet([b, a]);
    tail.emit("proposal.created", { id: "PR-0009" });
    await waitFor(() => {
      expect(cardIds()).toEqual(["PR-0005", "PR-0003"]);
    });
    expect(checkedIn("PR-0005")).toEqual(["[1] Spec to codeEdit the rulePrice: Balance changes"]);
    expect(checkedIn("PR-0003")).toEqual(["Leave open"]);
    fireEvent.click(stageButton());
    await waitFor(() => {
      expect(posted(client).map((answers) => answers.map(({ id, stage }) => [id, stage.decision === "approve" ? stage.option : null]))).toEqual([[["PR-0005", 1]]]);
    });
  });

  it("mark an answer whose item changed since and leave it out until picked again", async () => {
    const item = aRoundItem({ id: "PR-0005", options: OPTIONS });
    const { client, tail } = serving(aRoundSheet([item, aRoundItem({ id: "PR-0006" })]));
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0005" })).getByRole("radio", { name: /^\[0\]/ }));
    client.state.round = aRoundSheet([{ ...item, updated_at: T1, summary: "Revised" }, aRoundItem({ id: "PR-0006" })]);
    tail.emit("proposal.staged", { id: "PR-0005", staged: { decision: "reject", reason: "x" }, staged_at: T1 });
    expect(await within(card("PR-0005")).findByText("Changed since you answered")).toBeTruthy();
    expect(stageButton().textContent).toBe("Stage 0 answers");
    expect(screen.getByText(/^Left out, changed since you answered/).textContent).toBe("Left out, changed since you answered: PR-0005. Pick again to include them.");
    fireEvent.click(stageButton());
    expect(client.stageRoundAnswers).not.toHaveBeenCalled();
    fireEvent.click(within(card("PR-0005")).getByRole("button", { name: "Keep my answer for this version of PR-0005" }));
    expect(within(card("PR-0005")).queryByText("Changed since you answered")).toBeNull();
    fireEvent.click(stageButton());
    await waitFor(() => {
      expect(posted(client)[0]?.map(({ stage }) => stage.updated_at)).toEqual([T1]);
    });
  });

  it("mark a changed answer picked again by a click on its choice", async () => {
    const item = aRoundItem({ id: "PR-0005", options: OPTIONS });
    const { client, tail } = serving(aRoundSheet([item]));
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0005" })).getByRole("radio", { name: /^\[0\]/ }));
    client.state.round = aRoundSheet([{ ...item, updated_at: T1 }]);
    tail.emit("task.claimed", { id: "T-0004" });
    await within(card("PR-0005")).findByText("Changed since you answered");
    fireEvent.click(within(card("PR-0005")).getByRole("radio", { name: /^\[0\]/ }));
    expect(within(card("PR-0005")).queryByText("Changed since you answered")).toBeNull();
    expect(stageButton().textContent).toBe("Stage 1 answer");
  });

  it("drop the answer of an item no longer on the sheet, and say so", async () => {
    const { client, tail } = serving(aRoundSheet([aRoundItem({ id: "PR-0003" }), aRoundItem({ id: "PR-0005" })]));
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0005" })).getByRole("radio", { name: "Reject with a reason" }));
    fireEvent.change(within(card("PR-0005")).getByLabelText("Reason for rejecting PR-0005"), { target: { value: "dup" } });
    client.state.round = aRoundSheet([aRoundItem({ id: "PR-0003" })]);
    tail.emit("proposal.rejected", { id: "PR-0005", reason: "settled on a terminal" });
    const notice = (await screen.findByText("Your answer to PR-0005 was dropped")).closest('[role="status"]');
    expect(notice?.textContent).toContain("No longer on the sheet");
    // Back on the sheet later, it starts afresh: nothing kept.
    client.state.round = aRoundSheet([aRoundItem({ id: "PR-0003" }), aRoundItem({ id: "PR-0005" })]);
    tail.emit("proposal.created", { id: "PR-0005" });
    await screen.findByRole("article", { name: "PR-0005" });
    expect(checkedIn("PR-0005")).toEqual(["Leave open"]);
  });

  it("survive a switch to another section and back, the limit too; a reload (a new page) drops them", async () => {
    const { client } = serving(aRoundSheet([aRoundItem({ id: "PR-0003", working_answer: "yes" })], { more: 2 }));
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0003" })).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(screen.getByRole("button", { name: "Show all 3" }));
    await waitFor(() => {
      expect(client.getRound.mock.calls.map((call) => call[1])).toEqual([undefined, { limit: 3 }]);
    });
    act(() => {
      window.location.hash = "#/alpha/tasks";
    });
    await screen.findByRole("heading", { level: 1, name: "Tasks" });
    act(() => {
      window.location.hash = "#/alpha/questions";
    });
    await screen.findByRole("article", { name: "PR-0003" });
    expect(checkedIn("PR-0003")).toEqual(["Take the working answer"]);
    expect(client.getRound.mock.calls.at(-1)?.[1]).toEqual({ limit: 3 });
    expect(screen.getByText(/a reload drops them/)).toBeTruthy();
  });
});

describe("the terminal step (U-07)", () => {
  it("Copy takes exactly `spec round answer --staged`, the root shown, never copied; per-item commands only for PR-<4+ digits>", async () => {
    const writeText = vi.fn<(text: string) => Promise<void>>(() => Promise.resolve());
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    const stage = { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null, span_hash: null } as const;
    const items = [
      aRoundItem({ id: "PR-0003", staged: stage, staged_at: "2026-10-07T17:49:50Z" }),
      aRoundItem({ id: "PR-12", staged: { decision: "reject", reason: "dup" }, staged_at: "2026-10-07T17:50:00Z" }),
      aRoundItem({ id: "PR-0004" }),
    ];
    const { client } = serving(aRoundSheet(items));
    await openQuestions(client);
    const confirm = await screen.findByRole("region", { name: "Confirm on a terminal" });
    expect(confirm.querySelector(".round-confirm-line")?.textContent).toBe(
      "Confirm on a terminal in /work/alpha: spec round answer --staged; each answer is asked again there, one at a time.",
    );
    fireEvent.click(within(confirm).getByRole("button", { name: "Copy the command: spec round answer --staged" }));
    await waitFor(() => {
      expect(writeText.mock.calls).toEqual([["spec round answer --staged"]]);
    });
    const first = await within(card("PR-0003")).findByRole("region", { name: "Staged decision" });
    expect(first.querySelector(".staged-line")?.textContent).toBe("Staged 2026-10-07T17:49:50Z. Confirm on a terminal: spec approve PR-0003");
    expect(within(card("PR-0003")).getByText("Staged", { selector: ".staged-mark" })).toBeTruthy();
    const odd = await within(card("PR-12")).findByRole("region", { name: "Staged decision" });
    expect(odd.querySelector(".staged-line")?.textContent).toBe("Staged 2026-10-07T17:50:00Z. Confirm on a terminal: no command is offered for this ID.");
    expect(within(odd).queryByRole("button", { name: /^Copy/ })).toBeNull();
    expect(within(card("PR-0004")).queryByRole("region", { name: "Staged decision" })).toBeNull();
  });

  it("is absent while nothing on the sheet is staged", async () => {
    const { client } = serving(aRoundSheet([aRoundItem({ id: "PR-0004" })]));
    await openQuestions(client);
    await screen.findByRole("article", { name: "PR-0004" });
    expect(screen.queryByRole("region", { name: "Confirm on a terminal" })).toBeNull();
  });
});

describe("UTF-8 byte counters, never cut (U-08)", () => {
  it("counts 1 025 Cyrillic letters as 2050/2048 bytes, keeps them all and sends them all", async () => {
    const { client } = serving(aRoundSheet([aRoundItem({ id: "PR-0003" })]));
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0003" })).getByRole("radio", { name: "Your answer" }));
    const field = within(card("PR-0003")).getByLabelText("Your answer to PR-0003");
    const text = ZHE.repeat(1025);
    fireEvent.change(field, { target: { value: text } });
    expect(within(card("PR-0003")).getByText("2050/2048 bytes")).toBeTruthy();
    expect(field.getAttribute("maxlength")).toBeNull();
    expect(field.getAttribute("aria-invalid")).toBe("true");
    expect((field as HTMLTextAreaElement).value).toBe(text);
    fireEvent.click(stageButton());
    await waitFor(() => {
      expect(posted(client)[0]?.[0]?.stage).toMatchObject({ answer: text });
    });
  });

  it("counts a reason against its 4 096 bytes", async () => {
    const { client } = serving(aRoundSheet([aRoundItem({ id: "PR-0003" })]));
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0003" })).getByRole("radio", { name: "Reject with a reason" }));
    fireEvent.change(within(card("PR-0003")).getByLabelText("Reason for rejecting PR-0003"), { target: { value: ZHE.repeat(2048) } });
    expect(within(card("PR-0003")).getByText("4096/4096 bytes")).toBeTruthy();
    expect(within(card("PR-0003")).getByLabelText("Reason for rejecting PR-0003").getAttribute("aria-invalid")).toBe("false");
  });
});

describe("agent text verbatim, escaped, isolated (U-09)", () => {
  it("writes U+202E in a summary as \\u{202e} in an isolated paragraph; no raw override reaches the page", async () => {
    const item = aRoundItem({
      id: "PR-0003",
      summary: `Keep ${RLO}gnp.exe as is?\nline two`,
      options: [{ label: `left ${RLO}right`, effect: "e\nf", price: "p" }],
    });
    const { client } = serving(aRoundSheet([item]));
    await openQuestions(client);
    const article = await screen.findByRole("article", { name: "PR-0003" });
    const summary = within(article).getByText(/^Keep /);
    expect(summary.textContent).toBe("Keep \\u{202e}gnp.exe as is?\nline two");
    expect(summary.getAttribute("dir")).toBe("auto");
    const label = within(article).getByText("left \\u{202e}right");
    expect(label.tagName).toBe("BDI");
    expect(within(article).getByText("e\\nf").tagName).toBe("BDI");
    expect(document.body.textContent).not.toContain(RLO);
    expect(article.querySelector("strong, em, h3 + h4, a[href^='http']")).toBeNull();
  });

  it("shows markdown as the text it is", async () => {
    const { client } = serving(aRoundSheet([aRoundItem({ id: "PR-0003", summary: "**bold** and [a link](https://example.org)" })]));
    await openQuestions(client);
    const article = await screen.findByRole("article", { name: "PR-0003" });
    expect(within(article).getByText("**bold** and [a link](https://example.org)")).toBeTruthy();
    expect(article.querySelector("strong")).toBeNull();
    expect(within(article).queryByRole("link", { name: "a link" })).toBeNull();
  });
});

describe("live (U-10)", () => {
  it("reads the sheet again on every proposal and task event and on a gap", async () => {
    const { client, tail } = serving(aRoundSheet([aRoundItem({ id: "PR-0003" })]));
    await openQuestions(client);
    await screen.findByRole("article", { name: "PR-0003" });
    await waitFor(() => {
      expect(tail.open()).toBe(true);
    });
    const events: [string, unknown][] = [
      ["proposal.created", { id: "PR-0009" }],
      ["proposal.staged", { id: "PR-0003", staged: { decision: "reject", reason: "x" }, staged_at: T1 }],
      ["proposal.unstaged", { id: "PR-0003" }],
      ["proposal.applied", { id: "PR-0002", commit: "abc" }],
      ["task.claimed", { id: "T-0004" }],
      ["task.completed", { id: "T-0004" }],
    ];
    let reads = client.getRound.mock.calls.length;
    for (const [type, payload] of events) {
      tail.emit(type, payload);
      await waitFor(() => {
        expect([type, client.getRound.mock.calls.length]).toEqual([type, reads + 1]);
      });
      reads += 1;
    }
    tail.gap();
    await waitFor(() => {
      expect(client.getRound.mock.calls.length).toBe(reads + 1);
    });
  });

  it("aborts a read a newer event supersedes, and shows the newer one's answer", async () => {
    const { client, tail } = serving(aRoundSheet([aRoundItem({ id: "PR-0003" })]));
    const signals: AbortSignal[] = [];
    const answers: ((sheet: RoundSheet) => void)[] = [];
    client.getRound.mockImplementation((_project, _options, signal) => {
      if (signal !== undefined) {
        signals.push(signal);
      }
      if (signals.length === 1) {
        return Promise.resolve(aRoundSheet([aRoundItem({ id: "PR-0003" })]));
      }
      return new Promise<RoundSheet>((resolve) => {
        answers.push(resolve);
      });
    });
    await openQuestions(client);
    await screen.findByRole("article", { name: "PR-0003" });
    await waitFor(() => {
      expect(tail.open()).toBe(true);
    });
    tail.emit("proposal.created", { id: "PR-0009" });
    await waitFor(() => {
      expect(signals).toHaveLength(2);
    });
    tail.emit("task.planned", { id: "T-0004" });
    await waitFor(() => {
      expect(signals).toHaveLength(3);
    });
    expect(signals.map((signal) => signal.aborted)).toEqual([false, true, false]);
    const newest: RoundItem = aRoundItem({ id: "PR-0011" });
    act(() => {
      answers[1]?.(aRoundSheet([newest]));
    });
    expect(await screen.findByRole("article", { name: "PR-0011" })).toBeTruthy();
  });
});

/** Holds the stub's round POSTs until `release`; each then answers as the stub does. */
function holdPosts(client: StubClient) {
  const asUsual = client.stageRoundAnswers.getMockImplementation();
  if (asUsual === undefined) {
    throw new Error("the stub has no round POST");
  }
  let release: () => void = () => undefined;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  client.stageRoundAnswers.mockImplementation(async (project, answers) => {
    await gate;
    return asUsual(project, answers);
  });
  return {
    release() {
      act(() => {
        release();
      });
    },
  };
}

async function goTo(hash: string, heading: string) {
  act(() => {
    window.location.hash = hash;
  });
  await screen.findByRole("heading", { level: 1, name: heading });
}

describe("a pick the revised item no longer offers (review minor 1)", () => {
  it("an option past the revised item's options: marked changed, no Keep, never sent", async () => {
    const item = aRoundItem({ id: "PR-0005", options: OPTIONS });
    const { client, tail } = serving(aRoundSheet([item]));
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0005" })).getByRole("radio", { name: /^\[2\]/ }));
    client.state.round = aRoundSheet([{ ...item, options: OPTIONS.slice(0, 2), updated_at: T1 }]);
    tail.emit("proposal.created", { id: "PR-0009" });
    const notice = (await within(card("PR-0005")).findByText("Changed since you answered")).closest(".notice") as HTMLElement;
    expect(notice.textContent).toContain("no longer offers it");
    expect(within(notice).queryByRole("button")).toBeNull();
    expect(checkedIn("PR-0005")).toEqual(["Leave open"]);
    expect(stageButton().textContent).toBe("Stage 0 answers");
    fireEvent.click(stageButton());
    expect(client.stageRoundAnswers).not.toHaveBeenCalled();
  });

  it("an own answer on an item that has options now: no Keep, never sent; a pick of its options is", async () => {
    const item = aRoundItem({ id: "PR-0003" });
    const { client, tail } = serving(aRoundSheet([item]));
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0003" })).getByRole("radio", { name: "Your answer" }));
    fireEvent.change(within(card("PR-0003")).getByLabelText("Your answer to PR-0003"), { target: { value: "mine" } });
    client.state.round = aRoundSheet([{ ...item, options: OPTIONS, updated_at: T1 }]);
    tail.emit("task.refreshed", { id: "T-0004" });
    await within(card("PR-0003")).findByText("Changed since you answered");
    expect(within(card("PR-0003")).queryByRole("button", { name: /^Keep my answer/ })).toBeNull();
    expect(stageButton().textContent).toBe("Stage 0 answers");
    fireEvent.click(within(card("PR-0003")).getByRole("radio", { name: /^\[1\]/ }));
    fireEvent.click(stageButton());
    await waitFor(() => {
      expect(posted(client)).toEqual([[{ id: "PR-0003", stage: { decision: "approve", option: 1, answer: null, canon: null, editorial: null, note: null, updated_at: T1 } }]]);
    });
  });
});

describe("a POST on its way while the owner leaves and comes back (review minor 2)", () => {
  it("is not sent again from the screen mounted meanwhile, which shows its outcome and its drafts cleared once it settles", async () => {
    const { client } = serving(aRoundSheet([aRoundItem({ id: "PR-0003", working_answer: "yes" }), aRoundItem({ id: "PR-0004" })]));
    const held = holdPosts(client);
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0003" })).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(stageButton());
    await waitFor(() => {
      expect(client.stageRoundAnswers).toHaveBeenCalledTimes(1);
    });
    await goTo("#/alpha/tasks", "Tasks");
    await goTo("#/alpha/questions", "Questions");
    await screen.findByRole("article", { name: "PR-0003" });
    expect(checkedIn("PR-0003")).toEqual(["Take the working answer"]);
    expect([stageButton().textContent, stageButton().getAttribute("aria-disabled")]).toEqual(["Staging", "true"]);
    fireEvent.click(stageButton());
    expect(client.stageRoundAnswers).toHaveBeenCalledTimes(1);
    held.release();
    await waitFor(() => {
      expect(document.querySelector(".decision-result p")?.textContent).toMatch(/^Staged 1 answer\./);
    });
    await within(card("PR-0003")).findByRole("region", { name: "Staged decision" });
    expect(checkedIn("PR-0003")).toEqual(["Leave open"]);
    expect(stageButton().textContent).toBe("Stage 0 answers");
    // Away and back once more: nothing written back over the cleared drafts.
    await goTo("#/alpha/tasks", "Tasks");
    await goTo("#/alpha/questions", "Questions");
    await screen.findByRole("article", { name: "PR-0003" });
    expect(checkedIn("PR-0003")).toEqual(["Leave open"]);
    expect(client.stageRoundAnswers).toHaveBeenCalledTimes(1);
  });

  it("settled while the owner is away: the screen they come back to holds its outcome and no staged draft", async () => {
    const { client } = serving(aRoundSheet([aRoundItem({ id: "PR-0003", working_answer: "yes" })]));
    const held = holdPosts(client);
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0003" })).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(stageButton());
    await waitFor(() => {
      expect(client.stageRoundAnswers).toHaveBeenCalledTimes(1);
    });
    await goTo("#/alpha/tasks", "Tasks");
    held.release();
    // Said with no Questions screen on view: the words name the project.
    await waitFor(() => {
      expect(document.querySelector('[aria-live="polite"]')?.textContent).toMatch(/^Questions of alpha: Staged 1 answer\./);
    });
    await goTo("#/alpha/questions", "Questions");
    await within(await screen.findByRole("article", { name: "PR-0003" })).findByRole("region", { name: "Staged decision" });
    expect(checkedIn("PR-0003")).toEqual(["Leave open"]);
    expect(document.querySelector(".decision-result p")?.textContent).toMatch(/^Staged 1 answer\./);
    expect(stageButton().textContent).toBe("Stage 0 answers");
  });
});

describe("the sticky Stage bar (review minor 3)", () => {
  /** A rule's declarations in app.css, by its selector as written. */
  function rule(selector: string): string {
    const at = appCss.indexOf(`\n${selector} {`);
    expect([selector, at]).not.toEqual([selector, -1]);
    return appCss.slice(at, appCss.indexOf("}", at));
  }

  it("is capped in height, and the page keeps at least that much scroll padding at its foot while it is up", () => {
    const bar = rule(".round-stage");
    expect(bar).toContain("position: sticky;");
    const cap = /max-height: (\d+(?:\.\d+)?)rem;/.exec(bar)?.[1];
    expect(cap).toBeDefined();
    expect(bar).toContain("overflow-y: auto;");
    const padding = /scroll-padding-bottom: calc\((\d+(?:\.\d+)?)rem \+ var\(--space-\d\)\);/.exec(rule(":root:has(.round-stage)"))?.[1];
    expect(Number(padding)).toBeGreaterThanOrEqual(Number(cap));
  });

  it("holds the count, Stage and at most a one-line outcome: what a Stage replaces or leaves out, its failure and its outcome at length sit above it", async () => {
    const staged = aRoundItem({ id: "PR-0003", staged: { decision: "reject", reason: "x" }, staged_at: "2026-10-07T17:49:50Z" });
    const { client } = serving(aRoundSheet([staged, aRoundItem({ id: "PR-0004", options: OPTIONS }), aRoundItem({ id: "PR-0005" })]));
    client.stageRoundAnswers.mockRejectedValueOnce(new ClientError({ status: 503, message: "spec: the queue is locked" }));
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0003" })).getByRole("radio", { name: "Your answer" }));
    fireEvent.change(within(card("PR-0003")).getByLabelText("Your answer to PR-0003"), { target: { value: "no" } });
    fireEvent.click(within(card("PR-0005")).getByRole("radio", { name: "Reject with a reason" }));
    fireEvent.click(stageButton());
    const bar = screen.getByRole("region", { name: "Stage your answers" });
    expect(Array.from(bar.children).map((child) => child.tagName)).toEqual(["H2", "P", "BUTTON"]);
    fireEvent.click(stageButton());
    await screen.findByRole("alert");
    expect(Array.from(bar.children).map((child) => child.tagName)).toEqual(["H2", "P", "P", "BUTTON"]);
    expect(bar.textContent).not.toContain("spec: the queue is locked");
    const details = screen.getByRole("region", { name: "The answers to stage" });
    expect(within(details).getByRole("alert").textContent).toContain("spec: the queue is locked");
    expect(within(details).getByText(/^Replaces the choice staged on/)).toBeTruthy();
    expect(within(details).getByText(/^Left out, nothing written yet/)).toBeTruthy();
    expect(bar.compareDocumentPosition(details) & Node.DOCUMENT_POSITION_PRECEDING).toBeTruthy();
    // Staged at the second try: the outcome too sits above the bar, the bar still the count and Stage.
    fireEvent.click(stageButton());
    await waitFor(() => {
      expect(document.querySelector(".decision-result p")?.textContent).toMatch(/^Staged 1 answer\./);
    });
    const after = screen.getByRole("region", { name: "Stage your answers" });
    expect(Array.from(after.children).map((child) => child.tagName)).toEqual(["H2", "P", "P", "BUTTON"]);
    expect(after.textContent).not.toContain("Staged 1 answer.");
    expect(screen.getByRole("region", { name: "The answers to stage" }).querySelector(".decision-result")).not.toBeNull();
  });
});

describe("a staged option's label as the sheet writes it (review minor 4)", () => {
  it("writes U+202E in the staged option's label as \\u{202e}, isolated", async () => {
    const item = aRoundItem({
      id: "PR-0005",
      options: [{ label: `left ${RLO}right`, effect: "e", price: "p" }],
      staged: { decision: "approve", option: 0, answer: null, canon: null, editorial: null, note: null, span_hash: null },
      staged_at: "2026-10-07T17:49:50Z",
    });
    const { client } = serving(aRoundSheet([item]));
    await openQuestions(client);
    const staged = await within(await screen.findByRole("article", { name: "PR-0005" })).findByRole("region", { name: "Staged decision" });
    const label = within(staged).getByText("left \\u{202e}right");
    expect(label.tagName).toBe("BDI");
    expect(document.body.textContent).not.toContain(RLO);
  });
});

describe("review nits", () => {
  it("a new Stage clears the last outcome while it is on its way; the POST's notes are written as the sheet writes them", async () => {
    const { client } = serving(aRoundSheet([aRoundItem({ id: "PR-0003", working_answer: "yes" }), aRoundItem({ id: "PR-0004", working_answer: "yes" })]));
    const asUsual = client.stageRoundAnswers.getMockImplementation();
    if (asUsual === undefined) {
      throw new Error("the stub has no round POST");
    }
    client.stageRoundAnswers.mockImplementationOnce(async (project, answers) => ({ ...(await asUsual(project, answers)), notes: [`PR-0001: ${RLO}corrupt`] }));
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0003" })).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(stageButton());
    await waitFor(() => {
      expect(document.querySelector(".decision-result p")?.textContent).toMatch(/^Staged 1 answer\./);
    });
    expect(screen.getByRole("list", { name: "Notes from the daemon on the answers" }).textContent).toBe("PR-0001: \\u{202e}corrupt");
    expect(document.body.textContent).not.toContain(RLO);
    holdPosts(client);
    fireEvent.click(within(card("PR-0004")).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(stageButton());
    await waitFor(() => {
      expect(client.stageRoundAnswers).toHaveBeenCalledTimes(2);
    });
    expect(document.querySelector(".decision-result")).toBeNull();
  });

  it("Unstage does nothing while the last limit's sheet is kept up and another is read", async () => {
    const item = aRoundItem({ id: "PR-0003", staged: { decision: "reject", reason: "x" }, staged_at: "2026-10-07T17:49:50Z" });
    const { client } = serving(aRoundSheet([item], { more: 4 }));
    await openQuestions(client);
    const staged = await within(await screen.findByRole("article", { name: "PR-0003" })).findByRole("region", { name: "Staged decision" });
    client.getRound.mockImplementation(() => new Promise<RoundSheet>(() => undefined));
    fireEvent.click(screen.getByRole("button", { name: "Show all 5" }));
    await waitFor(() => {
      expect(screen.getByRole("list", { name: "Open questions and discrepancies, in round order" }).getAttribute("aria-busy")).toBe("true");
    });
    fireEvent.click(within(staged).getByRole("button", { name: "Unstage PR-0003" }));
    await act(async () => {
      await Promise.resolve();
    });
    expect(client.unstageDecision).not.toHaveBeenCalled();
  });
});

describe("the outcome where Stage was pressed (review iteration 3)", () => {
  /** The one-line outcome in the sticky bar, if any. */
  function outcome(): HTMLElement | null {
    return within(screen.getByRole("region", { name: "Stage your answers" })).queryByRole("button", { name: /below$/ });
  }

  it("a whole POST refused: the bar says so in one line, and its button brings the reason into view", async () => {
    const { client } = serving(aRoundSheet([aRoundItem({ id: "PR-0003", working_answer: "yes" })]));
    const message = "the body is over 4194304 bytes";
    client.stageRoundAnswers.mockRejectedValueOnce(new ClientError({ status: 413, message }));
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0003" })).getByRole("radio", { name: "Take the working answer" }));
    expect(outcome()).toBeNull();
    fireEvent.click(stageButton());
    const line = await waitFor(() => {
      const found = outcome();
      expect(found?.textContent).toBe("Not staged: see why below");
      return found as HTMLElement;
    });
    fireEvent.click(line);
    const details = screen.getByRole("region", { name: "The answers to stage" });
    expect(document.activeElement).toBe(details);
    expect(within(details).getByRole("alert").textContent).toContain(message);
  });

  it("no answer at all: the bar says the daemon did not answer", async () => {
    const { client } = serving(aRoundSheet([aRoundItem({ id: "PR-0003", working_answer: "yes" })]));
    client.stageRoundAnswers.mockRejectedValueOnce(new ClientError({ status: 0, message: "no response from the daemon (Failed to fetch)" }));
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0003" })).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(stageButton());
    await waitFor(() => {
      expect(outcome()?.textContent).toBe("No answer from the daemon: see below");
    });
  });

  it("a 200: the bar says how many of the answers sent were staged", async () => {
    const items = [aRoundItem({ id: "PR-0003", working_answer: "yes" }), aRoundItem({ id: "PR-0004", working_answer: "yes" })];
    const { client } = serving(aRoundSheet(items));
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0003" })).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(within(card("PR-0004")).getByRole("radio", { name: "Take the working answer" }));
    // PR-0004 changed meanwhile: the stub refuses it as changed since read.
    client.state.round = aRoundSheet([items[0] as RoundItem, { ...(items[1] as RoundItem), updated_at: T1 }]);
    fireEvent.click(stageButton());
    await waitFor(() => {
      expect(outcome()?.textContent).toBe("Staged 1 of 2: see the outcome below");
    });
    fireEvent.click(outcome() as HTMLElement);
    expect(document.activeElement).toBe(screen.getByRole("region", { name: "The answers to stage" }));
  });
});

describe("what a POST that settles after its screen left says (review iteration 3)", () => {
  it("with no Questions screen of the project on view, a refusal is spoken assertively and names the project", async () => {
    const { client } = serving(aRoundSheet([aRoundItem({ id: "PR-0003", working_answer: "yes" })]));
    const message = "spec: the queue is locked";
    let fail: () => void = () => undefined;
    client.stageRoundAnswers.mockImplementationOnce(
      () =>
        new Promise((_resolve, reject) => {
          fail = () => {
            reject(new ClientError({ status: 503, message }));
          };
        }),
    );
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0003" })).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(stageButton());
    await waitFor(() => {
      expect(client.stageRoundAnswers).toHaveBeenCalledTimes(1);
    });
    await goTo("#/alpha/tasks", "Tasks");
    act(() => {
      fail();
    });
    await waitFor(() => {
      expect(document.querySelector('[aria-live="assertive"]')?.textContent).toBe(
        `Questions of alpha: the daemon refused the answers; nothing was staged. ${message}`,
      );
    });
  });

  it("with a Questions screen of the project on view again, the refusal is its alert alone: nothing spoken twice", async () => {
    const { client } = serving(aRoundSheet([aRoundItem({ id: "PR-0003", working_answer: "yes" })]));
    const message = "spec: the queue is locked";
    let fail: () => void = () => undefined;
    client.stageRoundAnswers.mockImplementationOnce(
      () =>
        new Promise((_resolve, reject) => {
          fail = () => {
            reject(new ClientError({ status: 503, message }));
          };
        }),
    );
    await openQuestions(client);
    fireEvent.click(within(await screen.findByRole("article", { name: "PR-0003" })).getByRole("radio", { name: "Take the working answer" }));
    fireEvent.click(stageButton());
    await waitFor(() => {
      expect(client.stageRoundAnswers).toHaveBeenCalledTimes(1);
    });
    await goTo("#/alpha/tasks", "Tasks");
    await goTo("#/alpha/questions", "Questions");
    await screen.findByRole("article", { name: "PR-0003" });
    act(() => {
      fail();
    });
    const alert = await within(await screen.findByRole("region", { name: "The answers to stage" })).findByRole("alert");
    expect(alert.textContent).toContain(message);
    expect(document.querySelector('[aria-live="assertive"]')?.textContent).toBe("");
  });
});
