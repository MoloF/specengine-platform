import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ClientError } from "../api/client";
import { KNOWN_CHECK_VERDICTS, type CheckReport, type HealthReport, type Proposal } from "../api/types";
import { App } from "../app/App";
import { sectionHash } from "../app/routes";
import { queueOrder } from "../inbox/order";
import { aCheckFinding, aCheckReport, aHealthReport, aLink, anAmendment, aProposal, entryOf } from "../test/builders";
import { callsOf } from "../test/homeStub";
import { renderApp } from "../test/render";
import { argsOf, stubClient } from "../test/stubClient";

// docs/features/ui-health.md on a stub client: AC-01 (what the route reads), AC-02 (absent keys
// show nothing), AC-03 (verdicts), AC-04 (nothing measured is never 0), AC-05 (the queue's first
// five), AC-06 (groups, text verbatim, hostile text), AC-07 (links), AC-08 (debt, stale,
// budgets), AC-09 (reads and states per region), AC-10 (keyboard). docs/features/health-left.md:
// AC-U3 and AC-U4 (row 3, the unapplied amendments, its three states), AC-U5 (what entering and
// Check again read), AC-U7 (rows 4 to 6; kind-amendment's AC-U2 tests replaced).

const LF = String.fromCharCode(10);

/** Markup an author pasted; shown as text (assembled so no source line spells a dialog call). */
const HOSTILE = "<img src=x onerror=" + ["al", "ert(1)>"].join("");

/** The third known verdict: a check outcome, labelled "Fails the check". */
const FAILS = KNOWN_CHECK_VERDICTS[2];

const QUEUE: Proposal[] = [
  aProposal({ id: "PR-1", severity: "low", summary: "Low and old", created_at: "2026-10-01T08:00:00Z" }),
  aProposal({ id: "PR-2", severity: "high", summary: "High", created_at: "2026-10-03T09:30:00Z" }),
  aProposal({ id: "PR-3", severity: "normal", summary: "Normal", created_at: "2026-10-02T10:00:00Z" }),
  aProposal({ id: "PR-4", severity: null, summary: "No severity", created_at: "2026-09-30T10:00:00Z" }),
  aProposal({ id: "PR-5", severity: "urgent", summary: "Unknown severity", created_at: "2026-09-29T10:00:00Z" }),
  aProposal({ id: "PR-6", severity: "high", summary: "High and older", created_at: "2026-10-02T09:30:00Z" }),
  aProposal({ id: "PR-7", severity: "normal", summary: "Normal and newer", created_at: "2026-10-04T10:00:00Z" }),
];

/** A report holding one of each case: a budget, an error with debt, a warning, a fix, an unlinked path. */
const REPORT: CheckReport = aCheckReport({
  mode: "observe",
  verdict: "observed",
  counts: { documents: 41, errors: 2, warnings: 1, debt: 1, expired: 0, stale: 1, worst_w_bytes: 61234 },
  findings: [
    aCheckFinding({ code: "budget", path: "docs/canon/tides.md", line: 1, subject: "canon", message: "12950 bytes, over the canon cap of 12288: move detail down a tier; caps are never raised" }),
    aCheckFinding({ code: "ref-dangling", path: "docs/spec/berths/mooring.md", line: 12, subject: "RULE-TIDE-GATE", message: "dangling" }),
    aCheckFinding({
      code: "id-width",
      severity: "warning",
      path: "docs/spec/cranes.md",
      line: 3,
      subject: "CR-7",
      message: "too narrow",
      debt: { reason: "legacy import", expires: "2026-12-31", expired: false },
    }),
    aCheckFinding({ code: "generator-path", severity: "warning", path: "specengine.toml", line: 22, subject: "docs/index.md", message: "outside the roots" }),
    aCheckFinding({ code: "homoglyph", path: "docs/spec/api.md", line: 7, subject: "POL-X", message: "mixed", fix: { span: { start: 1, end: 6 }, text: "POL-X" } }),
  ],
  stale: [{ code: "file-name", path: "docs/spec/harbor.md", subject: "", reason: "legacy import", expires: "2026-12-31", line: 7 }],
});

function healthClient(report: CheckReport = REPORT, proposals: Proposal[] = QUEUE) {
  const client = stubClient(proposals);
  client.getCheck.mockImplementation(() => Promise.resolve(structuredClone(report)));
  return client;
}

function region(name: string): HTMLElement {
  return screen.getByRole("region", { name });
}

/** The four regions render together once their chunk has loaded (HealthView loads it lazily). */
async function regionsShown() {
  await screen.findByRole("heading", { level: 1, name: "Health" });
  await screen.findByRole("region", { name: "Debt and budgets" });
}

function view(): HTMLElement {
  const found = document.querySelector<HTMLElement>(".health-view");
  if (found === null) {
    throw new Error("no Health view");
  }
  return found;
}

async function openHealth(client = healthClient(), hash = "#/alpha/health") {
  renderApp(client, hash);
  await screen.findByRole("heading", { level: 1, name: "Health" });
  await waitFor(() => {
    expect(document.querySelectorAll('.health-region[aria-busy="false"]')).toHaveLength(4);
  });
  return client;
}

function rows(): HTMLElement[] {
  return Array.from(region("Findings").querySelectorAll<HTMLElement>("[data-finding]"));
}

function row(path: string): HTMLElement {
  const found = rows().find((item) => item.querySelector(".finding-place")?.textContent.startsWith(`${path}:`) === true);
  if (found === undefined) {
    throw new Error(`no row for ${path}`);
  }
  return found;
}

async function focused(element: HTMLElement) {
  await waitFor(() => {
    expect(document.activeElement).toBe(element);
  });
}

/** Runs the tasks jsdom queued: a hashchange, a late answer. */
async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

