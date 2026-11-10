import { act, fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import type { ShownLinks } from "../api/types";
import type { Translate } from "../i18n";
import { en } from "../i18n/en";
import { ru } from "../i18n/ru";
import { aCard, aCardFlow, aCardList, aCardRow, aCardWrite, aCheckFinding, aCheckReport, aFlowNode, aFlowView, aNode, aProposal, aRoadmap, aRoadmapGroup, aRoadmapItem, aTreeNode, aTreeView } from "../test/builders";
import { ASIDE, asideClient } from "../test/asideStub";
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

// DF-11 of docs/features/design-foundation.md: the work item's and the header's new words are the
// catalog's, so a Russian page shows the Board, the card's modal with Development and More fields
// open, and the header with no English of their own left; spec content and IDs stay as written.
describe("the work item and the header in Russian (DF-11 of design-foundation)", () => {
  it("shows the Board, the modal with Development and More fields open, and the header in Russian", async () => {
    const client = stubClient();
    client.getCards.mockResolvedValue(aCardList([aCardRow({ id: "C-1" }), aCardRow({ id: "C-2", parent: "C-1", column: "doing" })]));
    client.getCard.mockResolvedValue(
      aCard({
        id: "C-1",
        tasks: [
          { id: "T-0001", status: "review", title: "Light the north lamp", stale: false },
          { id: "T-0002", status: null, title: null, stale: null },
        ],
      }),
    );
    renderApp(client, "#/alpha/board/C-1");
    const dialog = await screen.findByRole("dialog", { name: "Card C-1" });
    await within(dialog).findByRole("region", { name: en.board.panel.details });
    fireEvent.change(languageSelect(en.language.label), { target: { value: "ru" } });
    await settle();
    fireEvent.click(within(dialog).getByRole("button", { name: `${ru.board.item.development} · ${ruT("board.item.agentTasks", { count: 2 })}` }));
    fireEvent.click(within(dialog).getByRole("button", { name: ruT("ui.moreFields", { n: "7" }) }));
    expect(within(dialog).getByRole("region", { name: ru.board.tasks.title })).toBeTruthy();
    expect(within(dialog).getByRole("region", { name: ru.board.item.status })).toBeTruthy();
    expect(within(dialog).getAllByText(ru.ui.emptyValue)).toHaveLength(7);
    expect(within(dialog).getByText(ru.board.item.flow)).toBeTruthy();
    const trail = within(dialog).getByRole("navigation", { name: ru.ui.breadcrumbs });
    expect(within(trail).getByRole("link", { name: ru.app.sections.board.label })).toBeTruthy();
    expect(within(document.querySelector(".app-header") as HTMLElement).getByRole("combobox", { name: ru.app.theme.label })).toBeTruthy();
    // No English of the slice's own is left: its words, the trail's name, the header's Theme.
    const english = [en.board.item.status, en.board.item.flow, en.board.item.noAgentTasks, enT("board.item.agentTasks", { count: 2 }), enT("ui.moreFields", { n: "7" }), en.ui.emptyValue, en.app.theme.label, en.app.theme.system];
    for (const word of english) {
      expect([word, screen.queryAllByText(word, { exact: false }).length]).toEqual([word, 0]);
    }
    expect(screen.queryByRole("navigation", { name: en.ui.breadcrumbs })).toBeNull();
    expect(screen.queryByRole("button", { name: new RegExp(`^${en.board.item.development}`) })).toBeNull();
    // The board under the modal: its heads' lozenges read the served titles, as written.
    expect(document.querySelector(".board-head .lozenge-label")?.textContent).toBe("To do");
  });
});

// FS-23 of flow-scenario (RM-0094): Flow in Russian with the Gaps control, Technical details and
// Records for the AI open, and no English of the slice's own left; the daemon's causes stay English
// under lang="en", spec titles as written.
describe("Flow's scenario in Russian (FS-23 of flow-scenario)", () => {
  it("shows the path, the Gaps control, Technical details and the Records dialog in Russian", async () => {
    const client = asideClient();
    renderApp(client, `#/${ASIDE}/flow?scope=WBAY-KILN`);
    await screen.findByRole("button", { name: /^Records for the AI/ });
    fireEvent.change(languageSelect(en.language.label), { target: { value: "ru" } });
    await settle();
    const places = screen.getByRole("navigation", { name: ru.flow.placesLabel });
    expect(within(places).getByRole("list", { name: ru.flow.path })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: new RegExp(`^${ru.flow.gaps.toggle}`) }));
    const gaps = screen.getByRole("region", { name: ru.flow.gaps.toggle });
    for (const details of within(gaps).getAllByRole("button", { name: ru.ui.technicalDetails })) {
      fireEvent.click(details);
    }
    expect(within(gaps).getByText(ruT("flow.gaps.zero", { count: 6 }))).toBeTruthy();
    const cause = within(gaps).getByText("no start and no entering edge to walk from");
    expect(cause.getAttribute("lang")).toBe("en");
    const panel = screen.getByRole("complementary", { name: ru.flow.panelLabel });
    fireEvent.click(within(panel).getByRole("button", { name: ru.ui.technicalDetails }));
    fireEvent.click(within(places).getByRole("button", { name: ruT("flow.records.open", { count: 141 }) }));
    const dialog = await screen.findByRole("dialog", { name: ruT("flow.records.open", { count: 141 }) });
    await within(dialog).findByRole("group", { name: "hunch" });
    expect(within(dialog).getByText(ruT("flow.records.about", { scope: "Kiln bay" }))).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: ru.ui.close })).toBeTruthy();
    const english = [
      en.flow.path,
      en.flow.gaps.toggle,
      en.flow.gaps.notAll,
      enT("flow.gaps.zero", { count: 6 }),
      en.ui.technicalDetails,
      enT("flow.records.open", { count: 141 }),
      en.flow.records.title,
      enT("flow.records.about", { scope: "Kiln bay" }),
      en.ui.close,
      en.flow.offWalk,
      en.flow.offWalkAbout,
      en.flow.gapMeaning.noStart,
      en.flow.fact.name,
    ];
    for (const word of english) {
      // The daemon's words stay as served, under lang="en".
      expect([word, screen.queryAllByText(word, { exact: false }).filter((element) => element.closest("[lang='en']") === null).map((element) => element.textContent)]).toEqual([word, []]);
    }
  });
});
