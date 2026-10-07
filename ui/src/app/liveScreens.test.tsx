import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { HttpClient } from "../api/http";
import type { CheckReport, GraphView } from "../api/types";
import { aCheckFinding, aCheckReport, aGraphEdge, aGraphNode, aGraphView } from "../test/builders";
import { errorAnswer, jsonAnswer, stubEventSource, stubFetch, urlsOf } from "../test/daemonStub";
import { renderApp } from "../test/render";

// AC-13 of docs/features/ui-live.md: Health and Graph over HttpClient, the daemon a stubbed
// `fetch`. Each shows the daemon's answer as data (every check verdict a 200 report, the graph's
// 404 the exit-1 document) or its refusal in its own words with Retry; never "Not built yet".

const NOT_BUILT = "Not built yet: the daemon has no endpoint for this read";

/** How the stubbed daemon answers the read under test (the check or the graph). */
type Answer = { status: number; body: CheckReport | GraphView } | { status: number; message: string };

/** The daemon: one project with an empty inbox, `read` answering `answer`; anything else its 404. */
function daemon(read: "check" | "graph", answer: Answer) {
  return stubFetch((url) => {
    if (url === "/api/projects") {
      return jsonAnswer(200, [{ slug: "alpha", name: "Alpha", root: "/work/alpha", branch: "main" }]);
    }
    if (url === "/api/projects/alpha/inbox") {
      return jsonAnswer(200, { proposals: [], notes: [] });
    }
    if (url.startsWith(`/api/projects/alpha/${read}`)) {
      return "message" in answer ? errorAnswer(answer.status, answer.message) : jsonAnswer(answer.status, answer.body);
    }
    return errorAnswer(404, `no route ${url}`);
  });
}

function region(name: string): HTMLElement {
  return screen.getByRole("region", { name });
}

/** Health with its four regions answered. */
async function openHealth() {
  renderApp(new HttpClient(), "#/alpha/health");
  await screen.findByRole("heading", { level: 1, name: "Health" });
  await waitFor(() => {
    expect(document.querySelectorAll('.health-region[aria-busy="false"]')).toHaveLength(4);
  });
}

function verdictLabel(): string | undefined {
  return region("Check").querySelector(".check-verdict .badge .badge-label")?.textContent;
}

function findingRows(): HTMLElement[] {
  return Array.from(region("Findings").querySelectorAll<HTMLElement>("[data-finding]"));
}

beforeEach(() => {
  stubEventSource();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("Health over the daemon (AC-13 of ui-live)", () => {
  it("a 200 `blocked` report: Fails the check, its finding listed, the check read once; never Not built yet", async () => {
    const report = aCheckReport({
      verdict: "blocked",
      counts: { documents: 9, errors: 1, worst_w_bytes: 24576 },
      findings: [aCheckFinding({ code: "key-missing", path: "docs/spec/movement/sprint.md", line: 1, subject: "status", message: "the key `status` is missing" })],
    });
    const fetchStub = daemon("check", { status: 200, body: report });
    await openHealth();
    expect(verdictLabel()).toBe("Fails the check");
    expect(findingRows()).toHaveLength(1);
    expect(findingRows()[0]?.textContent).toContain("the key `status` is missing");
    expect(region("Check").querySelector(".check-worst-w")?.textContent).toBe("24576 B");
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
    expect(screen.queryAllByRole("alert")).toEqual([]);
    expect(urlsOf(fetchStub).filter((url) => url.includes("/check"))).toEqual(["/api/projects/alpha/check"]);
  });

  it("a 200 `cannot-check` report: Could not check, the causes verbatim, W not measured", async () => {
    const report = aCheckReport({
      verdict: "cannot-check",
      counts: { documents: 0, worst_w_bytes: 0 },
      cannot_check: [
        { path: "", message: "today `x` is not a YYYY-MM-DD date" },
        { path: ".spec-debt.toml", message: "line 3: `expires` is not a YYYY-MM-DD date" },
      ],
    });
    daemon("check", { status: 200, body: report });
    await openHealth();
    expect(verdictLabel()).toBe("Could not check");
    // As the CLI prints a cause: `.` for no path (core check/report.rs `shown`).
    expect(Array.from(region("Check").querySelectorAll(".check-causes li"), (item) => item.textContent)).toEqual([
      ".: today `x` is not a YYYY-MM-DD date",
      ".spec-debt.toml: line 3: `expires` is not a YYYY-MM-DD date",
    ]);
    expect(region("Check").querySelector(".check-worst-w")?.textContent).toBe("Not measured");
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
  });

  it("a 503: the daemon's words in an alert, with Retry; never Not built yet", async () => {
    const message = "spec: specengine.toml: `[check] mode` must be one of observe, enforce-introduced, enforce";
    daemon("check", { status: 503, message });
    renderApp(new HttpClient(), "#/alpha/health");
    const alert = await within(await screen.findByRole("region", { name: "Check" })).findByRole("alert");
    expect(alert.textContent).toBe(`The check could not be read${message}`);
    expect(within(region("Check")).getByRole("button", { name: "Retry" })).toBeTruthy();
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
  });
});

describe("Graph over the daemon (AC-13 of ui-live)", () => {
  it("a 404 document: its reason, the REF and the default depth as the query; never Not built yet", async () => {
    const reason = "`R-404` resolves to no ID and no alias";
    const fetchStub = daemon("graph", { status: 404, body: aGraphView([], [], { ref: "R-404", reason, types: [], depth: null }) });
    renderApp(new HttpClient(), "#/alpha/graph/R-404");
    expect(await screen.findByText(reason)).toBeTruthy();
    expect(screen.getByText("R-404 names nothing to draw")).toBeTruthy();
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
    expect(urlsOf(fetchStub).filter((url) => url.includes("/graph"))).toEqual(["/api/projects/alpha/graph?ref=R-404&depth=2"]);
  });

  it("a 200: the walk drawn and counted", async () => {
    const answer = aGraphView(
      [aGraphNode({ id: "MEC-TIDES", distance: 0 }), aGraphNode({ id: "RULE-TIDE-WINDOW", distance: 1 })],
      [aGraphEdge({ src: "MEC-TIDES", type: "zeta_type", dst: "RULE-TIDE-WINDOW" })],
    );
    daemon("graph", { status: 200, body: answer });
    renderApp(new HttpClient(), "#/alpha/graph/MEC-TIDES");
    await waitFor(() => {
      expect(document.querySelector(".graph-count")?.textContent).toBe("2 nodes, 1 edge");
    });
    await waitFor(() => {
      expect(document.querySelectorAll(".react-flow__node").length).toBeGreaterThan(0);
    });
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
  });

  it("a 503: the daemon's words in an alert, with Retry", async () => {
    // Non-Latin text from the daemon, verbatim (escaped here: ADR-0024).
    const message = "spec: `MEC-\u0417\u0435` mixes scripts in an ID; IDs are Latin only";
    daemon("graph", { status: 503, message });
    renderApp(new HttpClient(), "#/alpha/graph/MEC-TIDES");
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(`The graph of MEC-TIDES could not be read${message}`);
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
    expect(screen.queryByText(NOT_BUILT)).toBeNull();
  });
});
