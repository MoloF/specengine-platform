import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import { App } from "../app/App";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// UO10 and UO11 of docs/features/usage-orphans.md "Acceptance criteria" over the real mock, as
// `pnpm dev` shows it (harbor-sim): a document's Links says who uses it and where; the Mechanics
// table's Used by column, Orphan badge and Used by 0; Health's orphan documents grouped by direction,
// then kind, with their tasks; `orphan-notes` the queue not read; `empty` none. The mock is imported
// here alone (policy AC-05).

function renderMock(hash: string, scenario: Scenario = "normal") {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0 });
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
}

/** Health's row 8, once measured. */
async function orphansPart(): Promise<HTMLElement> {
  return waitFor(() => {
    const found = document.querySelector<HTMLElement>('[data-row="orphans"] .health-left-total')?.closest<HTMLElement>('[data-row="orphans"]');
    expect(found).toBeTruthy();
    return found as HTMLElement;
  });
}

// The lazy chunks loaded once before the first test, so a first transform never eats a query's wait.
beforeAll(async () => {
  await Promise.all([import("../tree/DirectionCatalog"), import("../doc/DocBody"), import("../health/HealthRegions")]);
});

describe("who uses what over the mock (UO10, UO11)", () => {
  it("the tides' Links: Used by 5 \u00b7 Mentioned by 2, its users grouped, each with its places", async () => {
    renderMock("#/harbor-sim/inbox?doc=MEC-TIDES");
    const dialog = await screen.findByRole("dialog");
    const into = within(await within(dialog).findByRole("group", { name: "Links in" }));
    expect(into.getByText("Used by 5 \u00b7 Mentioned by 2")).toBeTruthy();
    const labels = into.getAllByRole("group").map((group) => document.getElementById(group.getAttribute("aria-labelledby") ?? "")?.textContent ?? "");
    expect(labels.reduce((sum, label) => sum + Number(label.split(" \u00b7 ")[1] ?? 0), 0)).toBe(5);
    expect(dialog.querySelectorAll(".doc-usage-place").length).toBeGreaterThanOrEqual(5);
  });

  it("an unused document says so: the harbour, used by none", async () => {
    renderMock("#/harbor-sim/inbox?doc=DOM-HARBOR");
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText("Declared, nothing uses it yet.")).toBeTruthy();
  });

  it("Mechanics: the Used by column, the cranes' Orphan badge, No declared use and Not cited keeping their rows, K of 130", async () => {
    window.localStorage.setItem("specengine.module.harbor-sim", "mechanics");
    renderMock("#/harbor-sim/tree");
    const table = await screen.findByRole("table", { name: "Mechanics: 130 nodes" });
    expect(within(table).getByRole("button", { name: "Used by" })).toBeTruthy();
    const cranes = within(table).getByRole("link", { name: "Cargo cranes" }).closest("tr");
    expect(cranes?.querySelector(".catalog-used")?.textContent).toBe("0 Orphan");
    const kept = (name: string) => {
      fireEvent.click(screen.getByRole("button", { name }));
      const caption = screen.getByRole("table").querySelector("caption")?.textContent ?? "";
      return Number(/^Mechanics: (\d+) of 130 nodes$/.exec(caption)?.[1]);
    };
    const undeclared = kept("No declared use");
    expect(undeclared).toBeGreaterThan(0);
    expect(undeclared).toBeLessThan(130);
    expect(Array.from(document.querySelectorAll(".catalog-used"), (cell) => cell.textContent.split(" ")[0])).toEqual(Array.from({ length: Math.min(undeclared, 50) }, () => "0"));
    expect(document.querySelector(".catalog-used-cited")).not.toBeNull();
    const uncited = kept("Not cited");
    expect(uncited).toBeGreaterThan(0);
    expect(uncited).toBeLessThan(undeclared);
    expect(Array.from(document.querySelectorAll(".catalog-used"), (cell) => cell.textContent.split(" ")[0])).toEqual(Array.from({ length: Math.min(uncited, 50) }, () => "0"));
    expect(document.querySelector(".catalog-used-cited")).toBeNull();
  });

  it("Health: six orphan documents over 26 live ones, Mechanics first, then no kind; the mechanics' open tasks", async () => {
    renderMock("#/harbor-sim/health");
    const part = await orphansPart();
    expect(part.querySelector(".health-left-total")?.textContent).toBe("6 orphan documents over 26 live documents");
    const groups = Array.from(part.querySelectorAll<HTMLElement>(".health-orphans-group"), (group) => [
      document.getElementById(group.getAttribute("aria-labelledby") ?? "")?.textContent,
      Array.from(group.querySelectorAll(".health-orphans-tasks"), (tasks) => tasks.textContent),
    ]);
    expect(groups).toEqual([
      ["Mechanics", ["1 \u00b7 1 open", "1 \u00b7 1 open"]],
      ["No kind", ["No task", "No task", "No task", "No task"]],
    ]);
    expect(within(part).getByRole("link", { name: "Cargo cranes" }).getAttribute("href")).toBe("#/harbor-sim/tree/MEC-CARGO-CRANES/page");
  });

  it("orphan-notes: no task line, the note verbatim; empty: None over none", async () => {
    renderMock("#/harbor-sim/health", "orphan-notes");
    const part = await orphansPart();
    expect(part.querySelectorAll(".health-orphans-tasks")).toHaveLength(0);
    expect(within(part).getByText(/^orphans: tasks not read: /).getAttribute("lang")).toBe("en");
  });

  it("empty: None, measured over no live document", async () => {
    renderMock("#/harbor-sim/health", "empty");
    const part = await orphansPart();
    expect(part.querySelector(".health-left-total")?.textContent).toBe("None: every live document links or is linked.");
    expect(part.querySelector(".health-orphans-scope")?.textContent).toBe("Measured over 0 live documents.");
  });

  it("large: 3 000, the first ten, then Show all 3000", async () => {
    renderMock("#/harbor-sim/health", "large");
    const part = await orphansPart();
    expect(part.querySelector(".health-left-total")?.textContent).toMatch(/^3000 orphan documents over \d+ live documents$/);
    expect(part.querySelectorAll(".health-orphan")).toHaveLength(10);
    expect(within(part).getByRole("button", { name: "Show all 3000" })).toBeTruthy();
  });
});
