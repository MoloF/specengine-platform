import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// DC-6 and DC-10 of docs/features/direction-catalog.md "Acceptance criteria" over the real mock, as
// `pnpm dev` shows it: harbor-sim's direction Mechanics chosen, the node column holds its table,
// 130 rows shown 50 at a time, Show more moving focus to the first row it shows; a press keeps the
// count shown; a corpus mechanic's title opens its document; `empty`, the empty direction. The
// `error` scenario refuses the workspaces read too, so no direction is chosen there: a refusal of the
// catalog itself is DirectionCatalog.test.tsx's.

function renderMock(scenario: Scenario, hash = "#/harbor-sim/tree") {
  window.localStorage.setItem("specengine.module.harbor-sim", "mechanics");
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0 });
  const getCatalog = vi.spyOn(client, "getCatalog");
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return { client, getCatalog };
}

function shownRows(): number {
  return document.querySelectorAll(".catalog-table tbody tr").length;
}

/** The title link of the table's `nth` row, counted from 1. */
function titleLink(nth: number): HTMLElement | null {
  return Array.from(document.querySelectorAll(".catalog-table tbody tr"))[nth - 1]?.querySelector<HTMLElement>("a") ?? null;
}

// The table's chunk loaded once before the first test, so its first transform never eats a query's wait.
beforeAll(async () => {
  await import("../tree/DirectionCatalog");
});

describe("harbor-sim's Mechanics over the mock (DC-6, DC-10)", () => {
  it("130 rows show 50; Show more shows 100 and moves focus to row 51's title; again, 130 and the button gone (M: every row at once)", async () => {
    const { getCatalog } = renderMock("normal");
    const table = await screen.findByRole("table", { name: "Mechanics: 130 nodes" });
    expect(table.closest(".node-column")).not.toBeNull();
    expect(getCatalog.mock.calls.at(-1)?.slice(0, 2)).toEqual(["harbor-sim", { kinds: ["mechanic"], archive: false }]);
    expect(shownRows()).toBe(50);
    fireEvent.click(screen.getByRole("button", { name: "Show 50 more" }));
    await waitFor(() => {
      expect(document.activeElement).toBe(titleLink(51));
    });
    expect(shownRows()).toBe(100);
    fireEvent.click(screen.getByRole("button", { name: "Show 30 more" }));
    await waitFor(() => {
      expect(document.activeElement).toBe(titleLink(101));
    });
    expect(shownRows()).toBe(130);
    expect(screen.queryByRole("button", { name: /^Show \d+ more$/ })).toBeNull();
  });

  it("a press on Last change keeps the 50 shown, the newest first, none read last", async () => {
    renderMock("normal");
    await screen.findByRole("table", { name: "Mechanics: 130 nodes" });
    fireEvent.click(screen.getByRole("button", { name: "Last change" }));
    expect(shownRows()).toBe(50);
    const times = Array.from(document.querySelectorAll(".catalog-table tbody time"), (time) => time.getAttribute("dateTime") ?? "");
    expect(times).toEqual([...times].sort().reverse());
    fireEvent.click(screen.getByRole("button", { name: "Show 50 more" }));
    fireEvent.click(screen.getByRole("button", { name: "Show 30 more" }));
    const cells = Array.from(document.querySelectorAll(".catalog-table tbody .catalog-changed"), (cell) => cell.textContent);
    const first = cells.indexOf("Not committed");
    expect(first).toBeGreaterThan(0);
    expect(cells.slice(first).every((text) => text === "Not committed")).toBe(true);
  });

  it("a corpus mechanic's title opens its document in the modal", async () => {
    renderMock("normal");
    const table = await screen.findByRole("table", { name: "Mechanics: 130 nodes" });
    const link = within(table).getByRole("link", { name: "Mooring" });
    fireEvent.click(link);
    expect(window.location.hash).toBe("#/harbor-sim/tree?doc=MEC-MOORING");
    const dialog = await waitFor(() => {
      const found = document.querySelector<HTMLElement>(".doc-modal");
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(await within(dialog).findByRole("heading", { level: 2, name: /Mooring/ })).not.toBeNull();
  });

  it("an unknown state raw under lang=en, a section's (file) with its hint, no email in the DOM", async () => {
    renderMock("normal");
    await screen.findByRole("table", { name: "Mechanics: 130 nodes" });
    const unknown = Array.from(document.querySelectorAll(".catalog-table .badge-label")).filter((label) => label.textContent === "parked");
    expect(unknown.length).toBeGreaterThan(0);
    expect(unknown.every((label) => label.getAttribute("lang") === "en")).toBe(true);
    expect(document.querySelector(".catalog-of-file")?.textContent).toBe("(file)");
    expect(document.querySelector(".catalog-hint")).not.toBeNull();
    const shown = [document.body.textContent, ...Array.from(document.querySelectorAll("*"), (element) => Array.from(element.attributes, (attribute) => attribute.value).join(" "))].join("\n");
    expect(shown).not.toContain("@harbor.example.invalid");
  });

  it("empty: the direction's empty state, no table", async () => {
    renderMock("empty");
    expect(await screen.findByRole("heading", { name: "No node in this direction" })).not.toBeNull();
    expect(document.querySelector(".catalog-table")).toBeNull();
  });
});