describe("what the route reads (AC-01; its counting stub adds getHealth, AC-U5 and AC-U7 of health-left)", () => {
  it("#/alpha/health: one getCheck, one getInbox and one getHealth, nothing else but the shell's projects", async () => {
    const client = await openHealth();
    await settle();
    expect(callsOf(client)).toEqual({ getProjects: 1, getCheck: 1, getInbox: 1, getHealth: 1 });
    expect(client.getCheck).toHaveBeenCalledWith("alpha");
    expect(argsOf(client.getInbox)).toContainEqual(["alpha"]);
    // A walk, as the check: no AbortSignal is handed over.
    expect(client.getHealth.mock.calls).toEqual([["alpha"]]);
  });

  it("#/alpha, the home: no getCheck, no getHealth", async () => {
    const client = healthClient();
    renderApp(client, "#/alpha");
    await screen.findByRole("heading", { level: 1, name: "Overview" });
    await waitFor(() => {
      expect(document.querySelectorAll('.home-region[aria-busy="false"]')).toHaveLength(2);
    });
    await settle();
    expect(client.getCheck).not.toHaveBeenCalled();
    expect(client.getHealth).not.toHaveBeenCalled();
  });

  it("an HttpClient-like refusal (not served) says Not built yet, no Retry, no alert", async () => {
    const client = healthClient();
    client.getCheck.mockRejectedValue(new ClientError({ status: 501, message: "Not served by the daemon yet: GET /api/projects/alpha/check" }, { notServed: true }));
    renderApp(client, "#/alpha/health");
    await screen.findAllByText("Not built yet: the daemon has no endpoint for this read");
    expect(within(region("Check")).queryByRole("button", { name: /^Retry/ })).toBeNull();
    expect(screen.queryAllByRole("alert").filter((alert) => alert.textContent.includes("Not served"))).toEqual([]);
  });
});

describe("absent keys and the verdict (AC-02, AC-03)", () => {
  it("shows no debt and no fix for a finding without them", async () => {
    await openHealth(healthClient(aCheckReport({ verdict: "observed", findings: [aCheckFinding({ code: "ref-dangling", path: "docs/a.md" })] })));
    const only = rows()[0];
    expect(only?.querySelector(".finding-debt")).toBeNull();
    expect(only?.querySelector(".finding-fix")).toBeNull();
    expect(only?.textContent).not.toMatch(/undefined|null|debt|Fix/i);
  });

  it("shows a debt and a fix where they are", async () => {
    await openHealth();
    expect(row("docs/spec/cranes.md").querySelector(".finding-debt")?.textContent).toBe("In debt until 2026-12-31: legacy import");
    expect(row("docs/spec/api.md").querySelector(".finding-fix-text")?.textContent).toBe("POL-X");
  });

  it.each([
    ["clean", "Clean"],
    ["observed", "Passes with findings"],
    [FAILS, "Fails the check"],
    ["cannot-check", "Could not check"],
  ])("labels the verdict %s as %s, with an icon, never its raw text", async (verdict, label) => {
    await openHealth(healthClient(aCheckReport({ verdict })));
    const badge = region("Check").querySelector(".check-verdict .badge");
    expect(badge?.querySelector(".badge-label")?.textContent).toBe(label);
    expect(badge?.querySelector("svg")).not.toBeNull();
    expect(view().textContent).not.toMatch(/block/i);
  });

  it("draws cannot-check in cannot-verify's role, never clean's", async () => {
    await openHealth(healthClient(aCheckReport({ verdict: "cannot-check", cannot_check: [{ path: "docs", message: "x" }] })));
    const tone = region("Check").querySelector(".check-verdict .badge")?.getAttribute("data-tone");
    expect(tone).toBe("check-cannot-check");
    expect(tone).not.toBe("check-clean");
  });

  it("shows another verdict and mode raw in a neutral badge", async () => {
    await openHealth(healthClient(aCheckReport({ verdict: "triage", mode: "strict" })));
    const badges = Array.from(region("Check").querySelectorAll(".badge"), (badge) => [badge.querySelector(".badge-label")?.textContent, badge.getAttribute("data-tone")]);
    expect(badges).toEqual([
      ["triage", "check-unknown"],
      ["strict", "check-unknown"],
    ]);
  });
});

describe("nothing measured is never 0 (AC-04)", () => {
  it("cannot-check: W not measured, no 0 B, the causes verbatim", async () => {
    const report = aCheckReport({
      verdict: "cannot-check",
      counts: { worst_w_bytes: 0, documents: 0 },
      cannot_check: [
        { path: "", message: "today `x` is not a YYYY-MM-DD date" },
        { path: "docs/spec/locks", message: "directory cannot be listed; its files are unchecked" },
      ],
    });
    await openHealth(healthClient(report));
    const check = region("Check");
    expect(check.textContent).not.toContain("0 B");
    expect(check.querySelector(".check-worst-w")?.textContent).toBe("Not measured");
    // As the CLI prints a cause: `.` for no path (core check/report.rs `shown`).
    expect(Array.from(check.querySelectorAll(".check-causes li"), (item) => item.textContent)).toEqual([
      ".: today `x` is not a YYYY-MM-DD date",
      "docs/spec/locks: directory cannot be listed; its files are unchecked",
    ]);
  });

  it("shows W in bytes as the report gives it, and only the counts the report holds", async () => {
    await openHealth();
    const check = region("Check");
    expect(check.querySelector(".check-worst-w")?.textContent).toBe("61234 B");
    expect(Array.from(check.querySelectorAll(".check-count"), (item) => item.getAttribute("data-count"))).toEqual([
      "documents",
      "errors",
      "warnings",
      "debt",
      "expired",
      "stale",
    ]);
    expect(check.textContent).not.toMatch(/Introduced|New debt/);
  });

  it("shows introduced and new debt only when present", async () => {
    await openHealth(healthClient(aCheckReport({ counts: { introduced: 2, new_debt: 0 } })));
    expect(Array.from(region("Check").querySelectorAll(".check-count"), (item) => item.textContent)).toContain("Introduced2");
    expect(Array.from(region("Check").querySelectorAll(".check-count"), (item) => item.textContent)).toContain("New debt0");
  });

  it("holds no digit in the value of rows 4 to 6 and of task W (narrowed from rows 3 to 6 by AC-U7 of health-left)", async () => {
    await openHealth();
    const values = Array.from(region("What is left").querySelectorAll(".health-unmeasured .health-value"), (cell) => cell.textContent);
    expect(values).toEqual(["Not measured yet", "Not measured yet", "Not measured yet"]);
    expect(Array.from(region("What is left").querySelectorAll(".health-unmeasured .health-source"), (cell) => cell.textContent)).toEqual([
      "Phase 3",
      "Phase 3",
      "Phase 3",
    ]);
    expect(region("What is left").querySelector('.health-unmeasured [data-row="amendments"]')).toBeNull();
    expect(region("Check").querySelector(".check-task-w")?.textContent).toBe("Not measured yet (the bundles log)");
    for (const text of [...values, region("Check").querySelector(".check-task-w")?.textContent ?? "0"]) {
      expect(text).not.toMatch(/\d/);
    }
  });
});

