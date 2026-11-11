import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { Card, CardRow, CardSplit, CardSplitBody, CardSplitChild } from "../api/types";
import { workspaceKey } from "../workspace/choice";
import { aCard, aCardFlow, aCardList, aCardRow, aTrace, aTraceNode, aWorkspace, aWorkspaces } from "../test/builders";
import { docClient, docView, FERRY, FERRY_CARD } from "../test/docStub";
import { renderApp } from "../test/render";
import { argsOf, type StubClient } from "../test/stubClient";
import { ACTING_KEY } from "./acting";

// U2 of docs/features/department-split.md "Acceptance criteria" on the river ferry: the Split
// dialog reads the daemon's plan once as it opens and shows it, rows under the departments whose
// flows name theirs, the suggested ones checked; Split posts the checked disciplines in row order,
// once. Nothing here computes a target, a suggestion or a place: the stub's plan is the daemon's.
// The mock end to end: src/mocks/split.smoke.test.tsx (a test outside src/mocks/ imports no mock).

/** The four subtasks the stub's daemon plans, in served order: two suggested by kind, one off its flow's name by the task's targets, one no department names. */
const PLANNED: readonly CardSplitChild[] = [
  { id: null, discipline: "narrative", flow: "narrative", flow_title: "Narrative", column: "draft", targets: [FERRY, "RULE-FERRY-TIDES"], from: "kinds", suggested: true },
  { id: null, discipline: "tools", flow: "development", flow_title: "Development", column: "backlog", targets: [FERRY], from: "parent", suggested: false },
  { id: null, discipline: "art", flow: "art", flow_title: "Art", column: "sketch", targets: ["MEC-BOARDING"], from: "kinds", suggested: true },
  { id: null, discipline: "sound", flow: "sound", flow_title: "Sound", column: "backlog", targets: [], from: "parent", suggested: false },
];

/** The departments: writing names narrative's flow, studio art's and development's, all every flow, so it takes no row; none names sound's. */
function departments() {
  return aWorkspaces([
    aWorkspace({ name: "writing", title: "Writing", flows: ["narrative"], disciplines: ["narrative"] }),
    aWorkspace({ name: "studio", title: "Studio", flows: ["art", "development"], disciplines: ["art", "development", "tools"] }),
    aWorkspace({ name: "all", title: "Everyone", flows: null, disciplines: ["narrative", "art", "development", "tools", "sound"] }),
  ]);
}

/** The ferry's client with the departments and a daemon that plans PLANNED and writes C-0040 with a subtask per child from C-0041. */
function splitClient(): StubClient {
  const client = docClient();
  client.getWorkspaces.mockImplementation(() => Promise.resolve(departments()));
  client.splitCard.mockImplementation((_project, body) => Promise.resolve(answerOf(body)));
  return client;
}

/** The stub daemon's answer: the plan, or the written split. */
function answerOf(body: CardSplitBody): CardSplit {
  const written = !body.dry_run;
  const parentId = body.parent ?? (written ? "C-0040" : null);
  const asked = body.child ?? PLANNED.map((child) => child.discipline);
  const children = asked.map((discipline, index) => ({ ...(PLANNED.find((child) => child.discipline === discipline) ?? (PLANNED[0] as CardSplitChild)), id: written ? `C-00${String(41 + index)}` : null }));
  return {
    dry_run: body.dry_run,
    parent: { id: parentId, created: body.parent === null, title: body.title ?? "Paint the ferry", discipline: body.discipline, flow: "art", flow_title: "Art", column: "sketch", targets: body.target ?? [] },
    children,
    reviewer: body.reviewer,
    notes: [],
  };
}

/** The ferry's modal over the Inbox, its acts landed, Split to departments… pressed: the dialog. */
async function fromDocument(client: StubClient = splitClient()): Promise<HTMLElement> {
  renderApp(client, `#/alpha/inbox?doc=${FERRY}`);
  const modal = await screen.findByRole("dialog", { name: "The ferry" });
  fireEvent.click(await within(modal).findByRole("button", { name: "Split to departments…" }));
  return screen.findByRole("dialog", { name: "Split to departments" });
}

