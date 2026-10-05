import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "../app/App";
import { BASIN_NAME_DECOMPOSED, HOSTILE_LINK, HOSTILE_TITLE, LONG_TOKEN } from "./harbor-sim/fixtures";
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

  it("shows the long document whole, the long token in it", async () => {
    renderMock("normal", "#/harbor-sim/tree/MEC-TIDE-TABLES");
    await screen.findByText("Tide tables", { selector: ".node-title-text" });
    const text = Array.from(document.querySelectorAll(".text-line-content"), (line) => line.textContent).join("");
    expect(text.length).toBeGreaterThan(40000);
    expect(text).toContain(LONG_TOKEN);
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
