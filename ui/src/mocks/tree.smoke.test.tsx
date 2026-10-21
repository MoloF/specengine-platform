import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "../app/App";
import { oneLine } from "../health/codeRows";
import { measuredRows, treeDrift } from "../tree/drift";
import { buildRows } from "../tree/rows";
import { LARGE_BINDINGS } from "./bindings";
import { BASIN_NAME_DECOMPOSED, DRAFT_FILE, HIGH_WATER_FILE, HOSTILE_LINK, HOSTILE_TITLE, LONG_TOKEN } from "./harbor-sim/fixtures";
import { CODE_GAP_CAUSE, CODE_OFF_CAUSE, harborDrift, LARGE_DRIFT, LOCK_CAUSE, LOCKED_AT, LOOKALIKE_BASIN } from "./health";
import { harborLockDiffRows } from "./lockDiff";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// The Spec tree over the real mock, as `pnpm dev` shows it: AC-05 of docs/features/ui-tree-node.md
// (routes by every REF form, the row current and revealed, a followed link, Back, an unknown REF,
// a look-alike letter), the hostile and long fixtures, the `empty` and `large` scenarios.

function renderMock(scenario: Scenario, hash: string) {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0 });
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return client;
}

function tree(): HTMLElement {
  return screen.getByRole("tree", { name: "Spec tree" });
}

function row(name: string, nth = 0): HTMLElement {
  const rows = within(tree())
    .getAllByRole("treeitem")
    .filter((item) => item.querySelector(".tree-row-name")?.textContent === name);
  const found = rows[nth];
  if (found === undefined) {
    throw new Error(`no row ${name}`);
  }
  return found;
}

function currentRows(): string[] {
  return within(tree())
    .getAllByRole("treeitem")
    .filter((item) => item.getAttribute("aria-current") === "page")
    .map((item) => item.querySelector(".tree-row-name")?.textContent ?? "");
}

async function heading(): Promise<HTMLElement> {
  return screen.findByRole("heading", { level: 1 });
}

/** Browser Back; jsdom traverses history in a task of its own, so wait for the hashchange. */
async function goBack() {
  const traversed = new Promise<void>((resolve) => {
    window.addEventListener(
      "hashchange",
      () => {
        resolve();
      },
      { once: true },
    );
  });
  await act(async () => {
    window.history.back();
    await traversed;
  });
}

