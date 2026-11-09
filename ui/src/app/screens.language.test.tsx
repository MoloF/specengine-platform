import { act, fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { ShownLinks } from "../api/types";
import type { Translate } from "../i18n";
import { en } from "../i18n/en";
import { ru } from "../i18n/ru";
import { aCard, aCardFlow, aCardList, aCardRow, aCardWrite, aCheckFinding, aCheckReport, aFlowNode, aFlowView, aNode, aProposal, aRoadmap, aRoadmapGroup, aRoadmapItem, aTreeNode, aTreeView } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient, STAGED_AT, type StubClient } from "../test/stubClient";
import { enT, ruT } from "../test/text";

// LS-3 and LS-4 of docs/features/ui-languages-sweep.md over the stub client: each screen RM-0090
// converted switches to Russian at once, its heading and its own group's words with it, reading
// nothing and keeping focus on the select, and back to English; in Russian the daemon's words stay
// byte for byte under lang="en", and spec content carries no lang. Russian comes from ru.ts (D13).

/** Every call the stub client has seen, over all its methods. */
function callsOf(client: StubClient): number {
  return Object.values(client).reduce((total: number, value: unknown) => total + (vi.isMockFunction(value) ? value.mock.calls.length : 0), 0);
}

function languageSelect(name: string): HTMLSelectElement {
  return screen.getByRole<HTMLSelectElement>("combobox", { name });
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 30));
  });
}

const NO_LINKS: ShownLinks = { outgoing: [], incoming: [], omitted: 0, left_out: { generated: 0, tier3: 0 } };

/** A screen of the sweep: where it lives, its read, its heading and words of its own group. */
interface SweptScreen {
  name: string;
  hash: string;
  client: () => StubClient;
  heading: (t: Translate) => string;
  own: (t: Translate) => string[];
  /** Headings of the screen's own that must read in the language shown, by level. */
  headings?: (t: Translate) => [number, string][];
}

const SCREENS: SweptScreen[] = [
  {
    name: "Inbox",
    hash: "#/alpha/inbox",
    client: () => stubClient([aProposal({ id: "PR-0001", status: "open" })]),
    heading: (t) => t("inbox.title"),
    own: (t) => [t("inbox.filter")],
  },
  {
    name: "Tasks",
    hash: "#/alpha/tasks",
    client: () => stubClient(),
    heading: (t) => t("tasks.title"),
    own: (t) => [t("tasks.noneOpen"), t("tasks.noneYet")],
  },
  {
    name: "Board",
    hash: "#/alpha/board",
    client: () => {
      const client = stubClient();
      client.getCards.mockResolvedValue(aCardList([aCardRow({ id: "C-1" })]));
      return client;
    },
    heading: (t) => t("board.view.title"),
    own: (t) => [t("board.view.showCancelled"), t("board.view.newCard")],
  },
  {
    // BH-21 of board-handoff: the lanes, a card's modal over the board and its hand-offs.
    name: "Board, its lanes and a card's modal",
    hash: "#/alpha/board/C-1",
    client: () => {
      window.localStorage.setItem("specengine.board.lanes.alpha", "parent");
      const client = stubClient();
      client.getCards.mockResolvedValue(aCardList([aCardRow({ id: "C-1" })]));
      client.getCard.mockResolvedValue(aCard({ id: "C-1" }));
      return client;
    },
    heading: (t) => t("board.view.title"),
    own: (t) => [t("board.lanes.label"), t("board.lanes.everythingElse"), t("board.modal.openPage"), t("board.handoff.action"), t("board.modal.handOffs")],
  },
  {
    name: "A card's page",
    hash: "#/alpha/board/C-1/page",
    client: () => {
      const client = stubClient();
      client.getCards.mockResolvedValue(aCardList([aCardRow({ id: "C-1" })]));
      client.getCard.mockResolvedValue(aCard({ id: "C-1" }));
      return client;
    },
    heading: () => "Card C-1",
    own: (t) => [t("board.page.openOnBoard"), t("board.handoff.action"), t("board.modal.handOffs")],
  },
  {
    name: "Roadmap",
    hash: "#/alpha/roadmap",
    client: () => {
      const client = stubClient();
      client.getRoadmap.mockResolvedValue(
        aRoadmap([aRoadmapGroup("release", "beta", [aRoadmapItem({ id: "RM-0001", title: "First item" })], { current: true }), aRoadmapGroup("pool", null, [aRoadmapItem({ id: "RM-0002" })])]),
      );
      return client;
    },
    heading: (t) => t("roadmap.title"),
    own: (t) => [t("roadmap.group.pool"), t("roadmap.lanes")],
    // A lane's name: the pool's heading, the screen's own word (M5: memoised with no dependency, it stays English).
    headings: (t) => [[2, t("roadmap.group.pool")]],
  },
  {
    name: "Flow, a node selected: its links and cards",
    hash: "#/alpha/flow/A",
    client: () => {
      const client = stubClient();
      client.getFlow.mockResolvedValue(aFlowView([aFlowNode({ id: "A", distance: 0, start: true, title: "Ask" })], []));
      client.getNode.mockImplementation((_project, ref) => Promise.resolve({ ref, reason: null, notes: [], nodes: [aNode({ id: ref, links: NO_LINKS })] }));
      return client;
    },
    heading: (t) => t("app.sections.flow.label"),
    own: (t) => [t("tree.links.none"), t("trace.title")],
  },
  {
    name: "Spec tree",
    hash: "#/alpha/tree",
    client: () => stubClient(),
    heading: (t) => t("tree.label"),
    own: (t) => [t("tree.view.includeArchive"), t("tree.node.noneOpen")],
  },
  {
    name: "Graph",
    hash: "#/alpha/graph",
    client: () => stubClient(),
    heading: (t) => t("graph.title"),
    own: (t) => [t("graph.empty")],
  },
  {
    name: "Health",
    hash: "#/alpha/health",
    client: () => stubClient(),
    heading: (t) => t("health.title"),
    own: (t) => [t("health.left.title"), t("health.check.title")],
  },
  {
    name: "Questions",
    hash: "#/alpha/questions",
    client: () => stubClient(),
    heading: (t) => t("round.title"),
    own: (t) => [t("round.emptyTitle")],
  },
];

