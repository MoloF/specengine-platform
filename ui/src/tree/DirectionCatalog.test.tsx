import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { CatalogRow } from "../api/types";
import appCss from "../styles/app.css?raw";
import { aCatalog, aCatalogChange, aCatalogRow, aDirection, aModule, aTreeNode, aTreeView, aWorkspace, aWorkspaces } from "../test/builders";
import { pinnedCatalog } from "../test/catalogStub";
import { renderApp } from "../test/render";
import { argsOf, type StubClient } from "../test/stubClient";
import { treeClient } from "../test/treeStub";

// DC-10 of docs/features/direction-catalog.md "Acceptance criteria" on the stub client: a kinds
// direction chosen and no node open, the node column holds the direction's table, captioned by its
// title and its row count; a rooted module or an open node holds none. A header's press orders the
// served rows, `aria-sort` saying how; a title opens the document modal, its anchor the opener; an
// unknown state is raw under `lang="en"`; no email, OID or proposal reaches the DOM. DC-6's 50, then
// Show more, here over built rows; over the mock in src/mocks/catalog.smoke.test.tsx.

const SCENES = aDirection({ name: "scenes", title: "Scenes", kinds: ["scene"] });
const CUES = aDirection({ name: "cues", title: "Cues", kinds: ["cue"] });

/** A client whose project configures the directions Scenes and Cues and the rooted module Shore; the catalog answers `rows`. */
function catalogClient(rows: CatalogRow[] = pinnedCatalog().rows, notes: string[] = []): StubClient {
  const client = treeClient();
  client.getWorkspaces.mockResolvedValue(aWorkspaces([aWorkspace({ name: "writing" })], [aModule({ name: "shore", root: "DOC-A" }), SCENES, CUES]));
  const read = client.getTree.getMockImplementation();
  client.getTree.mockImplementation((project, options, signal) =>
    options?.kinds !== undefined
      ? Promise.resolve(aTreeView([aTreeNode({ id: "SCN-ARRIVAL", depth: 0, kind: "scene", title: "Arrival" })], { kinds: options.kinds }))
      : (read?.(project, options, signal) ?? Promise.resolve(aTreeView([]))),
  );
  client.getCatalog.mockImplementation((_project, options) => Promise.resolve(aCatalog(rows.map((row) => ({ ...row })), { kinds: [...options.kinds], archive: options.archive, notes })));
  return client;
}

/** The Spec tree at `hash` with `module` chosen; the direction's table once it stands. */
async function openDirection(client: StubClient, module = "scenes", hash = "#/alpha/tree") {
  window.localStorage.setItem("specengine.module.alpha", module);
  renderApp(client, hash);
  return screen.findByRole("table");
}

/** The served names of the shown rows, in the table's order. */
function shownRows(): (string | null)[] {
  return Array.from(document.querySelectorAll(".catalog-table tbody tr"), (row) => row.getAttribute("data-row"));
}

function header(name: string): HTMLElement {
  const found = screen.getByRole("button", { name }).closest("th");
  if (found === null) {
    throw new Error(`no header ${name}`);
  }
  return found;
}

/** `count` rows, `SCN-001`…, their versions descending with the served place, a tie every other row. */
function manyRows(count: number): CatalogRow[] {
  return Array.from({ length: count }, (_, at) => aCatalogRow({ id: `SCN-${String(at + 1).padStart(3, "0")}`, title: `Scene ${String(at + 1)}`, rev: 1 + (at % 2) }));
}

// The table's chunk loaded once before the first test, so its first transform never eats a query's wait.
beforeAll(async () => {
  await import("./DirectionCatalog");
});