describe("what is left (AC-05)", () => {
  it("heads the queue as the queue, whatever its proposals' states", async () => {
    await openHealth();
    expect(Array.from(region("What is left").querySelectorAll("h3"), (heading) => heading.textContent)).toEqual([
      "Proposals in the queue",
      "Unapplied amendments",
      "Not measured yet",
    ]);
  });

  it("lists the queue's first five in the Inbox's order, linked, times as stored; severity counts sum to the queue", async () => {
    await openHealth();
    const left = region("What is left");
    const expected = queueOrder(QUEUE.map((proposal) => entryOf(proposal))).slice(0, 5);
    const links = Array.from(left.querySelectorAll<HTMLAnchorElement>(".health-left-link"));
    expect(links.map((link) => link.getAttribute("href"))).toEqual(expected.map((entry) => sectionHash("alpha", "inbox", entry.id)));
    expect(Array.from(left.querySelectorAll(".health-left-row time"), (time) => time.textContent)).toEqual(expected.map((entry) => entry.created_at));
    const counts = Array.from(left.querySelectorAll(".health-tally-count"), (count) => Number(count.textContent));
    expect(counts.reduce((total, count) => total + count, 0)).toBe(QUEUE.length);
    expect(left.querySelector(".health-left-total")?.textContent).toBe("7 proposals in the queue");
  });

  it("says when the queue is clear", async () => {
    await openHealth(healthClient(REPORT, []));
    expect(within(region("What is left")).getByText("No proposal waits for your decision.")).toBeTruthy();
  });
});

