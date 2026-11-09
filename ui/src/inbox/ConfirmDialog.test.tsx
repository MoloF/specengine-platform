import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClientError, type SpecEngineClient } from "../api/client";
import { ApiProvider } from "../api/provider";
import type { ConfirmRead, Proposal, QueueEvent, Stage, Team } from "../api/types";
import { TEAM_ACTS } from "../board/labels";
import { en } from "../i18n/en";
import { aProposal, aRoundItem, aRoundSheet } from "../test/builders";
import { renderApp } from "../test/render";
import { argsOf, STAGED_AT, stubClient, type StubClient } from "../test/stubClient";
import { ConfirmDialog } from "./ConfirmDialog";

// B10 and B12 of docs/features/hub-confirm.md on a stub client, one click per item as
// `docs/canon/hub-confirm.md` "Rules" states it: a login
// `allowed` lists has Confirm beside a staged choice, one item at a time; the dialog shows the diff or
// record, the question verbatim and the login, focus starting on Cancel, which keeps the stage; an
// applying kind with no diff shows the review's note, Confirm off; the POST carries the read's two
// strings and never a choice; a refusal keeps the dialog in the daemon's words, status 0 saying to
// read again, never that nothing was confirmed; all of it as text, every format character (Cf)
// written `\u{..}`; a local page has no control and reads nothing new. The wire is
// `src/api/confirm.http.test.ts`. Non-Latin and format characters are built from code points.

/** Status 0's heading, exactly: a drop after the commit is status 0 too (hub-confirm B10). */
const NO_ANSWER = "No answer: read again to see whether it was confirmed.";

/** The four format characters of B12, each with the escape the dialog writes. */
const FORMATS: readonly (readonly [string, string])[] = [0x200b, 0x2060, 0xfeff, 0xad].map((code) => [String.fromCodePoint(code), `\\u{${code.toString(16)}}`]);

/** `text` with each of the four between its words, as stored; and as the dialog writes it. */
function withFormats(text: string): { stored: string; shown: string } {
  return {
    stored: FORMATS.map(([char]) => `${text}${char}`).join(" "),
    shown: FORMATS.map(([, escape]) => `${text}${escape}`).join(" "),
  };
}

/** `allowed` as a hub's `lead` or `owner` gets it. */
const LEAD: Team = { login: "mira", name: "Mira K", allowed: ["stage", "accept", "clear", "group_clear", "confirm"] };

/** The same login, less `confirm`: a hub's `member`. */
const MEMBER: Team = { login: "mira", name: "Mira K", allowed: ["stage", "accept", "clear"] };

/** A login `[team]` does not list. */
const OUTSIDER: Team = { login: "mira", name: null, allowed: [] };

const APPROVE: Stage = { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null, span_hash: null };

const DIFF = "--- base hold.md\n+++ proposed hold.md\n@@ -1,1 +1,1 @@\n-The old rule.\n+The new rule.";

/** A proposal with an approve staged, as the review document gives it. */
function staged(id: string, fields: Partial<Omit<Proposal, "id">> = {}): Proposal {
  return aProposal({ id, summary: `Summary of ${id}`, target_id: "R-1", diff: DIFF, staged: APPROVE, staged_at: STAGED_AT, updated_at: STAGED_AT, ...fields });
}

/** What a terminal would ask of `id`: several lines, as the daemon sends them. */
function questionOf(id: string): string {
  return `staged ${STAGED_AT}: spec approve ${id}\n  [0] Keep the cap | Both stay | A note\napply ${id} to docs/spec/maps/hold.md on main in /work/alpha (applies, staged)? [y/N]`;
}

/** The daemon's confirm over the stub's queue: the read of what is stored; a POST applying it, out of the queue. */
function confirmable(client: StubClient) {
  client.getConfirm.mockImplementation((_project, id) => {
    const proposal = client.state.proposals.find((candidate) => candidate.id === id);
    return Promise.resolve({ id, question: questionOf(id), updated_at: proposal?.updated_at ?? STAGED_AT });
  });
  client.confirmProposal.mockImplementation((_project, id) => {
    const proposal = client.state.proposals.find((candidate) => candidate.id === id);
    client.state.proposals = client.state.proposals.filter((candidate) => candidate.id !== id);
    client.state.round = { ...client.state.round, items: client.state.round.items.filter((item) => item.id !== id) };
    return Promise.resolve({ ...(proposal ?? aProposal({ id })), status: "applied", staged: null, staged_at: null, applied_commit: "1a2b3c4" });
  });
}

/** A stub of a hub: `team` this login's answer, `proposals` the queue, each read answered as `confirmable`. */
function hubClient(team: Team | ClientError, proposals: Proposal[] = [staged("PR-0007")]): StubClient {
  const client = stubClient(proposals);
  client.getIdentity.mockResolvedValue({ login: "mira" });
  if (team instanceof ClientError) {
    client.getTeam.mockRejectedValue(team);
  } else {
    client.getTeam.mockResolvedValue(team);
  }
  confirmable(client);
  return client;
}

/** The Confirm control of `id`, once the team read has put it there. */
function control(id: string): Promise<HTMLElement> {
  return screen.findByRole("button", { name: `Confirm ${id}` });
}

/** Opens the dialog as a person does: focus on the control, then the click. */
async function opened(id = "PR-0007"): Promise<HTMLElement> {
  const button = await control(id);
  button.focus();
  fireEvent.click(button);
  return screen.findByRole("dialog", { name: `Confirm ${id}` });
}

/** The visible copy of what was spoken: the live region holds the same words. */
function resultLine(): string | null | undefined {
  return document.querySelector(".decision-result")?.textContent;
}

function buttonIn(dialog: HTMLElement, name: string | RegExp): HTMLElement {
  return within(dialog).getByRole("button", { name });
}

function confirmIn(dialog: HTMLElement): HTMLElement {
  return buttonIn(dialog, /^Confirm(ing)? PR-/);
}

function questionIn(dialog: HTMLElement): Promise<HTMLElement> {
  return within(dialog).findByRole("figure", { name: "The question a terminal would ask" });
}

/** The dialog once its read and its document are in: Confirm on. */
async function ready(dialog: HTMLElement): Promise<HTMLElement> {
  await waitFor(() => {
    expect(confirmIn(dialog).getAttribute("aria-disabled")).toBe("false");
  });
  return confirmIn(dialog);
}