describe("where the table stands (DC-10)", () => {
  it("a direction chosen, no node open: the table, captioned by the direction's title and its count, in the node column under the view's h1; read by its kinds, no archive, with a signal", async () => {
    const client = catalogClient();
    const table = await openDirection(client);
    expect(table.closest(".node-column")).not.toBeNull();
    expect(table.querySelector("caption")?.textContent).toBe("Scenes: 2 nodes");
    expect(table.querySelector("caption bdi")?.textContent).toBe("Scenes");
    expect(screen.getByRole("table", { name: "Scenes: 2 nodes" })).toBe(table);
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Spec tree");
    expect(screen.queryByText("No node open")).toBeNull();
    expect(argsOf(client.getCatalog).at(-1)).toEqual(["alpha", { kinds: ["scene"], archive: false }]);
    expect(client.getCatalog.mock.calls.at(-1)?.[2]).toBeInstanceOf(AbortSignal);
  });

  it("a rooted module holds none: where to start, the catalog never read", async () => {
    window.localStorage.setItem("specengine.module.alpha", "shore");
    const client = catalogClient();
    renderApp(client, "#/alpha/tree");
    await screen.findByText("No node open");
    expect(screen.queryByRole("table")).toBeNull();
    expect(client.getCatalog).not.toHaveBeenCalled();
  });

  it("an open node holds its pane, no table; leaving it, the table again", async () => {
    window.localStorage.setItem("specengine.module.alpha", "scenes");
    const client = catalogClient();
    renderApp(client, "#/alpha/tree/DOC-A");
    await screen.findByRole("heading", { level: 1, name: /DOC-A/ });
    expect(screen.queryByRole("table")).toBeNull();
    act(() => {
      window.location.hash = "#/alpha/tree";
    });
    expect(await screen.findByRole("table", { name: "Scenes: 2 nodes" })).not.toBeNull();
  });

  it("no direction chosen: where to start, no table", async () => {
    const client = catalogClient();
    renderApp(client, "#/alpha/tree");
    await screen.findByText("No node open");
    expect(screen.queryByRole("table")).toBeNull();
    expect(client.getCatalog).not.toHaveBeenCalled();
  });
});

