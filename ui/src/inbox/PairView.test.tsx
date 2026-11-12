import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Evidence, Pair } from "../api/types";
import PairView from "./PairView";

// docs/features/collision-pairs.md "Acceptance criteria", PP9, at the component's level: one region
// named "What conflicts with what", its two sides and Other evidence structured by their headings
// alone; each side's quotes at the served positions in served order, a position the item does not hold
// drawing nothing; a heading opening the document modal on the ID, else the path; a gone side saying
// so; every served string written as the CLI prints it. The screens': src/mocks/collisionPairs.smoke.test.tsx.

const SPRINT = "docs/spec/movement/sprint.md";
const STAMINA = "docs/spec/movement/stamina.md";

function evidence(): Evidence[] {
  return [
    { file: SPRINT, qpath: null, lines: "18-20", observed: "Sprinting pauses regeneration", documented: "Cost is drained per second" },
    { file: STAMINA, qpath: null, lines: "21-23", observed: "Regeneration never pauses", documented: "Refill starts after 1.5 s" },
    { file: "src/stamina.rs", qpath: "stamina::regen", lines: "40-52", observed: "regen runs during sprint", documented: "a pause is specified" },
  ];
}

const EXAMPLE: Pair = {
  left: { id: "RULE-SPRINT-COST", title: "Cost", path: SPRINT, evidence: [0] },
  right: { id: "RULE-STAM-REGEN", title: "Regeneration", path: STAMINA, evidence: [1] },
  other: [2],
};

function draw(pair: Pair, items: Evidence[] = evidence()) {
  const onOpen = vi.fn<(ref: string, opener: HTMLElement) => void>();
  render(<PairView pair={pair} evidence={items} onOpen={onOpen} />);
  return { onOpen, region: screen.getByRole("region", { name: "What conflicts with what" }) };
}

/** A side, or Other evidence, by its heading's name: the heading's section. */
function part(region: HTMLElement, name: string): HTMLElement {
  const section = within(region).getByRole("heading", { level: 3, name }).closest("section");
  if (section === null) {
    throw new Error(`no section under ${name}`);
  }
  return section;
}

/** The evidence entries' first lines (file and lines) in a scope, in order. */
function quotes(scope: HTMLElement): string[] {
  return Array.from(scope.querySelectorAll(".evidence > p:first-child"), (line) => line.textContent);
}

describe("PairView's landmarks", () => {
  it("is one region named by its label; the sides and Other evidence are headed sections with no name of their own", () => {
    const { region } = draw(EXAMPLE);
    // M: `aria-labelledby` back on a side or on Other evidence, four regions where the spec asks for one.
    expect(screen.getAllByRole("region")).toEqual([region]);
    expect(within(region).getAllByRole("heading", { level: 3 }).map((heading) => heading.textContent)).toEqual(["Cost", "Regeneration", "Other evidence"]);
    for (const name of ["Cost", "Regeneration", "Other evidence"]) {
      expect(part(region, name).hasAttribute("aria-labelledby")).toBe(false);
    }
  });
});

describe("PairView's evidence, as served", () => {
  it("lists each side's quotes at its positions in served order, the rest under Other evidence", () => {
    const { region } = draw({ ...EXAMPLE, left: { ...EXAMPLE.left, evidence: [2, 0] }, other: [1] });
    expect(quotes(part(region, "Cost"))).toEqual(["src/stamina.rs:40-52", `${SPRINT}:18-20`]);
    expect(quotes(part(region, "Regeneration"))).toEqual([`${STAMINA}:21-23`]);
    expect(quotes(part(region, "Other evidence"))).toEqual([`${STAMINA}:21-23`]);
  });

  it("draws nothing for a position past the end or below zero, keeping the rest in order", () => {
    const { region } = draw({ ...EXAMPLE, left: { ...EXAMPLE.left, evidence: [3, 0, -1, 1.5] }, other: [-2, 2, 9] });
    expect(quotes(part(region, "Cost"))).toEqual([`${SPRINT}:18-20`]);
    expect(quotes(part(region, "Other evidence"))).toEqual(["src/stamina.rs:40-52"]);
  });

  it("says no evidence cites the file in place of an empty list when no position resolves, and draws no Other evidence with none held", () => {
    const { region } = draw({ ...EXAMPLE, left: { ...EXAMPLE.left, evidence: [] }, right: { ...EXAMPLE.right, evidence: [5, -1] }, other: [7] });
    for (const name of ["Cost", "Regeneration"]) {
      const side = part(region, name);
      expect(within(side).getByText("No evidence cites this file")).toBeTruthy();
      expect(side.querySelector("ul")).toBeNull();
    }
    expect(within(region).queryByRole("heading", { level: 3, name: "Other evidence" })).toBeNull();
  });

  it("draws no Other evidence for an empty `other`", () => {
    const { region } = draw({ ...EXAMPLE, other: [] });
    expect(within(region).queryByRole("heading", { level: 3, name: "Other evidence" })).toBeNull();
  });
});

