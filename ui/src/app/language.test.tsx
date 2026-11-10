import { act, cleanup, fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import { LANGUAGE_KEY, languageName, startLanguage, switchLanguage } from "../i18n";
import { en } from "../i18n/en";
import { ru } from "../i18n/ru";
import { aCardList, aCardRow, aProposal, aTaskEntry, aTreeNode, aTreeView, aWorkspace, aWorkspaces, IMPLICIT_SECTIONS } from "../test/builders";
import { renderApp } from "../test/render";
import { ALL_SECTIONS } from "./routes";
import { isSectionId } from "./sections";
import { stubClient, type StubClient } from "../test/stubClient";
import { enT, ruT } from "../test/text";

// LG-2, LG-5 and LG-6 of docs/features/ui-languages.md over the stub client: the Language select
// switches the chrome at once, reads nothing, keeps focus and every view's state, sets `<html lang>`
// and the choice for the browser; the daemon's words stay byte for byte, tagged English; a first
// visit follows the browser's first language and stores nothing. Russian comes from ru.ts (D13).

const INBOX_NOTE = "queue: 1 proposal skipped: its file is not valid TOML";
const TASK_NOTE = "task list: T-0009 is unreadable and left out";
const CONFIG_NOTE = "specengine.toml:3: workspace `code`: `stage` is no section: left out";

/** Two workspaces (so the select shows), a note on each read, a task waiting, a proposal open. */
function languageClient(): StubClient {
  const client = stubClient([aProposal({ id: "PR-0001", status: "open" })], [INBOX_NOTE]);
  client.getWorkspaces.mockResolvedValue(
    aWorkspaces([aWorkspace({ name: "writers", title: "Writers" }), aWorkspace({ name: "code", title: "Code", sections: ["tasks", "board", "tree"] })], [], { notes: [CONFIG_NOTE] }),
  );
  client.getTasks.mockResolvedValue({ tasks: [aTaskEntry({ id: "T-0001", status: "review" }), aTaskEntry({ id: "T-0002", status: "paused" })], notes: [TASK_NOTE] });
  return client;
}

/** Every call the stub client has seen, over all its methods. */
function callsOf(client: StubClient): number {
  return Object.values(client).reduce((total: number, value: unknown) => total + (vi.isMockFunction(value) ? value.mock.calls.length : 0), 0);
}

function languageSelect(name: string): HTMLSelectElement {
  return screen.getByRole<HTMLSelectElement>("combobox", { name });
}

function nav(name: string): HTMLElement {
  return screen.getByRole("navigation", { name });
}

function navLabels(name: string): string[] {
  return within(nav(name))
    .getAllByRole("link")
    .map((link) => link.textContent);
}

/** What the home, the nav and the header say, in one language's catalog. */
function shownIn(catalog: typeof en | typeof ru) {
  return {
    lang: document.documentElement.lang,
    nav: navLabels(catalog.app.navLabel),
    regions: screen.getAllByRole("heading", { level: 2 }).map((heading) => heading.textContent),
    heading: screen.getByRole("heading", { level: 1 }).textContent,
    workspace: screen.getAllByRole("combobox", { name: catalog.workspace.workspaceLabel }).length,
    badges: [...document.querySelectorAll(".home-view .badge-label")].map((badge) => badge.textContent),
  };
}

/** The nav's labels in one catalog: Overview, then `sections`. */
function navIn(catalog: typeof en | typeof ru, sections: readonly string[]): string[] {
  return [catalog.app.overview, ...sections.filter(isSectionId).map((id) => catalog.app.sections[id].label)];
}

/** The same, as each catalog's values say it should be: the workspace "writers" lists the implicit sections. */
function expectedIn(catalog: typeof en | typeof ru, lang: string) {
  return {
    lang,
    nav: navIn(catalog, IMPLICIT_SECTIONS),
    regions: [catalog.home.tasks.title, catalog.home.inbox.title],
    heading: catalog.home.title,
    workspace: 1,
    badges: [catalog.tasks.status.review, catalog.inbox.status.open, catalog.inbox.status.open],
  };
}

async function homeShown(client: StubClient, catalog: typeof en | typeof ru = en) {
  renderApp(client, "#/alpha");
  await screen.findByRole("combobox", { name: catalog.workspace.workspaceLabel });
  await screen.findByText("T-0001");
  await screen.findByText("PR-0001");
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 30));
  });
}