describe("the Spec tree over the mock (AC-05)", () => {
  it("opens MEC-TIDES by ID: its row current, its ancestors expanded, its text whole", async () => {
    renderMock("normal", "#/harbor-sim/tree/MEC-TIDES");
    await waitFor(() => {
      expect(screen.getByRole("heading", { level: 1 }).textContent.startsWith("MEC-TIDES")).toBe(true);
    });
    await waitFor(() => {
      expect(currentRows()).toEqual(["MEC-TIDES"]);
    });
    expect(row("DOM-HARBOR").getAttribute("aria-expanded")).toBe("true");
    expect(row("DOM-WATER").getAttribute("aria-expanded")).toBe("true");
    expect(row("MEC-TIDES").getAttribute("aria-level")).toBe("3");
    expect(screen.getByText("Tide cycle", { selector: ".node-title-text" })).toBeTruthy();
  });

  it("opens MEC-TIDES#RULE-TIDE-WINDOW: the section, its row current", async () => {
    renderMock("normal", "#/harbor-sim/tree/MEC-TIDES%23RULE-TIDE-WINDOW");
    await waitFor(() => {
      expect(currentRows()).toEqual(["RULE-TIDE-WINDOW"]);
    });
    expect((await heading()).textContent).toContain("Entry only inside the tide window");
    expect(row("MEC-TIDES").getAttribute("aria-expanded")).toBe("true");
  });

  it("opens a document by its root-relative path, encoded once", async () => {
    renderMock("normal", "#/harbor-sim/tree/docs%2Fspec%2Ftides%2Ftide-cycle.md");
    await waitFor(() => {
      expect(currentRows()).toEqual(["MEC-TIDES"]);
    });
    expect((await heading()).textContent).toContain("Tide cycle");
  });

  it("names the ID-less document by its path, in the tree and the heading", async () => {
    renderMock("normal", "#/harbor-sim/tree/docs%2Fspec%2Fberths%2Fdraft-limits.md");
    await waitFor(() => {
      expect(currentRows()).toEqual(["docs/spec/berths/draft-limits.md"]);
    });
    expect((await heading()).textContent).toContain("docs/spec/berths/draft-limits.md");
  });

  it("focuses the new heading after a followed link, and Back returns", async () => {
    renderMock("normal", "#/harbor-sim/tree/MEC-TIDES");
    await waitFor(() => {
      expect(currentRows()).toEqual(["MEC-TIDES"]);
    });
    fireEvent.click(screen.getByRole("tab", { name: "Links" }));
    const panel = await screen.findByRole("tabpanel", { name: "Links" });
    const link = (await within(panel).findAllByRole("link", { name: "DOM-BERTHS" }))[0];
    if (link === undefined) {
      throw new Error("no link to DOM-BERTHS");
    }
    expect(link.getAttribute("href")).toBe("#/harbor-sim/tree/DOM-BERTHS");
    fireEvent.click(link);
    // jsdom follows a clicked hash link only a task later; the browser at once.
    window.location.hash = "#/harbor-sim/tree/DOM-BERTHS";
    await waitFor(() => {
      expect(currentRows()).toEqual(["DOM-BERTHS"]);
    });
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByRole("heading", { level: 1 }));
    });
    expect(document.activeElement?.textContent).toContain("Berths and moorings");
    await goBack();
    await waitFor(() => {
      expect(currentRows()).toEqual(["MEC-TIDES"]);
    });
    expect(window.location.hash).toBe("#/harbor-sim/tree/MEC-TIDES");
  });

  it("shows an unknown REF's reason verbatim, with no tabs", async () => {
    renderMock("normal", "#/harbor-sim/tree/R-404");
    expect(await screen.findByText("`R-404` resolves to no ID and no alias")).toBeTruthy();
    expect(screen.queryByRole("tablist")).toBeNull();
    expect((await heading()).textContent).toBe("R-404");
  });

  it("shows a look-alike letter's refusal verbatim, naming the Latin fix", async () => {
    // MEC-TIDES with a Cyrillic Es (U+0421) for the C.
    renderMock("normal", `#/harbor-sim/tree/${encodeURIComponent("ME\u0421-TIDES")}`);
    const alert = await within(screen.getByRole("article")).findByRole("alert");
    expect(alert.textContent).toContain(
      "spec: `ME\u0421-TIDES` mixes scripts or uses look-alike letters; IDs are Latin only: write `MEC-TIDES`",
    );
  });

  it("holds two holders of one ID under one h1, an h2 each", async () => {
    renderMock("normal", "#/harbor-sim/tree/RULE-FAIRWAY-SPEED");
    await waitFor(() => {
      expect(currentRows()).toEqual(["RULE-FAIRWAY-SPEED", "RULE-FAIRWAY-SPEED"]);
    });
    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
    expect(screen.getAllByRole("heading", { level: 2, name: /RULE-FAIRWAY-SPEED/ })).toHaveLength(2);
  });
});