async function tick() {
  await act(() => new Promise((settle) => setTimeout(settle, 0)));
}

/** Lets a test emit queue events: the stub's live tail, opened by the Shell. */
function withTail(client: StubClient): { emit: (event: QueueEvent) => void } {
  let listener: (event: QueueEvent) => void = () => undefined;
  const subscribe: SpecEngineClient["subscribe"] = (_project, onEvent) => {
    listener = onEvent;
    return () => undefined;
  };
  (client as { subscribe: SpecEngineClient["subscribe"] }).subscribe = subscribe;
  return {
    emit: (event) => {
      act(() => {
        listener(event);
      });
    },
  };
}

describe("the Confirm control", () => {
  it("stands beside a staged choice for a login `allowed` lists, in the Inbox", async () => {
    renderApp(hubClient(LEAD), "#/alpha/inbox");
    const button = await control("PR-0007");
    const unstage = screen.getByRole("button", { name: "Unstage PR-0007" });
    expect(button.parentElement).toBe(unstage.parentElement);
    // The terminal keeps its way: the staged choice still names the command.
    expect(screen.getByText("spec approve PR-0007")).toBeTruthy();
  });

  it("is off a proposal with nothing staged", async () => {
    const client = hubClient(LEAD, [aProposal({ id: "PR-0001", summary: "Not staged" })]);
    renderApp(client, "#/alpha/inbox");
    await screen.findByRole("group", { name: "Decide PR-0001" });
    await waitFor(() => {
      expect(client.getTeam).toHaveBeenCalled();
    });
    await tick();
    expect(screen.queryByRole("button", { name: /^Confirm/ })).toBeNull();
  });

  it.each([
    ["a `member`", MEMBER],
    ["a login outside `[team]`", OUTSIDER],
    ["a team read that failed", new ClientError({ status: 503, message: "spec index unavailable" })],
    ["a hub older than the word", new ClientError({ status: 404, message: "no route /api/projects/alpha/team" })],
  ])("is not there for %s, the rest of the card as ever", async (_case, team) => {
    const client = hubClient(team);
    renderApp(client, "#/alpha/inbox");
    await screen.findByRole("button", { name: "Unstage PR-0007" });
    await waitFor(() => {
      expect(client.getTeam).toHaveBeenCalled();
    });
    await tick();
    expect(screen.queryByRole("button", { name: /^Confirm/ })).toBeNull();
    expect(client.getConfirm).not.toHaveBeenCalled();
  });

  it("is not there while the team read is out, then follows the answer", async () => {
    let answer: (team: Team) => void = () => undefined;
    const client = hubClient(LEAD);
    client.getTeam.mockImplementation(
      () =>
        new Promise<Team>((resolve) => {
          answer = resolve;
        }),
    );
    renderApp(client, "#/alpha/inbox");
    await screen.findByRole("button", { name: "Unstage PR-0007" });
    await waitFor(() => {
      expect(client.getTeam).toHaveBeenCalled();
    });
    expect(screen.queryByRole("button", { name: "Confirm PR-0007" })).toBeNull();
    answer(LEAD);
    expect(await control("PR-0007")).toBeTruthy();
  });

  it("stays focusable and does nothing while an unstage is on its way", async () => {
    const client = hubClient(LEAD);
    let release: (proposal: Proposal) => void = () => undefined;
    client.unstageDecision.mockImplementation(
      () =>
        new Promise<Proposal>((resolve) => {
          release = resolve;
        }),
    );
    renderApp(client, "#/alpha/inbox");
    const button = await control("PR-0007");
    fireEvent.click(screen.getByRole("button", { name: "Unstage PR-0007" }));
    await waitFor(() => {
      expect(button.getAttribute("aria-disabled")).toBe("true");
    });
    fireEvent.click(button);
    await tick();
    expect(screen.queryByRole("dialog")).toBeNull();
    client.state.proposals = [aProposal({ id: "PR-0007" })];
    release(aProposal({ id: "PR-0007" }));
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "Confirm PR-0007" })).toBeNull();
    });
  });
});

describe("a local daemon (B8 of hub-confirm)", () => {
  function localClient(): StubClient {
    const client = stubClient([staged("PR-0007")]);
    client.state.round = aRoundSheet([aRoundItem({ id: "PR-0008", staged: APPROVE, staged_at: STAGED_AT })]);
    return client;
  }

  it.each([
    ["the Inbox", "#/alpha/inbox", "PR-0007"],
    ["the Questions screen", "#/alpha/questions", "PR-0008"],
  ])("%s: no control, no team read, no confirm read, and no new read of any other kind", async (_case, hash, id) => {
    const client = localClient();
    renderApp(client, hash);
    await screen.findByRole("button", { name: `Unstage ${id}` });
    await waitFor(() => {
      expect(client.getIdentity).toHaveBeenCalled();
    });
    await tick();
    expect(screen.queryByRole("button", { name: /^Confirm/ })).toBeNull();
    expect(client.getTeam).not.toHaveBeenCalled();
    expect(client.getConfirm).not.toHaveBeenCalled();
    expect(client.confirmProposal).not.toHaveBeenCalled();
  });

  it("offers none on a word a team read cannot have said: nothing but a resolved login asks", async () => {
    const client = localClient();
    client.getTeam.mockResolvedValue(LEAD);
    renderApp(client, "#/alpha/inbox");
    await screen.findByRole("button", { name: "Unstage PR-0007" });
    await tick();
    expect(screen.queryByRole("button", { name: /^Confirm/ })).toBeNull();
  });
});