describe("the switch (LG-2)", () => {
  it("turns the chrome Russian at once, reads nothing, keeps focus on the select and sets <html lang> and the choice", async () => {
    const client = languageClient();
    await homeShown(client);
    expect(shownIn(en)).toEqual(expectedIn(en, "en"));
    const select = languageSelect(en.language.label);
    // Each option its language's own name from Intl (languageName), Russian's before its chunk loads.
    expect([...select.options].map((option) => [option.value, option.textContent, option.lang])).toEqual([
      ["en", "English", "en"],
      ["ru", "\u0420\u0443\u0441\u0441\u043a\u0438\u0439", "ru"],
    ]);
    expect([languageName("en"), languageName("ru")]).toEqual(["English", "\u0420\u0443\u0441\u0441\u043a\u0438\u0439"]);
    select.focus();
    const before = callsOf(client);

    fireEvent.change(select, { target: { value: "ru" } });
    await settle();

    expect(callsOf(client)).toBe(before);
    expect(document.activeElement).toBe(languageSelect(ru.language.label));
    expect(window.localStorage.getItem(LANGUAGE_KEY)).toBe("ru");
    expect(shownIn(ru)).toEqual(expectedIn(ru, "ru"));

    // The palette's section options, in the language shown.
    fireEvent.click(screen.getByRole("button", { name: ru.app.jumpTo }));
    const list = await screen.findByRole("listbox", { name: ru.palette.listLabel });
    const sections = within(list).getByRole("group", { name: ru.palette.groups.sections });
    // Every section, whatever the nav lists.
    expect(within(sections).getAllByRole("option").map((option) => option.textContent)).toEqual(navIn(ru, ALL_SECTIONS));
    fireEvent.keyDown(screen.getByRole("combobox", { name: ru.palette.inputLabel }), { key: "Escape" });

    // English again: every string checked above back to its base text.
    fireEvent.change(languageSelect(ru.language.label), { target: { value: "en" } });
    await settle();
    expect(shownIn(en)).toEqual(expectedIn(en, "en"));
    expect(window.localStorage.getItem(LANGUAGE_KEY)).toBe("en");
    expect(callsOf(client)).toBe(before);
  });

  it("starts a fresh render, as after a reload, in the stored language", async () => {
    await homeShown(languageClient());
    fireEvent.change(languageSelect(en.language.label), { target: { value: "ru" } });
    await settle();
    cleanup();

    // The bootstrap's start, as a reload runs it (index.ts: `lng: startLanguage()`).
    switchLanguage("en");
    switchLanguage(startLanguage());
    await homeShown(languageClient(), ru);
    expect(shownIn(ru)).toEqual(expectedIn(ru, "ru"));
  });

  it("keeps a choice the browser refuses to store until a reload, which is English", async () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("The quota has been exceeded.", "QuotaExceededError");
    });
    await homeShown(languageClient());
    fireEvent.change(languageSelect(en.language.label), { target: { value: "ru" } });
    await settle();
    expect(shownIn(ru)).toEqual(expectedIn(ru, "ru"));
    expect(window.localStorage.getItem(LANGUAGE_KEY)).toBeNull();
    cleanup();

    switchLanguage("en");
    switchLanguage(startLanguage());
    await homeShown(languageClient());
    expect(shownIn(en)).toEqual(expectedIn(en, "en"));
  });

  it("labels one known badge per labels.ts in Russian, and in English again", async () => {
    const client = languageClient();
    client.getCards.mockResolvedValue(aCardList([aCardRow({ id: "C-1", accepted: true })]));
    client.getTree.mockResolvedValue(aTreeView([aTreeNode({ id: "W-1", depth: 0, mark: "dangling-parent" })]));
    renderApp(client, "#/alpha/board");
    await screen.findAllByText(enT("board.accepted"));
    fireEvent.change(languageSelect(en.language.label), { target: { value: "ru" } });
    expect((await screen.findAllByText(ruT("board.accepted"))).length).toBeGreaterThan(0);

    window.location.hash = "#/alpha/tree";
    expect(await screen.findByText(ru.tree.mark.danglingParent)).toBeTruthy();
    window.location.hash = "#/alpha/health";
    expect(await screen.findByText(ru.health.verdict.clean)).toBeTruthy();
    window.location.hash = "#/alpha/flow";
    expect((await screen.findAllByText(ru.flow.gap.none)).length).toBeGreaterThan(0);

    fireEvent.change(languageSelect(ru.language.label), { target: { value: "en" } });
    expect((await screen.findAllByText(en.flow.gap.none)).length).toBeGreaterThan(0);
    window.location.hash = "#/alpha/board";
    expect((await screen.findAllByText(en.board.accepted)).length).toBeGreaterThan(0);
  });
});

/** A list's language and its items' language and text: the list's name is the chrome's, each served item English. */
function itemsOf(list: HTMLElement): [string, string][] {
  expect(list.lang).toBe("");
  return within(list)
    .getAllByRole("listitem")
    .map((item) => [item.lang, item.textContent]);
}