async function shows(screenCase: SweptScreen, t: Translate) {
  expect(await screen.findByRole("heading", { level: 1, name: screenCase.heading(t) })).toBeTruthy();
  for (const word of screenCase.own(t)) {
    expect([word, (await screen.findAllByText(word)).length > 0]).toEqual([word, true]);
  }
  for (const [level, name] of screenCase.headings?.(t) ?? []) {
    expect([name, screen.queryAllByRole("heading", { level, name }).length > 0]).toEqual([name, true]);
  }
}

describe("each screen switches (LS-3)", () => {
  it.each(SCREENS)("$name: Russian at once, nothing read, focus on the select; English again", async (screenCase) => {
    const client = screenCase.client();
    renderApp(client, screenCase.hash);
    await shows(screenCase, enT);
    await settle();
    const before = callsOf(client);
    const select = languageSelect(en.language.label);
    select.focus();

    fireEvent.change(select, { target: { value: "ru" } });
    await settle();
    expect(screen.getByRole("heading", { level: 1, name: screenCase.heading(ruT) })).toBeTruthy();
    for (const word of screenCase.own(ruT)) {
      expect([word, screen.queryAllByText(word).length > 0]).toEqual([word, true]);
    }
    for (const [level, name] of screenCase.headings?.(ruT) ?? []) {
      expect([name, screen.queryAllByRole("heading", { level, name }).length > 0]).toEqual([name, true]);
    }
    expect(screenCase.own(ruT)).not.toEqual(screenCase.own(enT));
    expect(callsOf(client)).toBe(before);
    expect(document.activeElement).toBe(languageSelect(ru.language.label));

    fireEvent.change(languageSelect(ru.language.label), { target: { value: "en" } });
    await settle();
    await shows(screenCase, enT);
    expect(callsOf(client)).toBe(before);
  });
});

/** The served words' element: their text byte for byte, tagged English. */
function servedIn(element: Element | null | undefined): [string | null, string | null] {
  return [element?.getAttribute("lang") ?? null, element?.textContent ?? null];
}