/** The dialog once its plan has landed: its rows' checkboxes stand. */
async function planned(dialog: HTMLElement): Promise<HTMLElement[]> {
  return within(dialog).findAllByRole("checkbox");
}

/** Each checkbox's name and state, in row order. */
function rows(dialog: HTMLElement): [string, boolean][] {
  return within(dialog)
    .getAllByRole<HTMLInputElement>("checkbox")
    .map((box) => [box.closest("label")?.textContent ?? "", box.checked]);
}

/** The bodies the daemon was sent, in order. */
function bodies(client: StubClient): CardSplitBody[] {
  return argsOf(client.splitCard).map((args) => args[1] as CardSplitBody);
}

function splitButton(dialog: HTMLElement): HTMLButtonElement {
  return within(dialog).getByRole<HTMLButtonElement>("button", { name: "Split" });
}

describe("the Split dialog (U2 of department-split)", () => {
  it("reads the plan once as it opens, dry_run true and child null; rows in served order under the first department whose flows name theirs, Other flows last; the suggested checked; the suggested line", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    window.localStorage.setItem(workspaceKey("alpha"), "studio");
    const client = splitClient();
    const dialog = await fromDocument(client);
    await planned(dialog);
    expect(bodies(client)).toEqual([{ target: [FERRY], title: "The ferry", goal: null, discipline: "art", parent: null, child: null, reviewer: null, dry_run: true, as: "Ann" }]);
    // Writing's row, Studio's two in served order (development's then art's), then the one no department names; Everyone, all flows, takes none.
    expect(rows(dialog)).toEqual([
      ["Narrative ", true],
      ["Development tools", false],
      ["Art ", true],
      ["Sound ", false],
    ]);
    const legends = Array.from(dialog.querySelectorAll("legend"), (legend) => legend.textContent);
    expect(legends).toEqual(["Subtasks, one per department", "Writing", "Studio", "Other flows"]);
    expect(within(dialog).getByText("Suggested by the project's split templates")).toBeTruthy();
    // The raw discipline under the flow's title when it differs, as the daemon wrote it.
    expect(Array.from(dialog.querySelectorAll(".split-discipline"), (raw) => [raw.textContent, raw.getAttribute("lang")])).toEqual([["tools", "en"]]);
    expect(splitButton(dialog).disabled).toBe(false);
  });

  it("says where each subtask's targets come from, its targets folded as document links; none: No targets", async () => {
    const dialog = await fromDocument();
    await planned(dialog);
    const narrative = within(dialog).getByRole("button", { name: "By kind: 2 documents" });
    expect(narrative.getAttribute("aria-expanded")).toBe("false");
    expect(within(dialog).getByRole("button", { name: "By kind: 1 document" })).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: "The task's targets" })).toBeTruthy();
    expect(within(dialog).getByText("No targets")).toBeTruthy();
    fireEvent.click(narrative);
    const region = document.getElementById(narrative.getAttribute("aria-controls") ?? "");
    expect(Array.from(region?.querySelectorAll("a") ?? [], (link) => [link.textContent, link.getAttribute("href")])).toEqual([
      [FERRY, `#/alpha/tree/${FERRY}/page`],
      ["RULE-FERRY-TIDES", "#/alpha/tree/RULE-FERRY-TIDES/page"],
    ]);
  });

  it("from a document: Title prefilled, Goal empty, Discipline the chosen department's first among every discipline served once, the document's tasks as links to their work items", async () => {
    window.localStorage.setItem(workspaceKey("alpha"), "studio");
    const dialog = await fromDocument();
    expect(within(dialog).getByLabelText<HTMLInputElement>("Title").value).toBe("The ferry");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Goal").value).toBe("");
    const discipline = within(dialog).getByLabelText<HTMLSelectElement>("Discipline");
    expect(discipline.value).toBe("art");
    expect(Array.from(discipline.options, (option) => option.value)).toEqual(["", "narrative", "art", "development", "tools", "sound"]);
    expect(within(dialog).getByText("Tasks on this document:")).toBeTruthy();
    expect(within(dialog).getByRole("link", { name: "C-0007 Paint the ferry" }).getAttribute("href")).toBe("#/alpha/board/C-0007");
  });

  it("opened by an alias, its tasks come from the trace the document view holds under the route's REF, the row its target names (M: the held read keyed by the target)", async () => {
    const client = splitClient();
    // DOM-BOAT, an alias: the daemon serves the ferry under it, and its trace read the ferry's row.
    client.getNode.mockImplementation((_project, ref) => Promise.resolve({ ...docView(ref === "DOM-BOAT" ? FERRY : ref), ref }));
    client.getTrace.mockImplementation((_project, options) =>
      Promise.resolve(options?.ref === "DOM-BOAT" ? aTrace([aTraceNode(FERRY, [FERRY_CARD], { title: "The ferry" })], { ref: "DOM-BOAT" }) : aTrace([], { ref: options?.ref ?? null })),
    );
    renderApp(client, "#/alpha/inbox?doc=DOM-BOAT");
    const modal = await screen.findByRole("dialog", { name: "The ferry" });
    fireEvent.click(await within(modal).findByRole("button", { name: "Split to departments…" }));
    const dialog = await screen.findByRole("dialog", { name: "Split to departments" });
    await planned(dialog);
    // The parent's target is the ferry's canonical name; the list is read where the view read it.
    expect(bodies(client)[0]?.target).toEqual([FERRY]);
    expect(within(dialog).getByText("Tasks on this document:")).toBeTruthy();
    expect(within(dialog).getByRole("link", { name: "C-0007 Paint the ferry" }).getAttribute("href")).toBe("#/alpha/board/C-0007");
    expect(argsOf(client.getTrace).map((args) => (args[1] as { ref?: string } | undefined)?.ref)).not.toContain(FERRY);
  });

  it("with the first department chosen by default, its first discipline", async () => {
    const dialog = await fromDocument();
    expect(within(dialog).getByLabelText<HTMLSelectElement>("Discipline").value).toBe("narrative");
  });

  it("Split is off with none checked and says why; toggling and typing read nothing; Split posts once dry_run false, child the checked disciplines in row order, the form's title, goal, discipline, reviewer and the source (M: every suggested row sent; a plan read per toggle)", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = splitClient();
    const dialog = await fromDocument(client);
    const boxes = await planned(dialog);
    fireEvent.click(boxes[0] as HTMLElement);
    fireEvent.click(boxes[2] as HTMLElement);
    expect(rows(dialog).map(([, checked]) => checked)).toEqual([false, false, false, false]);
    expect(splitButton(dialog).disabled).toBe(true);
    expect(within(dialog).getByRole("status").textContent).toBe("Choose at least one department");
    // Checked out of row order: sound first, then tools; the narrative row stays off.
    fireEvent.click(boxes[3] as HTMLElement);
    fireEvent.click(boxes[1] as HTMLElement);
    fireEvent.change(within(dialog).getByLabelText("Title"), { target: { value: "Ferry, all trades" } });
    fireEvent.change(within(dialog).getByLabelText("Goal"), { target: { value: "  One crossing.  " } });
    fireEvent.change(within(dialog).getByLabelText("Discipline"), { target: { value: "tools" } });
    fireEvent.change(within(dialog).getByLabelText("Reviewer"), { target: { value: "Ann Lee" } });
    expect(client.splitCard).toHaveBeenCalledTimes(1);
    fireEvent.click(splitButton(dialog));
    await waitFor(() => {
      expect(client.splitCard).toHaveBeenCalledTimes(2);
    });
    expect(bodies(client)[1]).toEqual({ target: [FERRY], title: "Ferry, all trades", goal: "One crossing.", discipline: "tools", parent: null, child: ["tools", "sound"], reviewer: "Ann Lee", dry_run: false, as: "Ann" });
    for (const other of [client.createCard, client.assignCard, client.moveCard, client.editCard]) {
      expect(other).not.toHaveBeenCalled();
    }
  });

  it("a 200 closes the dialog, says the polite line and notes the parent under the acts as a link to its work item; focus back on the act", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const dialog = await fromDocument();
    await planned(dialog);
    const act = screen.getAllByRole("button", { name: "Split to departments…" })[0] as HTMLElement;
    fireEvent.click(splitButton(dialog));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Split to departments" })).toBeNull();
    });
    await waitFor(() => {
      expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe("Split into C-0040 and 2 subtasks");
    });
    const modal = screen.getByRole("dialog", { name: "The ferry" });
    const note = modal.querySelector(".doc-acts-note");
    expect(note?.textContent).toBe("Split into C-0040 and 2 subtasks");
    expect(within(note as HTMLElement).getByRole("link", { name: "C-0040" }).getAttribute("href")).toBe("#/alpha/board/C-0040");
    await waitFor(() => {
      expect(document.activeElement).toBe(act);
    });
  });

  it("a refusal (409) keeps the dialog and every field, the daemon's words in its alert under lang=en; the next Split goes again", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = splitClient();
    const reason = "child: no flow takes the discipline `narrative`: none lists it, and every `[[card.flow]]` lists its disciplines";
    client.splitCard.mockImplementation((_project, body) => (body.dry_run ? Promise.resolve(answerOf(body)) : Promise.reject(new ClientError({ status: 409, message: reason }))));
    const dialog = await fromDocument(client);
    await planned(dialog);
    fireEvent.change(within(dialog).getByLabelText("Title"), { target: { value: "Kept" } });
    fireEvent.click(splitButton(dialog));
    const alert = await within(dialog).findByText(reason);
    expect(alert.getAttribute("lang")).toBe("en");
    expect(alert.closest('[role="alert"]')).not.toBeNull();
    expect(screen.getByRole("dialog", { name: "Split to departments" })).toBe(dialog);
    expect(within(dialog).getByLabelText<HTMLInputElement>("Title").value).toBe("Kept");
    expect(rows(dialog).map(([, checked]) => checked)).toEqual([true, false, true, false]);
    fireEvent.click(splitButton(dialog));
    await waitFor(() => {
      expect(client.splitCard).toHaveBeenCalledTimes(3);
    });
  });

  it("a failed plan read: a skeleton first, then the daemon's words under lang=en and Retry, which reads again with the form's discipline; no Split until a plan stands", async () => {
    const client = splitClient();
    const reason = "no flow takes the discipline `narrative`: none lists it, and every `[[card.flow]]` lists its disciplines";
    let refuse: (error: Error) => void = () => undefined;
    client.splitCard.mockImplementationOnce(
      () =>
        new Promise<CardSplit>((_resolve, reject) => {
          refuse = reject;
        }),
    );
    const dialog = await fromDocument(client);
    expect(within(dialog).getByRole("status", { name: "Reading the plan" }).getAttribute("aria-busy")).toBe("true");
    expect(splitButton(dialog).disabled).toBe(true);
    refuse(new ClientError({ status: 409, message: reason }));
    const message = await within(dialog).findByText(reason);
    expect(message.getAttribute("lang")).toBe("en");
    expect(within(dialog).getByText("The plan could not be read")).toBeTruthy();
    expect(splitButton(dialog).disabled).toBe(true);
    expect(within(dialog).queryAllByRole("checkbox")).toEqual([]);
    fireEvent.change(within(dialog).getByLabelText("Discipline"), { target: { value: "art" } });
    expect(client.splitCard).toHaveBeenCalledTimes(1);
    fireEvent.click(within(dialog).getByRole("button", { name: "Retry" }));
    await planned(dialog);
    expect(bodies(client).map((body) => body.discipline)).toEqual(["narrative", "art"]);
    expect(bodies(client).every((body) => body.dry_run && body.child === null)).toBe(true);
  });

  it("a 400 naming --as stays in the dialog as a card write's does", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = splitClient();
    const line = "spec: --as: U+0007 in a name: one line, no control character or bidirectional mark";
    client.splitCard.mockImplementation((_project, body) => (body.dry_run ? Promise.resolve(answerOf(body)) : Promise.reject(new ClientError({ status: 400, message: line }))));
    const dialog = await fromDocument(client);
    await planned(dialog);
    fireEvent.click(splitButton(dialog));
    expect((await within(dialog).findByText(line)).closest('[role="alert"]')).not.toBeNull();
  });
});

