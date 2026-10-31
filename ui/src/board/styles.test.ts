import { describe, expect, it } from "vitest";
import appCss from "../styles/app.css?raw";

// AC-01 of docs/features/screen-fit.md: a column stands over the free height, so a drop anywhere in
// it lands. jsdom lays nothing out, so these pin the rules; the drop itself is AC-02, in a browser.

/** The declarations of each rule whose selector is `selector` alone, in file order, a media query's too. */
function rulesOf(selector: string): string[] {
  const escaped = selector.replaceAll(".", "\\.");
  return Array.from(appCss.matchAll(new RegExp(`^\\s*${escaped}\\s*\\{([^}]*)\\}`, "gm")), (match) => match[1] ?? "");
}

function ruleWith(selector: string, declaration: RegExp): string {
  const rule = rulesOf(selector).find((declarations) => declaration.test(declarations));
  if (rule === undefined) {
    throw new Error(`no ${selector} rule holds ${declaration.source}`);
  }
  return rule;
}

describe("a Board column's height (AC-01)", () => {
  it("stands the columns side by side over 60vh, each at the row's height", () => {
    const columns = ruleWith(".board-columns", /grid-auto-flow:\s*column;/);
    expect(columns).toMatch(/align-items:\s*stretch;/);
    expect(columns).toMatch(/min-height:\s*60vh;/);
  });

  it("starts a column's rows at its top: a stretch shared among them would grow the title", () => {
    expect(rulesOf(".board-column")).toHaveLength(1);
    expect(rulesOf(".board-column")[0]).toMatch(/align-content:\s*start;/);
  });

  it("takes the height back where the columns stack: a phone scrolls past no empty boxes", () => {
    expect(ruleWith(".board-columns", /grid-auto-flow:\s*row;/)).toMatch(/min-height:\s*0;/);
  });
});