describe("a row (DC-10)", () => {
  it("the spec's rows: the title a document link, its ID in mono, Version N, the state, open items, tasks, the last change by name", async () => {
    await openDirection(catalogClient());
    const arrival = document.querySelector<HTMLTableRowElement>('tr[data-row="SCN-ARRIVAL"]');
    if (arrival === null) {
      throw new Error("no SCN-ARRIVAL row");
    }
    const link = within(arrival).getByRole("link", { name: "Arrival" });
    expect(link.getAttribute("href")).toBe("#/alpha/tree/SCN-ARRIVAL/page");
    expect(arrival.querySelector(".catalog-id.mono")?.textContent).toBe("SCN-ARRIVAL");
    expect(Array.from(arrival.cells, (cell) => cell.textContent)).toEqual(["ArrivalSCN-ARRIVAL", "Version 1", "In work", "1", "1 · 1 open", "1", "2026-01-02 10:00 UTC by Ann Owner"]);
    expect(arrival.querySelector("time")?.getAttribute("dateTime")).toBe("2026-01-02T10:00:00Z");
    expect(arrival.querySelector(".catalog-changed bdi")?.textContent).toBe("Ann Owner");
    expect(arrival.querySelector("th")?.getAttribute("scope")).toBe("row");
    const storm = document.querySelector<HTMLTableRowElement>('tr[data-row="SCN-STORM"]');
    expect(Array.from(storm?.cells ?? [], (cell) => cell.textContent)).toEqual(["The stormSCN-STORM", "Version 1", "No task", "0", "0", "0", "2026-01-03 10:00 UTC by Bob Hand"]);
  });

  it("draws no email, OID or proposal of a change (M: `by` drawn)", async () => {
    await openDirection(catalogClient());
    const text = document.body.textContent;
    // Every attribute's value with the text: what a screen, a reader or a copy could show.
    const html = [text, ...Array.from(document.querySelectorAll("*"), (element) => Array.from(element.attributes, (attribute) => attribute.value).join(" "))].join("\n");
    for (const row of pinnedCatalog().rows) {
      const change = row.last_change;
      if (change === null) {
        throw new Error("the spec's rows each carry a change");
      }
      expect([change.by, text.includes(change.by)]).toEqual([change.by, false]);
      expect([change.commit, html.includes(change.commit), html.includes(change.commit.slice(0, 7))]).toEqual([change.commit, false, false]);
    }
    expect(html).not.toMatch(/@example\.org/);
    expect(text).not.toContain("PR-0001");
  });

  it("an unknown state raw under lang=en; no version; a section's change marked (file) with the hint once; none read, Not committed; no title, the ID as the link", async () => {
    const rows = [
      aCatalogRow({ id: "SCN-PARKED", state: "parked", rev: null }),
      aCatalogRow({ id: "SCN-PART", title: "A part", line: 12, path: "docs/spec/scn-parked.md", last_change: aCatalogChange({ of_file: true }) }),
      aCatalogRow({ id: "SCN-NEW", title: null, last_change: null }),
    ];
    await openDirection(catalogClient(rows));
    const parked = document.querySelector('tr[data-row="SCN-PARKED"]');
    const badge = parked?.querySelector(".badge-label");
    expect(badge?.textContent).toBe("parked");
    expect(badge?.getAttribute("lang")).toBe("en");
    expect(parked?.querySelector(".catalog-version")?.textContent).toBe("No version");
    expect(document.querySelector('tr[data-row="SCN-PART"] .catalog-changed')?.textContent).toBe("2026-01-02 10:00 UTC by Ann Owner (file)");
    expect(Array.from(document.querySelectorAll(".catalog-hint"), (hint) => hint.textContent)).toEqual([
      "(file): a section shows its file’s last change; its own is in the document’s History.",
    ]);
    const added = document.querySelector<HTMLElement>('tr[data-row="SCN-NEW"]');
    expect(added?.querySelector(".catalog-changed")?.textContent).toBe("Not committed");
    expect(within(added ?? document.body).getByRole("link", { name: "SCN-NEW" })).not.toBeNull();
    expect(added?.querySelector(".catalog-id")).toBeNull();
  });

  it("no section row shown: no hint", async () => {
    await openDirection(catalogClient());
    expect(document.querySelector(".catalog-hint")).toBeNull();
  });

  it("a title opens the document modal on its REF; Esc closes it, focus back to that title", async () => {
    await openDirection(catalogClient());
    const link = screen.getByRole("link", { name: "Arrival" });
    fireEvent.click(link);
    expect(window.location.hash).toBe("#/alpha/tree?doc=SCN-ARRIVAL");
    const dialog = await waitFor(() => {
      const found = document.querySelector<HTMLElement>(".doc-modal");
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    fireEvent.keyDown(document.activeElement ?? dialog, { key: "Escape" });
    await waitFor(() => {
      expect(document.querySelector(".doc-modal")).toBeNull();
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByRole("link", { name: "Arrival" }));
    });
  });
});

describe("a header's press (DC-10)", () => {
  const ROWS = [
    aCatalogRow({ id: "A", title: "Beacon", rev: 2, state: "in work" }),
    aCatalogRow({ id: "B", title: "\u00e9cluse", rev: null, state: "parked" }),
    aCatalogRow({ id: "C", title: "Anchor", rev: 3, state: "confirmed" }),
    aCatalogRow({ id: "D", title: "Zed", rev: 2, state: "no card" }),
  ];

  it("the served order stands until a press; Version descending, aria-sort saying so, then ascending; ties in served order, no version last", async () => {
    await openDirection(catalogClient(ROWS));
    expect(shownRows()).toEqual(["A", "B", "C", "D"]);
    expect(Array.from(document.querySelectorAll("th[aria-sort]"))).toEqual([]);
    fireEvent.click(screen.getByRole("button", { name: "Version" }));
    expect(shownRows()).toEqual(["C", "A", "D", "B"]);
    expect(header("Version").getAttribute("aria-sort")).toBe("descending");
    expect(Array.from(document.querySelectorAll("th[aria-sort]"))).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "Version" }));
    expect(shownRows()).toEqual(["A", "D", "C", "B"]);
    expect(header("Version").getAttribute("aria-sort")).toBe("ascending");
  });

  it("Title ascending by the viewer's collation, an accented title beside its letter; State by the rollups, unknown last", async () => {
    await openDirection(catalogClient(ROWS));
    fireEvent.click(screen.getByRole("button", { name: "Title" }));
    expect(shownRows()).toEqual(["C", "A", "B", "D"]);
    expect(header("Title").getAttribute("aria-sort")).toBe("ascending");
    expect(header("Version").hasAttribute("aria-sort")).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "State" }));
    expect(shownRows()).toEqual(["C", "A", "D", "B"]);
  });

  it("each header is a subtle compact button inside its th, seven in the table's order, Used by after Tasks (UO11 of usage-orphans)", async () => {
    const table = await openDirection(catalogClient(ROWS));
    const heads = Array.from(table.querySelectorAll("thead th"));
    expect(heads.map((head) => head.textContent)).toEqual(["Title", "Version", "State", "Open items", "Tasks", "Used by", "Last change"]);
    expect(heads.map((head) => head.querySelector("button")?.className)).toEqual(Array.from({ length: 7 }, () => "button button-quiet button-compact"));
    expect(heads.every((head) => head.getAttribute("scope") === "col")).toBe(true);
  });

  it("a new direction starts in served order again (M: the order kept across directions)", async () => {
    await openDirection(catalogClient(ROWS));
    fireEvent.click(screen.getByRole("button", { name: "Version" }));
    expect(shownRows()).toEqual(["C", "A", "D", "B"]);
    fireEvent.click(within(screen.getByRole("group", { name: "Directions" })).getByRole("button", { name: "Cues" }));
    await screen.findByRole("table", { name: "Cues: 4 nodes" });
    await waitFor(() => {
      expect(shownRows()).toEqual(["A", "B", "C", "D"]);
    });
    expect(Array.from(document.querySelectorAll("th[aria-sort]"))).toEqual([]);
  });
});

