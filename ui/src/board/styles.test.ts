import { describe, expect, it } from "vitest";
import appCss from "../styles/app.css?raw";

// BH-1 and BH-2 of board-handoff: a board that holds still. jsdom lays nothing out, so these pin the
// rules; the rects themselves are Looks, in a browser. No panel column beside the board; one grid of
// `minmax(14rem, 1fr)` columns under a sticky header row; notices over the board's bottom edge, out of
// the grid's flow; above 36rem the region scrolls both ways, under it the columns stack.

/** The text of each `@media (<query>)` block, braces matched. */
function mediaBlocks(query: string): string[] {
  const found: string[] = [];
  let from = 0;
  for (;;) {
    const at = appCss.indexOf(`@media (${query})`, from);
    if (at < 0) {
      return found;
    }
    const open = appCss.indexOf("{", at);
    let depth = 0;
    let end = open;
    for (; end < appCss.length; end += 1) {
      depth += appCss[end] === "{" ? 1 : appCss[end] === "}" ? -1 : 0;
      if (depth === 0) {
        break;
      }
    }
    found.push(appCss.slice(open + 1, end));
    from = end;
  }
}

/** The declarations of each rule whose selector is `selector` alone, in `css`'s order. */
function rulesOf(selector: string, css: string = appCss): string[] {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return Array.from(css.matchAll(new RegExp(`^\\s*${escaped}\\s*\\{([^}]*)\\}`, "gm")), (match) => match[1] ?? "");
}

function ruleWith(selector: string, declaration: RegExp, css: string = appCss): string {
  const rule = rulesOf(selector, css).find((declarations) => declaration.test(declarations));
  if (rule === undefined) {
    throw new Error(`no ${selector} rule holds ${declaration.source}`);
  }
  return rule;
}

const WIDE = mediaBlocks("min-width: 36.0625rem").join("\n");
const NARROW = mediaBlocks("max-width: 36rem").join("\n");

describe("the columns hold still (BH-1)", () => {
  it("draws no panel column beside the board: no `.board-stage-panel` (M1)", () => {
    expect(appCss).not.toMatch(/\.board-stage-panel/);
    expect(rulesOf(".board-stage").every((rule) => !/grid-template-columns/.test(rule))).toBe(true);
  });

  it("lays every column out at minmax(14rem, 1fr) in one grid, the header row its own subgrid", () => {
    expect(ruleWith(".board-grid", /grid-template-columns/)).toMatch(/grid-template-columns:\s*repeat\(var\(--board-columns, 1\), minmax\(14rem, 1fr\)\);/);
    expect(ruleWith(".board-head", /grid-template-columns/)).toMatch(/grid-template-columns:\s*subgrid;/);
    expect(ruleWith(".board-head", /grid-column/)).toMatch(/grid-column:\s*1 \/ -1;/);
  });

  it("lays the notices over the board's bottom edge, out of the grid's flow, their stage the anchor", () => {
    const notices = ruleWith(".board-notices", /position/);
    expect(notices).toMatch(/position:\s*absolute;/);
    expect(notices).toMatch(/bottom:/);
    expect(ruleWith(".board-stage", /position/)).toMatch(/position:\s*relative;/);
  });

  it("lays the acting name's line over the board, out of the toolbar's flow: the toolbar keeps its height", () => {
    expect(ruleWith(".board-acting", /position/)).toMatch(/position:\s*relative;/);
    const line = ruleWith(".board-acting .field-error", /position/);
    expect(line).toMatch(/position:\s*absolute;/);
    expect(line).toMatch(/top:\s*calc\(100% \+ var\(--space-1\)\);/);
  });

  it("starts a cell's rows at its top and gives an empty cell height to take a drop", () => {
    const cell = ruleWith(".board-cell", /align-content/);
    expect(cell).toMatch(/align-content:\s*start;/);
    expect(cell).toMatch(/min-height:\s*6rem;/);
    expect(ruleWith(".board-grid-single", /grid-template-rows/)).toMatch(/grid-template-rows:\s*auto 1fr;/);
    expect(ruleWith(".board-grid", /min-height/)).toMatch(/min-height:\s*100%;/);
  });
});