describe("unapplied amendments, row 3 (AC-U3, AC-U4 of health-left; replacing AC-U2 of kind-amendment)", () => {
  /** Two open amendments in the queue, as the daemon lists them: proposals, row 1's, never row 3's. */
  const QUEUED = ["docs/features/surge.md", "docs/features/swell.md"].map((path, at) =>
    aProposal({
      id: `PR-${String(8 + at)}`,
      kind: "amendment",
      severity: null,
      summary: null,
      rationale: `Amends the high-water rule: ${path}`,
      target_id: path,
      target_path: path,
      target_ids: [path],
      new_text: ["---", "class: spec", "links:", "  amends: [RULE-HIGH-WATER]", "---", ""].join(LF),
      created_at: `2026-10-0${String(5 + at)}T10:00:00Z`,
    }),
  );

  /** Two live amendments as the daemon sends them; listed out of byte order, so a sort here would show. */
  const LISTED = [
    anAmendment({
      path: "docs/spec/zz-last.md",
      id: "DEC-0014",
      title: "Quay lights stay amber",
      class: "decision",
      status: "accepted",
      links: [
        aLink({
          type: "amends",
          written: "RULE-QUAY-LIGHTS",
          name: null,
          state: "dangling",
          path: "docs/spec/zz-last.md",
          line: 8,
          reason: "`RULE-QUAY-LIGHTS` resolves to no ID and no alias",
        }),
      ],
    }),
    anAmendment({
      path: "docs/features/aa-first.md",
      id: null,
      title: null,
      class: null,
      status: "in-flight",
      links: [
        aLink({ type: "amends", written: "rule-alias", name: "R-1", path: "docs/features/aa-first.md", line: 5 }),
        aLink({ type: "amends", written: "other:R-9", name: null, state: "parked", path: "docs/features/aa-first.md", line: 6, reason: `  kept${LF}  verbatim  ` }),
      ],
    }),
  ];

  function healthOf(report: HealthReport, proposals: Proposal[] = QUEUE) {
    const client = healthClient(REPORT, proposals);
    client.getHealth.mockImplementation(() => Promise.resolve(structuredClone(report)));
    return client;
  }

  /** Row 3's part of What is left. */
  function part(): HTMLElement {
    const found = region("What is left").querySelector<HTMLElement>('[data-row="amendments"]');
    if (found === null) {
      throw new Error("no Unapplied amendments part");
    }
    return found;
  }

  function amendmentRows(): HTMLElement[] {
    return Array.from(part().querySelectorAll<HTMLElement>("[data-amendment]"));
  }

  function only<T>(items: readonly T[], at: number): T {
    const item = items[at];
    if (item === undefined) {
      throw new Error(`no item ${String(at)}`);
    }
    return item;
  }

  it("measured: the row count in .health-value, over the documents; rows in the daemon's order, never sorted here", async () => {
    await openHealth(healthOf(aHealthReport(LISTED, { documents: 41 })));
    expect(part().querySelector(".health-value")?.textContent).toBe("2");
    expect(part().querySelector(".health-value")?.classList.contains("health-measured")).toBe(true);
    expect(part().querySelector(".health-left-total")?.textContent).toBe("2 unapplied amendments over 41 documents");
    expect(amendmentRows().map((row) => row.dataset.amendment)).toEqual(["docs/spec/zz-last.md", "docs/features/aa-first.md"]);
  });

  it("links each path to its node in the tree; title, ID, class and status raw, an absent one not shown", async () => {
    await openHealth(healthOf(aHealthReport(LISTED)));
    const [decision, draft] = [only(amendmentRows(), 0), only(amendmentRows(), 1)];
    const path = decision.querySelector(".health-amendment-path");
    expect([path?.textContent, path?.getAttribute("href")]).toEqual(["docs/spec/zz-last.md", sectionHash("alpha", "tree", "docs/spec/zz-last.md")]);
    expect(decision.querySelector(".health-amendment-title")?.textContent).toBe("Quay lights stay amber");
    const facts = (row: HTMLElement) => Array.from(row.querySelectorAll(".health-amendment-facts .fact"), (fact) => [fact.querySelector("dt")?.textContent, fact.querySelector("dd")?.textContent]);
    expect(facts(decision)).toEqual([
      ["ID", "DEC-0014"],
      ["Class", "decision"],
      ["Status", "accepted"],
    ]);
    expect(draft.querySelector(".health-amendment-path")?.getAttribute("href")).toBe(sectionHash("alpha", "tree", "docs/features/aa-first.md"));
    expect(draft.querySelector(".health-amendment-title")).toBeNull();
    expect(facts(draft)).toEqual([["Status", "in-flight"]]);
    expect(part().textContent).not.toMatch(/null|undefined/);
  });

  it("per link: a resolved name linked in the tree, else the written form as text; the state's label and icon; the reason verbatim", async () => {
    await openHealth(healthOf(aHealthReport(LISTED)));
    const links = (row: HTMLElement) => Array.from(row.querySelectorAll<HTMLElement>(".health-amendment-link"));
    const [dangling] = links(only(amendmentRows(), 0));
    expect(dangling?.querySelector("a")).toBeNull();
    expect(dangling?.querySelector(".kind-tag")?.textContent).toBe("amends");
    expect(dangling?.querySelector(".link-name")?.textContent).toBe("RULE-QUAY-LIGHTS");
    expect(dangling?.querySelector(".badge")?.getAttribute("data-tone")).toBe("link-dangling");
    expect(dangling?.querySelector(".badge-label")?.textContent).toBe("Dangling");
    expect(dangling?.querySelector(".badge svg")).not.toBeNull();
    expect(dangling?.querySelector(".link-row-meta")?.textContent).toBe("docs/spec/zz-last.md:8");
    expect(dangling?.querySelector(".link-reason")?.textContent).toBe("`RULE-QUAY-LIGHTS` resolves to no ID and no alias");
    const [resolved, unknown] = links(only(amendmentRows(), 1));
    const name = resolved?.querySelector("a.link-name");
    expect([name?.textContent, name?.getAttribute("href")]).toEqual(["R-1", sectionHash("alpha", "tree", "R-1")]);
    expect(resolved?.querySelector(".badge-label")?.textContent).toBe("Resolved");
    expect(resolved?.querySelector(".link-row-meta")?.textContent).toBe("docs/features/aa-first.md:5as rule-alias");
    expect(resolved?.querySelector(".link-reason")).toBeNull();
    expect(unknown?.querySelector("a")).toBeNull();
    expect(unknown?.querySelector(".link-name")?.textContent).toBe("other:R-9");
    expect([unknown?.querySelector(".badge")?.getAttribute("data-tone"), unknown?.querySelector(".badge-label")?.textContent]).toEqual(["link-unknown", "parked"]);
    expect(unknown?.querySelector(".link-reason")?.textContent).toBe(`  kept${LF}  verbatim  `);
  });

  it("lists the daemon's notes on the read verbatim", async () => {
    const note = "docs/spec/locks/lock-c.md:4: frontmatter-yaml: mapping values are not allowed here; not judged";
    await openHealth(healthOf(aHealthReport([], { notes: [note] })));
    expect(within(part()).getByRole("list", { name: "Notes from the daemon on the unapplied amendments" }).textContent).toBe(note);
  });

  it("zero: its own text, naming the rule, over the documents; never nothing left, never a 0", async () => {
    await openHealth(healthOf(aHealthReport([], { documents: 41 })));
    expect(part().querySelector(".health-amendments-none")?.textContent).toBe("None: no live document of this tree declares an amends link.");
    expect(part().querySelector(".health-amendments-none code")?.textContent).toBe("amends");
    expect(part().querySelector(".health-amendments-scope")?.textContent).toBe("Measured over 41 documents.");
    expect(part().querySelector(".health-value")?.textContent).toBe("None");
    // Measured, so drawn as a measured value, never as rows 4 to 6's "Not measured yet".
    expect(part().querySelector(".health-value")?.classList.contains("health-measured")).toBe(true);
    expect(Array.from(region("What is left").querySelectorAll(".health-unmeasured .health-value"), (cell) => cell.classList.contains("health-measured"))).toEqual([
      false,
      false,
      false,
    ]);
    expect(part().textContent).not.toMatch(/nothing left|\b0\b/i);
    expect(amendmentRows()).toHaveLength(0);
  });

  it("counts from the health read alone: amendments waiting in the queue are proposals, row 1's, never row 3's", async () => {
    await openHealth(healthOf(aHealthReport([], { documents: 1 }), [...QUEUE, ...QUEUED]));
    expect(region("What is left").querySelector(".health-left-total")?.textContent).toBe("9 proposals in the queue");
    expect(part().querySelector(".health-amendments-none")).not.toBeNull();
    expect(part().querySelector(".health-amendments-scope")?.textContent).toBe("Measured over 1 document.");
    cleanup();
    await openHealth(healthOf(aHealthReport(LISTED.slice(0, 1), { documents: 1 }), [...QUEUE, ...QUEUED]));
    expect(part().querySelector(".health-left-total")?.textContent).toBe("1 unapplied amendment over 1 document");
  });

  it("over ten: the first ten rendered, the rest not until Show all <n>, which hands the focus to the eleventh; nothing read", async () => {
    const many = Array.from({ length: 12 }, (_, index) => anAmendment({ path: `docs/features/a${String(index).padStart(2, "0")}.md` }));
    const client = await openHealth(healthOf(aHealthReport(many)));
    expect(amendmentRows()).toHaveLength(10);
    expect(amendmentRows().map((row) => row.dataset.amendment)).toEqual(many.slice(0, 10).map((amendment) => amendment.path));
    expect(part().querySelector(".health-amendments-scope")?.textContent).toBe("The first 10 of 12, by path.");
    fireEvent.click(within(part()).getByRole("button", { name: "Show all 12" }));
    expect(amendmentRows().map((row) => row.dataset.amendment)).toEqual(many.map((amendment) => amendment.path));
    await focused(only(amendmentRows(), 10).querySelector<HTMLElement>(".health-amendment-path") as HTMLElement);
    expect(within(part()).queryByRole("button", { name: /^Show all/ })).toBeNull();
    await settle();
    expect(client.getHealth).toHaveBeenCalledTimes(1);
  });

  it("ten exactly: all ten, no Show all", async () => {
    const ten = Array.from({ length: 10 }, (_, index) => anAmendment({ path: `docs/features/b${String(index)}.md` }));
    await openHealth(healthOf(aHealthReport(ten)));
    expect(amendmentRows()).toHaveLength(10);
    expect(within(part()).queryByRole("button", { name: /^Show all/ })).toBeNull();
  });

  it("loading: a busy skeleton and no digit, the queue part answered beside it", async () => {
    const client = healthClient();
    client.getHealth.mockImplementation(() => new Promise(() => undefined));
    renderApp(client, "#/alpha/health");
    await regionsShown();
    await waitFor(() => {
      expect(region("What is left").querySelector(".health-left-total")?.textContent).toBe("7 proposals in the queue");
    });
    expect(part().getAttribute("aria-busy")).toBe("true");
    expect(region("What is left").getAttribute("aria-busy")).toBe("true");
    expect(within(part()).getByRole("status").getAttribute("aria-busy")).toBe("true");
    expect(part().querySelector(".health-value")).toBeNull();
    expect(part().textContent).not.toMatch(/\d/);
  });

  it.each([
    ["a 503 (the walk incomplete)", 503, "spec: the walk is incomplete: the directory `docs/spec/locks` cannot be listed: unapplied amendments not measured"],
    ["an older daemon's 404", 404, "no route GET /api/projects/alpha/health: this daemon serves projects, inbox, proposals, tree, nodes, search, bundle, graph, check"],
    ["no response (status 0)", 0, "GET /api/projects/alpha/health: no response from the daemon (Failed to fetch)"],
  ])("%s: the message verbatim, no digit in a value; Retry reads getHealth alone, once; the queue and the other regions stand", async (_name, status, message) => {
    const client = healthClient();
    client.getHealth.mockRejectedValueOnce(new ClientError({ status, message }));
    renderApp(client, "#/alpha/health");
    await regionsShown();
    const alert = await within(part()).findByRole("alert");
    expect(alert.textContent).toBe(`The unapplied amendments could not be read${message}`);
    expect(part().getAttribute("aria-busy")).toBe("false");
    expect(part().querySelector(".health-value")).toBeNull();
    for (const value of Array.from(region("What is left").querySelectorAll(".health-value"), (cell) => cell.textContent)) {
      expect(value).not.toMatch(/\d/);
    }
    await waitFor(() => {
      expect(region("What is left").querySelector(".health-left-total")?.textContent).toBe("7 proposals in the queue");
    });
    expect(region("Check").querySelector(".check-verdict")).not.toBeNull();
    expect(rows().length).toBeGreaterThan(0);
    const retry = within(part()).getByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    await waitFor(() => {
      expect(part().querySelector(".health-amendments-none")).not.toBeNull();
    });
    await focused(within(part()).getByRole("heading", { level: 3, name: "Unapplied amendments" }));
    await settle();
    expect(client.getHealth).toHaveBeenCalledTimes(2);
    expect(client.getInbox).toHaveBeenCalledTimes(1);
    expect(client.getCheck).toHaveBeenCalledTimes(1);
  });

  it("a failing queue leaves the amendments standing, its Retry reading the inbox alone", async () => {
    const client = healthOf(aHealthReport(LISTED));
    client.getInbox.mockRejectedValueOnce(new ClientError({ status: 503, message: "inbox locked" }));
    renderApp(client, "#/alpha/health");
    await regionsShown();
    await within(region("What is left")).findByText("inbox locked");
    await waitFor(() => {
      expect(amendmentRows()).toHaveLength(2);
    });
    fireEvent.click(within(region("What is left")).getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(region("What is left").querySelector(".health-left-total")?.textContent).toBe("7 proposals in the queue");
    });
    expect(client.getHealth).toHaveBeenCalledTimes(1);
  });

  it("a part that fails to render leaves the queue standing: each read its own boundary", async () => {
    const client = healthOf({ documents: 3, amendments: null, notes: [] } as unknown as HealthReport);
    const caught = vi.fn();
    window.history.replaceState(null, "", "/#/alpha/health");
    render(<App client={client} scenario={null} />, { onCaughtError: caught });
    await regionsShown();
    expect(await within(part()).findByText(/The unapplied amendments could not be shown/)).toBeTruthy();
    await waitFor(() => {
      expect(region("What is left").querySelector(".health-left-total")?.textContent).toBe("7 proposals in the queue");
    });
    expect(region("What is left").querySelectorAll(".health-unmeasured-row")).toHaveLength(3);
    expect(caught).toHaveBeenCalled();
  });
});