describe("served text verbatim (LG-5)", () => {
  it("shows the daemon's message byte for byte under lang=en, the panel's title and Retry in Russian", async () => {
    const message = "inbox: the index is locked by another `spec` process (pid 4711); try again";
    const client = languageClient();
    client.getInbox.mockRejectedValue(new ClientError({ status: 503, message }));
    switchLanguage("ru");
    renderApp(client, "#/alpha");
    const title = await screen.findByText(ru.home.inbox.failed);
    const panel = title.closest<HTMLElement>(".error-panel");
    const shown = panel?.querySelector<HTMLElement>(".error-message");
    expect(shown?.textContent).toBe(message);
    expect(shown?.lang).toBe("en");
    expect(within(panel as HTMLElement).getByRole("button", { name: ru.ui.retry })).toBeTruthy();
  });

  it("lists the daemon's notes and the config notes verbatim under lang=en", async () => {
    switchLanguage("ru");
    renderApp(languageClient(), "#/alpha");
    const inboxNotes = await screen.findByRole("list", { name: ru.home.inbox.notes });
    expect(itemsOf(inboxNotes)).toEqual([["en", INBOX_NOTE]]);
    const taskNotes = await screen.findByRole("list", { name: ru.home.tasks.notes });
    expect(itemsOf(taskNotes)).toEqual([["en", TASK_NOTE]]);
    fireEvent.click(await screen.findByRole("button", { name: ru.workspace.notesButton.replace("{{n}}", "1") }));
    const configNotes = screen.getByRole("list", { name: ru.workspace.notesTitle });
    expect(itemsOf(configNotes)).toEqual([["en", CONFIG_NOTE]]);
  });

  it("shows a project name as written in Home's skeleton label: data is text, never markup or a key (M15)", async () => {
    const name = "a&b <c> {{n}} $t(x)";
    const client = languageClient();
    client.getProjects.mockResolvedValue([{ slug: name, name: null, root: "/work/x", branch: null }]);
    client.getTasks.mockReturnValue(new Promise(() => undefined));
    client.getWorkspaces.mockResolvedValue(aWorkspaces([aWorkspace({ name: "writers" })]));
    switchLanguage("ru");
    renderApp(client, `#/${encodeURIComponent(name)}`);
    const skeleton = await screen.findByRole("status", { name: ru.home.tasks.loading.replace("{{project}}", name) });
    expect(skeleton.getAttribute("aria-label")).toBe(ru.home.tasks.loading.replace("{{project}}", name));
    switchLanguage("en");
    await settle();
    expect(screen.getByRole("status", { name: `Loading the agent tasks of ${name}` })).toBeTruthy();
  });

  it("shows an unknown task status raw beside a known one in Russian", async () => {
    switchLanguage("ru");
    renderApp(languageClient(), "#/alpha");
    await screen.findByText("T-0001");
    fireEvent.click(screen.getByRole("button", { name: ru.app.jumpTo }));
    const input = await screen.findByRole("combobox", { name: ru.palette.inputLabel });
    fireEvent.change(input, { target: { value: "T-000" } });
    const tasks = await screen.findByRole("group", { name: ru.palette.groups.tasks });
    expect([...tasks.querySelectorAll(".badge-label")].map((badge) => badge.textContent)).toEqual([ru.tasks.status.review, "paused"]);
  });
});

describe("the first visit (LG-6)", () => {
  function browser(languages: readonly string[], language: string) {
    vi.spyOn(window.navigator, "languages", "get").mockReturnValue(languages);
    vi.spyOn(window.navigator, "language", "get").mockReturnValue(language);
  }

  it("is English with lang=en and nothing stored under jsdom's own languages", async () => {
    expect([...navigator.languages]).toEqual(["en-US", "en"]);
    expect(startLanguage()).toBe("en");
    switchLanguage(startLanguage());
    await homeShown(languageClient());
    expect(shownIn(en)).toEqual(expectedIn(en, "en"));
    expect(window.localStorage.getItem(LANGUAGE_KEY)).toBeNull();
  });

  it("follows the browser's first language's primary subtag, storing nothing (M16)", () => {
    browser(["ru-RU", "en"], "ru-RU");
    expect(startLanguage()).toBe("ru");
    browser(["uk", "ru"], "uk");
    expect(startLanguage()).toBe("en");
    browser([], "ru");
    expect(startLanguage()).toBe("ru");
    browser(["RU"], "en");
    expect(startLanguage()).toBe("ru");
    browser([""], "ru");
    expect(startLanguage()).toBe("en");
    expect(window.localStorage.getItem(LANGUAGE_KEY)).toBeNull();
  });

  it("takes a stored choice over the browser, and counts anything but en or ru stored as none", () => {
    browser(["ru-RU"], "ru-RU");
    window.localStorage.setItem(LANGUAGE_KEY, "en");
    expect(startLanguage()).toBe("en");
    window.localStorage.setItem(LANGUAGE_KEY, "ru-RU");
    expect(startLanguage()).toBe("ru");
    browser(["en-GB"], "en-GB");
    window.localStorage.setItem(LANGUAGE_KEY, "ru");
    expect(startLanguage()).toBe("ru");
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("denied", "SecurityError");
    });
    expect(startLanguage()).toBe("en");
  });

  it("writes nothing to the console for a key no catalog holds (a Russian value missing: i18n/fallback.test.tsx)", () => {
    // @ts-expect-error: a key no catalog holds, as a stale build might ask for one.
    expect(ruT("app.gone")).toBe("app.gone");
  });
});