describe("the header stays; the page never scrolls sideways (BH-2)", () => {
  it("sticks the header row to the region's top, the region's scroll padding its height (M3)", () => {
    const head = ruleWith(".board-head", /position/);
    expect(head).toMatch(/position:\s*sticky;/);
    expect(head).toMatch(/top:\s*0;/);
    expect(ruleWith(".board-region", /scroll-padding-top/)).toMatch(/scroll-padding-top:\s*var\(--board-head-height/);
  });

  it("above 36rem: the view fills the viewport below the shell, measured, and the region scrolls both ways", () => {
    expect(WIDE).not.toBe("");
    const view = ruleWith(".board-view", /height/, WIDE);
    expect(view).toMatch(/height:\s*calc\(100dvh - var\(--board-top, 8rem\) - var\(--space-6\)\);/);
    expect(view).toMatch(/overflow:\s*hidden;/);
    const region = ruleWith(".board-region", /overflow/, WIDE);
    expect(region).toMatch(/overflow:\s*auto;/);
    expect(region).toMatch(/min-height:\s*0;/);
    expect(ruleWith(".app-body:has(.board-view)", /min-height/, WIDE)).toMatch(/min-height:\s*0;/);
  });

  it("under 36rem: the columns stack, one under the other, each cell titled in place; the header row not drawn; the card fills the screen", () => {
    expect(ruleWith(".board-grid", /grid-template-columns/, NARROW)).toMatch(/grid-template-columns:\s*minmax\(0, 1fr\);/);
    expect(ruleWith(".board-head", /display/, NARROW)).toMatch(/display:\s*none;/);
    expect(ruleWith(".board-cell-title", /display/, NARROW)).toMatch(/display:\s*flex;/);
    expect(ruleWith(".board-cell-title", /display/)).toMatch(/display:\s*none;/);
    expect(ruleWith(".card-modal", /width/, NARROW)).toMatch(/width:\s*100vw;/);
    expect(NARROW).not.toMatch(/\.board-region\s*\{[^}]*overflow:\s*auto/);
  });

  // Widened by handoff-progress (docs/features/handoff-progress.md "Acceptance criteria", M-1): a main
  // column of about 750 px beside a 360 px side column, so the modal is 1200 px wide; its body still scrolls inside.
  it("keeps the card modal inside the viewport: at most --modal-width-xl (1200 px) wide and the height less 2rem, its work item's body scrolling, its page as wide (M-1)", () => {
    const modal = ruleWith(".card-modal", /width/);
    expect(modal).toMatch(/width:\s*min\(var\(--modal-width-xl\), 100vw - 2rem\);/);
    expect(ruleWith(".card-page", /max-width/)).toMatch(/max-width:\s*var\(--modal-width-xl\);/);
    expect(modal).toMatch(/max-height:\s*calc\(100dvh - 2rem\);/);
    // At one column its body's scrolling is the work item's own in any dialog (docs/features/doc-modal.md "Acceptance criteria", DM-21's 320 px look);
    // at two, Main and the side scroll apart (docs/features/room-and-scroll.md "Acceptance criteria", RS-8).
    expect(ruleWith(".dialog > .issue-view > .issue-view-body", /overflow/)).toMatch(/overflow:\s*auto;/);
  });

  it("stretches the title's link over the tile, the other controls above it", () => {
    const after = ruleWith(".board-card-link::after", /inset/);
    expect(after).toMatch(/position:\s*absolute;/);
    expect(after).toMatch(/inset:\s*0;/);
    expect(ruleWith(".board-card", /position/)).toMatch(/position:\s*relative;/);
  });
});

/**
 * The value `property` takes on an element of `classes`, by the cascade over the stylesheet's rules
 * whose selector is classes alone (`.a`, `.a.b`): the most classes win, then the later rule.
 */
function cascaded(classes: readonly string[], property: string): string | null {
  const css = appCss.replace(/\/\*[\s\S]*?\*\//g, "");
  let won: { weight: number; value: string } | null = null;
  for (const match of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const value = new RegExp(`(?:^|;)\\s*${property}:\\s*([^;]+);`).exec(match[2] ?? "")?.[1]?.trim();
    if (value === undefined) {
      continue;
    }
    for (const selector of (match[1] ?? "").split(",").map((part) => part.trim())) {
      const named = /^(\.[\w-]+)+$/.test(selector) ? selector.slice(1).split(".") : null;
      if (named !== null && named.every((name) => classes.includes(name)) && (won === null || named.length >= won.weight)) {
        won = { weight: named.length, value };
      }
    }
  }
  return won?.value ?? null;
}

// A review minor of task-mandate (docs/features/task-mandate.md "Acceptance criteria", V3): a parent's
// Decisions from subtasks keeps its entries --space-3 apart, though it is a `.board-panel-list` too,
// whose rule of one class stands later in the sheet with --space-1.
describe("Decisions from subtasks keeps its entries apart (V3 of task-mandate)", () => {
  it("lays its entries --space-3 apart, over the panel list's --space-1 (M: the gap rule of one class, losing to .board-panel-list)", () => {
    expect(cascaded(["board-panel-list"], "gap")).toBe("var(--space-1)");
    expect(cascaded(["board-panel-list", "board-from-subtasks"], "gap")).toBe("var(--space-3)");
  });
});