describe("the mock's hard cases on screen", () => {
  it("renders the hostile title and text as text, the hostile link unanchored", async () => {
    renderMock("normal", "#/harbor-sim/tree/RULE-MOOR-NIGHT");
    await waitFor(() => {
      expect(currentRows()).toEqual(["RULE-MOOR-NIGHT"]);
    });
    expect(screen.getByText(HOSTILE_TITLE, { selector: ".tree-row-title" })).toBeTruthy();
    expect(screen.getByText(HOSTILE_TITLE, { selector: ".node-title-text" })).toBeTruthy();
    expect(document.querySelector("img")).toBeNull();
    expect(screen.getByRole("main").querySelector("script")).toBeNull();
    fireEvent.click(screen.getByRole("tab", { name: "Links" }));
    expect(await screen.findByText(HOSTILE_LINK, { selector: ".link-name" })).toBeTruthy();
    for (const anchor of Array.from(screen.getByRole("main").querySelectorAll("a[href]"))) {
      expect(anchor.getAttribute("href")?.startsWith("#/")).toBe(true);
    }
  });

  it("shows the long document whole in Source, the long token in it", async () => {
    renderMock("normal", "#/harbor-sim/tree/MEC-TIDE-TABLES");
    await screen.findByText("Tide tables", { selector: ".node-title-text" });
    fireEvent.click(within(screen.getByRole("group", { name: "Show the text as" })).getByRole("button", { name: "Source" }));
    const text = Array.from(document.querySelectorAll(".text-line-content"), (line) => line.textContent).join("");
    expect(text.length).toBeGreaterThan(40000);
    expect(text).toContain(LONG_TOKEN);
  });

  it("renders the long document's table whole: header cells, a named focusable region, all 520 rows (ui-markdown AC-11)", async () => {
    renderMock("normal", "#/harbor-sim/tree/MEC-TIDE-TABLES");
    const region = await screen.findByRole("region", { name: /^Table at line \d+$/ }, { timeout: 5000 });
    expect(region.tabIndex).toBe(0);
    expect(Array.from(region.querySelectorAll("thead th"), (cell) => cell.textContent)).toEqual(["Date", "First", "Second", "Third", "Fourth"]);
    expect(region.querySelectorAll("tbody tr")).toHaveLength(520);
    const text = screen.getByRole("tabpanel", { name: "Text" });
    expect(text.textContent).toContain(LONG_TOKEN);
    expect(text.querySelector(".md-front-matter")?.textContent.startsWith("---\nid: MEC-TIDE-TABLES\n")).toBe(true);
  });

  it("anchors a markdown link the links read resolved; a dangling one is text with its label (ui-markdown AC-04)", async () => {
    renderMock("normal", "#/harbor-sim/tree/DOM-BERTHS");
    const text = await screen.findByRole("tabpanel", { name: "Text" });
    const anchor = await within(text).findByRole("link", { name: "the tide cycle" });
    expect(anchor.getAttribute("href")).toBe("#/harbor-sim/tree/MEC-TIDES");
    const quays = within(text).getByText("quay plans");
    expect(quays.closest("a")).toBeNull();
    expect(quays.closest(".md-link")?.querySelector(".badge-label")?.textContent).toBe("Dangling");
  });

  it("shows a section named by a heading attribute: its title, its ID as a label, no braces (ui-markdown AC-07)", async () => {
    renderMock("normal", "#/harbor-sim/tree/RULE-HARBOR-CLOCK");
    await waitFor(() => {
      expect(currentRows()).toEqual(["RULE-HARBOR-CLOCK"]);
    });
    const text = await screen.findByRole("tabpanel", { name: "Text" });
    const heading = await within(text).findByRole("heading", { name: "One simulated clock RULE-HARBOR-CLOCK" });
    expect(heading.tagName).toBe("H2");
    expect(heading.querySelector(".md-heading-id")?.textContent).toBe("RULE-HARBOR-CLOCK");
    expect(text.textContent).not.toContain("{#");
    expect(text.querySelector("[id]")).toBeNull();
  });

  it("finds the decomposed non-Latin name with the same decomposed query", async () => {
    renderMock("normal", "#/harbor-sim/tree");
    await screen.findByRole("tree");
    fireEvent.change(screen.getByLabelText("Search the spec"), { target: { value: BASIN_NAME_DECOMPOSED.split(" ")[1] } });
    fireEvent.submit(screen.getByRole("search"));
    const hits = await screen.findByRole("listbox", { name: /Search hits/ });
    expect(within(hits).getAllByRole("option").length).toBeGreaterThan(0);
    expect(hits.querySelector("mark")).not.toBeNull();
  });

  it("empty: the tree's note verbatim and the next step", async () => {
    renderMock("empty", "#/harbor-sim/tree");
    expect(await screen.findByRole("heading", { name: "No document in the tree" })).toBeTruthy();
    expect(screen.getByText('no document under [paths] spec "docs/spec"; give a ROOT')).toBeTruthy();
    expect(screen.getByText(/Next: write a spec document/)).toBeTruthy();
  });

  it("large: 3 000 and more nodes, the roots open, a hundred rows arrowed", async () => {
    const client = renderMock("large", "#/harbor-sim/tree");
    const view = await client.getTree("harbor-sim");
    expect(view.nodes.length).toBeGreaterThanOrEqual(3000);
    expect(view.truncated).toBe(false);
    expect(Math.max(...view.nodes.map((node) => node.depth))).toBe(5);
    await screen.findByRole("tree");
    const first = within(tree()).getAllByRole("treeitem")[0];
    if (first === undefined) {
      throw new Error("no row");
    }
    first.focus();
    for (let step = 0; step < 100; step += 1) {
      fireEvent.keyDown(document.activeElement ?? first, { key: "ArrowDown" });
      await act(async () => {
        await Promise.resolve();
      });
    }
    expect(document.activeElement?.getAttribute("role")).toBe("treeitem");
    expect(document.activeElement).not.toBe(first);
  });
});

