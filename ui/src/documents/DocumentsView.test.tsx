import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import { ApiProvider } from "../api/provider";
import type { LibraryDocument, LibraryRow, WorkspaceDocument } from "../api/types";
import { docOf } from "../app/routes";
import { switchLanguage } from "../i18n";
import { aDirection, aLibrary, aLibraryChange, aLibraryDecision, aLibraryRow, aModule, aWorkspace, aWorkspaces } from "../test/builders";
import { renderApp } from "../test/render";
import { argsOf, stubClient, type StubClient } from "../test/stubClient";
import { DocumentsView, type DocumentsState } from "./DocumentsView";
import { ALL_CHOSEN, facetChoices, keptRows, moduleChoices, NO_FILTERS, toggleCount } from "./library";

// DL18 and DL19 of docs/features/doc-library.md "Acceptance criteria" on the stub client at
// `#/alpha/documents`: the table drawn, Class at canon when a row is canon and its count line "K of N
// documents", facets AND and OR with each value's count checked against the pure library.ts, All
// classes and All modules one press away, Module at the stored choice (per login on a hub) else the
// department's, the toggles, the search, a title's DocLink, the decisions expanded with their dates,
// commit times and marks, Include archive read again with the table kept, 200 a page, the sort; the
// states, an older daemon's 404, served values as written in either language, the region that
// scrolls. M: OR across facets; canon chosen when no row is canon.

const collator = new Intl.Collator("en", { numeric: true });

const DECIDED = aLibraryChange({ at: "2026-10-01T09:00:00Z", by_name: "Mira Holt" });

/** Five rows: three canon (one behind, with decisions in three groups), a spec page, an ID-less note with a Cyrillic title and a Japanese tag; two modules. */
function served(): LibraryRow[] {
  return [
    aLibraryRow({
      name: "DOM-HARBOR",
      title: "Harbour",
      class: "canon",
      kind: "domain",
      status: "accepted",
      tier: 1,
      owner: "Mira Holt",
      reviewed: "2026-09-20",
      tags: ["clock", "simulation"],
      rev: 4,
      modules: ["waters"],
      used_by: 3,
      last_change: aLibraryChange({ at: "2026-09-20T08:30:00Z", by_name: "Mira Holt" }),
      decisions: [
        aLibraryDecision({ name: "DEC-0012", title: "Weather is simulated", group: "accepted", date: "2026-09-28", section: "RULE-HARBOR-CLOCK", last_change: DECIDED, newer: true }),
        aLibraryDecision({ name: "DEC-0009", title: null, status: "superseded-by DEC-0012", group: "superseded", date: null, last_change: null, newer: null }),
        aLibraryDecision({ name: "DEC-0008", title: "Moons", status: "withdrawn", group: "withdrawn", date: "2026-01-01", last_change: aLibraryChange({ at: "2026-01-01T00:00:00Z" }), newer: false }),
      ],
      behind: true,
    }),
    aLibraryRow({ name: "DOM-WATER", title: "Waters", class: "canon", kind: "domain", status: "accepted", tier: 2, owner: "Ann", tags: ["tides"], modules: ["waters"], used_by: 1 }),
    aLibraryRow({ name: "MEC-TIDES", title: "Tides", class: "canon", kind: "mechanic", status: "draft", tier: 2, owner: null, tags: ["tides", "tuning"], modules: ["waters", "mechanics"], used_by: 5 }),
    aLibraryRow({ name: "MEC-FAIRWAY", title: "Fairway", class: "spec", kind: "mechanic", status: "accepted", tags: ["navigation"], modules: ["mechanics"], orphan: true }),
    aLibraryRow({ name: "docs/notes/cats.md", id: null, path: "docs/notes/cats.md", title: "\u041a\u043e\u0442\u044b \u0433\u0430\u0432\u0430\u043d\u0438", class: null, kind: null, status: null, tags: ["\u6f6e"], rev: null }),
  ];
}

/** Two modules: the rooted Waters, the direction Mechanics; the department `writing` holds Waters. */
function departments(module: string | null = null): WorkspaceDocument {
  return aWorkspaces([aWorkspace({ name: "writing", module })], [aModule({ name: "waters", title: "Waters", root: "DOM-WATER" }), aDirection({ name: "mechanics", title: "Mechanics", kinds: ["mechanic"] })]);
}