describe("findings (AC-06)", () => {
  it("groups by code, errors first, the counts summing to the findings", async () => {
    await openHealth();
    const groups = Array.from(region("Findings").querySelectorAll<HTMLElement>(".finding-group"));
    expect(groups.map((group) => group.dataset.code)).toEqual(["budget", "homoglyph", "ref-dangling", "generator-path", "id-width"]);
    const counts = groups.map((group) => Number(group.querySelector(".health-count")?.textContent.replace(/\D+/g, "")));
    expect(counts.reduce((total, count) => total + count, 0)).toBe(REPORT.findings.length);
  });

  it("shows message, subject and fix exactly as sent, blank lines and trailing spaces kept; markup as text", async () => {
    const message = `${LF}${LF}  indented message with trailing spaces   `;
    const subject = `  ${HOSTILE}  `;
    const fix = `POL-X  ${LF}`;
    await openHealth(
      healthClient(
        aCheckReport({
          verdict: "observed",
          findings: [aCheckFinding({ code: "homoglyph", path: "docs/a.md", subject, message: HOSTILE + message, fix: { span: { start: 0, end: 1 }, text: fix } })],
        }),
      ),
    );
    const only = rows()[0];
    expect(only?.querySelector(".finding-message")?.textContent).toBe(HOSTILE + message);
    expect(only?.querySelector(".finding-subject-text")?.textContent).toBe(subject);
    expect(only?.querySelector(".finding-fix-text")?.textContent).toBe(fix);
    expect(view().querySelector("img")).toBeNull();
  });

  it("groups each code as a group, not a landmark", async () => {
    await openHealth();
    expect(region("Findings").querySelectorAll("section")).toHaveLength(0);
    const group = within(region("Findings")).getByRole("group", { name: "ref-dangling, 1 finding" });
    expect(group.dataset.code).toBe("ref-dangling");
  });

  it("gives the focus to the toggle when closing a group removes the focused row (a click focuses no button in Safari)", async () => {
    await openHealth();
    const target = row("docs/spec/berths/mooring.md");
    target.focus();
    const toggle = within(region("Findings")).getByRole("button", { name: "ref-dangling, 1 finding" });
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    await focused(toggle);
    fireEvent.click(toggle);
    expect(document.activeElement).toBe(toggle);
  });

  it("collapses a group over ten rows until opened; chips and the filter read nothing", async () => {
    const many = Array.from({ length: 11 }, (_, index) => aCheckFinding({ code: "ref-dangling", path: `docs/r${String(index).padStart(2, "0")}.md` }));
    const client = await openHealth(healthClient(aCheckReport({ verdict: "observed", findings: [...many, aCheckFinding({ code: "id-width", severity: "warning" })] })));
    const toggle = within(region("Findings")).getByRole("button", { name: /^ref-dangling,/ });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect(toggle.textContent).toBe("ref-dangling, 11 findings");
    expect(rows()).toHaveLength(1);
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(rows()).toHaveLength(12);
    fireEvent.click(within(region("Findings")).getByRole("button", { name: /^Warning/ }));
    expect(rows()).toHaveLength(11);
    fireEvent.change(within(region("Findings")).getByLabelText("Filter by path, subject, message or code"), { target: { value: "R03" } });
    expect(rows()).toHaveLength(1);
    expect(region("Findings").querySelector(".finding-total")?.textContent).toBe("1 of 12 findings shown, by code, errors first");
    await settle();
    expect(client.getCheck).toHaveBeenCalledTimes(1);
  });
});