// The Bindings tab over the mock (UI-2, UI-3, UI-4, UI-8 of docs/features/ui-node-bindings.md):
// harbor-sim's rows in the daemon's order, its states; large's 3 000 rows; code-cannot's two degrades;
// ledger-api without [code].

/** Opens the Bindings tab of the node at `hash` and waits for its read. */
async function bindingsTab(scenario: Scenario, hash: string, ready: string): Promise<HTMLElement> {
  renderMock(scenario, hash);
  fireEvent.click(await screen.findByRole("tab", { name: "Bindings" }));
  const panel = await screen.findByRole("tabpanel", { name: "Bindings" });
  await within(panel).findAllByText(ready, {}, { timeout: 5000 });
  return panel;
}

/** Each binding line's first two paragraphs and its sync badge's label. */
function lineHeads(panel: HTMLElement): string[][] {
  return Array.from(panel.querySelectorAll(".binding-line"), (line) => [
    line.querySelector("p")?.textContent ?? "",
    line.querySelector(".badge-label")?.textContent ?? "",
  ]);
}

describe("the Bindings tab over the mock (UI-2, UI-3, UI-4, UI-8 of ui-node-bindings)", () => {
  it("normal: rows in the daemon's order, never by sync or file; the node and its sections; a moved row", async () => {
    const panel = await bindingsTab("normal", "#/harbor-sim/tree/MEC-MOORING", "4 rows");
    expect(lineHeads(panel)).toEqual([
      ["MEC-MOORINGimplementssrc/sim/crew.rs:31-48", "Code ahead"],
      ["MEC-MOORINGverifiestests/mooring.rs:6-21", "In sync"],
      ["RULE-MOOR-CREWimplementssrc/sim/crew.rs", "Bad marker"],
      ["RULE-MOOR-NIGHTimplementssrc/sim/crew.rs", "Cannot verify"],
    ]);
    expect(panel.querySelector(".binding-line")?.textContent).toContain("Moved fromsrc/sim/berths.rs | item harbor_sim::berths::moor function_item");
    expect(within(panel).getByRole("list", { name: "Commands for MEC-MOORING" })).toBeTruthy();
    expect(panel.querySelector('[data-evidence-state="none"]')?.textContent).toBe("None: no [[tested]] block covers this node or a section within it.");
  });

  it("normal: a look-alike row as written, under its Latin fix's document, after the fix's own row", async () => {
    const panel = await bindingsTab("normal", "#/harbor-sim/tree/DOM-BERTHS", "2 rows");
    expect(lineHeads(panel)).toEqual([
      ["RULE-BASIN-NAMEimplementssrc/sim/berths.rs:37-41", "In sync"],
      [`${LOOKALIKE_BASIN}implementssrc/sim/berths.rs:31-35`, "Bad marker"],
    ]);
  });

  it("normal: the test evidence, a verified entry among them; two holders each their part", async () => {
    const tides = await bindingsTab("normal", "#/harbor-sim/tree/MEC-TIDES", "3 rows");
    const evidence = within(tides).getByRole("list", { name: "Test evidence of MEC-TIDES, by node" });
    expect(Array.from(evidence.querySelectorAll(".badge-label"), (label) => label.textContent)).toEqual(["Stale", "Unverifiable"]);
    cleanup();
    const speed = await bindingsTab("normal", "#/harbor-sim/tree/RULE-FAIRWAY-SPEED", "1 row");
    expect(within(speed).getAllByRole("heading", { level: 2 }).filter((heading) => heading.classList.contains("holder-text-title"))).toHaveLength(2);
    expect(within(speed).getAllByText("1 row")).toHaveLength(2);
    cleanup();
    const tables = await bindingsTab("normal", "#/harbor-sim/tree/MEC-TIDE-TABLES", "1 row");
    expect(within(tables).getByRole("list", { name: "Test evidence of MEC-TIDE-TABLES, by node" }).textContent).toContain("Evidence: Verified");
  });

  it("large: DOM-GEN-01's 3 000 rows, ten drawn, then Show all 3000", async () => {
    const panel = await bindingsTab("large", "#/harbor-sim/tree/DOM-GEN-01", `${String(LARGE_BINDINGS)} rows`);
    expect(panel.querySelectorAll(".binding-line")).toHaveLength(10);
    fireEvent.click(within(panel).getByRole("button", { name: "Show all 3000" }));
    expect(panel.querySelectorAll(".binding-line")).toHaveLength(LARGE_BINDINGS);
    expect(lineHeads(panel).at(-1)).toEqual(["DOM-GEN-01implementssrc/gen/d01/part_3000.rs", "Unlocked"]);
  }, 20000);

  it("code-cannot: harbor-sim not read, its note as the CLI prints it, no number; ledger-api's rows listed, Not judged, test evidence not judged", async () => {
    const harbor = await bindingsTab("code-cannot", "#/harbor-sim/tree/MEC-TIDES", "Not read");
    expect(within(harbor).getByRole("list", { name: "Notes from the daemon on the bindings" }).textContent).toBe(oneLine(`bindings: ${CODE_GAP_CAUSE}`));
    expect(harbor.querySelector(".holder-bindings")?.textContent).not.toMatch(/\d|None/);
    cleanup();
    const ledger = await bindingsTab("code-cannot", "#/ledger-api/tree/SVC-REFUNDS", "1 row");
    expect(within(ledger).getByRole("list", { name: "Notes from the daemon on the bindings" }).textContent).toBe(`bindings: ${LOCK_CAUSE}`);
    expect(lineHeads(ledger)).toEqual([["SVC-REFUNDSimplementssrc/refunds/service.rs", "Not judged"]]);
    expect(ledger.querySelector('[data-evidence-state="not-judged"]')?.textContent).toMatch(/^Not judged: the \[\[tested\]\] entries were not judged/);
  });

  it("ledger-api: no [code] table, Not read and the note; not-configured: its rows in sync", async () => {
    const off = await bindingsTab("normal", "#/ledger-api/tree/SVC-REFUNDS", "Not read");
    expect(within(off).getByRole("list", { name: "Notes from the daemon on the bindings" }).textContent).toBe(`bindings: ${CODE_OFF_CAUSE}`);
    cleanup();
    const measured = await bindingsTab("not-configured", "#/ledger-api/tree/SVC-LEDGER", "None");
    expect(measured.querySelectorAll(".binding-line")).toHaveLength(0);
    cleanup();
    const refunds = await bindingsTab("not-configured", "#/ledger-api/tree/SVC-REFUNDS", "1 row");
    expect(lineHeads(refunds)).toEqual([["SVC-REFUNDSimplementssrc/refunds/service.rs", "In sync"]]);
  });
});