/** C-0001, a task with a reviewer, and its board. */
function taskClient(reviewer: string | null = "Ivo Brandt"): StubClient {
  const client = splitClient();
  const card = aCard({ id: "C-0001", flow: "narrative", title: "Lamp room dialogue", targets: [FERRY], reviewer, revision: "4" });
  client.getCards.mockImplementation(() => Promise.resolve(aCardList([aCardRow({ id: "C-0001", flow: "narrative", title: "Lamp room dialogue", targets: [FERRY], revision: "4" })], { flows: [aCardFlow({ name: "narrative", disciplines: ["narrative"] })] })));
  client.getCard.mockImplementation((_project, id) => Promise.resolve(id === "C-0001" ? structuredClone(card) : { id, reason: `no card ${id}` }));
  return client;
}

async function fromTask(client: StubClient): Promise<{ modal: HTMLElement; dialog: HTMLElement }> {
  renderApp(client, "#/alpha/board/C-0001");
  const modal = await screen.findByRole("dialog", { name: "Lamp room dialogue" });
  fireEvent.click(await within(modal).findByRole("button", { name: "Split to departments…" }));
  return { modal, dialog: await screen.findByRole("dialog", { name: "Split C-0001 to departments" }) };
}

describe("from a task (U1, U2 of department-split)", () => {
  it("names the task in its heading, has no Title, Goal or Discipline, plans under the stored parent, the reviewer the task's; Split says how many subtasks the task has now", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = taskClient();
    const { modal, dialog } = await fromTask(client);
    for (const absent of ["Title", "Goal", "Discipline"]) {
      expect([absent, within(dialog).queryByLabelText(absent)]).toEqual([absent, null]);
    }
    expect(within(dialog).queryByText("Tasks on this document:")).toBeNull();
    expect(within(dialog).getByLabelText<HTMLInputElement>("Reviewer").value).toBe("Ivo Brandt");
    await planned(dialog);
    expect(bodies(client)).toEqual([{ target: null, title: null, goal: null, discipline: null, parent: "C-0001", child: null, reviewer: null, dry_run: true, as: "Ann" }]);
    fireEvent.click(splitButton(dialog));
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Split C-0001 to departments" })).toBeNull();
    });
    expect(bodies(client)[1]).toEqual({ target: null, title: null, goal: null, discipline: null, parent: "C-0001", child: ["narrative", "art"], reviewer: "Ivo Brandt", dry_run: false, as: "Ann" });
    await waitFor(() => {
      expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe("2 subtasks for C-0001");
    });
    expect(screen.getAllByRole("dialog")).toEqual([modal]);
    // No other write: the stored parent is read, never written.
    for (const other of [client.createCard, client.assignCard, client.moveCard, client.editCard]) {
      expect(other).not.toHaveBeenCalled();
    }
  });

  it("a task with no reviewer: the acting name, unless it names no person; required", async () => {
    window.localStorage.setItem(ACTING_KEY, "agent");
    const client = taskClient(null);
    const { dialog } = await fromTask(client);
    const reviewer = within(dialog).getByLabelText<HTMLInputElement>("Reviewer");
    expect(reviewer.value).toBe("");
    await planned(dialog);
    fireEvent.click(splitButton(dialog));
    expect((await within(dialog).findByRole("alert")).textContent).toBe("Name who checks the result: a person on the side that asks.");
    expect(reviewer.getAttribute("aria-invalid")).toBe("true");
    expect(client.splitCard).toHaveBeenCalledTimes(1);
  });

  it("on a hub: the Reviewer read-only, the login, and sent as the reviewer", async () => {
    const client = taskClient(null);
    client.getIdentity.mockResolvedValue({ login: "mira" });
    client.getTeam.mockResolvedValue({ login: "mira", name: null, allowed: ["stage", "accept", "clear", "group_clear"] });
    const { dialog } = await fromTask(client);
    const reviewer = await within(dialog).findByLabelText<HTMLInputElement>("Reviewer");
    expect([reviewer.value, reviewer.readOnly]).toEqual(["mira", true]);
    await planned(dialog);
    fireEvent.click(splitButton(dialog));
    await waitFor(() => {
      expect(client.splitCard).toHaveBeenCalledTimes(2);
    });
    expect(bodies(client)[1]).toMatchObject({ reviewer: "mira", as: "mira" });
  });

  const closed: [string, string, Partial<Card>][] = [
    ["accepted", "Acceptance", { acceptance: { by: "Ivo Brandt", at: "2026-10-30T10:00:00Z", note: null, verdict: "briefed", departs: [], discrepancy: null } }],
    ["cancelled", "Cancellation", { cancellation: { by: "Ivo Brandt", at: "2026-10-30T10:00:00Z", reason: "Folded into the lamp room's own task." } }],
  ];

  it.each(closed)("the act stands on an open task alone: none on a %s one (U1)", async (_state, part, fields) => {
    const client = taskClient();
    client.getCard.mockImplementation((_project, id) => Promise.resolve(aCard({ id, flow: "narrative", title: "Lamp room dialogue", targets: [FERRY], ...fields })));
    renderApp(client, "#/alpha/board/C-0001");
    const modal = await screen.findByRole("dialog", { name: "Lamp room dialogue" });
    await within(modal).findByRole("region", { name: part });
    expect(within(modal).queryByRole("button", { name: "Split to departments…" })).toBeNull();
  });

  it("holds the task's modal while the split is out: Esc, Close and the scrim close nothing; once it settles the modal stands alone (M: the write outside the card writes' key)", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = taskClient();
    let answer: () => void = () => undefined;
    client.splitCard.mockImplementation((_project, body) =>
      body.dry_run
        ? Promise.resolve(answerOf(body))
        : new Promise<CardSplit>((resolve) => {
            answer = () => {
              resolve(answerOf(body));
            };
          }),
    );
    const { modal, dialog } = await fromTask(client);
    await planned(dialog);
    fireEvent.click(splitButton(dialog));
    await waitFor(() => {
      expect(client.splitCard).toHaveBeenCalledTimes(2);
    });
    fireEvent.keyDown(modal, { key: "Escape" });
    fireEvent.click(within(modal).getByRole("button", { name: /^Close/ }));
    fireEvent.click(document.querySelectorAll("[data-dialog-scrim]")[0] as Element);
    expect(screen.getAllByRole("dialog")).toEqual([modal, dialog]);
    expect(window.location.hash).toBe("#/alpha/board/C-0001");
    await act(async () => {
      answer();
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.getAllByRole("dialog")).toEqual([modal]);
    });
    expect(window.location.hash).toBe("#/alpha/board/C-0001");
  });

  it("after a split the task's Subtasks are read again and list what it made (M: the card lists not read again)", async () => {
    window.localStorage.setItem(ACTING_KEY, "Ann");
    const client = taskClient();
    const flows = [aCardFlow({ name: "narrative", disciplines: ["narrative"] }), aCardFlow({ name: "art", disciplines: ["art"] })];
    const board = [aCardRow({ id: "C-0001", flow: "narrative", title: "Lamp room dialogue", targets: [FERRY], revision: "4" })];
    // The children read answers by `parent`, as the daemon does: none until the split is written.
    let made: CardRow[] = [];
    client.getCards.mockImplementation((_project, options) => Promise.resolve(aCardList(structuredClone(options?.parent === "C-0001" ? made : board), { flows })));
    client.splitCard.mockImplementation((_project, body) => {
      const answer = answerOf(body);
      if (!body.dry_run) {
        made = answer.children.map((child) => aCardRow({ id: child.id ?? "", flow: child.flow, column: child.column, title: "Lamp room dialogue", parent: "C-0001" }));
      }
      return Promise.resolve(answer);
    });
    const childrenReads = () => argsOf(client.getCards).filter((args) => (args[1] as { parent?: string } | undefined)?.parent === "C-0001").length;
    const { modal, dialog } = await fromTask(client);
    const part = await within(modal).findByRole("region", { name: "Subtasks" });
    await within(part).findByText("None yet.");
    const before = childrenReads();
    await planned(dialog);
    fireEvent.click(splitButton(dialog));
    await waitFor(() => {
      expect(screen.getAllByRole("dialog")).toEqual([modal]);
    });
    await waitFor(() => {
      expect(within(part).getAllByRole("link").map((link) => link.textContent)).toEqual(expect.arrayContaining(["C-0041", "C-0042"]));
    });
    expect(childrenReads()).toBe(before + 1);
    expect(within(part).queryByText("None yet.")).toBeNull();
  });
});