describe("links (AC-07)", () => {
  it("links an .md path to its node in the tree; specengine.toml, no file and every subject are text", async () => {
    await openHealth(
      healthClient(
        aCheckReport({
          verdict: "observed",
          findings: [
            aCheckFinding({ code: "a", path: "", subject: "S-1" }),
            aCheckFinding({ code: "b", path: "docs/spec/x y.md", line: 4, subject: "docs/spec/other.md" }),
            aCheckFinding({ code: "c", path: "specengine.toml", line: 2, subject: "S-3" }),
          ],
        }),
      ),
    );
    const links = Array.from(region("Findings").querySelectorAll("a"));
    expect(links.map((link) => [link.textContent, link.getAttribute("href")])).toEqual([["docs/spec/x y.md:4", sectionHash("alpha", "tree", "docs/spec/x y.md")]]);
    expect(Array.from(region("Findings").querySelectorAll(".finding-subject-text"), (subject) => subject.closest("a"))).toEqual([null, null, null]);
    expect(rows().map((item) => item.querySelector(".finding-place")?.textContent)).toEqual(["No file", "docs/spec/x y.md:4", "specengine.toml:2"]);
  });

  it("shows a stale entry by its line in .spec-debt.toml, never as path:line", async () => {
    await openHealth();
    const stale = Array.from(region("Debt and budgets").querySelectorAll(".stale-row"), (item) => item.textContent);
    expect(stale).toEqual([".spec-debt.toml line 7: file-name on docs/spec/harbor.md matches nothing"]);
    expect(view().textContent).not.toContain("docs/spec/harbor.md:7");
  });
});

describe("debt and budgets (AC-08)", () => {
  it("on cannot-check, says only that nothing was reported, never that the baseline matches", async () => {
    await openHealth(healthClient(aCheckReport({ verdict: "cannot-check", cannot_check: [{ path: "docs", message: "x" }] })));
    expect(Array.from(region("Debt and budgets").querySelectorAll(".health-part > p"), (item) => item.textContent)).toEqual([
      "No finding in debt was reported, but the check could not read everything.",
      "No stale entry was reported, but the check could not read everything.",
      "No document was reported over its budget, but the check could not read everything.",
    ]);
  });

  it("lists exactly the budget findings, slot and message verbatim", async () => {
    await openHealth(
      healthClient(
        aCheckReport({
          verdict: "observed",
          findings: [
            aCheckFinding({ code: "budget", path: "docs/canon/a.md", subject: "canon", message: "12950 bytes, over the canon cap of 12288" }),
            aCheckFinding({ code: "id-width", message: "99 bytes, over the tier1 cap of 10" }),
            aCheckFinding({ code: "budget", path: "docs/README.md", subject: "tier1", message: "no number here" }),
          ],
        }),
      ),
    );
    const budgets = Array.from(region("Debt and budgets").querySelectorAll(".budget-row"), (item) => [
      item.querySelector(".finding-subject-text")?.textContent,
      item.querySelector(".finding-message")?.textContent,
    ]);
    expect(budgets).toEqual([
      ["canon", "12950 bytes, over the canon cap of 12288"],
      ["tier1", "no number here"],
    ]);
  });

  it("orders the debt expired first, then by the stored expiry; one line per stale entry", async () => {
    const debt = (expires: string, expired: boolean) => ({ reason: `until ${expires}`, expires, expired });
    await openHealth(
      healthClient(
        aCheckReport({
          verdict: "observed",
          findings: [
            aCheckFinding({ code: "a", debt: debt("2027-01-01", false) }),
            aCheckFinding({ code: "b", debt: debt("2026-01-01", true) }),
            aCheckFinding({ code: "c", debt: debt("2026-06-30", false) }),
          ],
          stale: [
            { code: "x", path: "docs/x.md", subject: "", reason: "r", expires: "2026-12-31", line: 3 },
            { code: "y", path: "docs/y.md", subject: "Y-1", reason: "r", expires: "2026-12-31", line: 9 },
          ],
        }),
      ),
    );
    const debtRows = Array.from(region("Debt and budgets").querySelectorAll(".debt-row"), (item) => [
      item.querySelector(".debt-code")?.textContent,
      item.querySelector(".finding-debt")?.textContent,
    ]);
    expect(debtRows).toEqual([
      ["b", "Debt expired 2026-01-01: until 2026-01-01"],
      ["c", "In debt until 2026-06-30: until 2026-06-30"],
      ["a", "In debt until 2027-01-01: until 2027-01-01"],
    ]);
    expect(region("Debt and budgets").querySelectorAll(".stale-row")).toHaveLength(2);
  });
});