// "Changes since the lock" over the mock (UI-2, UI-7 of docs/features/lock-diff-ui.md): the four rows
// of its "Data" on their nodes, each figure's hunks the mock's byte for byte; ledger-api's two states.

/** Opens the Bindings tab of the node at `hash`, waits for its read, expands "Changes since the lock"; its section. */
async function lockDiffOf(scenario: Scenario, hash: string, ready: string): Promise<HTMLElement> {
  const panel = await bindingsTab(scenario, hash, ready);
  const toggle = within(panel).getByRole("button", { name: "Changes since the lock" });
  fireEvent.click(toggle);
  const section = toggle.closest("section");
  if (section === null) {
    throw new Error("no lock diff section");
  }
  await waitFor(() => {
    expect(section.querySelector(".lock-diff-body")).not.toBeNull();
  });
  return section;
}

/** A figure's `<code>` text, its screen-reader words left out. */
function hunksOf(figure: Element | undefined): string | null {
  const code = figure?.querySelector("code")?.cloneNode(true) as Element | undefined;
  code?.querySelectorAll(".sr-only").forEach((spoken) => {
    spoken.remove();
  });
  return code?.textContent ?? null;
}

/** The mock's kept row of `node`. */
function keptOf(node: string) {
  const found = harborLockDiffRows().find(({ row }) => row.node === node);
  if (found === undefined) {
    throw new Error(`no kept row of ${node}`);
  }
  return found;
}

