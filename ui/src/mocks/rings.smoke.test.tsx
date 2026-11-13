import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { docOf } from "../app/routes";
import { showRings } from "../test/render";
import { MockClient } from "./MockClient";
import { LEDGER_RING_NOTE } from "./rings";
import type { Scenario } from "./scenario";

// docs/features/scope-rings.md "Acceptance criteria", SR7 and SR9 over the real mock, as `pnpm dev`
// shows it at `#/harbor-sim/graph`: the Graph's home holds the Rings view under the REF's empty
// state, its four rings of 12, 5, 1 and 0 sectors; the long title cut on the drawing and whole in its
// name and the List; the keyboard reaches and walks the rings; a sector opens its document; ledger-api
// shows its bad table's note; `empty`, none configured; `error`, the daemon's words.

async function renderMock(scenario: Scenario, hash = "#/harbor-sim/graph") {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0 });
  const getRings = vi.spyOn(client, "getRings");
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  await showRings();
  return { client, getRings };
}

function sectors(): SVGPathElement[] {
  return Array.from(document.querySelectorAll<SVGPathElement>(".rings-svg .rings-sector"));
}

/** The long-titled harbour log's sector. */
function harbourLog(): SVGPathElement {
  const found = sectors().find((element) => element.getAttribute("aria-label")?.startsWith("Harbour log:"));
  if (found === undefined) {
    throw new Error("no harbour log");
  }
  return found;
}

beforeAll(async () => {
  await import("../rings/RingsView");
});

describe("harbor-sim's rings over the mock (SR7, SR9)", () => {
  it("draws four rings under the REF's empty state, 19 sectors, the legend counting 13, 5, 1 and 0, read once", async () => {
    const { getRings } = await renderMock("normal");
    const drawing = await screen.findByRole("group", { name: "Scope rings" });
    expect(screen.getByRole("heading", { level: 1, name: "Graph" })).toBeTruthy();
    expect(screen.getByText("Give a REF to draw its links")).toBeTruthy();
    expect(drawing.closest(".graph-home")).not.toBeNull();
    expect(sectors()).toHaveLength(19);
    expect(getRings).toHaveBeenCalledTimes(1);
    const legend = screen.getByRole("region", { name: "Legend" });
    expect(within(within(legend).getByRole("list", { name: "Scope rings" })).getAllByRole("listitem").map((item) => item.textContent)).toEqual([
      "Every mechanic 13 sectors",
      "Berths 5 sectors",
      "Pilot station 1 sector",
      "Locks 0 sectors",
    ]);
    expect(document.querySelectorAll(".rings-svg .rings-band")).toHaveLength(1);
  });

  it("names the long title whole and draws it cut, if at all, with an ellipsis", async () => {
    await renderMock("normal");
    await screen.findByRole("group", { name: "Scope rings" });
    const name = harbourLog().getAttribute("aria-label") ?? "";
    expect(name).toMatch(/^Harbour log: every call, .+ to leaving\., mechanic, .+, ring Every mechanic$/);
    for (const label of Array.from(document.querySelectorAll(".rings-svg .rings-label"), (element) => element.textContent)) {
      expect(label.length).toBeLessThan(40);
    }
    expect(Array.from(document.querySelectorAll(".rings-svg .rings-label"), (element) => element.textContent).some((label) => label.endsWith("\u2026"))).toBe(true);
  });

  it("is reached once by Tab and walked by the arrows, and Enter opens the sector's document", async () => {
    await renderMock("normal");
    await screen.findByRole("group", { name: "Scope rings" });
    expect(sectors().filter((element) => element.getAttribute("tabindex") === "0")).toHaveLength(1);
    const first = sectors()[0];
    if (first === undefined) {
      throw new Error("no sector");
    }
    act(() => {
      first.focus();
    });
    fireEvent.keyDown(first, { key: "ArrowDown" });
    expect(document.activeElement?.getAttribute("aria-label")).toMatch(/, ring Berths$/);
    fireEvent.keyDown(document.activeElement ?? first, { key: "ArrowDown" });
    expect(document.activeElement?.getAttribute("aria-label")).toMatch(/^Pilot station, domain, .+, ring Pilot station$/);
    // The empty ring holds nothing to go to.
    fireEvent.keyDown(document.activeElement ?? first, { key: "ArrowDown" });
    expect(document.activeElement?.getAttribute("aria-label")).toMatch(/, ring Pilot station$/);
    fireEvent.keyDown(document.activeElement ?? first, { key: "Enter" });
    await waitFor(() => {
      expect(docOf(window.location.hash)).toBe("DOM-PILOTS");
    });
  });

  it("lists every sector by ring in the List, the long title whole", async () => {
    await renderMock("normal");
    await screen.findByRole("group", { name: "Scope rings" });
    fireEvent.click(screen.getByRole("tab", { name: "List" }));
    const list = document.querySelector<HTMLElement>(".rings-list");
    expect(list?.querySelectorAll("li.rings-list-row")).toHaveLength(19);
    expect(Array.from(list?.querySelectorAll(".rings-list-where") ?? [], (line) => line.textContent)).toEqual([
      "kinds mechanic",
      "root DOM-BERTHS \u00b7 kinds domain, rule",
      "root DOM-PILOTS",
      "root MEC-LOCK-A \u00b7 kinds rule",
    ]);
    expect(within(list ?? document.body).getByRole("link", { name: /^Harbour log: every call/ }).textContent).toMatch(/to leaving\.$/);
  });

  it("shows ledger-api's bad table: its note verbatim and the next step, no drawing", async () => {
    await renderMock("normal", "#/ledger-api/graph");
    const notes = await screen.findByRole("list", { name: "Config notes" });
    expect(notes.textContent).toBe(LEDGER_RING_NOTE);
    expect(document.querySelector(".rings-example")).not.toBeNull();
    expect(document.querySelector(".rings-svg")).toBeNull();
  });

  it("says none is configured in `empty`", async () => {
    await renderMock("empty");
    expect(await screen.findByRole("heading", { level: 3, name: "No rings configured" })).toBeTruthy();
  });

  it("shows the daemon's refusal in `error`, with Retry", async () => {
    await renderMock("error");
    expect(await screen.findByText("The rings could not be read")).toBeTruthy();
    expect(screen.getAllByText(/the database is locked by another process/).length).toBeGreaterThan(0);
  });
});
