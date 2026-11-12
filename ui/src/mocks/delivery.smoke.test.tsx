import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// DA-12 and DA-13 of docs/features/delivery-analytics.md "Acceptance criteria": the Delivery tab
// over the real mock, as `pnpm dev` shows it: harbor-sim's every shape at the viewer's day, a day
// moved past a due date, and the empty project.

/** Noon of 2026-11-12 on the viewer's clock: the mock's facts and the tab's day agree. */
const NOON = new Date(2026, 10, 12, 12, 0);

function renderMock(scenario: Scenario) {
  window.history.replaceState(null, "", `/?scenario=${scenario}#/harbor-sim/roadmap`);
  const client = new MockClient(scenario, { now: () => NOON.getTime() });
  const getDelivery = vi.spyOn(client, "getDelivery");
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return { client, getDelivery };
}

async function delivery() {
  fireEvent.click(await screen.findByRole("tab", { name: "Delivery" }));
}

function firstColumn(name: string): (string | null)[] {
  return Array.from(screen.getByRole("table", { name }).querySelectorAll("tr"), (row) => row.firstElementChild?.textContent ?? null);
}

function lateIds(): (string | null)[] {
  return Array.from(document.querySelectorAll("[data-late]"), (task) => task.getAttribute("data-late"));
}

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"], now: NOON });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("the Delivery tab over the mock (DA-12)", () => {
  it("normal: harbor-sim's groups, departments, total and late tasks at the viewer's day, each reason drawn", async () => {
    const { getDelivery } = renderMock("normal");
    await delivery();
    await screen.findByRole("table", { name: "By release" });
    await waitFor(() => {
      expect(getDelivery.mock.calls.at(-1)?.[1]).toEqual({ asOf: "2026-11-12" });
    });
    expect(firstColumn("By release")).toEqual(["Release", "keel", "hull", "sails", "Pool", "Unplaced", "milestone", "No release"]);
    const milestone = screen.getByRole("table", { name: "By release" }).querySelector('[data-group="milestone"]');
    expect(milestone?.querySelector("bdi")?.getAttribute("lang")).toBe("en");
    expect(milestone?.children[1]?.textContent).toBe("1 of 1");
    expect(firstColumn("By department")).toEqual(["Department", "Galley", "Foundry", "Chandlery", "All tasks"]);
    expect(screen.getByRole("table", { name: "By department" }).querySelector('[data-department="chandlery"]')?.children[1]?.textContent).toBe("0 of 0");
    expect(lateIds()).toEqual(["C-0005", "C-0008", "C-0001"]);
    const foreword = document.querySelector<HTMLElement>('[data-late="C-0001"]');
    if (foreword === null) {
      throw new Error("no C-0001");
    }
    expect(foreword.querySelector(".lozenge")?.textContent).toBe("Overdue by 2 days");
    expect(within(foreword).getByRole("link", { name: "C-0001" }).getAttribute("href")).toBe("#/harbor-sim/board/C-0001");
    expect(within(foreword).getByRole("link", { name: "PR-0055" }).getAttribute("href")).toBe("#/harbor-sim/inbox/PR-0055");
    expect(Array.from(foreword.querySelectorAll(".delivery-more"), (line) => line.textContent)).toEqual(["1 more not listed"]);
    expect(Array.from(foreword.querySelectorAll("[data-reason]"), (line) => line.getAttribute("data-reason"))).toEqual([
      ...Array.from({ length: 16 }, () => "return"),
      "reestimate",
      "cancelled",
      "item",
      "item",
      "item",
    ]);
    expect(Array.from(document.querySelectorAll('[data-late="C-0008"] [data-reason="reestimate"]')).at(-1)?.textContent).toBe(
      "Estimate 3 cleared on 2026-11-11 10:00 UTCNo estimate until the survey lands",
    );
    expect(document.querySelector('[data-late="C-0005"] .lozenge')?.textContent).toBe("Delivered 5 days late");
  });

  it("a day moved past a due date: C-0004 falls overdue, C-0003 the day after", async () => {
    const { getDelivery } = renderMock("normal");
    await delivery();
    await screen.findByRole("table", { name: "By release" });
    fireEvent.change(screen.getByLabelText("As of"), { target: { value: "2026-11-13" } });
    await waitFor(() => {
      expect(lateIds()).toEqual(["C-0008", "C-0005", "C-0001", "C-0004"]);
    });
    expect(document.querySelector('[data-late="C-0004"] .lozenge')?.textContent).toBe("Overdue by 1 day");
    fireEvent.change(screen.getByLabelText("As of"), { target: { value: "2026-11-14" } });
    await waitFor(() => {
      expect(lateIds()).toContain("C-0003");
    });
    expect(getDelivery.mock.calls.map((call) => call[1].asOf)).toEqual(expect.arrayContaining(["2026-11-12", "2026-11-13", "2026-11-14"]));
  });

  it("empty: the project with no task is the EmptyState, no table", async () => {
    renderMock("empty");
    await delivery();
    await waitFor(() => {
      expect(document.querySelector(".delivery h3.empty-state-title")?.textContent).toBe("No task to count yet");
    });
    expect(screen.queryByRole("table")).toBeNull();
  });
});