describe("served text stays as served (LS-4)", () => {
  it("Inbox: the daemon's note and its refused unstage, byte for byte under lang=en", async () => {
    const note = "queue: 1 proposal skipped: its file is not valid TOML";
    const staged = aProposal({ id: "PR-0004", status: "open", staged: { decision: "reject", reason: "dup" }, staged_at: STAGED_AT });
    // A severity and a status no table knows: served words, shown raw.
    const odd = aProposal({ id: "PR-0005", status: "escalated", severity: "urgent" });
    const client = stubClient([staged, odd], [note]);
    const message = "`PR-0004` is approved, not open: nothing changed";
    client.unstageDecision.mockRejectedValueOnce(new ClientError({ status: 409, message }));
    renderApp(client, "#/alpha/inbox/PR-0004");
    await screen.findByRole("heading", { level: 1, name: en.inbox.title });
    fireEvent.change(languageSelect(en.language.label), { target: { value: "ru" } });
    await settle();
    const notes = screen.getByRole("list", { name: ru.inbox.notes });
    expect(servedIn(within(notes).getByRole("listitem"))).toEqual(["en", note]);
    const raw = Array.from(document.querySelectorAll(".queue-list .badge-label"), (label) => servedIn(label)).filter(([, text]) => text === "urgent" || text === "escalated");
    expect(raw).toEqual([
      ["en", "urgent"],
      ["en", "escalated"],
    ]);
    // A known word is the catalog's, untagged.
    expect(servedIn(document.querySelector(".queue-list .badge-label"))).toEqual([null, ru.inbox.severity.normal]);
    fireEvent.click(await screen.findByRole("button", { name: ruT("inbox.staged.unstage", { id: "PR-0004" }) }));
    const refusal = await screen.findByText(ru.inbox.staged.refused);
    // M6: the refusal's lang dropped.
    expect(servedIn(refusal.closest(".refusal")?.querySelector(".verbatim"))).toEqual(["en", message]);
  });

  it("Board: the daemon's notes byte for byte under lang=en", async () => {
    const note = "cards: C-9 is unreadable and left out";
    const client = stubClient();
    client.getCards.mockResolvedValue(aCardList([aCardRow({ id: "C-1" })], { notes: [note] }));
    renderApp(client, "#/alpha/board");
    await screen.findByRole("heading", { level: 1, name: en.board.view.title });
    fireEvent.change(languageSelect(en.language.label), { target: { value: "ru" } });
    await settle();
    const notes = await screen.findByRole("list", { name: ru.board.view.notes });
    expect(servedIn(within(notes).getByRole("listitem"))).toEqual(["en", note]);
  });

  it("Board: a refusal in the Return dialog byte for byte under lang=en, the dialog's words Russian (BH-21)", async () => {
    const message = "C-1 enters `doing`, its flow's return column: say why with `--note`; nothing changed";
    const client = stubClient();
    client.getCards.mockResolvedValue(aCardList([aCardRow({ id: "C-1" })], { flows: [aCardFlow({ return: "doing" })] }));
    client.moveCard.mockRejectedValueOnce(new ClientError({ status: 409, message }));
    client.moveCard.mockResolvedValue(aCardWrite("C-1"));
    renderApp(client, "#/alpha/board");
    await screen.findByRole("heading", { level: 1, name: en.board.view.title });
    fireEvent.change(languageSelect(en.language.label), { target: { value: "ru" } });
    await settle();
    const tile = document.querySelector<HTMLElement>('[data-card="C-1"]');
    tile?.focus();
    fireEvent.keyDown(tile ?? document.body, { key: "r" });
    const dialog = await screen.findByRole("dialog", { name: ruT("board.return.title", { id: "C-1" }) });
    fireEvent.change(within(dialog).getByLabelText(ru.board.field.reason), { target: { value: "\u041d\u0435 \u0442\u043e" } });
    fireEvent.click(within(dialog).getByRole("button", { name: ru.board.return.submit }));
    const refusal = await within(dialog).findByText(message);
    expect(servedIn(refusal)).toEqual(["en", message]);
    expect(within(dialog).getByText(ru.board.view.refused)).toBeTruthy();
  });

  it("Roadmap: the plan's notes byte for byte under lang=en, the lanes' names translated around them", async () => {
    const note = 'RM-0050: release "someday" is not in [roadmap] releases';
    const client = stubClient();
    client.getRoadmap.mockResolvedValue(aRoadmap([aRoadmapGroup("pool", null, [aRoadmapItem({ id: "RM-0002" })])], { notes: [note] }));
    renderApp(client, "#/alpha/roadmap");
    await screen.findByRole("heading", { level: 1, name: en.roadmap.title });
    fireEvent.change(languageSelect(en.language.label), { target: { value: "ru" } });
    await settle();
    const notes = document.querySelector(".roadmap-notes .note-list");
    expect(servedIn(notes?.querySelector("li"))).toEqual(["en", note]);
    expect(screen.getByRole("heading", { level: 2, name: ru.roadmap.notes })).toBeTruthy();
  });

  it("Health: a finding's message byte for byte under lang=en", async () => {
    const message = "R-9 names DOC-7, which no document declares";
    const client = stubClient();
    client.getCheck.mockResolvedValue(aCheckReport({ verdict: "fail", findings: [aCheckFinding({ code: "ref-dangling", message })], counts: { errors: 1 } }));
    renderApp(client, "#/alpha/health");
    await screen.findByText(message);
    fireEvent.change(languageSelect(en.language.label), { target: { value: "ru" } });
    await settle();
    expect(screen.getByRole("heading", { level: 1, name: ru.health.title })).toBeTruthy();
    expect(servedIn(screen.getByText(message))).toEqual(["en", message]);
  });

  it("Spec tree: a node's title is spec content, shown as written with no lang", async () => {
    const title = "Le titre: \u0422\u0438\u0442\u0443\u043b \u00e9crit";
    const client = stubClient();
    client.getTree.mockResolvedValue(aTreeView([aTreeNode({ id: "DOC-1", depth: 0, title })]));
    renderApp(client, "#/alpha/tree");
    await screen.findByText(title);
    fireEvent.change(languageSelect(en.language.label), { target: { value: "ru" } });
    await settle();
    const shown = screen.getByText(title);
    expect([shown.getAttribute("lang"), shown.closest("[lang]")?.tagName]).toEqual([null, "HTML"]);
  });
});
