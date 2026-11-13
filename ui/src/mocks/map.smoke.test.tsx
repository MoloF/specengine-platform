import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "../app/App";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// docs/features/corpus-map.md "Acceptance criteria", CM8 and CM14's look over the mock, as
// `pnpm dev` shows it at `#/harbor-sim/graph`: the Map is the Graph's home, its count line the
// mock's, its legend a colour per kind, most held first, and the grey, its List every kept document;
// the Cyrillic basin among the sections, found by the search in its composed form; `large`'s 5 000
// and its eight coloured kinds; `error`'s refusal in the daemon's words.

function renderMock(scenario: Scenario, hash = "#/harbor-sim/graph") {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  render(<App client={new MockClient(scenario, { delayMs: 0 })} scenario={scenario === "normal" ? null : scenario} />);
}

describe("harbor-sim's Map over the mock", () => {
  it("opens on the Map with the mock's counts, three kinds' colours and the grey in the legend, and 26 rows in the List", async () => {
    renderMock("normal");
    expect(await screen.findByText(/^26 documents, \d+ links, 2 orphans$/)).toBeTruthy();
    const legend = await screen.findByRole("heading", { level: 3, name: "Legend" });
    const items = Array.from(legend.closest("section")?.querySelectorAll(".map-legend-item") ?? [], (item) => item.textContent);
    expect(items.slice(0, 4)).toEqual(["mechanic13 nodes", "domain4 nodes", "rule3 nodes", "Other kinds6 nodes"]);
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    expect(document.querySelectorAll(".map-table tbody tr")).toHaveLength(26);
  });

  it("finds the Cyrillic basin, a section, by its decomposed title typed composed, once Sections is on", async () => {
    renderMock("normal");
    await screen.findByRole("region", { name: "Map picture" });
    fireEvent.click(screen.getByRole("checkbox", { name: "Sections" }));
    const search = screen.getByRole("searchbox", { name: "Find on the map" });
    fireEvent.change(search, { target: { value: "\u0417\u0435\u043b\u0451\u043d\u044b\u0439" } });
    fireEvent.keyDown(search, { key: "Enter" });
    await waitFor(() => {
      expect(document.querySelector(".map-card .mono.map-card-place")?.textContent).toBe("docs/spec/berths/README.md:17");
    });
  });

  it("serves `large`'s 5 000 nodes with no cap line, and `error`'s refusal with Retry", async () => {
    renderMock("large");
    expect(await screen.findByText(/^4500 documents, 15000 links, \d+ orphans$/)).toBeTruthy();
    expect(screen.queryByText(/most used of/)).toBeNull();
    const legend = screen.getByRole("region", { name: "Legend" });
    const kinds = Array.from(within(legend).getByRole("list", { name: "Kinds" }).querySelectorAll(".map-legend-item"), (item) => [item.querySelector(".map-legend-kind")?.textContent, item.querySelector(".map-swatch")?.className]);
    expect(kinds).toEqual([
      ["mechanic", "map-swatch map-slot-1"],
      ["rule", "map-swatch map-slot-2"],
      ["item", "map-swatch map-slot-3"],
      ["quest", "map-swatch map-slot-4"],
      ["character", "map-swatch map-slot-5"],
      ["dialogue", "map-swatch map-slot-6"],
      ["domain", "map-swatch map-slot-7"],
      ["event", "map-swatch map-slot-8"],
      ["Other kinds", "map-swatch map-slot-0"],
    ]);
  });

  it("shows `error`'s refusal in the daemon's words", async () => {
    renderMock("error");
    expect(await screen.findByText("The map could not be read")).toBeTruthy();
    expect(within(document.querySelector(".map-view") as HTMLElement).getAllByText(/the database is locked by another process/).length).toBeGreaterThan(0);
  });
});