function client(rows: LibraryRow[] = served(), fields: Partial<LibraryDocument> = {}, workspaces: WorkspaceDocument = departments()): StubClient {
  const stub = stubClient();
  stub.getWorkspaces.mockResolvedValue(workspaces);
  stub.getLibrary.mockImplementation((_project, options) => Promise.resolve(aLibrary(rows.map((row) => ({ ...row })), { archive: options.archive, ...fields })));
  return stub;
}

/** Documents at `#/alpha/documents`; its table once it stands. */
async function open(stub: StubClient = client()) {
  renderApp(stub, "#/alpha/documents");
  return screen.findByRole("table");
}

/** The shown rows' names, in the table's order. */
function shown(): (string | null)[] {
  return Array.from(document.querySelectorAll(".documents-table tbody tr[data-row]"), (row) => row.getAttribute("data-row"));
}

/** A facet's head button, by its name. */
function facet(name: string): HTMLElement {
  return screen.getByRole("button", { name: new RegExp(`^${name}: `) });
}

/** The chips of an open facet's panel: each label and count. */
function chips(name: string): string[] {
  const panel = screen.getByRole("group", { name });
  return within(panel)
    .getAllByRole("button")
    .map((button) => button.textContent);
}

function count(): string {
  return document.querySelector(".documents-count")?.textContent ?? "";
}

// The view's chunk loaded once before the first test, so its first transform never eats a query's wait.
beforeAll(async () => {
  await import("./DocumentsView");
});

describe("the table and its start (DL18)", () => {
  it("#/<p>/documents draws the table under the h1; Class starts at canon, the count line 'K of N documents' names it; read with no archive and a signal", async () => {
    const stub = client();
    const table = await open(stub);
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Documents");
    expect(shown()).toEqual(["DOM-HARBOR", "DOM-WATER", "MEC-TIDES"]);
    expect(count()).toBe("3 of 5 documents · 1 behind its decision");
    expect(screen.getByRole("table", { name: "3 of 5 documents · 1 behind its decision" })).toBe(table);
    expect(facet("Class").textContent).toBe("Class: canon");
    expect(argsOf(stub.getLibrary).at(-1)).toEqual(["alpha", { archive: false }]);
    expect(stub.getLibrary.mock.calls.at(-1)?.[2]).toBeInstanceOf(AbortSignal);
    // Every column, in order.
    expect(within(table).getAllByRole("columnheader").map((header) => header.textContent)).toEqual([
      "Title",
      "Class",
      "Kind",
      "Status",
      "Version",
      "Owner",
      "Reviewed",
      "Last change",
      "Used by",
      "Decisions",
      "Tags",
    ]);
  });

  it("with no canon row Class starts at All and every row shows: 'N documents' (M: canon chosen over no canon row)", async () => {
    await open(client(served().map((row) => ({ ...row, class: row.class === "canon" ? "spec" : row.class }))));
    expect(shown()).toHaveLength(5);
    expect(count()).toBe("5 documents · 1 behind its decision");
    expect(facet("Class").textContent).toBe("Class: All");
  });

  it("All classes is one press away, and back at All focus moves to the facet's head", async () => {
    await open();
    fireEvent.click(screen.getByRole("button", { name: "All classes" }));
    expect(shown()).toHaveLength(5);
    expect(count()).toBe("5 documents · 1 behind its decision");
    expect(screen.queryByRole("button", { name: "All classes" })).toBeNull();
    await waitFor(() => {
      expect(document.activeElement).toBe(facet("Class"));
    });
  });
});