describe("the lock diff over the mock (UI-2, UI-7 of lock-diff-ui)", () => {
  it("normal: changes since the lock, the four Mocks rows on their nodes", async () => {
    const tides = await lockDiffOf("normal", "#/harbor-sim/tree/MEC-TIDES", "3 rows");
    const conflict = keptOf("MEC-TIDES");
    const figures = await within(tides).findAllByRole("figure");
    expect(figures.map((figure) => [figure.getAttribute("aria-label"), hunksOf(figure)])).toEqual([
      [`Node text since the lock: ${conflict.spec?.path ?? ""} at ${LOCKED_AT}`, conflict.spec?.diff],
      [`Code since the lock: src/sim/tide.rs, blob ${conflict.code?.blob ?? ""}`, conflict.code?.diff],
    ]);
    expect(Array.from(tides.querySelector(".lock-diff-sides")?.children ?? [], (side) => side.getAttribute("data-side"))).toEqual(["spec", "code"]);
    expect(within(tides).getByRole("list", { name: "Commands for the row of MEC-TIDES" })).toBeTruthy();
    cleanup();

    const mooring = await lockDiffOf("normal", "#/harbor-sim/tree/MEC-MOORING", "4 rows");
    const moved = keptOf("MEC-MOORING");
    const [lone] = await within(mooring).findAllByRole("figure");
    expect([lone?.getAttribute("aria-label"), hunksOf(lone)]).toEqual([`Code since the lock: src/sim/berths.rs, blob ${moved.code?.blob ?? ""}`, moved.code?.diff]);
    expect(Array.from(mooring.querySelector(".lock-diff-sides")?.children ?? [], (side) => side.getAttribute("data-side"))).toEqual(["code"]);
    expect(mooring.querySelector(".health-code-row")?.textContent).toContain("Moved from src/sim/berths.rs | item harbor_sim::berths::moor function_item");
    cleanup();

    const draft = await lockDiffOf("normal", "#/harbor-sim/tree/RULE-BERTH-DRAFT", "1 row");
    expect(Array.from(draft.querySelectorAll(".lock-diff-side"), (side) => side.textContent)).toEqual([
      `Node text since the lock: ${DRAFT_FILE} at ${LOCKED_AT}No diff: ${DRAFT_FILE} cannot be read at ${LOCKED_AT}`,
    ]);
    expect(within(draft).queryAllByRole("figure")).toHaveLength(0);
    cleanup();

    const highWater = await lockDiffOf("normal", "#/harbor-sim/tree/RULE-HIGH-WATER", "1 row");
    const gone = keptOf("RULE-HIGH-WATER");
    expect(Array.from(highWater.querySelectorAll(".lock-diff-side .lock-diff-side-title, .lock-diff-side .bindings-state"), (part) => part.textContent)).toEqual([
      `Node text since the lock: ${HIGH_WATER_FILE} at ${LOCKED_AT}`,
      "No diff: git shows no hunk for bytes that differ",
      `Code since the lock: src/sim/tide_table.rs, blob ${gone.code?.blob ?? ""}`,
    ]);
    const [removed] = within(highWater).getAllByRole("figure");
    expect(hunksOf(removed)).toBe(gone.code?.diff);
    expect(within(highWater).getAllByRole("figure")).toHaveLength(1);
  });

  it("ledger-api: no [code], not judged and its one note, no count; not-configured: none", async () => {
    const off = await lockDiffOf("normal", "#/ledger-api/tree/SVC-REFUNDS", "Not read");
    await within(off).findByText("Not judged: the daemon\u2019s note above says why.");
    expect(within(off).getByRole("list", { name: "Notes from the daemon on the changes since the lock" }).textContent).toBe(`bindings: ${CODE_OFF_CAUSE}`);
    expect(off.querySelector(".lock-diff-holder")?.textContent).not.toMatch(/\d|None/);
    cleanup();
    const none = await lockDiffOf("not-configured", "#/ledger-api/tree/SVC-REFUNDS", "1 row");
    expect((await within(none).findByText("None: no drifted binding of this node has a lock entry.")).getAttribute("data-diff-state")).toBe("none");
  });
});

