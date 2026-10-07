import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// Health over the real mock, as `pnpm dev` shows it (docs/features/ui-health.md AC-01, AC-03,
// AC-04, AC-05, AC-09; "Owner's manual check" for cannot-check, empty, large, error and slow).

const METHODS = ["getProjects", "getInbox", "getTree", "getNode", "search", "getBundle", "getGraph", "getTasks", "getTask", "getCheck", "stageDecision", "unstageDecision"] as const;

function renderMock(scenario: Scenario, hash: string, delayMs = 0) {
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs, now: () => Date.parse("2026-10-06T12:00:00Z") });
  const spies = Object.fromEntries(METHODS.map((name) => [name, vi.spyOn(client, name)]));
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return { client, spies };
}

function calls(spies: Record<string, { mock: { calls: unknown[] } }>): Record<string, number> {
  return Object.fromEntries(Object.entries(spies).flatMap(([name, spy]) => (spy.mock.calls.length > 0 ? [[name, spy.mock.calls.length]] : [])));
}

function region(name: string): HTMLElement {
  return screen.getByRole("region", { name });
}

async function loadedHealth(scenario: Scenario = "normal", slug = "harbor-sim") {
  const rendered = renderMock(scenario, `#/${slug}/health`);
  await screen.findByRole("heading", { level: 1, name: "Health" });
  await waitFor(() => {
    expect(document.querySelectorAll('.health-region[aria-busy="false"]')).toHaveLength(4);
  });
  return rendered;
}

function verdict(): string {
  return region("Check").querySelector(".check-verdict .badge-label")?.textContent ?? "";
}

describe("Health over the mock", () => {
  it("#/harbor-sim/health reads the projects, the check and the inbox once each; the home reads no check (AC-01)", async () => {
    const { spies } = await loadedHealth();
    expect(calls(spies)).toEqual({ getProjects: 1, getCheck: 1, getInbox: 1 });
    cleanup();
    const home = renderMock("normal", "#/harbor-sim");
    await screen.findByRole("heading", { level: 1, name: "Overview" });
    await waitFor(() => {
      expect(document.querySelectorAll('.home-region[aria-busy="false"]')).toHaveLength(2);
    });
    expect(calls(home.spies).getCheck).toBeUndefined();
  });

  it("harbor-sim passes with findings: its sample's verdict, W, findings, debt, stale entry and budget", async () => {
    await loadedHealth();
    expect(verdict()).toBe("Passes with findings");
    expect(region("Check").querySelector(".check-worst-w")?.textContent).toBe("61234 B");
    expect(region("Findings").querySelectorAll("[data-finding]")).toHaveLength(3);
    expect(region("Debt and budgets").querySelectorAll(".debt-row")).toHaveLength(1);
    expect(Array.from(region("Debt and budgets").querySelectorAll(".stale-row"), (item) => item.textContent)).toEqual([
      ".spec-debt.toml line 7: file-name on docs/spec/harbor.md matches nothing",
    ]);
    expect(region("Debt and budgets").querySelector(".budget-row .finding-message")?.textContent).toBe(
      "12950 bytes, over the canon cap of 12288: move detail down a tier; caps are never raised",
    );
  });

  it("ledger-api fails the check, said without any hold wording (AC-03)", async () => {
    await loadedHealth("normal", "ledger-api");
    expect(verdict()).toBe("Fails the check");
    expect(document.querySelector(".health-view")?.textContent).not.toMatch(/block/i);
    expect(region("Findings").querySelector(".finding-fix-text")?.textContent).toBe("POL-IDEMPOTENCY");
  });

  it.each(["harbor-sim", "ledger-api"])("lists %s's first five as the Inbox view does, the severity counts summing to its queue (AC-05)", async (slug) => {
    renderMock("normal", `#/${slug}/inbox`);
    const list = await screen.findByRole("listbox", { name: "Proposals by severity, then age" });
    const queue = within(list)
      .getAllByRole("option")
      .map((option) => option.dataset.proposal ?? "");
    cleanup();
    await loadedHealth("normal", slug);
    const left = region("What is left");
    const first = Array.from(left.querySelectorAll(".health-left-link"), (link) => /^(\S+):/.exec(link.textContent)?.[1]);
    expect(first).toEqual(queue.slice(0, 5));
    const counts = Array.from(left.querySelectorAll(".health-tally-count"), (count) => Number(count.textContent));
    expect(counts.reduce((total, count) => total + count, 0)).toBe(queue.length);
  });

  it("cannot-check: Could not check, W not measured, two causes verbatim (AC-04)", async () => {
    await loadedHealth("cannot-check");
    expect(verdict()).toBe("Could not check");
    expect(region("Check").querySelector(".check-worst-w")?.textContent).toBe("Not measured");
    expect(region("Check").textContent).not.toContain("0 B");
    expect(Array.from(region("Check").querySelectorAll(".check-causes li"), (item) => item.textContent)).toEqual([
      "docs/spec/cranes.md: cannot read: Permission denied (os error 13)",
      "docs/spec/locks: directory cannot be listed; its files are unchecked",
    ]);
    expect(region("Findings").textContent).toContain("the check could not read everything");
  });

  it("empty: the clean text and the Inbox link (AC-09)", async () => {
    await loadedHealth("empty");
    expect(region("Check").querySelector(".health-clean p")?.textContent).toBe("The check is clean: 4 documents, no findings, no debt.");
    expect(region("Check").querySelector(".health-clean a")?.getAttribute("href")).toBe("#/harbor-sim/inbox");
  });

  it("large: 3 000 findings in twelve groups, each over ten rows collapsed", async () => {
    await loadedHealth("large");
    const groups = Array.from(region("Findings").querySelectorAll<HTMLElement>(".finding-group"));
    expect(groups).toHaveLength(12);
    const toggles = groups.map((group) => group.querySelector(".finding-group-toggle"));
    expect(toggles.map((toggle) => toggle?.getAttribute("aria-expanded"))).toEqual([
      "true",
      ...Array.from({ length: 11 }, () => "false"),
    ]);
    expect(region("Findings").querySelectorAll("[data-finding]")).toHaveLength(8);
    expect(region("Findings").querySelector(".finding-total")?.textContent).toBe("3000 findings, by code, errors first");
  });

  it("error: each region in the daemon's words with a Retry (AC-09)", async () => {
    renderMock("error", "#/harbor-sim/health");
    await screen.findByRole("region", { name: "Debt and budgets" });
    for (const name of ["Check", "What is left", "Findings", "Debt and budgets"]) {
      expect(await within(region(name)).findByText(/^spec index unavailable/)).toBeTruthy();
      expect(within(region(name)).getByRole("button", { name: "Retry" })).toBeTruthy();
    }
  });

  it("slow: every region busy with a skeleton until its read answers (AC-09)", async () => {
    renderMock("slow", "#/harbor-sim/health", 50);
    await screen.findByRole("region", { name: "Debt and budgets" });
    for (const name of ["Check", "What is left", "Findings", "Debt and budgets"]) {
      expect(region(name).getAttribute("aria-busy")).toBe("true");
      expect(within(region(name)).getByRole("status").getAttribute("aria-busy")).toBe("true");
    }
    await waitFor(() => {
      expect(document.querySelectorAll('.health-region[aria-busy="false"]')).toHaveLength(4);
    });
  });
});