describe("the facets (DL18)", () => {
  it("a facet's values with the rows the other facets keep, checked against library.ts; two values OR, two facets AND (M: OR across facets)", async () => {
    await open();
    fireEvent.click(facet("Kind"));
    expect(facet("Kind").getAttribute("aria-expanded")).toBe("true");
    const start = { ...NO_FILTERS, chosen: { ...ALL_CHOSEN, class: ["canon"] } };
    const expected = facetChoices(served(), start, null, "kind", collator).map((choice) => `${choice.value ?? "No kind"} ${String(choice.count)}`);
    expect(chips("Kind")).toEqual(expected);
    expect(chips("Kind")).toEqual(["domain 2", "mechanic 1", "No kind 0"]);
    // Two values of Kind OR.
    fireEvent.click(within(screen.getByRole("group", { name: "Kind" })).getByRole("button", { name: "domain 2" }));
    expect(shown()).toEqual(["DOM-HARBOR", "DOM-WATER"]);
    fireEvent.click(within(screen.getByRole("group", { name: "Kind" })).getByRole("button", { name: "mechanic 1" }));
    expect(shown()).toEqual(["DOM-HARBOR", "DOM-WATER", "MEC-TIDES"]);
    expect(facet("Kind").textContent).toBe("Kind: 2 chosen");
    // Class canon AND Kind mechanic, after All classes widens Class: the spec mechanic joins.
    fireEvent.click(screen.getByRole("button", { name: "All classes" }));
    expect(shown()).toEqual(["DOM-HARBOR", "DOM-WATER", "MEC-TIDES", "MEC-FAIRWAY"]);
    fireEvent.click(facet("Tags"));
    const filters = { ...NO_FILTERS, chosen: { ...ALL_CHOSEN, kind: ["domain", "mechanic"] } };
    expect(chips("Tags")).toEqual(facetChoices(served(), filters, null, "tags", collator).map((choice) => `${choice.value ?? "No tags"} ${String(choice.count)}`));
    fireEvent.click(within(screen.getByRole("group", { name: "Tags" })).getByRole("button", { name: /^tides / }));
    // Kind (domain OR mechanic) AND Tags tides.
    expect(shown()).toEqual(["DOM-WATER", "MEC-TIDES"]);
    expect(keptRows(served(), { ...filters, chosen: { ...filters.chosen, tags: ["tides"] } }, null).map((row) => row.name)).toEqual(shown());
  });

  it("'No class' lists the rows with none; a value's chip is pressed with a tick, never colour alone", async () => {
    await open();
    fireEvent.click(screen.getByRole("button", { name: "All classes" }));
    fireEvent.click(facet("Class"));
    const none = within(screen.getByRole("group", { name: "Class" })).getByRole("button", { name: "No class 1" });
    fireEvent.click(none);
    expect(none.getAttribute("aria-pressed")).toBe("true");
    expect(none.querySelector(".chip-check svg")).not.toBeNull();
    expect(shown()).toEqual(["docs/notes/cats.md"]);
  });

  it("Module starts at the stored choice, lists All modules then the modules by title with their counts, and writes the stored choice", async () => {
    window.localStorage.setItem("specengine.module.alpha", "mechanics");
    await open();
    expect(facet("Module").textContent).toBe("Module: Mechanics");
    expect(shown()).toEqual(["MEC-TIDES"]);
    fireEvent.click(facet("Module"));
    const start = { ...NO_FILTERS, chosen: { ...ALL_CHOSEN, class: ["canon"] } };
    const { all, choices } = moduleChoices(served(), start, departments().modules, collator);
    expect(chips("Module")).toEqual([`All modules ${String(all)}`, ...choices.map((choice) => `${choice.module.title} ${String(choice.count)}`)]);
    expect(chips("Module")).toEqual(["All modules 3", "Mechanics 1", "Waters 3"]);
    fireEvent.click(within(screen.getByRole("group", { name: "Module" })).getByRole("button", { name: "Waters 3" }));
    expect(window.localStorage.getItem("specengine.module.alpha")).toBe("waters");
    expect(shown()).toEqual(["DOM-HARBOR", "DOM-WATER", "MEC-TIDES"]);
  });

  it("All modules is one press away and stores All modules", async () => {
    window.localStorage.setItem("specengine.module.alpha", "mechanics");
    await open();
    fireEvent.click(screen.getByRole("button", { name: "All modules" }));
    expect(window.localStorage.getItem("specengine.module.alpha")).toBe("");
    expect(facet("Module").textContent).toBe("Module: All");
    expect(shown()).toEqual(["DOM-HARBOR", "DOM-WATER", "MEC-TIDES"]);
  });

  it("Module unset starts at the department's module", async () => {
    await open(client(served(), {}, departments("waters")));
    expect(facet("Module").textContent).toBe("Module: Waters");
    expect(shown()).toEqual(["DOM-HARBOR", "DOM-WATER", "MEC-TIDES"]);
  });

  it("on a hub Module reads the login's stored choice, never the browser-wide one", async () => {
    const stub = client();
    stub.getIdentity.mockResolvedValue({ login: "mira" });
    window.localStorage.setItem("specengine.module.alpha", "waters");
    window.localStorage.setItem("specengine.module.alpha.mira", "mechanics");
    await open(stub);
    await waitFor(() => {
      expect(facet("Module").textContent).toBe("Module: Mechanics");
    });
    expect(shown()).toEqual(["MEC-TIDES"]);
  });

  it("no module served: no Module facet", async () => {
    await open(client(served(), {}, aWorkspaces([aWorkspace({ name: "writing" })])));
    expect(screen.queryByRole("button", { name: /^Module: / })).toBeNull();
  });

  it("Behind its decision and Orphans keep those rows, each with the rows it would keep", async () => {
    await open();
    const behind = screen.getByRole("button", { name: "Behind its decision 1" });
    fireEvent.click(behind);
    expect(behind.getAttribute("aria-pressed")).toBe("true");
    expect(shown()).toEqual(["DOM-HARBOR"]);
    fireEvent.click(behind);
    fireEvent.click(screen.getByRole("button", { name: "All classes" }));
    expect(toggleCount(served(), NO_FILTERS, null, "orphans")).toBe(1);
    fireEvent.click(screen.getByRole("button", { name: "Orphans 1" }));
    expect(shown()).toEqual(["MEC-FAIRWAY"]);
  });

  it("search covers title, name, path and tags, case folded, in any script", async () => {
    await open();
    fireEvent.click(screen.getByRole("button", { name: "All classes" }));
    const field = screen.getByLabelText("Search");
    const search = (text: string) => {
      fireEvent.change(field, { target: { value: text } });
      return shown();
    };
    expect(search("waters")).toEqual(["DOM-WATER"]);
    expect(search("mec-fair")).toEqual(["MEC-FAIRWAY"]);
    expect(search("NOTES/CATS")).toEqual(["docs/notes/cats.md"]);
    expect(search("\u043a\u043e\u0442\u044b")).toEqual(["docs/notes/cats.md"]);
    expect(search("tuning")).toEqual(["MEC-TIDES"]);
    expect(search("\u6f6e")).toEqual(["docs/notes/cats.md"]);
    expect(count()).toBe("1 of 5 documents");
  });

  it("no match: an empty state with Clear filters, which sets every facet to All, focus on the search", async () => {
    await open();
    fireEvent.change(screen.getByLabelText("Search"), { target: { value: "nothing like it" } });
    const title = screen.getByRole("heading", { name: "No document matches the filters" });
    expect(title.tagName).toBe("H2");
    expect(screen.queryByRole("table")).toBeNull();
    fireEvent.click(within(title.closest(".empty-state-box") as HTMLElement).getByRole("button", { name: "Clear filters" }));
    expect(shown()).toHaveLength(5);
    expect(count()).toBe("5 documents · 1 behind its decision");
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByLabelText("Search"));
    });
  });
});