// Show drift over the mock (UI-8 of docs/features/ui-tree-drift.md): the spec's mock table, row 6 as
// src/mocks/health.ts serves it in each scenario; `large` within 20 s.

/** Turns Show drift on in the tree on screen. */
function showDrift() {
  fireEvent.click(screen.getByRole("checkbox", { name: "Show drift" }));
}

/** A tree line's drift badges, as shown. */
function badgesOf(item: HTMLElement): string[] {
  return Array.from(item.querySelectorAll(".tree-drift-badge .badge-label"), (label) => label.textContent);
}

/** Show drift's lines after the toggles. */
function driftLines(): string[] {
  return Array.from(document.querySelectorAll(".tree-drift-line"), (line) => line.textContent);
}

describe("Show drift over the mock (UI-8 of ui-tree-drift)", () => {
  it("normal, harbor-sim: each node's own rows on its line, never its parent's; the look-alike the one row on no line", async () => {
    renderMock("normal", "#/harbor-sim/tree/RULE-TIDE-WINDOW");
    await waitFor(() => {
      expect(currentRows()).toEqual(["RULE-TIDE-WINDOW"]);
    });
    showDrift();
    await waitFor(() => {
      expect(driftLines()).toEqual(["1 drift row names no line of this tree. See them in Health"]);
    });
    expect(badgesOf(row("MEC-TIDES"))).toEqual(["Fix 1"]);
    expect(badgesOf(row("RULE-TIDE-WINDOW"))).toEqual(["Fix 1"]);
    expect(badgesOf(row("DOM-WATER"))).toEqual([]);
    expect(screen.getByRole("link", { name: "See them in Health" }).getAttribute("href")).toBe("#/harbor-sim/health");
    // The twelve rows: eleven on their nodes' lines, the look-alike as written on none.
    const client = new MockClient("normal", { delayMs: 0 });
    const lines = buildRows((await client.getTree("harbor-sim")).nodes);
    const rows = measuredRows((await client.getHealth("harbor-sim")).drift) ?? [];
    expect(rows).toHaveLength(harborDrift().length);
    const drift = treeDrift(lines, rows, null);
    expect(drift.unmatched).toBe(1);
    expect([...drift.byRow.values()].reduce((sum, own) => sum + own.fix + own.check + own.cannot + own.unlisted, 0)).toBe(11);
    expect(rows.find((line) => line.node === LOOKALIKE_BASIN)).toBeDefined();
  });

  it("normal, ledger-api: the code layer off, its cause, no badge", async () => {
    renderMock("normal", "#/ledger-api/tree");
    await screen.findByRole("tree");
    showDrift();
    await waitFor(() => {
      expect(driftLines()).toEqual([`No drift badges: Code layer off: ${CODE_OFF_CAUSE}`]);
    });
    expect(document.querySelector(".tree-drift-badge")).toBeNull();
  });

  it("code-cannot: could not be measured, the cause on one line, no badge; ledger-api its invalid lock", async () => {
    renderMock("code-cannot", "#/harbor-sim/tree");
    await screen.findByRole("tree");
    showDrift();
    await waitFor(() => {
      expect(driftLines()).toEqual([`No drift badges: Could not be measured: ${oneLine(CODE_GAP_CAUSE)}`]);
    });
    expect(document.querySelector(".tree-drift-badge")).toBeNull();
    cleanup();
    renderMock("code-cannot", "#/ledger-api/tree");
    await screen.findByRole("tree");
    showDrift();
    await waitFor(() => {
      expect(driftLines()).toEqual([`No drift badges: Could not be measured: ${LOCK_CAUSE}`]);
    });
  });

  it("slow: the tree, then Measuring drift, then the badges", async () => {
    window.history.replaceState(null, "", "/?scenario=slow#/harbor-sim/tree");
    window.localStorage.setItem("specengine.tree.show-drift.harbor-sim", "on");
    const client = new MockClient("slow");
    render(<App client={client} scenario="slow" />);
    await screen.findByRole("tree", {}, { timeout: 6000 });
    expect(driftLines()).toEqual(["Measuring drift"]);
    expect(document.querySelector(".tree-drift-badge")).toBeNull();
    await waitFor(
      () => {
        expect(driftLines()).toEqual(["1 drift row names no line of this tree. See them in Health"]);
      },
      { timeout: 6000 },
    );
    expect(badgesOf(row("DOM-HARBOR"))).toEqual([]);
  }, 15000);

  it("empty: the empty tree's note, no drift said so, no summary", async () => {
    renderMock("empty", "#/harbor-sim/tree");
    await screen.findByRole("heading", { name: "No document in the tree" });
    showDrift();
    await waitFor(() => {
      expect(driftLines()).toEqual(["No drift: every binding spec verify judges is in sync."]);
    });
    expect(screen.getByText('no document under [paths] spec "docs/spec"; give a ROOT')).toBeTruthy();
  });

  it("large: 3 108 generated nodes against 3 000 rows, each generated row Check 1 on its line, within 20 s", async () => {
    const client = renderMock("large", "#/harbor-sim/tree");
    await screen.findByRole("tree");
    showDrift();
    await waitFor(
      () => {
        expect(badgesOf(row("DOM-GEN-01"))).toEqual(["Check 1"]);
      },
      { timeout: 15000 },
    );
    expect(badgesOf(row("DOM-GEN-12"))).toEqual(["Check 1"]);
    expect(driftLines()).toEqual(["1 drift row names no line of this tree. See them in Health"]);
    const nodes = (await client.getTree("harbor-sim")).nodes;
    expect(nodes.filter((node) => node.path.startsWith("docs/spec/gen/"))).toHaveLength(3108);
    const rows = measuredRows((await client.getHealth("harbor-sim")).drift) ?? [];
    expect(rows).toHaveLength(LARGE_DRIFT);
    const drift = treeDrift(buildRows(nodes), rows, null);
    expect(drift.unmatched).toBe(1);
    const generated = buildRows(nodes).filter((line) => line.node.path.startsWith("docs/spec/gen/") && drift.byRow.has(line.key));
    expect(generated).toHaveLength(LARGE_DRIFT - harborDrift().length);
    for (const line of generated) {
      expect([line.key, drift.byRow.get(line.key)]).toEqual([line.key, { fix: 0, check: 1, cannot: 0, unlisted: 0 }]);
    }
  }, 20000);
});