describe("PairView's headings", () => {
  it("opens the document modal on a side's ID from the button pressed", () => {
    const { region, onOpen } = draw(EXAMPLE);
    const cost = within(region).getByRole("button", { name: "Cost" });
    expect(cost.getAttribute("aria-haspopup")).toBe("dialog");
    fireEvent.click(cost);
    fireEvent.click(within(region).getByRole("button", { name: "Regeneration" }));
    expect(onOpen.mock.calls).toEqual([
      ["RULE-SPRINT-COST", cost],
      ["RULE-STAM-REGEN", within(region).getByRole("button", { name: "Regeneration" })],
    ]);
  });

  it("a path side opens its path; the line under it holds the path alone", () => {
    const { region, onOpen } = draw({ ...EXAMPLE, right: { id: null, title: "Stamina", path: STAMINA, evidence: [1] } });
    const right = part(region, "Stamina");
    expect(right.querySelector(".collision-pair-where")?.textContent).toBe(STAMINA);
    fireEvent.click(within(right).getByRole("button", { name: "Stamina" }));
    expect(onOpen.mock.calls.map(([ref]) => ref)).toEqual([STAMINA]);
  });

  it("a gone side with an ID and a path: the ID heads it once, it says so, the line under it holds the path", () => {
    const { region, onOpen } = draw({ ...EXAMPLE, left: { id: "RULE-SPRINT-COST", title: null, path: SPRINT, evidence: [0] } });
    const gone = part(region, "RULE-SPRINT-COST");
    expect(within(gone).getByText("Not in the spec now")).toBeTruthy();
    // M: the ID repeated under the heading that already names it.
    expect(gone.querySelector(".collision-pair-where")?.textContent).toBe(SPRINT);
    expect(within(gone).getAllByText("RULE-SPRINT-COST")).toHaveLength(1);
    fireEvent.click(within(gone).getByRole("button", { name: "RULE-SPRINT-COST" }));
    expect(onOpen.mock.calls.map(([ref]) => ref)).toEqual(["RULE-SPRINT-COST"]);
  });

  it("a gone side with an ID and no path: its heading opens nothing, no line under it repeats the ID", () => {
    const { region } = draw({ ...EXAMPLE, right: { id: "RULE-STAM-PAUSE", title: null, path: null, evidence: [] } });
    const gone = part(region, "RULE-STAM-PAUSE");
    expect(within(gone).queryByRole("button")).toBeNull();
    expect(within(gone).getByText("Not in the spec now")).toBeTruthy();
    expect(gone.querySelector(".collision-pair-where")).toBeNull();
    expect(within(gone).getAllByText("RULE-STAM-PAUSE")).toHaveLength(1);
  });

  it("a gone path side: the path heads it once and opens", () => {
    const { region, onOpen } = draw({ ...EXAMPLE, right: { id: null, title: null, path: STAMINA, evidence: [1] } });
    const gone = part(region, STAMINA);
    expect(gone.querySelector(".collision-pair-where")).toBeNull();
    fireEvent.click(within(gone).getByRole("button", { name: STAMINA }));
    expect(onOpen.mock.calls.map(([ref]) => ref)).toEqual([STAMINA]);
  });

  it("a side known by nothing now says so as its heading, a heading that opens nothing", () => {
    const { region } = draw({ ...EXAMPLE, right: { id: null, title: null, path: null, evidence: [] } });
    const gone = part(region, "Not in the spec now");
    expect(within(gone).queryByRole("button")).toBeNull();
    expect(gone.querySelector(".collision-pair-where")).toBeNull();
  });
});

describe("PairView's served text, as the CLI prints it", () => {
  it("writes the controls and bidi characters of a title, an ID, a path and every slot of a quote `\\u{..}`", () => {
    const quote: Evidence = {
      file: "src/st\u202eamina.rs",
      qpath: "stamina::\u202eregen",
      lines: "40\u000752",
      observed: "regen runs\u202e during\u0007 sprint\nsecond line",
      documented: "a pause\u0000 is\u2066 specified",
    };
    const { region } = draw(
      {
        left: { id: null, title: "Co\u202est\nnow", path: "docs/\u202ea.md", evidence: [0] },
        right: { id: "RULE-\u202eX", title: null, path: "docs/\u0007b.md", evidence: [] },
        other: [],
      },
      [quote],
    );
    const text = region.textContent;
    expect(text).not.toMatch(/[\u0000\u0007\u202e\u2066]/u);
    const left = part(region, "Co\\u{202e}st\\nnow");
    expect(left.querySelector(".collision-pair-where")?.textContent).toBe("docs/\\u{202e}a.md");
    expect(quotes(left)).toEqual(["src/st\\u{202e}amina.rs:40\\u{7}52"]);
    expect(within(left).getByText("stamina::\\u{202e}regen")).toBeTruthy();
    // A quote keeps its line feed; its controls are written out.
    expect(Array.from(left.querySelectorAll(".evidence dd"), (dd) => dd.textContent)).toEqual(["regen runs\\u{202e} during\\u{7} sprint\nsecond line", "a pause\\u{0} is\\u{2066} specified"]);
    // A gone side's heading (its ID) and the path under it.
    const right = part(region, "RULE-\\u{202e}X");
    expect(right.querySelector(".collision-pair-where")?.textContent).toBe("docs/\\u{7}b.md");
  });

  it("isolates each served slot in a <bdi>", () => {
    const { region } = draw(EXAMPLE);
    const cost = part(region, "Cost");
    expect(within(cost).getByRole("button", { name: "Cost" }).querySelector("bdi")?.textContent).toBe("Cost");
    const entry = cost.querySelector(".evidence");
    expect(Array.from(entry?.querySelectorAll("bdi") ?? [], (bdi) => bdi.textContent)).toEqual([SPRINT, "18-20", "Sprinting pauses regeneration", "Cost is drained per second"]);
  });
});
