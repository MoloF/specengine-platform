import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { BASIN_NAME_DECOMPOSED, HOSTILE_TITLE } from "./harbor-sim/fixtures";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// Health over the real mock, as `pnpm dev` shows it (docs/features/ui-health.md AC-01, AC-03,
// AC-04, AC-05, AC-09; "Owner's manual check" for cannot-check, empty, large, error and slow).
// docs/features/health-left.md AC-U3 to AC-U6: row 3 over the mock's scenarios (W13).
// docs/features/health-code-ui.md UI-1, UI-2, UI-4, UI-6: rows 4 and 5 over the mock's scenarios.

const METHODS = [
  "getProjects",
  "getInbox",
  "getTree",
  "getNode",
  "search",
  "getBundle",
  "getGraph",
  "getTasks",
  "getTask",
  "getCheck",
  "getHealth",
  "stageDecision",
  "unstageDecision",
] as const;

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

/** Row 4 or 5 of What is left: the code assuming a decision, the unbound nodes. */
function codePart(row: "assumes" | "unbound"): HTMLElement {
  const found = region("What is left").querySelector<HTMLElement>(`[data-row="${row}"]`);
  if (found === null) {
    throw new Error(`no ${row} part`);
  }
  return found;
}

/** The IDs of a code row's lines, as rendered. */
function codeIds(row: "assumes" | "unbound"): (string | undefined)[] {
  return Array.from(codePart(row).querySelectorAll<HTMLElement>(`[data-${row}]`), (line) => line.dataset[row]);
}

/** Row 3 of What is left: the unapplied amendments. */
function amendments(): HTMLElement {
  const found = region("What is left").querySelector<HTMLElement>('[data-row="amendments"]');
  if (found === null) {
    throw new Error("no Unapplied amendments part");
  }
  return found;
}

function amendmentPaths(): (string | undefined)[] {
  return Array.from(amendments().querySelectorAll<HTMLElement>("[data-amendment]"), (row) => row.dataset.amendment);
}

function verdict(): string {
  return region("Check").querySelector(".check-verdict .badge-label")?.textContent ?? "";
}

