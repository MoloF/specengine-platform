import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { FlowOptions, TraceOptions } from "../api/client";
import { App } from "../app/App";
import { HARBOUR_LOG_TITLE, HARBOUR_LOG_TOKEN } from "./harbor-sim/fixtures";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// The Flow studio over the real mock, as `pnpm dev` shows it (docs/features/writer-studio.md):
// ST-2 Back and reload, ST-6 a stage across groups, ST-12 a gap to the AI, ST-13 the reads stay
// bounded; and harbor-sim's badges, ledger-api with no `[flow]`, `large` past the cap, `empty`,
// `error`, `no-gaps`. The mock is imported here alone (policy AC-05).

/** All modules stored, as the module bar's first item stores it, unless `stored` says otherwise. */
function renderMock(hash: string, scenario: Scenario = "normal", stored: string | null = "") {
  if (stored !== null) {
    window.localStorage.setItem("specengine.module.harbor-sim", stored);
  }
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0 });
  const spies = {
    getFlow: vi.spyOn(client, "getFlow"),
    getTrace: vi.spyOn(client, "getTrace"),
    getNode: vi.spyOn(client, "getNode"),
    getTree: vi.spyOn(client, "getTree"),
    createCard: vi.spyOn(client, "createCard"),
    getTasks: vi.spyOn(client, "getTasks"),
    getTask: vi.spyOn(client, "getTask"),
  };
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return spies;
}

type Spies = ReturnType<typeof renderMock>;

function boxes(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(".react-flow__node[data-id^='n:']"));
}

function drawnNames(): string[] {
  return boxes().map((element) => (element.dataset.id ?? "").slice(2));
}

function box(name: string): HTMLElement {
  const found = boxes().find((element) => element.dataset.id === `n:${name}`);
  if (found === undefined) {
    throw new Error(`no box ${name}`);
  }
  return found;
}

async function drawn(name: string): Promise<HTMLElement> {
  await waitFor(() => {
    expect(drawnNames()).toContain(name);
  });
  return box(name);
}

function panel(): HTMLElement {
  return screen.getByRole("complementary", { name: "Details" });
}

/** The panel's own heading: the scope's, the box's or the arrow's. */
function panelTitle(): string {
  return Array.from(panel().querySelectorAll(".flow-panel-title")).find((title) => title.closest("[hidden]") === null)?.textContent ?? "";
}

function places(): HTMLElement {
  return screen.getByRole("navigation", { name: "Flow places" });
}

/** The places pane's link to a hash. */
async function placeLink(hash: string): Promise<HTMLAnchorElement> {
  return waitFor(() => {
    const found = Array.from(places().querySelectorAll<HTMLAnchorElement>("a")).find((link) => link.getAttribute("href") === hash);
    if (found === undefined) {
      throw new Error(`no place ${hash}`);
    }
    return found;
  });
}

async function hashIs(hash: string) {
  await waitFor(() => {
    expect(window.location.hash).toBe(hash);
  });
}

/** Each read the spies saw, as a line: what ST-13 counts. */
function readLines(spies: Spies): string[] {
  return [
    ...spies.getFlow.mock.calls.map((call) => `flow ${JSON.stringify(call[1])}`),
    ...spies.getTrace.mock.calls.map((call) => `trace ${JSON.stringify(call[1] ?? {})}`),
    ...spies.getNode.mock.calls.map((call) => `node ${call[1]}`),
    ...spies.getTree.mock.calls.map((call) => `tree ${JSON.stringify(call[1] ?? {})}`),
  ];
}