describe("the rows (DL18)", () => {
  it("a title is a DocLink by name opening ?doc=<name>, the name below in mono when it differs", async () => {
    await open();
    const link = screen.getByRole("link", { name: "Harbour" });
    expect(link.getAttribute("href")).toBe("#/alpha/tree/DOM-HARBOR/page");
    expect(link.closest("th")?.querySelector(".documents-name")?.textContent).toBe("DOM-HARBOR");
    fireEvent.click(link);
    expect(docOf(window.location.hash)).toBe("DOM-HARBOR");
  });

  it("Decisions expands its groups in served order, a raw group under lang=en, each with its date, commit time, section and Newer mark; Behind its decision shows on the behind row", async () => {
    await open();
    const row = document.querySelector('tr[data-row="DOM-HARBOR"]') as HTMLElement;
    expect(within(row).getByText("Behind its decision")).not.toBeNull();
    expect(document.querySelector('tr[data-row="DOM-WATER"] .lozenge')).toBeNull();
    const button = within(row).getByRole("button", { name: "1 accepted · 1 superseded · 1 withdrawn" });
    expect(button.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(button);
    expect(button.getAttribute("aria-expanded")).toBe("true");
    const detail = document.getElementById(button.getAttribute("aria-controls") ?? "");
    expect(detail?.tagName).toBe("TR");
    expect(detail?.querySelector("td")?.getAttribute("colspan")).toBe("11");
    const groups = within(detail as HTMLElement).getByRole("group", { name: "Decisions of Harbour" });
    const titles = Array.from(groups.querySelectorAll(".documents-group-title"));
    expect(titles.map((title) => [title.textContent, title.getAttribute("lang")])).toEqual([
      ["Accepted", null],
      ["Superseded", null],
      ["withdrawn", "en"],
    ]);
    const accepted = within(groups).getByRole("group", { name: "Accepted" });
    expect(within(accepted).getByRole("link", { name: "Weather is simulated" }).getAttribute("href")).toBe("#/alpha/tree/DEC-0012/page");
    expect(accepted.textContent).toContain("2026-09-28");
    expect(accepted.textContent).toContain("Committed 2026-10-01 09:00 UTC");
    expect(accepted.textContent).toContain("Section RULE-HARBOR-CLOCK");
    expect(within(accepted).getByText("Newer than this document")).not.toBeNull();
    const superseded = within(groups).getByRole("group", { name: "Superseded" });
    expect(within(superseded).getByRole("link", { name: "DEC-0009" })).not.toBeNull();
    expect(superseded.textContent).toContain("No date");
    expect(superseded.textContent).toContain("Not committed");
    expect(within(superseded).queryByText("Newer than this document")).toBeNull();
    fireEvent.click(button);
    expect(document.getElementById(detail?.id ?? "")).toBeNull();
  });

  it("a row with no decision shows a dash; no OID, email or proposal reaches the page", async () => {
    await open();
    const water = document.querySelector('tr[data-row="DOM-WATER"]') as HTMLElement;
    expect(within(water).queryByRole("button", { name: /accepted/ })).toBeNull();
    expect(document.body.textContent).not.toContain("example.org");
    expect(document.body.textContent).not.toContain("a".repeat(40));
  });

  it("Include archive reads again with archive, the last table up meanwhile", async () => {
    const stub = client();
    await open(stub);
    let land: () => void = () => undefined;
    stub.getLibrary.mockImplementationOnce(
      (_project, options) =>
        new Promise((resolve) => {
          land = () => {
            resolve(aLibrary([...served(), aLibraryRow({ name: "MEC-OLD", class: "canon", archived: true, tier: 3 })], { archive: options.archive }));
          };
        }),
    );
    fireEvent.click(screen.getByLabelText("Include archive"));
    await waitFor(() => {
      expect(argsOf(stub.getLibrary).at(-1)).toEqual(["alpha", { archive: true }]);
    });
    expect(shown()).toEqual(["DOM-HARBOR", "DOM-WATER", "MEC-TIDES"]);
    expect(document.querySelector(".documents-view")?.getAttribute("aria-busy")).toBe("true");
    await act(async () => {
      land();
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(shown()).toEqual(["DOM-HARBOR", "DOM-WATER", "MEC-TIDES", "MEC-OLD"]);
    });
  });

  it("200 rows a page, then Show 200 more, focus on the first new row's link", async () => {
    const many = Array.from({ length: 450 }, (_, at) => aLibraryRow({ name: `DOC-${String(at).padStart(3, "0")}` }));
    await open(client(many));
    expect(shown()).toHaveLength(200);
    fireEvent.click(screen.getByRole("button", { name: "Show 200 more" }));
    expect(shown()).toHaveLength(400);
    await waitFor(() => {
      expect(document.activeElement?.textContent).toBe("Title of DOC-200");
    });
    fireEvent.click(screen.getByRole("button", { name: "Show 50 more" }));
    expect(shown()).toHaveLength(450);
    expect(screen.queryByRole("button", { name: /^Show \d+ more$/ })).toBeNull();
    // 450 rows drawn in jsdom: about 0.6 s alone, several on a laptop busy with a Rust build.
  }, 20_000);

  it("a header's press sorts as the catalog does: Title ascending first, Used by descending first, aria-sort saying so", async () => {
    await open();
    const title = screen.getByRole("button", { name: "Title" });
    fireEvent.click(title);
    expect(title.closest("th")?.getAttribute("aria-sort")).toBe("ascending");
    expect(shown()).toEqual(["DOM-HARBOR", "MEC-TIDES", "DOM-WATER"]);
    fireEvent.click(screen.getByRole("button", { name: "Used by" }));
    expect(screen.getByRole("button", { name: "Used by" }).closest("th")?.getAttribute("aria-sort")).toBe("descending");
    expect(title.closest("th")?.hasAttribute("aria-sort")).toBe(false);
    expect(shown()).toEqual(["MEC-TIDES", "DOM-HARBOR", "DOM-WATER"]);
    // Decisions and Tags hold lists: no press.
    expect(within(screen.getByRole("table")).getByRole("columnheader", { name: "Tags" }).querySelector("button")).toBeNull();
  });

  it("the table scrolls in its own focusable region, named by the count line", async () => {
    await open();
    const region = screen.getByRole("region", { name: "3 of 5 documents · 1 behind its decision" });
    expect(region.getAttribute("tabindex")).toBe("0");
    expect(region.querySelector("table")).not.toBeNull();
  });
});

describe("the states and languages (DL19)", () => {
  it("a skeleton while reading", async () => {
    const stub = client();
    stub.getLibrary.mockImplementation(() => new Promise(() => undefined));
    renderApp(stub, "#/alpha/documents");
    expect(await screen.findByRole("status", { name: "Reading the documents of alpha" })).not.toBeNull();
    expect(screen.queryByRole("table")).toBeNull();
  });

  it("no document: the empty state under the h1, no filter bar", async () => {
    renderApp(client([]), "#/alpha/documents");
    const title = await screen.findByRole("heading", { name: "No document yet" });
    expect(title.tagName).toBe("H2");
    expect(screen.queryByLabelText("Search")).toBeNull();
  });

  it("an older daemon's 404 in its own words with Retry; Retry reads again and the table lands", async () => {
    const stub = client();
    const answer = stub.getLibrary.getMockImplementation();
    const message = "no route /api/projects/alpha/library: the routes are /api/projects and the web UI";
    stub.getLibrary.mockRejectedValueOnce(new ClientError({ status: 404, message }));
    renderApp(stub, "#/alpha/documents");
    expect(await screen.findByText(message)).not.toBeNull();
    expect(screen.getByText("The documents could not be read")).not.toBeNull();
    expect(screen.getByText(message).getAttribute("lang")).toBe("en");
    if (answer !== undefined) {
      stub.getLibrary.mockImplementation(answer);
    }
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("table")).not.toBeNull();
    expect(screen.queryByText(message)).toBeNull();
  });

  it("the daemon's notes under lang=en", async () => {
    await open(client(served(), { notes: ["library: last change not read: not a repository"] }));
    const notes = screen.getByRole("list", { name: "Notes from the daemon" });
    expect(within(notes).getByText("library: last change not read: not a repository").getAttribute("lang")).toBe("en");
  });

  it("served values stay as written in Russian: titles, tags, kinds, statuses, owners; the chrome translated", async () => {
    await open();
    act(() => {
      switchLanguage("ru");
    });
    await screen.findByRole("heading", { level: 1, name: "\u0414\u043e\u043a\u0443\u043c\u0435\u043d\u0442\u044b" });
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("\u0414\u043e\u043a\u0443\u043c\u0435\u043d\u0442\u044b");
    const row = document.querySelector('tr[data-row="DOM-HARBOR"]') as HTMLElement;
    for (const value of ["Harbour", "domain", "accepted", "Mira Holt", "clock", "simulation"]) {
      expect([value, within(row).getAllByText(value).length > 0]).toEqual([value, true]);
    }
    fireEvent.click(screen.getByRole("button", { name: /^\u0412\u0441\u0435 \u043a\u043b\u0430\u0441\u0441\u044b$/ }));
    expect(screen.getByRole("link", { name: "\u041a\u043e\u0442\u044b \u0433\u0430\u0432\u0430\u043d\u0438" })).not.toBeNull();
    expect(within(document.querySelector('tr[data-row="docs/notes/cats.md"]') as HTMLElement).getByText("\u6f6e")).not.toBeNull();
  });
});