// UO11 of docs/features/usage-orphans.md "Acceptance criteria": the served count, its mentions, an
// orphan's neutral badge, never wrapping; a press puts the highest first. The orchestrator's working
// answer for the owner (2026-11-13): a 0 with mentions shows them beside it at full weight, "0 · 6
// mentions"; two filters, No declared use (served used_by 0) and Not cited (served used_by and
// mentioned_by 0), one at a time, the caption saying how many of all, none an empty state. Named
// mutations: No declared use on `mentioned_by`; Not cited on `used_by` alone.
describe("Used by (UO11 of usage-orphans)", () => {
  const USED = [
    aCatalogRow({ id: "A", title: "Anchor", used_by: 2, mentioned_by: 3 }),
    aCatalogRow({ id: "B", title: "Buoy", used_by: 0, mentioned_by: 6 }),
    aCatalogRow({ id: "C", title: "Capstan", used_by: 5, mentioned_by: 0 }),
    aCatalogRow({ id: "D", title: "Dredge", used_by: 0, mentioned_by: 0, orphan: true }),
    aCatalogRow({ id: "E", title: "Notes on the dredge", used_by: 0, mentioned_by: 0, orphan: null }),
    aCatalogRow({ id: "F", title: "Fender", used_by: 0, mentioned_by: 1 }),
  ];

  function usedCell(id: string): HTMLElement | null {
    return document.querySelector<HTMLElement>(`tr[data-row="${id}"] .catalog-used`);
  }

  function caption(): string | undefined {
    return screen.getByRole("table").querySelector("caption")?.textContent;
  }

  it("a 0 with mentions shows them beside it at full weight, 0 · 6 mentions, never the small +; a count above 0 keeps + M mentions in small text", async () => {
    await openDirection(catalogClient(USED));
    expect(usedCell("B")?.textContent).toBe("0 \u00b7 6 mentions");
    expect(usedCell("B")?.querySelector(".catalog-used-cited")?.textContent).toBe("6 mentions");
    expect(usedCell("B")?.querySelector(".catalog-used-mentions")).toBeNull();
    expect(usedCell("F")?.textContent).toBe("0 \u00b7 1 mention");
    expect(rulesOf(".catalog-used-cited")).not.toMatch(/font-size/);
    expect(usedCell("A")?.textContent).toBe("2 + 3 mentions");
    expect(usedCell("A")?.querySelector(".catalog-used-mentions")?.textContent).toBe("+ 3 mentions");
    expect(usedCell("C")?.textContent).toBe("5");
  });

  it("the Orphan badge on a flagged document alone, neutral, never on a section; the cell on one line", async () => {
    await openDirection(catalogClient(USED));
    const orphan = usedCell("D")?.querySelector(".badge");
    expect([usedCell("D")?.textContent, orphan?.getAttribute("data-tone"), orphan?.querySelector("svg") !== null]).toEqual(["0 Orphan", "usage-orphan", true]);
    expect(usedCell("E")?.textContent).toBe("0");
    expect(document.querySelectorAll(".catalog-used .badge")).toHaveLength(1);
    expect(rulesOf(".catalog-used .badge")).toMatch(/white-space:\s*nowrap;/);
    expect(usedCell("A")?.className).toContain("catalog-count");
  });

  it("a press on Used by puts the highest first, aria-sort descending; ties in served order", async () => {
    await openDirection(catalogClient(USED));
    fireEvent.click(screen.getByRole("button", { name: "Used by" }));
    expect(shownRows()).toEqual(["C", "A", "B", "D", "E", "F"]);
    expect(header("Used by").getAttribute("aria-sort")).toBe("descending");
  });

  it("No declared use keeps the rows the daemon counts no use for, a mentioned one too; the caption K of N; pressed again, all (M: the filter on mentioned_by)", async () => {
    await openDirection(catalogClient(USED));
    const undeclared = screen.getByRole("button", { name: "No declared use" });
    expect(undeclared.getAttribute("aria-pressed")).toBe("false");
    // The two toggles in one group named for what they filter by.
    const group = screen.getByRole("group", { name: "Filter by use" });
    expect(within(group).getAllByRole("button").map((button) => button.textContent)).toEqual(["No declared use", "Not cited"]);
    fireEvent.click(undeclared);
    expect(undeclared.getAttribute("aria-pressed")).toBe("true");
    expect(shownRows()).toEqual(["B", "D", "E", "F"]);
    expect(caption()).toBe("Scenes: 4 of 6 nodes");
    expect(screen.getByRole("region", { name: "Scenes: 4 of 6 nodes" })).toBeTruthy();
    fireEvent.click(undeclared);
    expect(shownRows()).toEqual(["A", "B", "C", "D", "E", "F"]);
    expect(caption()).toBe("Scenes: 6 nodes");
  });

  it("Not cited keeps the rows neither used nor mentioned, never a mentioned one; one filter at a time (M: Not cited on used_by alone)", async () => {
    await openDirection(catalogClient(USED));
    const undeclared = screen.getByRole("button", { name: "No declared use" });
    const uncited = screen.getByRole("button", { name: "Not cited" });
    fireEvent.click(uncited);
    expect([uncited.getAttribute("aria-pressed"), undeclared.getAttribute("aria-pressed")]).toEqual(["true", "false"]);
    expect(shownRows()).toEqual(["D", "E"]);
    expect(caption()).toBe("Scenes: 2 of 6 nodes");
    fireEvent.click(undeclared);
    expect([uncited.getAttribute("aria-pressed"), undeclared.getAttribute("aria-pressed")]).toEqual(["false", "true"]);
    expect(shownRows()).toEqual(["B", "D", "E", "F"]);
  });

  it("none kept: an empty state in the table's place, the filter kept pressed to undo", async () => {
    await openDirection(catalogClient(USED.filter((row) => row.mentioned_by > 0)));
    fireEvent.click(screen.getByRole("button", { name: "Not cited" }));
    expect(screen.queryByRole("table")).toBeNull();
    expect(screen.getByRole("heading", { level: 2, name: "No node here matches the filter" })).toBeTruthy();
    const uncited = screen.getByRole("button", { name: "Not cited" });
    expect(uncited.getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(uncited);
    expect(shownRows()).toEqual(["A", "B", "F"]);
  });
});

describe("50, then Show more (DC-6)", () => {
  it("120 rows show 50; Show 50 more shows 100, focus on row 51's title; Show 20 more shows 120, the button gone (M: every row at once)", async () => {
    await openDirection(catalogClient(manyRows(120)));
    expect(shownRows()).toHaveLength(50);
    expect(screen.getByRole("table", { name: "Scenes: 120 nodes" })).not.toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Show 50 more" }));
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByRole("link", { name: "Scene 51" }));
    });
    expect(shownRows()).toHaveLength(100);
    fireEvent.click(screen.getByRole("button", { name: "Show 20 more" }));
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByRole("link", { name: "Scene 101" }));
    });
    expect(shownRows()).toHaveLength(120);
    expect(screen.queryByRole("button", { name: /^Show \d+ more$/ })).toBeNull();
  });

  it("a press keeps the count shown and shows the first of the new order", async () => {
    await openDirection(catalogClient(manyRows(120)));
    fireEvent.click(screen.getByRole("button", { name: "Show 50 more" }));
    fireEvent.click(screen.getByRole("button", { name: "Version" }));
    expect(shownRows()).toHaveLength(100);
    // Version 2 first, every other row from the second, in served order.
    expect(shownRows().slice(0, 3)).toEqual(["SCN-002", "SCN-004", "SCN-006"]);
    expect(screen.getByRole("button", { name: "Show 20 more" })).not.toBeNull();
  });

  it("50 rows exactly: no Show more", async () => {
    await openDirection(catalogClient(manyRows(50)));
    expect(shownRows()).toHaveLength(50);
    expect(screen.queryByRole("button", { name: /^Show \d+ more$/ })).toBeNull();
  });
});