describe("reads and states (AC-09)", () => {
  it("Check again reads the check exactly once more; window focus, a chip and the filter read nothing", async () => {
    const client = await openHealth();
    // TanStack Query's focus manager listens for visibilitychange on the window.
    act(() => {
      window.dispatchEvent(new Event("focus"));
      window.dispatchEvent(new Event("visibilitychange"));
      document.dispatchEvent(new Event("visibilitychange"));
    });
    fireEvent.click(within(region("Findings")).getByRole("button", { name: /^Error/ }));
    fireEvent.change(within(region("Findings")).getByLabelText("Filter by path, subject, message or code"), { target: { value: "x" } });
    await settle();
    expect(client.getCheck).toHaveBeenCalledTimes(1);
    const again = within(region("Check")).getByRole("button", { name: "Check again" });
    fireEvent.click(again);
    fireEvent.click(again);
    await waitFor(() => {
      expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe("Checked again: Passes with findings.");
    });
    expect(within(region("Check")).getByRole("button", { name: "Check again" }).getAttribute("aria-disabled")).toBe("false");
    expect(client.getCheck).toHaveBeenCalledTimes(2);
    expect(client.getInbox).toHaveBeenCalledTimes(1);
    // AC-U5 of health-left: Check again reads the unapplied amendments once more too; focus read none.
    expect(client.getHealth).toHaveBeenCalledTimes(2);
  });

  it("slow: a busy skeleton in Check, Findings and Debt and budgets", async () => {
    const client = healthClient();
    client.getCheck.mockImplementation(() => new Promise(() => undefined));
    renderApp(client, "#/alpha/health");
    await regionsShown();
    for (const name of ["Check", "Findings", "Debt and budgets"]) {
      expect(region(name).getAttribute("aria-busy")).toBe("true");
      expect(within(region(name)).getByRole("status").getAttribute("aria-busy")).toBe("true");
    }
    await waitFor(() => {
      expect(region("What is left").getAttribute("aria-busy")).toBe("false");
    });
  });

  it("says one document in the singular", async () => {
    await openHealth(healthClient(aCheckReport({ counts: { documents: 1 } }), []));
    expect(region("Check").querySelector(".health-clean p")?.textContent).toBe("The check is clean: 1 document, no findings, no debt.");
  });

  it("empty: the clean text and a link to the Inbox", async () => {
    await openHealth(healthClient(aCheckReport({ counts: { documents: 4 } }), []));
    const clean = region("Check").querySelector(".health-clean");
    expect(clean?.querySelector("p")?.textContent).toBe("The check is clean: 4 documents, no findings, no debt.");
    expect(clean?.querySelector("a")?.getAttribute("href")).toBe(sectionHash("alpha", "inbox"));
  });

  it("error: the daemon's words in each region of the check, one alert; Retry reads once more, focus then on the heading", async () => {
    const client = healthClient();
    const message = "spec check could not run: the index is locked (code 5)";
    client.getCheck.mockRejectedValueOnce(new ClientError({ status: 503, message }));
    renderApp(client, "#/alpha/health");
    await regionsShown();
    const alert = await within(region("Check")).findByRole("alert");
    expect(alert.textContent).toBe(`The check could not be read${message}`);
    for (const name of ["Findings", "Debt and budgets"]) {
      expect(within(region(name)).getByText(message)).toBeTruthy();
      expect(within(region(name)).queryByRole("alert")).toBeNull();
    }
    const retry = within(region("Findings")).getByRole("button", { name: "Retry" });
    retry.focus();
    fireEvent.click(retry);
    await within(region("Findings")).findByRole("button", { name: /^ref-dangling,/ });
    expect(client.getCheck).toHaveBeenCalledTimes(2);
    await focused(within(region("Findings")).getByRole("heading", { level: 2 }));
  });

  it("a failing inbox leaves Check, Findings and Debt and budgets standing; its Retry reads the inbox alone", async () => {
    const client = healthClient();
    client.getInbox.mockRejectedValueOnce(new ClientError({ status: 503, message: "inbox locked" }));
    renderApp(client, "#/alpha/health");
    await regionsShown();
    expect((await within(region("What is left")).findByRole("alert")).textContent).toBe("The queue could not be loadedinbox locked");
    await waitFor(() => {
      expect(rows().length).toBeGreaterThan(0);
    });
    expect(region("Check").querySelector(".check-verdict")).not.toBeNull();
    expect(region("Debt and budgets").querySelectorAll(".budget-row")).toHaveLength(1);
    fireEvent.click(within(region("What is left")).getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(region("What is left").querySelector(".health-left-total")?.textContent).toBe("7 proposals in the queue");
    });
    expect(client.getInbox).toHaveBeenCalledTimes(2);
    expect(client.getCheck).toHaveBeenCalledTimes(1);
  });

  it("a region that fails to render leaves the others working", async () => {
    const broken = { ...REPORT, stale: null } as unknown as CheckReport;
    const client = healthClient(broken);
    const caught = vi.fn();
    window.history.replaceState(null, "", "/#/alpha/health");
    render(<App client={client} scenario={null} />, { onCaughtError: caught });
    await regionsShown();
    expect(await within(region("Debt and budgets")).findByText(/The debt and budgets could not be shown/)).toBeTruthy();
    expect(rows().length).toBe(REPORT.findings.length);
    expect(region("Check").querySelector(".check-verdict")).not.toBeNull();
  });
});