describe("the dialog", () => {
  it("shows the diff, the question verbatim and the login, focus on Cancel", async () => {
    renderApp(hubClient(LEAD), "#/alpha/inbox");
    const dialog = await opened();
    const shown = await questionIn(dialog);
    expect(shown.textContent).toBe(questionOf("PR-0007"));
    await ready(dialog);
    const diff = within(dialog).getByRole("figure", { name: "Section diff" });
    expect(diff.textContent).toContain("The new rule.");
    expect(within(dialog).getByText("Confirming as").nextElementSibling?.textContent).toBe("mira (Mira K)");
    expect(document.activeElement).toBe(buttonIn(dialog, "Cancel"));
  });

  it("shows the record where there is no diff", async () => {
    const proposal = staged("PR-0007", { diff: null, record_id: "DEC-0024", record_text: "---\nid: DEC-0024\n---\n\n# Keep the cap\n" });
    renderApp(hubClient(LEAD, [proposal]), "#/alpha/inbox");
    const dialog = await opened();
    await ready(dialog);
    expect(within(dialog).queryByRole("figure", { name: "Section diff" })).toBeNull();
    expect(within(dialog).getByRole("figure", { name: "Record DEC-0024" }).textContent).toBe(proposal.record_text);
  });

  it("says neither a diff nor a record is missing for a deciding kind: its record stands in the question", async () => {
    renderApp(hubClient(LEAD, [staged("PR-0007", { diff: null })]), "#/alpha/inbox");
    const dialog = await opened();
    await ready(dialog);
    expect(within(dialog).queryByText("No section diff attached.")).toBeNull();
    expect(within(dialog).queryByRole("figure", { name: /^Record/ })).toBeNull();
    expect((await questionIn(dialog)).textContent).toBe(questionOf("PR-0007"));
  });

  it("reads once per opening, the question never kept for the next", async () => {
    const client = hubClient(LEAD);
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    await ready(dialog);
    expect(argsOf(client.getConfirm)).toEqual([["alpha", "PR-0007"]]);
    fireEvent.click(buttonIn(dialog, "Cancel"));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    await ready(await opened());
    expect(client.getConfirm).toHaveBeenCalledTimes(2);
  });

  it("Cancel keeps the stage: nothing sent, focus back on the control", async () => {
    const client = hubClient(LEAD);
    renderApp(client, "#/alpha/inbox");
    const trigger = await control("PR-0007");
    const dialog = await opened();
    await ready(dialog);
    fireEvent.click(buttonIn(dialog, "Cancel"));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(document.activeElement).toBe(trigger);
    expect(client.confirmProposal).not.toHaveBeenCalled();
    expect(client.unstageDecision).not.toHaveBeenCalled();
    expect(client.state.proposals[0]?.staged).toEqual(APPROVE);
    expect(screen.getByRole("button", { name: "Unstage PR-0007" })).toBeTruthy();
  });

  it("Esc keeps it too", async () => {
    const client = hubClient(LEAD);
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    await ready(dialog);
    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.confirmProposal).not.toHaveBeenCalled();
  });

  it("keeps Confirm off until the question and the document are in", async () => {
    let answer: (read: ConfirmRead) => void = () => undefined;
    const client = hubClient(LEAD);
    client.getConfirm.mockImplementation(
      () =>
        new Promise<ConfirmRead>((resolve) => {
          answer = resolve;
        }),
    );
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    expect(confirmIn(dialog).getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(confirmIn(dialog));
    await tick();
    expect(client.confirmProposal).not.toHaveBeenCalled();
    answer({ id: "PR-0007", question: questionOf("PR-0007"), updated_at: STAGED_AT });
    await ready(dialog);
  });

  it("keeps the read's bytes for the POST when the screen writes a control as the CLI does, one line each its own direction", async () => {
    const question = `staged ${STAGED_AT}: note \u202ereversed\napply PR-0007? [y/N]`;
    const client = hubClient(LEAD);
    client.getConfirm.mockResolvedValue({ id: "PR-0007", question, updated_at: STAGED_AT });
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    const shown = await questionIn(dialog);
    expect(shown.textContent).toBe(`staged ${STAGED_AT}: note \\u{202e}reversed\napply PR-0007? [y/N]`);
    expect(shown.querySelector("pre")?.getAttribute("dir")).toBe("auto");
    fireEvent.click(await ready(dialog));
    await waitFor(() => {
      expect(client.confirmProposal).toHaveBeenCalledTimes(1);
    });
    expect(client.confirmProposal.mock.calls[0]?.[2].question).toBe(question);
  });

  it("renders agent text as text: no markup is made of a question, a record, a summary, a diff or a daemon's refusal", async () => {
    const attack = '<img src=x onerror="window.hit = 1"><script>window.hit = 2</script>';
    const proposal = staged("PR-0007", { summary: attack, diff: `@@ -1 +1 @@\n-${attack}\n+${attack}` });
    const record = staged("PR-0008", { summary: attack, diff: null, record_id: "DEC-0024", record_text: attack });
    const client = hubClient(LEAD, [proposal, record]);
    client.getConfirm.mockImplementation((_project, id) => Promise.resolve({ id, question: attack, updated_at: STAGED_AT }));
    client.confirmProposal.mockRejectedValue(new ClientError({ status: 409, message: attack }));
    renderApp(client, "#/alpha/inbox/PR-0007");
    const dialog = await opened();
    fireEvent.click(await ready(dialog));
    await within(dialog).findByRole("alert");
    expect((await questionIn(dialog)).textContent).toBe(attack);
    expect(dialog.querySelector("img, script")).toBeNull();
    fireEvent.click(buttonIn(dialog, "Cancel"));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    fireEvent.click(await screen.findByRole("option", { name: /PR-0008/ }));
    const second = await opened("PR-0008");
    await ready(second);
    expect(within(second).getByRole("figure", { name: "Record DEC-0024" }).textContent).toBe(attack);
    expect(second.querySelector("img, script")).toBeNull();
  });

  it("shows the review's note in a missing diff's place for a kind that applies, Confirm off (B10)", async () => {
    const note = "no diff: git diff --no-index failed: exit 128";
    const proposal = staged("PR-0007", { diff: null, new_text: "The new rule.\n", preview: "applies", notes: [note] });
    const client = hubClient(LEAD, [proposal]);
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    expect((await questionIn(dialog)).textContent).toBe(questionOf("PR-0007"));
    const notes = await within(dialog).findByRole("list", { name: "The review's notes" });
    expect(notes.textContent).toBe(note);
    expect(within(dialog).getByText(/^No diff to show/)).toBeTruthy();
    expect(within(dialog).queryByRole("figure", { name: "Section diff" })).toBeNull();
    await tick();
    expect(confirmIn(dialog).getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(confirmIn(dialog));
    await tick();
    expect(client.confirmProposal).not.toHaveBeenCalled();
    // A read again that brings the diff turns it on: the note was the only thing in the way.
    client.state.proposals = [{ ...proposal, diff: DIFF }];
    fireEvent.click(buttonIn(dialog, "Read again"));
    await ready(dialog);
    expect(within(dialog).queryByRole("list", { name: "The review's notes" })).toBeNull();
  });

  it("writes every format character of the diff, the record, the summary and the question as `\\u{..}`, and POSTs the read's bytes (B12)", async () => {
    const diffText = withFormats("rule");
    const recordText = withFormats("record");
    const summary = withFormats("summary");
    const asked = withFormats("question");
    const proposal = staged("PR-0007", { summary: summary.stored, diff: `@@ -1 +1 @@\n-old\n+${diffText.stored}` });
    const record = staged("PR-0008", { diff: null, record_id: "DEC-0024", record_text: recordText.stored });
    const client = hubClient(LEAD, [proposal, record]);
    client.getConfirm.mockImplementation((_project, id) => Promise.resolve({ id, question: `${asked.stored}\napply ${id}? [y/N]`, updated_at: STAGED_AT }));
    renderApp(client, "#/alpha/inbox/PR-0007");
    const dialog = await opened();
    await ready(dialog);
    const diff = within(dialog).getByRole("figure", { name: "Section diff" });
    expect(diff.textContent).toContain(`+${diffText.shown}`);
    expect(within(dialog).getByText(summary.shown)).toBeTruthy();
    expect((await questionIn(dialog)).textContent).toBe(`${asked.shown}\napply PR-0007? [y/N]`);
    for (const [char] of FORMATS) {
      expect(dialog.textContent).not.toContain(char);
    }
    fireEvent.click(confirmIn(dialog));
    await waitFor(() => {
      expect(client.confirmProposal).toHaveBeenCalledTimes(1);
    });
    expect(client.confirmProposal.mock.calls[0]?.[2].question).toBe(`${asked.stored}\napply PR-0007? [y/N]`);
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    fireEvent.click(await screen.findByRole("option", { name: /PR-0008/ }));
    const second = await opened("PR-0008");
    await ready(second);
    expect(within(second).getByRole("figure", { name: "Record DEC-0024" }).textContent).toBe(recordText.shown);
  });

  it("shows a question the daemon escaped as sent: a written `\\u{200b}` and the character read alike", async () => {
    const question = "staged: keep\\u{200b}the cap\napply PR-0007? [y/N]";
    const client = hubClient(LEAD);
    client.getConfirm.mockResolvedValue({ id: "PR-0007", question, updated_at: STAGED_AT });
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    expect((await questionIn(dialog)).textContent).toBe(question);
  });

  it("shows a non-Latin login, name and question as sent", async () => {
    const login = "\u0430\u043d\u043d\u0430";
    const name = "\u0410\u043d\u043d\u0430 \u041f\u0435\u0442\u0440\u043e\u0432\u0430";
    const question = "staged: \u043e\u0441\u0442\u0430\u0432\u0438\u0442\u044c \u043f\u0440\u0435\u0434\u0435\u043b\napply PR-0007? [y/N]";
    const client = hubClient({ ...LEAD, login, name });
    client.getConfirm.mockResolvedValue({ id: "PR-0007", question, updated_at: STAGED_AT });
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    await ready(dialog);
    expect(within(dialog).getByText("Confirming as").nextElementSibling?.textContent).toBe(`${login} (${name})`);
    expect((await questionIn(dialog)).textContent).toBe(question);
  });
});

describe("a confirm", () => {
  it("sends the read's question and updated_at and nothing else, once, whatever the stage holds", async () => {
    const stage: Stage = { ...APPROVE, option: 1, note: "keep the cap" };
    const client = hubClient(LEAD, [staged("PR-0007", { staged: stage, updated_at: "2026-11-02T09:00:00Z" })]);
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    const confirm = await ready(dialog);
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.confirmProposal.mock.calls).toEqual([["alpha", "PR-0007", { question: questionOf("PR-0007"), updated_at: "2026-11-02T09:00:00Z" }]]);
    expect(client.stageDecision).not.toHaveBeenCalled();
  });

  it("shows the result with its commit, the item gone, the next one selected and focused", async () => {
    const client = hubClient(LEAD, [staged("PR-0007", { severity: "high" }), staged("PR-0008")]);
    renderApp(client, "#/alpha/inbox");
    fireEvent.click(await ready(await opened()));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    const line = "Confirmed PR-0007: applied as commit 1a2b3c4, in this hub's checkout; this click pushes nothing.";
    await waitFor(() => {
      expect(resultLine()).toBe(line);
    });
    expect(document.querySelector('[aria-live="polite"]')?.textContent).toContain(line);
    const option = within(await screen.findByRole("listbox")).getByRole("option", { selected: true });
    expect(option.textContent).toContain("PR-0008");
    await waitFor(() => {
      expect(document.activeElement).toBe(option);
    });
    expect(screen.queryByRole("option", { name: /PR-0007/ })).toBeNull();
  });

  it("a staged approve says what the click does, true on every hub: one commit here, under the login, and the click pushes nothing (U1 of hub-git)", async () => {
    renderApp(hubClient(LEAD), "#/alpha/inbox");
    const dialog = await opened();
    await ready(dialog);
    expect(dialog.querySelector(".dialog-effect")?.textContent).toBe(
      "Confirming applies this staged choice as one commit in this hub's checkout, under your login; this click pushes nothing.",
    );
    expect(dialog.textContent).not.toMatch(/nothing is pushed/);
  });

  it("a staged reject says so, sends the same two strings and shows no commit", async () => {
    const stage: Stage = { decision: "reject", reason: "duplicate of PR-0003" };
    const client = hubClient(LEAD, [staged("PR-0007", { staged: stage })]);
    client.confirmProposal.mockImplementation((_project, id) => {
      client.state.proposals = [];
      return Promise.resolve(aProposal({ id, status: "rejected" }));
    });
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    const confirm = await ready(dialog);
    expect(confirm.classList.contains("button-danger")).toBe(true);
    expect(within(dialog).getByText(/^Confirming rejects this proposal/)).toBeTruthy();
    fireEvent.click(confirm);
    await waitFor(() => {
      expect(client.confirmProposal).toHaveBeenCalledTimes(1);
    });
    expect(Object.keys(client.confirmProposal.mock.calls[0]?.[2] ?? {})).toEqual(["question", "updated_at"]);
    await waitFor(() => {
      expect(resultLine()).toBe("Confirmed PR-0007: rejected; no commit.");
    });
  });

  it("holds the dialog while it is out: Cancel, Esc and a second click do nothing", async () => {
    const client = hubClient(LEAD);
    let answer: (proposal: Proposal) => void = () => undefined;
    client.confirmProposal.mockImplementation(
      () =>
        new Promise<Proposal>((resolve) => {
          answer = resolve;
        }),
    );
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    fireEvent.click(await ready(dialog));
    await waitFor(() => {
      expect(confirmIn(dialog).textContent).toContain("Confirming");
    });
    fireEvent.click(buttonIn(dialog, "Cancel"));
    fireEvent.keyDown(dialog, { key: "Escape" });
    fireEvent.click(confirmIn(dialog));
    await tick();
    expect(screen.getByRole("dialog")).toBe(dialog);
    expect(client.confirmProposal).toHaveBeenCalledTimes(1);
    client.state.proposals = [];
    answer(aProposal({ id: "PR-0007", status: "applied", applied_commit: "1a2b3c4" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
  });

  it("leaves the queue's keys where they were: a letter on the open dialog opens no decision", async () => {
    renderApp(hubClient(LEAD), "#/alpha/inbox");
    const dialog = await opened();
    await ready(dialog);
    fireEvent.keyDown(buttonIn(dialog, "Cancel"), { key: "a" });
    await tick();
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
  });

  it("stills the queue from the click on: while its chunk loads focus is still there, and no key moves or opens anything", async () => {
    renderApp(hubClient(LEAD, [staged("PR-0007", { severity: "high" }), staged("PR-0008")]), "#/alpha/inbox");
    const dialog = await opened();
    await ready(dialog);
    const before = window.location.hash;
    const option = within(screen.getByRole("listbox", { hidden: true })).getByRole("option", { selected: true, hidden: true });
    fireEvent.keyDown(option, { key: "j" });
    fireEvent.keyDown(option, { key: "a" });
    await tick();
    expect(window.location.hash).toBe(before);
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
  });
});

describe("a refusal", () => {
  const REFUSALS: readonly (readonly [string, number, string])[] = [
    ["a 409", 409, "`PR-0007` changed since the question: its staged choice was replaced or removed; nothing changed"],
    ["a 403", 403, "refused: mira leads maps; docs/spec/brief.md lies outside them"],
    ["a 409 naming the terminal", 409, "`PR-0007` is decided on a terminal: a stage with `editorial: true`; run spec approve PR-0007"],
    ["a 503", 503, "spec index unavailable"],
    ["status 0", 0, "POST /api/projects/alpha/proposals/PR-0007/confirm: no response from the daemon (Failed to fetch)"],
  ];

  it.each(REFUSALS)("on the POST, %s: the dialog stays, the daemon's words verbatim, Confirm off until read again", async (_case, status, message) => {
    const client = hubClient(LEAD);
    client.confirmProposal.mockRejectedValueOnce(new ClientError({ status, message }));
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    fireEvent.click(await ready(dialog));
    const alert = await within(dialog).findByRole("alert");
    expect(alert.querySelector(".verbatim")?.textContent).toBe(message);
    expect(screen.getByRole("dialog")).toBe(dialog);
    expect(client.state.proposals).toHaveLength(1);
    await waitFor(() => {
      expect(confirmIn(dialog).getAttribute("aria-disabled")).toBe("true");
    });
    fireEvent.click(confirmIn(dialog));
    await tick();
    expect(client.confirmProposal).toHaveBeenCalledTimes(1);
  });

  it.each(REFUSALS)("on the read, %s: the dialog opens on the daemon's words, nothing to confirm", async (_case, status, message) => {
    const client = hubClient(LEAD);
    client.getConfirm.mockRejectedValue(new ClientError({ status, message }));
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    const alert = await within(dialog).findByRole("alert");
    expect(alert.querySelector(".verbatim")?.textContent).toBe(message);
    expect(within(dialog).queryByRole("figure", { name: "The question a terminal would ask" })).toBeNull();
    expect(confirmIn(dialog).getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(confirmIn(dialog));
    await tick();
    expect(client.confirmProposal).not.toHaveBeenCalled();
  });

  it.each([
    ["the POST", "post"],
    ["the read, a one-factor session's first answer", "read"],
  ])("status 0 on %s says to read again, never that nothing was confirmed", async (_case, where) => {
    const error = new ClientError({ status: 0, message: "GET /api/projects/alpha/proposals/PR-0007/confirm: no response from the daemon (Failed to fetch)" });
    const client = hubClient(LEAD);
    if (where === "read") {
      client.getConfirm.mockRejectedValue(error);
    } else {
      client.confirmProposal.mockRejectedValue(error);
    }
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    if (where === "post") {
      fireEvent.click(await ready(dialog));
    }
    const alert = await within(dialog).findByRole("alert");
    expect(alert.querySelector(".refusal-title")?.textContent).toBe(NO_ANSWER);
    expect(dialog.textContent).not.toMatch(/nothing (was|is) confirmed/i);
    expect(buttonIn(dialog, "Read again")).toBeTruthy();
  });

  it("a POST's status 0 after the commit, its own event first, never flashes that nothing was confirmed", async () => {
    const applied = "2026-11-02T10:00:00Z";
    const client = hubClient(LEAD);
    const { emit } = withTail(client);
    let drop: (error: ClientError) => void = () => undefined;
    client.confirmProposal.mockImplementation(
      () =>
        new Promise<Proposal>((_resolve, reject) => {
          drop = reject;
        }),
    );
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    fireEvent.click(await ready(dialog));
    // The daemon committed: its event reads the document again while the POST is still out.
    client.state.proposals = [staged("PR-0007", { status: "applied", staged: null, staged_at: null, updated_at: applied, applied_commit: "1a2b3c4" })];
    emit({ seq: 9, type: "proposal.applied", payload: { id: "PR-0007", commit: "1a2b3c4" } });
    await waitFor(() => {
      expect(client.getProposal.mock.calls.length).toBeGreaterThan(1);
    });
    await tick();
    expect(dialog.textContent).not.toMatch(/nothing (was|is) confirmed/i);
    drop(new ClientError({ status: 0, message: "POST /api/projects/alpha/proposals/PR-0007/confirm: no response from the daemon (Failed to fetch)" }));
    const alert = await within(dialog).findByRole("alert");
    expect(alert.querySelector(".refusal-title")?.textContent).toBe(NO_ANSWER);
    expect(dialog.textContent).not.toMatch(/nothing (was|is) confirmed/i);
  });

  it("any other status says the daemon refused, not to read again for an answer", async () => {
    const client = hubClient(LEAD);
    client.confirmProposal.mockRejectedValue(new ClientError({ status: 409, message: "moved" }));
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    fireEvent.click(await ready(dialog));
    const heading = (await within(dialog).findByRole("alert")).querySelector(".refusal-title")?.textContent;
    expect(heading).toBe("The daemon refused to confirm it.");
    expect(heading).not.toBe(NO_ANSWER);
  });

  it("Read again takes the moved question: a new click answers the new words, and no earlier one", async () => {
    const moved = "2026-11-02T10:00:00Z";
    const client = hubClient(LEAD);
    const { calls } = client.confirmProposal.mock;
    client.confirmProposal.mockImplementationOnce(() => {
      client.state.proposals = [staged("PR-0007", { updated_at: moved })];
      return Promise.reject(new ClientError({ status: 409, message: "`PR-0007` changed since the question; nothing changed" }));
    });
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    fireEvent.click(await ready(dialog));
    await within(dialog).findByRole("alert");
    // The queue was read again, so a stale card shows what moved; the question waits for Read again.
    await waitFor(() => {
      expect(client.getInbox.mock.calls.length).toBeGreaterThan(1);
    });
    expect(client.getConfirm).toHaveBeenCalledTimes(1);
    client.getConfirm.mockResolvedValue({ id: "PR-0007", question: "the new words? [y/N]", updated_at: moved });
    const again = buttonIn(dialog, "Read again");
    again.focus();
    fireEvent.click(again);
    await waitFor(async () => {
      expect((await questionIn(dialog)).textContent).toBe("the new words? [y/N]");
    });
    // Read again went with its answer: focus is where it started, never on the page's body.
    expect(document.activeElement).toBe(buttonIn(dialog, "Cancel"));
    expect(within(dialog).queryByRole("alert")).toBeNull();
    fireEvent.click(await ready(dialog));
    await waitFor(() => {
      expect(calls.at(-1)).toEqual(["alpha", "PR-0007", { question: "the new words? [y/N]", updated_at: moved }]);
    });
  });

  it("a read refused at opening can be asked again once the person has stepped up", async () => {
    const client = hubClient(LEAD);
    client.getConfirm.mockRejectedValueOnce(new ClientError({ status: 0, message: "no response from the daemon (Failed to fetch)" }));
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    await within(dialog).findByRole("alert");
    fireEvent.click(buttonIn(dialog, "Read again"));
    await waitFor(async () => {
      expect((await questionIn(dialog)).textContent).toBe(questionOf("PR-0007"));
    });
    await ready(dialog);
    expect(within(dialog).queryByRole("alert")).toBeNull();
  });
});

describe("a proposal that moves under the dialog", () => {
  it("is said so and takes Confirm away: a live event never swaps the words under a click", async () => {
    const moved = "2026-11-02T10:00:00Z";
    const client = hubClient(LEAD);
    const { emit } = withTail(client);
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    await ready(dialog);
    client.state.proposals = [staged("PR-0007", { updated_at: moved, diff: "@@ -1 +1 @@\n+Another change" })];
    emit({ seq: 9, type: "proposal.staged", payload: { id: "PR-0007", staged: APPROVE, staged_at: moved } });
    expect(await within(dialog).findByRole("alert")).toBeTruthy();
    expect(within(dialog).getByRole("alert").textContent).toContain("changed since this dialog read it");
    expect(confirmIn(dialog).getAttribute("aria-disabled")).toBe("true");
    expect(client.getConfirm).toHaveBeenCalledTimes(1);
    expect((await questionIn(dialog)).textContent).toBe(questionOf("PR-0007"));
    fireEvent.click(confirmIn(dialog));
    await tick();
    expect(client.confirmProposal).not.toHaveBeenCalled();
    fireEvent.click(buttonIn(dialog, "Read again"));
    await ready(dialog);
    expect(client.getConfirm).toHaveBeenCalledTimes(2);
  });

  it("never pairs an older document in hand with a newer question: Confirm waits for the document", async () => {
    const moved = "2026-11-02T10:00:00Z";
    const client = hubClient(LEAD);
    renderApp(client, "#/alpha/inbox");
    const trigger = await control("PR-0007");
    // Moved after the card's read: the card's document is older than the question will be.
    client.state.proposals = [staged("PR-0007", { updated_at: moved, diff: "@@ -1 +1 @@\n+Moved" })];
    let answer: (proposal: Proposal) => void = () => undefined;
    client.getProposal.mockImplementationOnce(
      () =>
        new Promise<Proposal>((resolve) => {
          answer = resolve;
        }),
    );
    fireEvent.click(trigger);
    const dialog = await screen.findByRole("dialog");
    await questionIn(dialog);
    expect(confirmIn(dialog).getAttribute("aria-disabled")).toBe("true");
    answer(structuredClone(client.state.proposals[0] ?? staged("PR-0007")));
    await ready(dialog);
    expect(within(dialog).getByRole("figure", { name: "Section diff" }).textContent).toContain("Moved");
  });
});

describe("one click per item", () => {
  function roundClient(team: Team = LEAD): StubClient {
    const ids = ["PR-0001", "PR-0002", "PR-0003"];
    const client = hubClient(team, ids.map((id) => staged(id, { kind: "question" })));
    client.state.round = aRoundSheet(ids.map((id) => aRoundItem({ id, staged: APPROVE, staged_at: STAGED_AT, updated_at: STAGED_AT })));
    return client;
  }

  it("gives each staged item of a round its own control, naming that item alone, and none to a set", async () => {
    renderApp(roundClient(), "#/alpha/questions");
    await control("PR-0003");
    const names = screen.getAllByRole("button", { name: /confirm/i }).map((button) => button.textContent.replace(/\s+/g, " ").trim());
    expect(names).toEqual(["Confirm PR-0001", "Confirm PR-0002", "Confirm PR-0003"]);
    // The sheet's own confirm is still the terminal's: fixed words, nothing to press here.
    expect(screen.getByRole("heading", { name: "Confirm on a terminal" })).toBeTruthy();
  });

  it("a Confirm takes one item: one read, one POST, the other two left staged for their own clicks", async () => {
    const client = roundClient();
    renderApp(client, "#/alpha/questions");
    const dialog = await opened("PR-0002");
    expect((await questionIn(dialog)).textContent).toBe(questionOf("PR-0002"));
    fireEvent.click(await ready(dialog));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.confirmProposal.mock.calls.map((call) => call[1])).toEqual(["PR-0002"]);
    expect(argsOf(client.getConfirm)).toEqual([["alpha", "PR-0002"]]);
    expect(await control("PR-0001")).toBeTruthy();
    expect(await control("PR-0003")).toBeTruthy();
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "Confirm PR-0002" })).toBeNull();
    });
    expect(resultLine()).toMatch(/^Confirmed PR-0002: applied as commit 1a2b3c4/);
    expect(document.activeElement).toBe(document.querySelector(".round"));
  });

  it("three items take three clicks, each its own read of its own words", async () => {
    const client = roundClient();
    renderApp(client, "#/alpha/questions");
    for (const id of ["PR-0003", "PR-0001", "PR-0002"]) {
      fireEvent.click(await ready(await opened(id)));
      await waitFor(() => {
        expect(screen.queryByRole("dialog")).toBeNull();
      });
      await waitFor(() => {
        expect(screen.queryByRole("button", { name: `Confirm ${id}` })).toBeNull();
      });
    }
    expect(client.confirmProposal.mock.calls.map((call) => [call[1], call[2].question])).toEqual(["PR-0003", "PR-0001", "PR-0002"].map((id) => [id, questionOf(id)]));
  });

  it("Stage only stages: the round's one POST confirms nothing", async () => {
    const client = hubClient(LEAD, []);
    client.state.round = aRoundSheet([aRoundItem({ id: "PR-0004", options: [{ label: "Code to spec", effect: "Fix the code", price: "One test" }] })]);
    renderApp(client, "#/alpha/questions");
    fireEvent.click(await screen.findByRole("radio", { name: /Code to spec/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Stage 1 answer" }));
    await waitFor(() => {
      expect(client.stageRoundAnswers).toHaveBeenCalledTimes(1);
    });
    await tick();
    expect(client.confirmProposal).not.toHaveBeenCalled();
    expect(client.getConfirm).not.toHaveBeenCalled();
    expect(await control("PR-0004")).toBeTruthy();
  });

  it("a member sees the sheet with no Confirm", async () => {
    const client = roundClient(MEMBER);
    renderApp(client, "#/alpha/questions");
    await screen.findByRole("button", { name: "Unstage PR-0003" });
    await waitFor(() => {
      expect(client.getTeam).toHaveBeenCalled();
    });
    await tick();
    expect(screen.queryByRole("button", { name: /^Confirm PR/ })).toBeNull();
  });

  it("a refusal stays though the re-read empties the sheet under it", async () => {
    const client = hubClient(LEAD, [staged("PR-0005", { kind: "question" })]);
    client.state.round = aRoundSheet([aRoundItem({ id: "PR-0005", staged: APPROVE, staged_at: STAGED_AT, updated_at: STAGED_AT })]);
    const message = "`PR-0005` was decided elsewhere; nothing changed";
    client.confirmProposal.mockImplementation(() => {
      client.state.round = aRoundSheet([]);
      return Promise.reject(new ClientError({ status: 409, message }));
    });
    renderApp(client, "#/alpha/questions");
    const dialog = await opened("PR-0005");
    fireEvent.click(await ready(dialog));
    await screen.findByRole("heading", { name: "No open questions or discrepancies." });
    expect(screen.getByRole("dialog")).toBe(dialog);
    expect(within(dialog).getByRole("alert").textContent).toContain(message);
    fireEvent.click(buttonIn(dialog, "Cancel"));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
  });

  it("names no set in the sources: no control, route or method confirms more than one", () => {
    const sources = import.meta.glob<string>("/src/**/*.{ts,tsx}", { query: "?raw", import: "default", eager: true });
    const app = Object.entries(sources).filter(([path]) => !/\.test\.tsx?$/.test(path));
    expect(app.length).toBeGreaterThan(50);
    const wording = /confirm[ _-]?(all|every|each|many|group|set|items|answers|batch)\b|confirmAll|confirmMany|confirmRound/i;
    expect(app.filter(([, text]) => wording.test(text)).map(([path]) => path)).toEqual([]);
    const client = sources["/src/api/client.ts"] ?? "";
    expect(client).toContain("confirmProposal(project: string, id: string, body: ConfirmBody): Promise<Proposal>;");
  });

  it("keeps actOff to the acts with a control to stand off: confirm has none", () => {
    expect(Object.keys(en.board.actOff)).not.toContain("confirm");
    expect(TEAM_ACTS).not.toContain("confirm");
  });
});

// U1 of docs/features/hub-git.md ("Status and the screen"): after a 200 whose review document holds
// notes, 10a's gap warning among them (docs/features/apply-outputs.md), the dialog stays open with
// `Confirmed.` and the notes as text, escaped as the rest of the dialog, Confirm gone and focus on
// Close; `onConfirmed` fires once, on Close. With no notes, as a clean confirm's document holds
// (crates/specengine-cli: `with_diff` writes none, the gap note only when the outputs were not
// rendered), it fires at once.

/** 10a's gap warning as an applying confirm's review document carries it in `notes`. */
const GAP =
  "the commit carries `docs/spec/maps/hold.md` alone: the index's outputs were not rendered: the `[paths] roots` entry `notes` names no directory and no `.md` file; `spec export index` writes them";

/** The stub's POST, answering the applied document with `notes`. */
function confirmedWith(client: StubClient, notes: string[]) {
  client.confirmProposal.mockImplementation((_project, id) => {
    const proposal = client.state.proposals.find((candidate) => candidate.id === id);
    client.state.proposals = client.state.proposals.filter((candidate) => candidate.id !== id);
    client.state.round = { ...client.state.round, items: client.state.round.items.filter((item) => item.id !== id) };
    return Promise.resolve({ ...(proposal ?? aProposal({ id })), status: "applied", staged: null, staged_at: null, applied_commit: "1a2b3c4", notes });
  });
}

/** The dialog alone over a hub's stub, its two callbacks counted. */
function dialogAlone(client: StubClient) {
  const onClose = vi.fn<() => void>();
  const onConfirmed = vi.fn<(review: Proposal) => void>();
  render(
    <ApiProvider client={client}>
      <ConfirmDialog project="alpha" id="PR-0007" team={LEAD} onClose={onClose} onConfirmed={onConfirmed} />
    </ApiProvider>,
  );
  return { onClose, onConfirmed };
}

describe("a confirm answered with notes (U1 of hub-git)", () => {
  it("keeps the dialog open: `Confirmed.`, the gap warning as text, Confirm gone, focus on Close", async () => {
    const client = hubClient(LEAD, [staged("PR-0007", { severity: "high" }), staged("PR-0008")]);
    confirmedWith(client, [GAP]);
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    fireEvent.click(await ready(dialog));
    expect(await within(dialog).findByText("Confirmed.")).toBeTruthy();
    expect(screen.getByRole("dialog")).toBe(dialog);
    expect(within(dialog).getByRole("list", { name: "The daemon's notes" }).textContent).toBe(GAP);
    expect(within(dialog).queryByRole("button", { name: /^Confirm/ })).toBeNull();
    expect(within(dialog).queryByRole("button", { name: "Cancel" })).toBeNull();
    expect(within(dialog).queryByRole("alert")).toBeNull();
    expect(dialog.textContent).not.toMatch(/nothing (was|is) confirmed/i);
    await waitFor(() => {
      expect(document.activeElement).toBe(buttonIn(dialog, "Close"));
    });
    // Not handed on yet: no result line until Close.
    expect(resultLine()).toBeUndefined();
    expect(client.confirmProposal).toHaveBeenCalledTimes(1);
  });

  it("Close hands the document on: the result with its commit, the dialog gone, the next item focused", async () => {
    const client = hubClient(LEAD, [staged("PR-0007", { severity: "high" }), staged("PR-0008")]);
    confirmedWith(client, [GAP]);
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    fireEvent.click(await ready(dialog));
    fireEvent.click(await within(dialog).findByRole("button", { name: "Close" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(resultLine()).toBe("Confirmed PR-0007: applied as commit 1a2b3c4, in this hub's checkout; this click pushes nothing.");
    const option = within(await screen.findByRole("listbox")).getByRole("option", { selected: true });
    expect(option.textContent).toContain("PR-0008");
    await waitFor(() => {
      expect(document.activeElement).toBe(option);
    });
  });

  it("fires onConfirmed once, on Close, with the document as sent; Esc and a second press hand nothing more", async () => {
    const client = hubClient(LEAD);
    confirmedWith(client, [GAP, "a second note"]);
    const { onClose, onConfirmed } = dialogAlone(client);
    const dialog = await screen.findByRole("dialog", { name: "Confirm PR-0007" });
    fireEvent.click(await ready(dialog));
    const close = await within(dialog).findByRole("button", { name: "Close" });
    await tick();
    expect(onConfirmed).not.toHaveBeenCalled();
    expect(within(dialog).getAllByRole("listitem").map((item) => item.textContent)).toEqual([GAP, "a second note"]);
    fireEvent.click(close);
    fireEvent.click(close);
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(onConfirmed).toHaveBeenCalledTimes(1);
    expect(onConfirmed.mock.calls[0]?.[0]).toMatchObject({ id: "PR-0007", status: "applied", applied_commit: "1a2b3c4", notes: [GAP, "a second note"] });
    expect(onClose).not.toHaveBeenCalled();
  });

  it("Esc on it hands the document on as Close does", async () => {
    const client = hubClient(LEAD);
    confirmedWith(client, [GAP]);
    const { onClose, onConfirmed } = dialogAlone(client);
    const dialog = await screen.findByRole("dialog", { name: "Confirm PR-0007" });
    fireEvent.click(await ready(dialog));
    await within(dialog).findByText("Confirmed.");
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(onConfirmed).toHaveBeenCalledTimes(1);
    expect(onClose).not.toHaveBeenCalled();
  });

  it("with no notes hands the document on at once: no `Confirmed.` in the dialog, nothing to close", async () => {
    const client = hubClient(LEAD);
    confirmedWith(client, []);
    const { onClose, onConfirmed } = dialogAlone(client);
    const dialog = await screen.findByRole("dialog", { name: "Confirm PR-0007" });
    fireEvent.click(await ready(dialog));
    await waitFor(() => {
      expect(onConfirmed).toHaveBeenCalledTimes(1);
    });
    expect(onConfirmed.mock.calls[0]?.[0]).toMatchObject({ id: "PR-0007", status: "applied", notes: [] });
    expect(within(dialog).queryByText("Confirmed.")).toBeNull();
    expect(within(dialog).queryByRole("button", { name: "Close" })).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("writes the notes as text, every control and format character `\\u{..}`, no element made of them", async () => {
    const note = `<b>x</b>${String.fromCodePoint(0x202e)} hold${String.fromCodePoint(0x200b)}`;
    const client = hubClient(LEAD);
    confirmedWith(client, [note]);
    renderApp(client, "#/alpha/inbox");
    const dialog = await opened();
    fireEvent.click(await ready(dialog));
    const notes = await within(dialog).findByRole("list", { name: "The daemon's notes" });
    expect(notes.textContent).toBe("<b>x</b>\\u{202e} hold\\u{200b}");
    expect(notes.querySelector("b")).toBeNull();
    expect(notes.querySelector("li")?.getAttribute("dir")).toBe("auto");
    expect(dialog.textContent).not.toContain(String.fromCodePoint(0x202e));
  });

  it("on the Questions screen too: the dialog stays on the notes, Close hands it on and the sheet takes focus", async () => {
    const client = hubClient(LEAD, [staged("PR-0002", { kind: "question" })]);
    client.state.round = aRoundSheet([aRoundItem({ id: "PR-0002", staged: APPROVE, staged_at: STAGED_AT, updated_at: STAGED_AT })]);
    confirmedWith(client, [GAP]);
    renderApp(client, "#/alpha/questions");
    const dialog = await opened("PR-0002");
    fireEvent.click(await ready(dialog));
    await within(dialog).findByText("Confirmed.");
    expect(resultLine()).toBeUndefined();
    fireEvent.click(buttonIn(dialog, "Close"));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(resultLine()).toMatch(/^Confirmed PR-0002: applied as commit 1a2b3c4/);
    await waitFor(() => {
      expect(document.activeElement).toBe(document.querySelector(".round"));
    });
  });
});