/** A client whose `archive=true` read fails in the daemon's words; the live read answers `served()`. */
function archiveFails(message: string): StubClient {
  const stub = client();
  const answer = stub.getLibrary.getMockImplementation();
  stub.getLibrary.mockImplementation((project, options, signal) =>
    options.archive ? Promise.reject(new ClientError({ status: 503, message })) : (answer?.(project, options, signal) ?? Promise.reject(new Error("no answer"))),
  );
  return stub;
}

/** Moves the hash as a followed link does, the view following it. */
async function go(hash: string) {
  await act(async () => {
    window.location.hash = hash;
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

function archiveBox(): HTMLInputElement {
  return screen.getByLabelText("Include archive");
}

// The review of 2026-11-19: a failed Include archive read has no placeholder (TanStack keeps one only
// while pending), so the view keeps the last answered rows and their bar, and remembers Include
// archive only once its read has answered. M: `data = read.data` (the bar vanishes); remembering the
// asked `archive` (coming back asks the failing read again).
describe("a failed Include archive (review of DL18)", () => {
  const message = "spec library: the walk stopped: docs/archive is unreadable";

  it("keeps the last answered rows and the bar over them, the checkbox checked and reachable, the failure in the daemon's words", async () => {
    const stub = archiveFails(message);
    await open(stub);
    fireEvent.click(archiveBox());
    expect(await screen.findByText(message)).not.toBeNull();
    expect(screen.getByText(message).getAttribute("lang")).toBe("en");
    expect(shown()).toEqual(["DOM-HARBOR", "DOM-WATER", "MEC-TIDES"]);
    expect(archiveBox().checked).toBe(true);
    expect(facet("Class").textContent).toBe("Class: canon");
    // One press off: the live rows again, read from the cache, the failure gone.
    fireEvent.click(archiveBox());
    await waitFor(() => {
      expect(screen.queryByText(message)).toBeNull();
    });
    expect(archiveBox().checked).toBe(false);
    expect(shown()).toEqual(["DOM-HARBOR", "DOM-WATER", "MEC-TIDES"]);
  });

  it("is not remembered while its read has not answered: coming back reads no archive", async () => {
    const stub = archiveFails(message);
    await open(stub);
    fireEvent.click(archiveBox());
    expect(await screen.findByText(message)).not.toBeNull();
    await go("#/alpha/inbox");
    await screen.findByRole("heading", { level: 1, name: "Inbox" });
    await go("#/alpha/documents");
    await screen.findByRole("table");
    expect(archiveBox().checked).toBe(false);
    expect(screen.queryByText(message)).toBeNull();
    expect(stub.getLibrary.mock.calls.filter(([, options]) => options.archive)).toHaveLength(1);
  });

  it("once answered it is remembered: coming back reads with archive", async () => {
    const stub = client();
    await open(stub);
    fireEvent.click(archiveBox());
    await waitFor(() => {
      expect(argsOf(stub.getLibrary).at(-1)).toEqual(["alpha", { archive: true }]);
    });
    await waitFor(() => {
      expect(document.querySelector(".documents-view")?.getAttribute("aria-busy")).toBe("false");
    });
    await go("#/alpha/inbox");
    await screen.findByRole("heading", { level: 1, name: "Inbox" });
    await go("#/alpha/documents");
    await screen.findByRole("table");
    expect(archiveBox().checked).toBe(true);
  });

  /** DocumentsView alone, its page memory recalling Include archive on, so its first read is the archive's. */
  function aloneWithArchive(stub: StubClient, kept: DocumentsState[] = []) {
    const memory = {
      recall: () => ({ filters: null, archive: true, sort: null }),
      remember: (_project: string, state: DocumentsState) => {
        kept.push(state);
      },
    };
    render(
      <ApiProvider client={stub}>
        <DocumentsView project="alpha" memory={memory} modules={[]} module={null} onChooseModule={() => undefined} moduleKnown />
      </ApiProvider>,
    );
  }

  it("with no answer yet, the checkbox stands alone under the failure and keeps the focus while the live rows are read; once they land focus moves to the bar's checkbox (M: the lone checkbox unmounted while reading)", async () => {
    const stub = archiveFails(message);
    const answer = stub.getLibrary.getMockImplementation();
    let land: () => void = () => undefined;
    stub.getLibrary.mockImplementation((project, options, signal) =>
      options.archive
        ? Promise.reject(new ClientError({ status: 503, message }))
        : new Promise((resolve, reject) => {
            land = () => {
              (answer?.(project, options, signal) ?? Promise.reject(new Error("no answer"))).then(resolve, reject);
            };
          }),
    );
    const kept: DocumentsState[] = [];
    aloneWithArchive(stub, kept);
    expect(await screen.findByText(message)).not.toBeNull();
    expect(screen.queryByRole("table")).toBeNull();
    expect(archiveBox().checked).toBe(true);
    // The failing read is not what the page remembers.
    expect(kept.at(-1)?.archive).toBe(false);
    const lone = archiveBox();
    lone.focus();
    fireEvent.click(lone);
    await waitFor(() => {
      expect(argsOf(stub.getLibrary).at(-1)).toEqual(["alpha", { archive: false }]);
    });
    // Reading: the failure kept up, the lone checkbox still there, unchecked, with the focus.
    expect(screen.getByText(message)).not.toBeNull();
    expect(archiveBox()).toBe(lone);
    expect(lone.checked).toBe(false);
    expect(document.activeElement).toBe(lone);
    await act(async () => {
      land();
      await Promise.resolve();
    });
    expect(await screen.findByRole("table")).not.toBeNull();
    expect(screen.queryByText(message)).toBeNull();
    await waitFor(() => {
      expect(document.activeElement).toBe(archiveBox());
    });
    expect(archiveBox().closest(".documents-facets")).not.toBeNull();
    expect(document.activeElement).not.toBe(document.body);
  });

  it("says the kept rows are without the archive under a checked Include archive whose read failed", async () => {
    await open(archiveFails(message));
    expect(screen.queryByText("Showing the last answer, without the archive")).toBeNull();
    fireEvent.click(archiveBox());
    expect(await screen.findByText(message)).not.toBeNull();
    expect(screen.getByText("Showing the last answer, without the archive")).not.toBeNull();
    // Unchecked, the rows are the asked ones again: no line.
    fireEvent.click(archiveBox());
    await waitFor(() => {
      expect(screen.queryByText("Showing the last answer, without the archive")).toBeNull();
    });
  });

  it("says the kept rows are with the archive under an unchecked Include archive whose read failed, in either language", async () => {
    const stub = client();
    const answer = stub.getLibrary.getMockImplementation();
    stub.getLibrary.mockImplementation((project, options, signal) =>
      options.archive ? (answer?.(project, options, signal) ?? Promise.reject(new Error("no answer"))) : Promise.reject(new ClientError({ status: 503, message })),
    );
    aloneWithArchive(stub);
    await screen.findByRole("table");
    expect(archiveBox().checked).toBe(true);
    expect(screen.queryByText("Showing the last answer, with the archive")).toBeNull();
    fireEvent.click(archiveBox());
    expect(await screen.findByText(message)).not.toBeNull();
    expect(archiveBox().checked).toBe(false);
    expect(screen.getByRole("table")).not.toBeNull();
    expect(screen.getByText("Showing the last answer, with the archive")).not.toBeNull();
    act(() => {
      switchLanguage("ru");
    });
    expect(await screen.findByText("\u041f\u043e\u043a\u0430\u0437\u0430\u043d \u043f\u043e\u0441\u043b\u0435\u0434\u043d\u0438\u0439 \u043e\u0442\u0432\u0435\u0442, \u0441 \u0430\u0440\u0445\u0438\u0432\u043e\u043c")).not.toBeNull();
  });
});

describe("the review's nits (DL18)", () => {
  it("a decision group this build does not know is raw under lang=en in the Decisions cell too", async () => {
    await open();
    const row = document.querySelector('tr[data-row="DOM-HARBOR"]') as HTMLElement;
    const raw = within(row).getByText("withdrawn");
    expect(raw.getAttribute("lang")).toBe("en");
    expect(raw.closest("button")?.textContent).toBe("1 accepted \u00b7 1 superseded \u00b7 1 withdrawn");
  });

  it("a served empty value and none are two chips, keyed apart", async () => {
    await open(client([aLibraryRow({ name: "DOC-A", class: "canon", owner: "" }), aLibraryRow({ name: "DOC-B", class: "canon", owner: null })]));
    fireEvent.click(facet("Owner"));
    expect(chips("Owner")).toEqual([" 1", "No owner 1"]);
  });
});

// docs/features/department-practices.md "Acceptance criteria", DP21 and "The library": the Discipline
// facet, after Tags, keeps the rows whose `disciplines` hold a chosen one: each department's corner.
describe("the Discipline facet (DP21 of department-practices)", () => {
  function practices(): LibraryRow[] {
    return served().map((row) => ({ ...row, disciplines: row.name === "DOM-HARBOR" ? ["copy", "plates"] : row.name === "MEC-TIDES" ? ["copy"] : [] }));
  }

  it("stands after Tags, before Module; its values counted as library.ts counts them, a press keeping that discipline's practices", async () => {
    await open(client(practices()));
    const heads = within(screen.getByRole("group", { name: "Filters" }))
      .getAllByRole("button")
      .map((button) => button.textContent.split(": ")[0]);
    expect(heads.slice(heads.indexOf("Tags"), heads.indexOf("Tags") + 3)).toEqual(["Tags", "Discipline", "Module"]);
    fireEvent.click(facet("Discipline"));
    const start = { ...NO_FILTERS, chosen: { ...ALL_CHOSEN, class: ["canon"] } };
    expect(chips("Discipline")).toEqual(facetChoices(practices(), start, null, "disciplines", collator).map((choice) => `${choice.value ?? "No discipline"} ${String(choice.count)}`));
    expect(chips("Discipline")).toEqual(["copy 2", "plates 1", "No discipline 1"]);
    fireEvent.click(within(screen.getByRole("group", { name: "Discipline" })).getByRole("button", { name: "plates 1" }));
    expect(shown()).toEqual(["DOM-HARBOR"]);
    expect(facet("Discipline").textContent).toBe("Discipline: plates");
    fireEvent.click(screen.getByRole("button", { name: "All disciplines" }));
    expect(shown()).toEqual(["DOM-HARBOR", "DOM-WATER", "MEC-TIDES"]);
    // The panel stays open: "No discipline" keeps the rows naming none.
    fireEvent.click(within(screen.getByRole("group", { name: "Discipline" })).getByRole("button", { name: "No discipline 1" }));
    expect(shown()).toEqual(["DOM-WATER"]);
  });
});