/** Waits for the studio to read nothing more: its region and the stages say so. */
async function settled() {
  await waitFor(() => {
    expect(document.querySelector(".flow-studio")?.getAttribute("aria-busy")).toBe("false");
    expect(within(places()).queryByRole("status")).toBeNull();
  });
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/** D2's three forms, each cut to the flow (flow-scenario D6), and nothing else. */
function isAForm(options: FlowOptions): boolean {
  const keys = Object.keys(options).sort().join(",");
  return options.trace === true && options.in_flow === true && (keys === "depth,in_flow,trace" ? options.depth === 0 : keys === "depth,in_flow,root,trace" ? options.depth === 1 : keys === "in_flow,root,stage,trace");
}

/** The bar's Gaps control, opened (flow-scenario D10). */
function openGaps(): HTMLElement {
  const toggle = screen.getByRole("button", { name: /^Gaps/ });
  if (toggle.getAttribute("aria-expanded") !== "true") {
    fireEvent.click(toggle);
  }
  return screen.getByRole("region", { name: "Gaps" });
}

function checkForms(spies: Spies) {
  for (const call of spies.getFlow.mock.calls) {
    expect([call[1], isAForm(call[1])]).toEqual([call[1], true]);
  }
  for (const call of spies.getTrace.mock.calls) {
    const options: TraceOptions = call[1] ?? {};
    expect([options, options.ref !== undefined && options.stage === undefined]).toEqual([options, true]);
  }
}

const LENS = "#/harbor-sim/flow?module=";

describe("Back and reload (ST-2)", () => {
  it("pushes a scope and a stage, replaces a selection; Back walks the scopes back; a fresh mount draws the hash; a bare hash takes the last place", async () => {
    renderMock(LENS);
    await drawn("DOM-BERTHS");
    const start = window.history.length;
    fireEvent.click(await placeLink("#/harbor-sim/flow?module=&scope=DOM-BERTHS"));
    await hashIs("#/harbor-sim/flow?module=&scope=DOM-BERTHS");
    await drawn("MEC-MOORING");
    fireEvent.click(await placeLink("#/harbor-sim/flow?module=&scope=DOM-BERTHS&stage=quayside"));
    await hashIs("#/harbor-sim/flow?module=&scope=DOM-BERTHS&stage=quayside");
    await waitFor(() => {
      expect(drawnNames()).toEqual(["MEC-MOORING"]);
    });
    // MEC-OLD-QUAYS is archived, so no flow read holds it: the second selection is Esc's, a replace too.
    fireEvent.click(box("MEC-MOORING"));
    await hashIs("#/harbor-sim/flow/MEC-MOORING?module=&scope=DOM-BERTHS&stage=quayside");
    fireEvent.keyDown(box("MEC-MOORING"), { key: "Escape" });
    await hashIs("#/harbor-sim/flow?module=&scope=DOM-BERTHS&stage=quayside");
    // M4: a selection pushing would grow it by 4.
    expect(window.history.length).toBe(start + 2);
    act(() => {
      window.history.back();
    });
    await hashIs("#/harbor-sim/flow?module=&scope=DOM-BERTHS");
    await drawn("MEC-MOORING");
    // The berth limits hold no flow link: the cut leaves them to Records for the AI (flow-scenario D2).
    expect(drawnNames()).not.toContain("docs/spec/berths/draft-limits.md");
    expect(panelTitle()).toBe("Berths and moorings");
    act(() => {
      window.history.back();
    });
    await hashIs(LENS);
    await drawn("DOM-WATER");
    // In the same Shell, the nav's bare link takes the last place.
    act(() => {
      window.location.hash = "#/harbor-sim/tree";
    });
    await screen.findByRole("tree", { name: "Spec tree" });
    act(() => {
      window.location.hash = "#/harbor-sim/flow";
    });
    await hashIs(LENS);
    // A hash naming a place is that place, whatever the Shell kept (M5).
    act(() => {
      window.location.hash = "#/harbor-sim/tree";
    });
    await screen.findByRole("tree", { name: "Spec tree" });
    act(() => {
      window.location.hash = "#/harbor-sim/flow?module=&scope=DOM-WATER";
    });
    await drawn("MEC-TIDES");
    expect(panelTitle()).toBe("Water and weather");
    cleanup();
    // M5: a fresh mount draws the hash, whatever a Shell kept.
    renderMock("#/harbor-sim/flow/MEC-MOORING?module=&scope=DOM-BERTHS&stage=quayside", "normal", "berths");
    await waitFor(() => {
      expect(drawnNames()).toEqual(["MEC-MOORING"]);
    });
    expect(panelTitle()).toBe("Mooring");
  });

  it("the module bar in the places pushes and stores the viewer's default (M6)", async () => {
    renderMock(LENS);
    await drawn("DOM-BERTHS");
    const berths = await placeLink("#/harbor-sim/flow?module=berths");
    expect((await placeLink(LENS)).getAttribute("aria-current")).toBe("true");
    fireEvent.click(berths);
    await hashIs("#/harbor-sim/flow?module=berths");
    expect(window.localStorage.getItem("specengine.module.harbor-sim")).toBe("berths");
    await drawn("MEC-MOORING");
    expect(panelTitle()).toBe("Berths and moorings");
    // A push: Back returns to All modules, the stored choice aside.
    act(() => {
      window.history.back();
    });
    await hashIs(LENS);
    await drawn("DOM-WATER");
  });
});

describe("a stage narrows (ST-6)", () => {
  it("under DOM-HARBOR, landfall pushes one rooted stage read: its four steps across groups, nothing dimmed, their groups in the pane", async () => {
    const spies = renderMock(LENS);
    await drawn("MEC-FAIRWAY");
    fireEvent.click(await placeLink("#/harbor-sim/flow?module=&stage=landfall"));
    await hashIs("#/harbor-sim/flow?module=&stage=landfall");
    await waitFor(() => {
      expect(drawnNames().sort()).toEqual(["MEC-PILOT-CALL", "MEC-PILOTAGE", "RULE-HIGH-WATER", "RULE-SPRING-WINDOW"]);
    });
    expect(spies.getFlow.mock.calls.map((call) => call[1])).toContainEqual({ root: "DOM-HARBOR", stage: "landfall", in_flow: true, trace: true });
    expect(document.querySelector("[data-id='n:MEC-FAIRWAY']")).toBeNull();
    const groups = within(within(places()).getByRole("list", { name: "Groups here" }));
    const named = groups.getAllByRole("link").map((link) => link.getAttribute("href"));
    expect(named).toEqual(["#/harbor-sim/flow?module=&scope=DOM-WATER", "#/harbor-sim/flow?module=&scope=MEC-TIDES", "#/harbor-sim/flow?module=&scope=DOM-PILOTS"]);
    expect(panelTitle()).toBe("Harbor simulation");
    fireEvent.click(await placeLink("#/harbor-sim/flow?module=&stage=-"));
    await waitFor(() => {
      expect(drawnNames()).toEqual(["MEC-NIGHT-PASSAGE"]);
    });
    // Each a push: Back returns to landfall, then to all stages.
    act(() => {
      window.history.back();
    });
    await hashIs("#/harbor-sim/flow?module=&stage=landfall");
    act(() => {
      window.history.back();
    });
    await hashIs(LENS);
    checkForms(spies);
  });

  it("the long-text step: its 160-character title and its 300-character token stand in the panel, which wraps them", async () => {
    renderMock("#/harbor-sim/flow/MEC-HARBOUR-LOG?module=&scope=DOM-PILOTS");
    await drawn("MEC-HARBOUR-LOG");
    expect(Array.from(HARBOUR_LOG_TITLE)).toHaveLength(160);
    expect(within(panel()).getAllByText(HARBOUR_LOG_TITLE).length).toBeGreaterThan(0);
    expect(await within(panel()).findByText(new RegExp(HARBOUR_LOG_TOKEN))).toBeTruthy();
    // Its links agree with the arrow the canvas draws into it.
    await waitFor(() => {
      expect(within(panel()).getAllByText("MEC-PILOT-CALL").length).toBeGreaterThan(0);
    });
    expect(within(panel()).queryByText(/No link is written in this node/)).toBeNull();
  });
});

describe("a gap to the AI (ST-12)", () => {
  it("under DOM-HARBOR, cycle-no-repeat's item opens New card with both targets, the cause verbatim, the title; Create is one card, no task", async () => {
    const spies = renderMock(LENS);
    await drawn("MEC-FAIRWAY");
    const row = within(openGaps()).getByText("cycle-no-repeat").closest("li.flow-gap");
    const item = within(row as HTMLElement).getByRole("button", { name: "Fairway + Pilot boarding" }).closest("li");
    fireEvent.click(within(item as HTMLElement).getByRole("button", { name: /^Hand to your AI/ }));
    const dialog = await screen.findByRole("dialog", { name: "New card" });
    const cause = "a loop with no repeat edge: write the edge closing it as {to: ID, kind: repeat}";
    expect(within(dialog).getByLabelText<HTMLInputElement>("Title").value).toBe("Gap cycle-no-repeat: MEC-FAIRWAY + MEC-PILOTAGE");
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Goal").value).toBe(cause);
    expect(within(dialog).getByLabelText<HTMLTextAreaElement>("Source", { exact: false }).value).toBe("MEC-FAIRWAY\nMEC-PILOTAGE");
    // harbor-sim's card flows each list their disciplines: the owner picks one, as on the Board.
    fireEvent.change(within(dialog).getByLabelText("Discipline", { exact: false }), { target: { value: "copy" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Create card" }));
    await waitFor(() => {
      expect(spies.createCard).toHaveBeenCalledTimes(1);
    });
    // M24, M25: both members in order, the cause as written.
    expect(spies.createCard.mock.calls[0]?.[1]).toMatchObject({ target: ["MEC-FAIRWAY", "MEC-PILOTAGE"], goal: cause });
    await within(panel()).findByText(/created\.$/);
    fireEvent.click(box("MEC-FAIRWAY"));
    const cards = await within(panel()).findByRole("region", { name: /cards/i });
    expect(await within(cards).findByText("Gap cycle-no-repeat: MEC-FAIRWAY + MEC-PILOTAGE")).toBeTruthy();
    expect(spies.getTask).not.toHaveBeenCalled();
    expect(spies.getTasks).not.toHaveBeenCalled();
  });
});

describe("the reads stay bounded (ST-13)", () => {
  it("(a) to (e): the lens root's four reads; a stage one; a box two; a scope three; Back and Forward none", async () => {
    const spies = renderMock(LENS);
    await drawn("DOM-BERTHS");
    await settled();
    expect(readLines(spies).sort()).toEqual(
      ['flow {"depth":0,"in_flow":true,"trace":true}', 'flow {"root":"DOM-HARBOR","depth":1,"in_flow":true,"trace":true}', 'trace {"ref":"DOM-HARBOR","summary":true}', "node DOM-HARBOR"].sort(),
    );
    let before = readLines(spies);
    fireEvent.click(await placeLink("#/harbor-sim/flow?module=&stage=quayside"));
    await drawn("MEC-PILOT-RETURN");
    await settled();
    expect(readLines(spies).filter((line) => !before.includes(line))).toEqual(['flow {"root":"DOM-HARBOR","stage":"quayside","in_flow":true,"trace":true}']);
    before = readLines(spies);
    fireEvent.click(box("MEC-MOORING"));
    await waitFor(() => {
      expect(panelTitle()).toBe("Mooring");
    });
    await settled();
    expect(readLines(spies).filter((line) => !before.includes(line)).sort()).toEqual(["node MEC-MOORING", 'trace {"ref":"MEC-MOORING"}']);
    before = readLines(spies);
    fireEvent.click(within(panel()).getByRole("link", { name: "Open Berths and moorings" }));
    await waitFor(() => {
      expect(panelTitle()).toBe("Berths and moorings");
    });
    await settled();
    expect(readLines(spies).filter((line) => !before.includes(line)).sort()).toEqual(['flow {"root":"DOM-BERTHS","depth":1,"in_flow":true,"trace":true}', 'trace {"ref":"DOM-BERTHS","summary":true}', "node DOM-BERTHS"].sort());
    const count = readLines(spies).length;
    for (const step of ["back", "back", "forward"] as const) {
      act(() => {
        if (step === "back") {
          window.history.back();
        } else {
          window.history.forward();
        }
      });
      await waitFor(() => {
        expect(window.location.hash).not.toBe("");
      });
      await settled();
    }
    await hashIs("#/harbor-sim/flow/MEC-MOORING?module=&stage=quayside");
    expect(readLines(spies)).toHaveLength(count);
    checkForms(spies);
    expect(spies.getTree).not.toHaveBeenCalled();
  });

  it("(f) an old link with All modules stored: the lens root's reads, one tree, the parent's three, the box's two; the hash replaced", async () => {
    const spies = renderMock("#/harbor-sim/flow/MEC-PILOT-RETURN");
    const before = window.history.length;
    await hashIs("#/harbor-sim/flow/MEC-PILOT-RETURN?module=&scope=DOM-PILOTS");
    await drawn("MEC-PILOT-RETURN");
    await settled();
    expect(window.history.length).toBe(before);
    expect(readLines(spies).sort()).toEqual(
      [
        'flow {"depth":0,"in_flow":true,"trace":true}',
        'flow {"root":"DOM-HARBOR","depth":1,"in_flow":true,"trace":true}',
        'trace {"ref":"DOM-HARBOR","summary":true}',
        "node DOM-HARBOR",
        "tree {}",
        'flow {"root":"DOM-PILOTS","depth":1,"in_flow":true,"trace":true}',
        'trace {"ref":"DOM-PILOTS","summary":true}',
        "node DOM-PILOTS",
        'trace {"ref":"MEC-PILOT-RETURN"}',
        "node MEC-PILOT-RETURN",
      ].sort(),
    );
    checkForms(spies);
  });

  it("(g) module=berths reads no top", async () => {
    const spies = renderMock("#/harbor-sim/flow?module=berths");
    await drawn("MEC-MOORING");
    await settled();
    expect(spies.getFlow.mock.calls.map((call) => call[1])).toEqual([{ root: "DOM-BERTHS", depth: 1, in_flow: true, trace: true }]);
    checkForms(spies);
    expect(spies.getTree).not.toHaveBeenCalled();
  });
});

describe("the studio over the mock's scenarios", () => {
  it("harbor-sim: badges, a group's counts, Entry and End as the scope's read folds them", async () => {
    renderMock(LENS);
    await drawn("DOM-BERTHS");
    expect(box("MEC-PILOTAGE").querySelector(".badge")?.textContent).toBe("No card");
    expect(box("DOM-WATER").querySelector(".flow-box-state")?.textContent).toBe("2 confirmed, 2 in work, 1 departed");
    expect(box("DOM-WATER").querySelector(".flow-mark-start")?.textContent).toBe("Entry");
    expect(box("DOM-BERTHS").querySelector(".flow-mark-end")?.textContent).toBe("End");
    await waitFor(() => {
      expect(document.querySelectorAll(".react-flow__edge").length).toBeGreaterThan(5);
    });
  });

  it("ledger-api: no `[flow]`, the canvas drawn, rows Not configured, causes verbatim, no digit in them (R7)", async () => {
    renderMock("#/ledger-api/flow", "normal", null);
    await waitFor(() => {
      expect(boxes().length).toBeGreaterThan(0);
    });
    const shown = within(openGaps());
    expect(shown.getByText("7 rows need [flow] rules in specengine.toml")).toBeTruthy();
    expect(shown.getAllByText("Not configured")).toHaveLength(7);
    const row = shown.getByText("no-start").closest("li");
    expect(row?.textContent).toContain("no [[flow.start]] rule: add one naming the first step by paths");
    expect(row?.textContent).not.toMatch(/\d/);
    expect(document.querySelector("[aria-label='Modules']")).toBeNull();
  });

  it("large: 200 boxes past the cap, the bar's line, the List whole (R6)", async () => {
    renderMock("#/harbor-sim/flow?module=&scope=ZONE-GEN-02", "large");
    await drawn("ZONE-GEN-02-01");
    expect(boxes()).toHaveLength(19);
    cleanup();
    renderMock(LENS, "large");
    await drawn("ZONE-GEN-25");
    expect(boxes()).toHaveLength(25);
    expect(within(places()).getByText("Stages show inside a group: open one.")).toBeTruthy();
  });

  it("empty: the meaning and the next step, the rows standing", async () => {
    renderMock(LENS, "empty");
    expect(await screen.findByText("No flow in this project yet. Next: have your AI add flow links.")).toBeTruthy();
    expect(within(openGaps()).getByText("Cannot measure")).toBeTruthy();
  });

  it("error: the daemon's message verbatim, Retry", async () => {
    renderMock(LENS, "error");
    const region = await screen.findByRole("region", { name: "Flow canvas" });
    expect(await within(region).findByRole("button", { name: "Retry" })).toBeTruthy();
    expect(within(region).getByText(/database is locked/)).toBeTruthy();
  });

  it("no-gaps: the toggle says None; open, one line in the rows' place (flow-scenario D10)", async () => {
    renderMock(LENS, "no-gaps");
    await drawn("DOM-BERTHS");
    expect(within(screen.getByRole("button", { name: /^Gaps/ })).getByText("None")).toBeTruthy();
    const control = openGaps();
    expect(within(control).getByText("10 rows find nothing")).toBeTruthy();
    expect(control.querySelectorAll("li.flow-gap")).toHaveLength(0);
  });
});

// flow-scenario FS-13, FS-18 (RM-0094) over the mock's `aside` and `aside40`: harbor-sim's spec is the
// pottery workshop of `fixtures/flow-aside`, its membership the mock's.
describe("the aside scenarios (flow-scenario)", () => {
  it("aside: the lens root draws the scenario alone; at the kiln bay Records for the AI reads once, when asked, its records by kind", async () => {
    const spies = renderMock("#/harbor-sim/flow", "aside", null);
    await drawn("KRUN-FIRING");
    expect(drawnNames()).toHaveLength(11);
    expect(drawnNames().some((name) => /^(EDICT|HUNCH|PUZZLE|DESK)-|^docs\/notes\//.test(name))).toBe(false);
    fireEvent.click(await placeLink("#/harbor-sim/flow?scope=WBAY-KILN"));
    await drawn("CONE-GLOST");
    await settled();
    expect(drawnNames().sort()).toEqual(["CONE-BISQUE", "CONE-GLOST", "HUNCH-0007"]);
    checkForms(spies);
    const button = within(places()).getByRole("button", { name: "Records for the AI (141)" });
    const before = spies.getFlow.mock.calls.length;
    fireEvent.click(button);
    const dialog = await screen.findByRole("dialog", { name: "Records for the AI (141)" });
    await within(dialog).findByRole("group", { name: "puzzle" });
    expect(spies.getFlow.mock.calls.slice(before).map((call) => call[1])).toEqual([{ root: "WBAY-KILN", depth: 1 }]);
    expect(["edict", "hunch", "puzzle"].map((kind) => within(dialog).getByRole("group", { name: kind }).querySelectorAll("li").length)).toEqual([32, 59, 50]);
    expect(within(dialog).getAllByRole("link")[0]?.getAttribute("href")).toBe("#/harbor-sim/tree/EDICT-0001");
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(document.activeElement).toBe(within(places()).getByRole("button", { name: "Records for the AI (141)" }));
  });

  it("aside: the path at the glost cone reads by titles from its own node read; the front office stands empty, its two papers for the AI", async () => {
    renderMock("#/harbor-sim/flow?scope=CONE-GLOST", "aside", null);
    await waitFor(() => {
      const path = within(places()).getByRole("list", { name: "Path" });
      expect(within(path).getAllByRole("link").map((link) => link.textContent)).toEqual(["The pottery", "Kiln bay", "Glost cone"]);
    });
    act(() => {
      window.location.hash = "#/harbor-sim/flow?scope=WBAY-OFFICE";
    });
    expect(await screen.findByRole("heading", { level: 2, name: "Nothing here is part of the flow." })).toBeTruthy();
    expect(screen.getByText("2 documents here are for your AI.")).toBeTruthy();
  });

  it("aside40: the kiln bay draws its three boxes; Records for the AI lists 200, then Show all 5679", async () => {
    renderMock("#/harbor-sim/flow?scope=WBAY-KILN", "aside40", null);
    await drawn("CONE-GLOST");
    expect(drawnNames()).toHaveLength(3);
    fireEvent.click(within(places()).getByRole("button", { name: "Records for the AI (5679)" }));
    const dialog = await screen.findByRole("dialog", { name: "Records for the AI (5679)" });
    await within(dialog).findByRole("group", { name: "edict" });
    expect(dialog.querySelectorAll(".flow-records-group li")).toHaveLength(200);
    expect(within(dialog).getByRole("button", { name: "Show all 5679" })).toBeTruthy();
  }, 30_000);
});