describe("the read's states (DC-10)", () => {
  it("no row: the empty state under the view's h1, no table; the daemon's notes under lang=en", async () => {
    window.localStorage.setItem("specengine.module.alpha", "scenes");
    renderApp(catalogClient([], ["catalog: last change not read: git: not a repository"]), "#/alpha/tree");
    const title = await screen.findByRole("heading", { name: "No node in this direction" });
    expect(title.tagName).toBe("H2");
    expect(title.closest(".node-column")).not.toBeNull();
    expect(screen.queryByRole("table")).toBeNull();
    const notes = screen.getByRole("list", { name: "Notes from the daemon" });
    expect(within(notes).getByText("catalog: last change not read: git: not a repository").getAttribute("lang")).toBe("en");
  });

  it("a refusal in the daemon's words with Retry; Retry reads again and the table lands", async () => {
    const client = catalogClient();
    const answer = client.getCatalog.getMockImplementation();
    const message = "no route /api/projects/alpha/catalog: the routes are /api/projects and the web UI";
    client.getCatalog.mockRejectedValueOnce(new ClientError({ status: 404, message }));
    window.localStorage.setItem("specengine.module.alpha", "scenes");
    renderApp(client, "#/alpha/tree");
    expect(await screen.findByText(message)).not.toBeNull();
    expect(screen.getByText("The table of Scenes could not be read")).not.toBeNull();
    expect(screen.getByText(message).getAttribute("lang")).toBe("en");
    if (answer !== undefined) {
      client.getCatalog.mockImplementation(answer);
    }
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("table", { name: "Scenes: 2 nodes" })).not.toBeNull();
    expect(screen.queryByText(message)).toBeNull();
  });

  it("Include archive reads the catalog again with archive, the last table up meanwhile", async () => {
    const client = catalogClient();
    await openDirection(client);
    let land: () => void = () => undefined;
    client.getCatalog.mockImplementationOnce(
      (_project, options) =>
        new Promise((resolve) => {
          land = () => {
            resolve(aCatalog([...pinnedCatalog().rows, aCatalogRow({ id: "SCN-OLD", archived: true })], { archive: options.archive }));
          };
        }),
    );
    fireEvent.click(screen.getByLabelText("Include archive"));
    await waitFor(() => {
      expect(argsOf(client.getCatalog).at(-1)).toEqual(["alpha", { kinds: ["scene"], archive: true }]);
    });
    expect(screen.getByRole("table", { name: "Scenes: 2 nodes" })).not.toBeNull();
    act(() => {
      land();
    });
    expect(await screen.findByRole("table", { name: "Scenes: 3 nodes" })).not.toBeNull();
  });
});

