import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { MockClient } from "./MockClient";
import { GREEK_TITLE, JAPANESE_TITLE } from "./roadmap";
import type { Scenario } from "./scenario";

// UI-6 of docs/features/roadmap-screen.md: the Roadmap over the real mock, as `pnpm dev` shows it
// (its "Owner's manual check", step 6): harbor-sim's plan in normal; ledger-api's 503 (the CLI's
// words verbatim, Retry, the fixed line); empty; error; large (every card listed, the canvas's
// count); slow.

const FIXED_LINE = "A project has a plan once its specengine.toml has a [roadmap] table";

function renderMock(scenario: Scenario, hash: string, delayMs = 0) {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs, now: () => Date.parse("2026-10-06T12:00:00Z") });
  const getRoadmap = vi.spyOn(client, "getRoadmap");
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return { client, getRoadmap };
}

async function loaded() {
  await screen.findByRole("heading", { level: 1, name: "Roadmap" });
  await screen.findByRole("tab", { name: "Lanes" });
}

function card(id: string): HTMLElement {
  const found = Array.from(document.querySelectorAll<HTMLElement>("[data-item]")).find((element) => element.dataset.item === id);
  if (found === undefined) {
    throw new Error(`no card ${id}`);
  }
  return found;
}

describe("the Roadmap over the mock (UI-6)", () => {
  it("normal: harbor-sim's plan, one read; its lanes, marks, cut, tasks, pool tasks, titles and notes", async () => {
    const { getRoadmap } = renderMock("normal", "#/harbor-sim/roadmap");
    await loaded();
    expect(getRoadmap).toHaveBeenCalledTimes(1);
    expect(document.querySelector(".roadmap-line")?.textContent).toBe("13 open, 5 done; current hull; to keel 0 tides; total 16-21 tides, 2 unestimated");
    expect(screen.getAllByRole("heading", { level: 2 }).map((heading) => heading.textContent)).toEqual(["keel", "hull", "sails", "Pool", "Unplaced", "Notes"]);
    expect(document.querySelector(".roadmap-cut-rule")?.textContent).toBe("MVP cut: 0 tides");
    expect(within(card("PLAN-0006")).getByRole("link", { name: "T-0109" }).getAttribute("href")).toBe("#/harbor-sim/tasks/T-0109");
    expect(card("PLAN-0006").querySelector("[data-task] .badge-label")?.textContent).toBe("In progress");
    expect(card("PLAN-0012").querySelector("[data-task] .badge-label")?.textContent).toBe("Ready");
    expect(card("PLAN-0010").querySelector(".roadmap-card-name")?.textContent).toBe(JAPANESE_TITLE);
    expect(card("PLAN-0013").querySelector(".roadmap-card-name")?.textContent).toBe(GREEK_TITLE);
    expect(card("PLAN-0010").querySelector(".tone-roadmap-status .badge-label")?.textContent).toBe("status parked");
    expect(card("PLAN-0031").querySelector(".roadmap-release")?.textContent).toBe("release not a string");
    expect(card("PLAN-0030").querySelector(".roadmap-release")?.textContent).toBe("release someday");
    expect(Array.from(document.querySelectorAll<HTMLElement>("[data-pool-task]"), (row) => row.dataset.poolTask)).toEqual(["T-0107", "T-0108", "T-0110", "T-0111", "T-0113"]);
    expect(document.querySelectorAll(".roadmap-notes li")).toHaveLength(6);
    expect(document.querySelector(".roadmap-view")?.textContent).not.toMatch(/block/i);
  });

  it("normal: the Chain tab draws harbor-sim's waits, across lanes too, and no edge for the one-item cycle", async () => {
    renderMock("normal", "#/harbor-sim/roadmap");
    await loaded();
    fireEvent.click(screen.getByRole("tab", { name: "Chain" }));
    await screen.findByRole("application", { name: "Roadmap canvas" });
    await waitFor(() => {
      expect(document.querySelectorAll('.react-flow__node[data-id^="i:"]')).toHaveLength(13);
    });
    const edges = Array.from(document.querySelectorAll(".react-flow__edge"), (edge) => edge.getAttribute("aria-label"));
    expect(edges).toEqual(expect.arrayContaining(["PLAN-0012 waits on PLAN-0007", "PLAN-0021 waits on PLAN-0014", "PLAN-0014 waits on PLAN-0008"]));
    expect(edges.filter((edge) => edge?.startsWith("PLAN-0013"))).toEqual([]);
  });

  it("ledger-api: the 503 in the CLI's words, verbatim, with Retry and the fixed line", async () => {
    const { getRoadmap } = renderMock("normal", "#/ledger-api/roadmap");
    await screen.findByRole("heading", { level: 1, name: "Roadmap" });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe("The plan could not be readspec: specengine.toml has no [roadmap]");
    expect(alert.querySelector(".error-message")?.textContent).toBe("spec: specengine.toml has no [roadmap]");
    expect(document.querySelector(".roadmap-no-plan")?.textContent).toBe(FIXED_LINE);
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(getRoadmap).toHaveBeenCalledTimes(2);
    });
    expect(await screen.findByText(FIXED_LINE)).toBeTruthy();
  });

  it("empty: no plan items yet, naming the dir and the prefix; the lanes drawn", async () => {
    renderMock("empty", "#/harbor-sim/roadmap");
    await loaded();
    expect(document.querySelector(".roadmap-empty")?.textContent).toBe("No plan items yet: an item is a file in docs/plan whose id starts with PLAN-");
    expect(screen.getAllByRole("heading", { level: 2 }).map((heading) => heading.textContent)).toEqual(["keel", "hull", "sails", "Pool", "Unplaced"]);
    expect(document.querySelectorAll("[data-item]")).toHaveLength(0);
  });

  it("error: the scenario's 503 verbatim, Retry, the fixed line", async () => {
    renderMock("error", "#/harbor-sim/roadmap");
    const alert = await screen.findByRole("alert");
    expect(alert.querySelector(".error-message")?.textContent).toBe("spec index unavailable: the database is locked by another process (mock scenario: error)");
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
    expect(document.querySelector(".roadmap-no-plan")?.textContent).toBe(FIXED_LINE);
  });

  it("large: every one of the 500 cards listed; the canvas draws whole lanes to 200 and names the rest", async () => {
    renderMock("large", "#/harbor-sim/roadmap");
    await loaded();
    expect(document.querySelectorAll("[data-item]")).toHaveLength(500);
    fireEvent.click(screen.getByRole("tab", { name: "Chain" }));
    await screen.findByRole("application", { name: "Roadmap canvas" });
    expect(screen.getByRole("note").textContent).toBe("344 of 500 items not drawn");
    await waitFor(() => {
      expect(document.querySelectorAll('.react-flow__node[data-id^="i:"]')).toHaveLength(156);
    });
  });

  it("slow: a skeleton with aria-busy first, then the plan", async () => {
    renderMock("slow", "#/harbor-sim/roadmap", 40);
    const skeleton = await screen.findByRole("status", { name: "Reading the plan of harbor-sim" });
    expect(skeleton.getAttribute("aria-busy")).toBe("true");
    await loaded();
    expect(document.querySelectorAll("[data-item]")).toHaveLength(13);
  });
});
