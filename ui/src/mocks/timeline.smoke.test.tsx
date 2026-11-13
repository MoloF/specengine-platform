import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// DT8 and DT9 of docs/features/delivery-timeline.md "Acceptance criteria" over the real mock, as
// `pnpm dev` shows it: harbor-sim's timeline at the viewer's day, every shape; `large`'s 2 000 tasks,
// 200 rows mounted, then 200 more; `empty`'s line. Nothing here is timed: jsdom is no browser, and
// DT9's figure is Chrome's on a production build (the orchestrator's look).

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
  await screen.findByRole("table", { name: "By release" }, { timeout: 5000 });
}

function view(name: "Tables" | "Timeline") {
  fireEvent.click(within(screen.getByRole("group", { name: "View" })).getByRole("button", { name }));
}

function rowIds(): (string | undefined)[] {
  return Array.from(document.querySelectorAll<HTMLElement>(".delivery-timeline-row"), (row) => row.dataset.task);
}

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"], now: NOON });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("the Delivery timeline over the mock (DT8)", () => {
  it("normal: harbor-sim's rows under their groups at the viewer's day, an overdue, a late and an untaken row, an accept from the first column", async () => {
    const { getDelivery } = renderMock("normal");
    await delivery();
    view("Timeline");
    await screen.findByRole("region", { name: "Tasks over time" });
    expect(getDelivery.mock.calls.at(-1)?.[1]).toEqual({ asOf: "2026-11-12", timeline: true });
    expect(Array.from(document.querySelectorAll(".delivery-timeline-group h2"), (heading) => heading.textContent)).toEqual(["keel", "hull", "sails", "Pool", "Unplaced", "milestone"]);
    expect(rowIds()).toHaveLength(17);
    expect(rowIds().slice(0, 3)).toEqual(["C-0005", "C-0001", "C-0013"]);
    const describedBy = (id: string) => {
      let description = "";
      within(document.querySelector<HTMLElement>(`[data-task="${id}"]`) ?? document.body).getByRole("link", {
        name: id,
        description: (computed) => {
          description = computed;
          return true;
        },
      });
      return description;
    };
    expect(describedBy("C-0001")).toBe("Write the tide table foreword Created 29 Oct 2026. Taken 31 Oct 2026. Due 10 Nov 2026. Overdue by 2 days.");
    expect(describedBy("C-0005")).toBe("Mooring fees table Created 27 Oct 2026. Taken 28 Oct 2026. Accepted 8 Nov 2026. Due 3 Nov 2026. Delivered 5 days late.");
    expect(describedBy("C-0002")).toBe("Caption the berth plans Created 9 Nov 2026. Due 18 Nov 2026.");
    const spring = document.querySelector<HTMLElement>('[data-task="C-0012"] .delivery-timeline-work');
    expect([spring?.style.width, spring?.dataset.accepted]).toEqual(["0%", "true"]);
    expect(document.querySelector(".delivery-timeline-span")?.textContent).toBe("From 27 Oct 2026 to 22 Nov 2026, as of 12 Nov 2026");
  });

  it("empty: the line for no task to place, no region", async () => {
    renderMock("empty");
    fireEvent.click(await screen.findByRole("tab", { name: "Delivery" }));
    await screen.findByRole("heading", { name: "No task to count yet" });
    view("Timeline");
    expect(await screen.findByRole("heading", { name: "No task to place in time" })).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Tasks over time" })).toBeNull();
  });
});

describe("`large`: 2 000 tasks, 200 rows at a time (DT9)", () => {
  it("mounts exactly 200 rows, at most 12 elements a row; Show 200 more 400, the focus on the first new row", async () => {
    renderMock("large");
    await delivery();
    view("Timeline");
    await waitFor(
      () => {
        expect(document.querySelectorAll(".delivery-timeline-row")).toHaveLength(200);
      },
      { timeout: 10_000 },
    );
    expect(Math.max(...Array.from(document.querySelectorAll(".delivery-timeline-row"), (row) => 1 + row.querySelectorAll("*").length))).toBeLessThanOrEqual(12);
    expect(Array.from(document.querySelectorAll(".delivery-timeline-group h2"), (heading) => heading.textContent)).toEqual(["keel"]);
    // Again, the chunk loaded and the read kept: the render alone.
    view("Tables");
    await screen.findByRole("table", { name: "By release" });
    view("Timeline");
    await waitFor(() => {
      expect(document.querySelectorAll(".delivery-timeline-row")).toHaveLength(200);
    });
    fireEvent.click(screen.getByRole("button", { name: "Show 200 more (1800 left)" }));
    await waitFor(() => {
      expect(document.querySelectorAll(".delivery-timeline-row")).toHaveLength(400);
    });
    expect(document.activeElement?.textContent).toBe(rowIds()[200]);
  }, 30_000);
});