/** The declarations of every rule of app.css whose selector list names `selector` exactly, joined. */
function rulesOf(selector: string): string {
  return Array.from(appCss.matchAll(/(?:^|\n)([^{}@/]+?)\s*\{([^}]*)\}/g))
    .filter((match) => (match[1] ?? "").split(",").map((part) => part.trim()).includes(selector))
    .map((match) => match[2] ?? "")
    .join("\n");
}

const NOWRAP = /white-space:\s*nowrap;/;

// The table in a 744 px node column at 1440 x 900, Russian, 67 rows: titles three or four lines,
// the state's lozenge on two, an ID broken after a hyphen. jsdom lays nothing out, so these pin the
// rules and the cells they reach; the widths themselves are a Look, in a browser.
describe("the table's layout (DC-10)", () => {
  it("keeps a state's lozenge, an ID, a version, the counts and a time on one line (M: the badge's own `white-space: normal` left in the table)", async () => {
    await openDirection(catalogClient());
    const arrival = document.querySelector<HTMLTableRowElement>('tr[data-row="SCN-ARRIVAL"]');
    expect(arrival?.cells[2]?.className).toBe("catalog-state");
    expect(arrival?.querySelector(".catalog-state > .badge .badge-label")?.textContent).toBe("In work");
    expect(arrival?.querySelector(".catalog-id")?.textContent).toBe("SCN-ARRIVAL");
    expect(rulesOf(".catalog-state .badge")).toMatch(NOWRAP);
    expect(rulesOf(".catalog-id")).toMatch(NOWRAP);
    expect(rulesOf(".catalog-version")).toMatch(NOWRAP);
    expect(rulesOf(".catalog-count")).toMatch(NOWRAP);
    expect(rulesOf(".catalog-time")).toMatch(NOWRAP);
    // The primitive is untouched: a badge elsewhere still wraps.
    expect(rulesOf(".badge")).toMatch(/white-space:\s*normal;/);
  });

  it("gives the title's column 18ch and breaks no word in it; a header breaks between words, not letters (M: the body's `anywhere` inherited)", () => {
    const title = rulesOf(".catalog-table tbody th");
    expect(title).toMatch(/min-width:\s*18ch;/);
    expect(title).toMatch(/overflow-wrap:\s*break-word;/);
    expect(title).not.toMatch(/anywhere/);
    expect(rulesOf(".catalog-table thead th")).toMatch(/overflow-wrap:\s*break-word;/);
    expect(rulesOf(".catalog-table thead th")).not.toMatch(NOWRAP);
  });

  it("the tasks cell compact: the pair where there is a task, a bare 0 where none", async () => {
    await openDirection(catalogClient());
    expect(document.querySelector('tr[data-row="SCN-ARRIVAL"] .catalog-tasks')?.textContent).toBe("1 · 1 open");
    expect(document.querySelector('tr[data-row="SCN-STORM"] .catalog-tasks')?.textContent).toBe("0");
    expect(rulesOf(".catalog-count")).toMatch(/font-variant-numeric:\s*tabular-nums;/);
  });

  it("scrolls a wide table in its own focusable region, named by the caption, never the page", async () => {
    const table = await openDirection(catalogClient());
    const region = screen.getByRole("region", { name: "Scenes: 2 nodes" });
    expect(table.parentElement).toBe(region);
    expect(region.className).toBe("catalog-table-wrap");
    expect(region.tabIndex).toBe(0);
    expect(rulesOf(".catalog-table-wrap")).toMatch(/overflow-x:\s*auto;/);
  });
});