describe("keyboard (AC-10)", () => {
  it("gives the findings one Tab stop and moves it with the arrows, j and k, Home and End; nothing opens", async () => {
    await openHealth();
    expect(rows().filter((item) => item.tabIndex === 0)).toHaveLength(1);
    const order = rows();
    const before = window.history.length;
    const first = order[0];
    if (first === undefined) {
      throw new Error("no rows");
    }
    first.focus();
    fireEvent.keyDown(first, { key: "ArrowDown" });
    await focused(order[1] as HTMLElement);
    fireEvent.keyDown(order[1] as HTMLElement, { key: "j" });
    await focused(order[2] as HTMLElement);
    fireEvent.keyDown(order[2] as HTMLElement, { key: "k" });
    await focused(order[1] as HTMLElement);
    fireEvent.keyDown(order[1] as HTMLElement, { key: "ArrowUp" });
    await focused(first);
    fireEvent.keyDown(first, { key: "End" });
    await focused(order[order.length - 1] as HTMLElement);
    fireEvent.keyDown(order[order.length - 1] as HTMLElement, { key: "Home" });
    await focused(first);
    expect(rows().filter((item) => item.tabIndex === 0)).toEqual([first]);
    expect(window.history.length).toBe(before);
  });

  it("opens a spec document's finding with Enter: one history entry, to the tree", async () => {
    await openHealth();
    const before = window.history.length;
    const target = row("docs/spec/berths/mooring.md");
    target.focus();
    fireEvent.keyDown(target, { key: "Enter" });
    await waitFor(() => {
      expect(window.location.hash).toBe(sectionHash("alpha", "tree", "docs/spec/berths/mooring.md"));
    });
    expect(window.history.length).toBe(before + 1);
    expect(await screen.findByRole("heading", { level: 1 })).toBeTruthy();
  });

  it("does nothing with Enter on a finding outside the spec, nor with Ctrl, Alt or Cmd, nor in the filter", async () => {
    await openHealth();
    const toml = row("specengine.toml");
    toml.focus();
    fireEvent.keyDown(toml, { key: "Enter" });
    for (const modifier of [{ ctrlKey: true }, { metaKey: true }, { altKey: true }]) {
      fireEvent.keyDown(toml, { key: "ArrowDown", ...modifier });
    }
    const field = within(region("Findings")).getByLabelText("Filter by path, subject, message or code");
    field.focus();
    for (const key of ["j", "k", "Home", "End", "Enter", "?"]) {
      fireEvent.keyDown(field, { key });
    }
    await settle();
    expect(window.location.hash).toBe("#/alpha/health");
    expect(document.activeElement).toBe(field);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("names a focused row by its text: severity, place, subject and message", async () => {
    await openHealth();
    const target = row("docs/spec/berths/mooring.md");
    expect(target.getAttribute("aria-labelledby")).toBe(target.id);
    expect(target.id).not.toBe("");
    expect(target.textContent).toBe("Severity: Errordocs/spec/berths/mooring.md:12Subject: RULE-TIDE-GATEdangling");
  });

  it("leaves the focus in the filter after Home on the first row or End on the last (no request left pending)", async () => {
    await openHealth();
    const field = within(region("Findings")).getByLabelText("Filter by path, subject, message or code");
    for (const [key, at] of [
      ["Home", 0],
      ["End", -1],
    ] as const) {
      const target = rows().at(at) as HTMLElement;
      target.focus();
      await focused(target);
      fireEvent.keyDown(target, { key });
      field.focus();
      fireEvent.change(field, { target: { value: "d" } });
      fireEvent.change(field, { target: { value: "" } });
      await settle();
      expect(document.activeElement).toBe(field);
    }
  });

  it("lists the keys with ?", async () => {
    await openHealth();
    const first = rows()[0] as HTMLElement;
    first.focus();
    fireEvent.keyDown(first, { key: "?" });
    const dialog = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(within(dialog).getByText("Findings: next finding")).toBeTruthy();
    expect(within(dialog).getByText("Findings: open the finding's document in the spec tree")).toBeTruthy();
  });

  it("adds no key listener to the document or the window but the shell's chord listener; one h1", async () => {
    const onDocument = vi.spyOn(document, "addEventListener");
    const onWindow = vi.spyOn(window, "addEventListener");
    await openHealth();
    const keyListeners = [
      ...onDocument.mock.calls.map(([type, , options]) => ["document", type, options]),
      ...onWindow.mock.calls.map(([type, , options]) => ["window", type, options]),
    ].filter(([, type]) => typeof type === "string" && /^key/.test(type));
    expect(keyListeners).toEqual([["document", "keydown", true]]);
    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
  });

  it("Clear filters leaves focus on the findings, never on the page's body", async () => {
    await openHealth();
    const field = within(region("Findings")).getByLabelText("Filter by path, subject, message or code");
    fireEvent.change(field, { target: { value: "nothing matches this" } });
    const clear = within(region("Findings")).getByRole("button", { name: "Clear filters" });
    clear.focus();
    fireEvent.click(clear);
    await waitFor(() => {
      expect(document.activeElement).not.toBe(document.body);
    });
    expect(document.activeElement?.getAttribute("data-finding")).not.toBeNull();
  });
});