// U1 of docs/features/department-split.md "Acceptance criteria": the dialog is a chunk of its own,
// reached from each opener by a dynamic import alone; no module imports it statically.
const OPENERS = import.meta.glob<string>(["/src/board/CardActs.tsx", "/src/doc/DocBody.tsx", "/src/flow/FlowView.tsx"], { query: "?raw", import: "default", eager: true });
const SOURCES = import.meta.glob<string>(["/src/**/*.ts", "/src/**/*.tsx", "!/src/**/*.test.ts", "!/src/**/*.test.tsx"], { query: "?raw", import: "default", eager: true });

/** A module's static import declarations, one per entry, whitespace folded. */
function importLines(text: string): string[] {
  return Array.from(text.matchAll(/^import\s[^;]*;/gm), (match) => match[0].replace(/\s+/g, " "));
}

describe("the Split dialog's chunk (U1 of department-split)", () => {
  it.each([
    ["/src/board/CardActs.tsx", "./SplitDialog"],
    ["/src/doc/DocBody.tsx", "../board/SplitDialog"],
    ["/src/flow/FlowView.tsx", "../board/SplitDialog"],
  ])("%s reaches it by lazy(() => import(...)) alone", (path, specifier) => {
    const text = OPENERS[path] ?? "";
    expect(text).toContain(`const SplitDialog = lazy(() => import("${specifier}").then((module) => ({ default: module.SplitDialog })));`);
    expect(text).toMatch(/<Suspense fallback=\{null\}>\s*<SplitDialog/);
  });

  it("is imported statically by no module, but for its types", () => {
    const value = Object.entries(SOURCES).flatMap(([path, text]) =>
      importLines(text)
        .filter((line) => /["'](\.\.?\/)+(board\/)?SplitDialog["']/.test(line) && !line.startsWith("import type "))
        .map((line) => `${path}: ${line}`),
    );
    expect(value).toEqual([]);
    expect(Object.keys(SOURCES)).toContain("/src/board/SplitDialog.tsx");
  });
});