describe("Health over the mock", () => {
  it("#/harbor-sim/health reads the projects, the check, the inbox and the amendments once each; the home reads no check and no amendments (AC-01; AC-U5 of health-left)", async () => {
    const { spies } = await loadedHealth();
    expect(calls(spies)).toEqual({ getProjects: 1, getCheck: 1, getInbox: 1, getHealth: 1 });
    cleanup();
    const home = renderMock("normal", "#/harbor-sim");
    await screen.findByRole("heading", { level: 1, name: "Overview" });
    await waitFor(() => {
      expect(document.querySelectorAll('.home-region[aria-busy="false"]')).toHaveLength(2);
    });
    expect(calls(home.spies).getCheck).toBeUndefined();
    expect(calls(home.spies).getHealth).toBeUndefined();
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

  it("error: each region in the daemon's words with a Retry, What is left one per part: the queue's and rows 3 to 5's (AC-09; AC-U4 of health-left; health-code-ui)", async () => {
    renderMock("error", "#/harbor-sim/health");
    await screen.findByRole("region", { name: "Debt and budgets" });
    for (const [name, reads] of [
      ["Check", 1],
      ["What is left", 4],
      ["Findings", 1],
      ["Debt and budgets", 1],
    ] as const) {
      expect(await within(region(name)).findAllByText(/^spec index unavailable/)).toHaveLength(reads);
      expect(within(region(name)).getAllByRole("button", { name: "Retry" })).toHaveLength(reads);
    }
    expect(within(amendments()).getByText(/^spec index unavailable/)).toBeTruthy();
    expect(amendments().querySelector(".health-value")).toBeNull();
  });

  it("slow: every region busy with a skeleton until its read answers (AC-09)", async () => {
    renderMock("slow", "#/harbor-sim/health", 50);
    await screen.findByRole("region", { name: "Debt and budgets" });
    for (const name of ["Check", "What is left", "Findings", "Debt and budgets"]) {
      expect(region(name).getAttribute("aria-busy")).toBe("true");
      // What is left: one skeleton per part, the queue's and rows 3 to 5's (AC-U4 of health-left; health-code-ui).
      const skeletons = within(region(name)).getAllByRole("status");
      expect(skeletons).toHaveLength(name === "What is left" ? 4 : 1);
      expect(skeletons.map((skeleton) => skeleton.getAttribute("aria-busy"))).toEqual(skeletons.map(() => "true"));
    }
    await waitFor(() => {
      expect(document.querySelectorAll('.health-region[aria-busy="false"]')).toHaveLength(4);
    });
  });

  it("normal: harbor-sim's two in path order, the decision's dangling link as written, the draft's resolved one linked (AC-U3, AC-U6 of health-left)", async () => {
    await loadedHealth();
    expect(amendments().querySelector(".health-left-total")?.textContent).toBe("2 unapplied amendments over 41 documents");
    expect(amendmentPaths()).toEqual(["docs/decisions/DEC-0014.md", "docs/features/deep-berth.md"]);
    const [decision, draft] = Array.from(amendments().querySelectorAll<HTMLElement>("[data-amendment]"));
    expect(decision?.querySelector(".health-amendment-title")?.textContent).toBe("Quay lights stay amber");
    expect(Array.from(decision?.querySelectorAll(".health-amendment-facts dd") ?? [], (fact) => fact.textContent)).toEqual(["DEC-0014", "decision", "accepted"]);
    expect(decision?.querySelector(".health-amendment-link a")).toBeNull();
    expect(decision?.querySelector(".health-amendment-link .link-name")?.textContent).toBe("RULE-QUAY-LIGHTS");
    expect(decision?.querySelector(".health-amendment-link .badge-label")?.textContent).toBe("Dangling");
    expect(decision?.querySelector(".health-amendment-link .link-row-meta")?.textContent).toBe("docs/decisions/DEC-0014.md:8");
    expect(Array.from(draft?.querySelectorAll(".health-amendment-facts dd") ?? [], (fact) => fact.textContent)).toEqual(["spec", "draft"]);
    expect(draft?.querySelector(".health-amendment-link a")?.getAttribute("href")).toBe("#/harbor-sim/tree/RULE-BERTH-DRAFT");
    expect(draft?.querySelector(".health-amendment-link .badge-label")?.textContent).toBe("Resolved");
  });

  it.each([
    ["docs/decisions/DEC-0014.md", "Quay lights stay amber"],
    ["docs/features/deep-berth.md", "Berth 4 takes 13.0 m draft after the dredging"],
  ])("normal: %s's path opens its document in the tree, a node of the mock's corpus", async (path, title) => {
    const { spies } = await loadedHealth();
    const link = amendments().querySelector<HTMLAnchorElement>(`[data-amendment="${path}"] .health-amendment-path`);
    const href = link?.getAttribute("href") ?? "";
    expect(href).toBe(`#/harbor-sim/tree/${encodeURIComponent(path)}`);
    fireEvent.click(link as HTMLAnchorElement);
    // jsdom follows a clicked hash link only a task later; the browser at once.
    window.location.hash = href;
    await waitFor(() => {
      expect(screen.getByRole("heading", { level: 1 }).textContent).toContain(title);
    });
    expect(spies.getNode).toHaveBeenCalled();
    expect(screen.queryByText(/resolves to no ID and no alias/)).toBeNull();
  });

  it.each([
    ["normal", "ledger-api", "Measured over 12 documents."],
    ["empty", "harbor-sim", "Measured over 4 documents."],
  ] as const)("%s, %s: none, said as the rule (AC-U4 of health-left)", async (scenario, slug, scope) => {
    await loadedHealth(scenario, slug);
    expect(amendments().querySelector(".health-amendments-none")?.textContent).toBe("None: no live document of this tree declares an amends link.");
    expect(amendments().querySelector(".health-amendments-scope")?.textContent).toBe(scope);
  });

  it("large: 3 000 by path, the first ten rendered until Show all 3000 (AC-U3 of health-left)", async () => {
    await loadedHealth("large");
    expect(amendments().querySelector(".health-left-total")?.textContent).toBe("3000 unapplied amendments over 3149 documents");
    expect(amendmentPaths()).toEqual(Array.from({ length: 10 }, (_, index) => `docs/features/gen/amend-${String(index + 1).padStart(4, "0")}.md`));
    expect(within(amendments()).getByRole("button", { name: "Show all 3000" })).toBeTruthy();
  });

  // Rendering 3 000 rows takes jsdom about a second alone; the wider limit keeps a loaded run green.
  it("large: Show all 3000 renders every one, in the daemon's order", { timeout: 20_000 }, async () => {
    await loadedHealth("large");
    fireEvent.click(within(amendments()).getByRole("button", { name: "Show all 3000" }));
    const paths = amendmentPaths();
    expect(paths).toHaveLength(3000);
    expect(paths.at(-1)).toBe("docs/features/gen/amend-3000.md");
  });

  it("cannot-check: the walk-gap 503 verbatim with a Retry, no digit in a value; the queue still listed (AC-U4, AC-U6 of health-left)", async () => {
    await loadedHealth("cannot-check");
    const alert = within(amendments()).getByRole("alert");
    expect(alert.textContent).toBe(
      "The unapplied amendments could not be readspec: the walk is incomplete: `docs/spec/cranes.md` cannot be read (Permission denied (os error 13)): unapplied amendments not measured",
    );
    expect(within(amendments()).getByRole("button", { name: "Retry" })).toBeTruthy();
    expect(amendments().querySelector(".health-value")).toBeNull();
    expect(region("What is left").querySelectorAll(".health-left-link").length).toBeGreaterThan(0);
  });

  // docs/features/health-code-ui.md, rows 4 and 5 over the mock's scenarios (UI-1, UI-2, UI-4, UI-6).

  it("normal: harbor-sim's row 4 in the daemon's order, one line per state, by ID; its row 5 by path, never sorted here (UI-2)", async () => {
    await loadedHealth();
    expect(codePart("assumes").querySelector(".health-left-total")?.textContent).toBe("8 rows of 11 markers");
    expect(codeIds("assumes")).toEqual(["MEC-OLD-QUAYS", "PR-0012", "PR-0019", "PR-0023", "PR-0027", "PR-0041", "PR-7", "RULE-BERTH-DRAFT"]);
    expect(Array.from(codePart("assumes").querySelectorAll(".badge-label"), (label) => label.textContent)).toEqual([
      "Archived",
      "Applied",
      "Decided as assumed",
      "Decided otherwise",
      "Rejected",
      "Pending",
      "No such item",
      "In question",
    ]);
    expect(codePart("unbound").querySelector(".health-left-total")?.textContent).toBe("5 of 14 selected nodes");
    expect(codeIds("unbound")).toEqual(["RULE-BASIN-NAME", "RULE-MOOR-CREW", "RULE-MOOR-NIGHT", "MEC-FAIRWAY", "RULE-PILOT-REQ"]);
    expect(Array.from(codePart("unbound").querySelectorAll(".health-code-marker .badge-label"), (label) => label.textContent)).toEqual([
      "Look-alike ID",
      "Unanchored",
      "Cannot verify",
      "Another relation",
    ]);
  });

  it("normal: the stored texts as the CLI prints them: the rejection's line feed a space, its override \\u{202e}; the pasted title as text, the basin's name in its script", async () => {
    await loadedHealth();
    const rejected = codePart("assumes").querySelector('[data-assumes="PR-0027"] .health-code-tail');
    expect(rejected?.textContent).toBe("discrepancy rejected: Not a discrepancy: the pilot rule was right;\\u{202e} keep 15 minutes");
    const night = codePart("unbound").querySelector('[data-unbound="RULE-MOOR-NIGHT"]');
    expect(night?.querySelector(".health-code-title")?.textContent).toBe(HOSTILE_TITLE);
    expect(night?.querySelector("img")).toBeNull();
    expect(codePart("unbound").querySelector('[data-unbound="RULE-BASIN-NAME"] .health-code-title')?.textContent).toBe(BASIN_NAME_DECOMPOSED);
  });

  it("normal: an item's ID opens its inbox card (UI-6)", async () => {
    await loadedHealth();
    const link = codePart("assumes").querySelector<HTMLAnchorElement>('[data-assumes="PR-0041"] a.health-code-id');
    const href = link?.getAttribute("href") ?? "";
    expect(href).toBe("#/harbor-sim/inbox/PR-0041");
    fireEvent.click(link as HTMLAnchorElement);
    window.location.hash = href;
    const card = await screen.findByRole("article");
    expect(await within(card).findByText("Entry only inside the tide window")).toBeTruthy();
  });

  it.each([
    ["assumes", "RULE-BERTH-DRAFT", "Draft limit at a berth"],
    ["unbound", "RULE-MOOR-CREW", "Mooring crew"],
  ] as const)("normal: %s's %s opens its node in the tree (UI-6)", async (row, id, title) => {
    const { spies } = await loadedHealth();
    const link = codePart(row).querySelector<HTMLAnchorElement>(`[data-${row}="${id}"] a.health-code-id`);
    const href = link?.getAttribute("href") ?? "";
    expect(href).toBe(`#/harbor-sim/tree/${id}`);
    fireEvent.click(link as HTMLAnchorElement);
    window.location.hash = href;
    await waitFor(() => {
      expect(screen.getByRole("heading", { level: 1 }).textContent).toContain(title);
    });
    expect(spies.getNode).toHaveBeenCalled();
  });

  it("normal, ledger-api: no [code] table, both rows not measured, the cause verbatim, no digit, no Retry", async () => {
    await loadedHealth("normal", "ledger-api");
    for (const row of ["assumes", "unbound"] as const) {
      expect(codePart(row).querySelector(".health-code-off")?.textContent).toBe("Not measured: Code layer off");
      expect(codePart(row).querySelector(".health-code-causes")?.textContent).toBe("specengine.toml has no [code] table: the code layer is off (add [code] with roots)");
      expect(within(codePart(row)).queryByRole("button", { name: "Retry" })).toBeNull();
      for (const value of Array.from(codePart(row).querySelectorAll(".health-value"), (cell) => cell.textContent)) {
        expect(value).not.toMatch(/\d/);
      }
    }
  });

  it.each(["harbor-sim", "ledger-api"])("empty, %s: both rows None, over what they measured", async (slug) => {
    await loadedHealth("empty", slug);
    for (const row of ["assumes", "unbound"] as const) {
      expect(codePart(row).querySelector(".health-code-none .health-value")?.textContent).toBe("None");
    }
    expect(codePart("assumes").querySelector(".health-amendments-scope")?.textContent).toMatch(/^Measured over \d+ markers in \d+ code files\.$/);
    expect(codePart("unbound").querySelector(".health-amendments-scope")?.textContent).toMatch(/^Measured over \d+ selected nodes\.$/);
  });

  it("large: row 5's 3 000 by path, the first ten rendered until Show all 3000", async () => {
    await loadedHealth("large");
    expect(codePart("unbound").querySelector(".health-left-total")?.textContent).toBe("3000 of 3122 selected nodes");
    expect(codeIds("unbound")).toHaveLength(10);
    expect(codeIds("unbound").slice(0, 4)).toEqual(["RULE-BASIN-NAME", "RULE-MOOR-CREW", "RULE-MOOR-NIGHT", "MEC-FAIRWAY"]);
    expect(within(codePart("unbound")).getByRole("button", { name: "Show all 3000" })).toBeTruthy();
  });

  it("code-cannot: rows 4-5 could not be measured, the causes as the CLI prints them, a Retry, no value; row 3 still measured", async () => {
    const { spies } = await loadedHealth("code-cannot");
    expect(Array.from(codePart("assumes").querySelectorAll(".health-code-causes li"), (item) => item.textContent)).toEqual([
      "the code walk is incomplete: `src/sim/tide\\u{1b}[2J.rs` cannot be read (Permission denied (os error 13))",
    ]);
    expect(Array.from(codePart("unbound").querySelectorAll(".health-code-causes li"), (item) => item.textContent)).toHaveLength(2);
    for (const row of ["assumes", "unbound"] as const) {
      expect(codePart(row).querySelector(".health-code-cannot-title")?.textContent).toBe("Could not be measured");
      expect(codePart(row).querySelector(".health-value")).toBeNull();
      expect(within(codePart(row)).getByRole("button", { name: "Retry" })).toBeTruthy();
    }
    expect(amendments().querySelector(".health-left-total")?.textContent).toBe("2 unapplied amendments over 41 documents");
    fireEvent.click(within(codePart("unbound")).getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(spies.getHealth).toHaveBeenCalledTimes(2);
    });
    expect(spies.getCheck).toHaveBeenCalledTimes(1);
  });

  it("not-configured: row 5 without a rule, not measured, its sentence verbatim; row 4 measured", async () => {
    await loadedHealth("not-configured");
    expect(codePart("unbound").querySelector(".health-code-off")?.textContent).toBe("Not measured: Not configured");
    expect(codePart("unbound").querySelector(".health-code-causes")?.textContent).toBe("no [[health.bound]] rule: which nodes must be bound is the project's to declare");
    expect(codePart("assumes").querySelector(".health-value")?.textContent).toBe("8");
  });

  it("cannot-check: rows 4-5 give the walk-gap 503 verbatim, shown, not spoken, each with a Retry", async () => {
    await loadedHealth("cannot-check");
    for (const row of ["assumes", "unbound"] as const) {
      expect(within(codePart(row)).getByText(/^spec: the walk is incomplete: /)).toBeTruthy();
      expect(within(codePart(row)).queryByRole("alert")).toBeNull();
      expect(within(codePart(row)).getByRole("button", { name: "Retry" })).toBeTruthy();
      expect(codePart(row).querySelector(".health-value")).toBeNull();
    }
  });
});
