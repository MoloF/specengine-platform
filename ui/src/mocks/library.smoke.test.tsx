import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "../app/App";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// docs/features/doc-library.md "Description and interactions" over the mock, as `pnpm dev` shows it at
// `#/harbor-sim/documents`: galley's nav lists Documents after Spec tree; the department's module
// (Berths) and Class canon start the table, each one press from All; the harbour's page behind its
// decision, its decisions expanded; `large`'s 5 000 rows paged; `empty`'s empty state; `error`'s
// refusal in the daemon's words.

function renderMock(scenario: Scenario, hash = "#/harbor-sim/documents") {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  render(<App client={new MockClient(scenario, { delayMs: 0 })} scenario={scenario === "normal" ? null : scenario} />);
}

describe("harbor-sim's Documents over the mock", () => {
  it("lists Documents after Spec tree; starts at Berths and canon, each one press from All; the harbour's page behind its decision", async () => {
    renderMock("normal");
    const table = await screen.findByRole("table");
    const nav = screen.getByRole("navigation", { name: "Sections" });
    const labels = within(nav).getAllByRole("link").map((link) => link.textContent);
    expect(labels.indexOf("Documents")).toBe(labels.indexOf("Spec tree") + 1);
    expect(screen.getByRole("button", { name: /^Module: / }).textContent).toBe("Module: Berths");
    expect(screen.getByRole("button", { name: /^Class: / }).textContent).toBe("Class: canon");
    expect(within(table).getAllByRole("rowheader").map((cell) => cell.querySelector("a")?.textContent)).toEqual(["Berths and moorings"]);
    fireEvent.click(screen.getByRole("button", { name: "All modules" }));
    expect(within(table).getAllByRole("rowheader")).toHaveLength(6);
    const harbour = document.querySelector('tr[data-row="DOM-HARBOR"]') as HTMLElement;
    expect(within(harbour).getByText("Behind its decision")).toBeTruthy();
    fireEvent.click(within(harbour).getByRole("button", { name: "1 accepted \u00b7 1 superseded \u00b7 1 rejected" }));
    expect(screen.getByRole("link", { name: "Weather is simulated" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "All classes" }));
    expect(document.querySelector(".documents-count")?.textContent).toMatch(/^\d+ documents \u00b7 1 behind its decision$/);
  });

  it("serves `large`'s 5 000 rows, 200 a page", async () => {
    renderMock("large", "#/harbor-sim/documents");
    await screen.findByRole("table");
    expect(document.querySelector(".documents-count")?.textContent).toMatch(/^\d+ of 5000 documents/);
    fireEvent.click(screen.getByRole("button", { name: "All modules" }));
    fireEvent.click(screen.getByRole("button", { name: "All classes" }));
    expect(document.querySelector(".documents-count")?.textContent).toMatch(/^5000 documents \u00b7 \d+ behind their decision$/);
    expect(document.querySelectorAll(".documents-table tbody tr[data-row]")).toHaveLength(200);
    expect(screen.getByRole("button", { name: "Show 200 more" })).toBeTruthy();
    // 5 000 rows filtered and 200 drawn in jsdom: room for a laptop busy with a Rust build.
  }, 20_000);

  it("shows `empty`'s empty state and `error`'s refusal in the daemon's words", async () => {
    renderMock("empty");
    expect(await screen.findByRole("heading", { name: "No document yet" })).toBeTruthy();
  });

  it("shows `error`'s refusal with Retry", async () => {
    renderMock("error");
    expect(await screen.findByText("The documents could not be read")).toBeTruthy();
    expect(within(document.querySelector(".documents-view") as HTMLElement).getAllByText(/the database is locked by another process/).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
  });
});
