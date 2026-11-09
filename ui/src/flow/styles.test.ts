import { describe, expect, it } from "vitest";
import appCss from "../styles/app.css?raw";
import tokensCss from "../styles/tokens.css?raw";

// docs/features/writer-studio.md ST-4, ST-8, ST-11 (and ST-6's no dimming): jsdom lays nothing
// out, so these pin the studio's rules, as the Board's styles.test.ts does; the looks L1 to L4 hold
// them to a browser.

/** The file's top-level parts: each rule, and each `@media` block with the rules inside it. */
interface Block {
  media: string | null;
  selector: string;
  body: string;
}

function blocksOf(css: string): Block[] {
  const found: Block[] = [];
  const text = css.replace(/\/\*[\s\S]*?\*\//g, "");
  let at = 0;
  const walk = (until: number, media: string | null) => {
    while (at < until) {
      const open = text.indexOf("{", at);
      if (open < 0 || open >= until) {
        at = until;
        return;
      }
      const head = text.slice(at, open).trim();
      if (head.startsWith("@media")) {
        let depth = 1;
        let close = open + 1;
        for (; close < text.length && depth > 0; close += 1) {
          depth += text[close] === "{" ? 1 : text[close] === "}" ? -1 : 0;
        }
        at = open + 1;
        walk(close - 1, head.slice("@media".length).trim());
        at = close;
      } else {
        const close = text.indexOf("}", open);
        found.push({ media, selector: head, body: text.slice(open + 1, close) });
        at = close + 1;
      }
    }
  };
  walk(text.length, null);
  return found;
}

const blocks = blocksOf(appCss);

/** The rules naming exactly `selector`, with their media. */
function rulesOf(selector: string): Block[] {
  return blocks.filter((block) => block.selector.split(",").some((part) => part.trim() === selector));
}

function ruleWith(selector: string, media: string | null, declaration: RegExp): Block {
  const rule = rulesOf(selector).find((block) => block.media === media && declaration.test(block.body));
  if (rule === undefined) {
    throw new Error(`no ${selector} rule${media === null ? "" : ` under ${media}`} holds ${declaration.source}`);
  }
  return rule;
}

const WIDE = "(min-width: 64rem)";
const WIDEST = "(min-width: 80rem)";
const STACKED = "(max-width: 63.99rem)";

describe("the page holds still (ST-4)", () => {
  it("fills the viewport less the shell above 64rem, measured, and hides its overflow", () => {
    const studio = ruleWith(".flow-studio", WIDE, /height:/);
    expect(studio.body).toMatch(/height:\s*calc\(100dvh - var\(--flow-studio-top[^)]*\) - var\(--space-6\)\);/);
    expect(studio.body).toMatch(/overflow:\s*hidden;/);
  });

  it("scrolls the places and the panel inside, the panel at its width", () => {
    for (const pane of [".flow-places", ".flow-panel"]) {
      const rule = ruleWith(pane, null, /overflow:\s*auto;/);
      expect([pane, /min-height:\s*0;/.test(rule.body)]).toEqual([pane, true]);
    }
    expect(ruleWith(".flow-panel", WIDE, /width:/).body).toMatch(/width:\s*var\(--flow-panel-width\);/);
  });

  it("positions every pane that scrolls, and the studio that clips them: a visually hidden label inside adds nothing to the page's scroll", () => {
    const positioned = (selector: string, media: string | null) =>
      rulesOf(selector).some((block) => (block.media === null || block.media === media) && /position:\s*(relative|absolute|sticky|fixed);/.test(block.body));
    const scrolling = blocks.filter((block) => /\.flow-/.test(block.selector) && /overflow(-[xy])?:\s*(auto|scroll);/.test(block.body));
    expect(scrolling.map((block) => block.selector)).toEqual(expect.arrayContaining([".flow-panel", ".flow-places", ".flow-legend", ".flow-view-area > .tab-panel", ".flow-places.is-open .flow-places-body"]));
    for (const block of scrolling) {
      for (const part of block.selector.split(",").map((one) => one.trim())) {
        expect([part, block.media, positioned(part, block.media)]).toEqual([part, block.media, true]);
      }
    }
    for (const part of [".flow-studio", ".flow-list-region", ".flow-list", ".flow-canvas-region", ".flow-places-body"]) {
      expect([part, positioned(part, null)]).toEqual([part, true]);
    }
    expect(rulesOf(".flow-studio").some((block) => /position:\s*static/.test(block.body))).toBe(false);
  });

  it("lets the shell's body take its content's height on Flow above 64rem, a wrapped header measured; other screens keep their floor", () => {
    expect(ruleWith(".app-body:has(.flow-studio)", WIDE, /min-height:/).body).toMatch(/min-height:\s*0;/);
    expect(ruleWith(".app-body", null, /min-height:/).body).toMatch(/min-height:\s*calc\(100vh - 3\.5rem\);/);
  });

  it("holds no clamp, no 68vh and no align-items: start in any rule of the Flow (M10)", () => {
    const flow = blocks.filter((block) => /\.flow-/.test(block.selector));
    expect(flow.length).toBeGreaterThan(40);
    for (const block of flow) {
      expect([block.selector, /clamp\(|68vh|align-items:\s*start/.test(block.body)]).toEqual([block.selector, false]);
    }
  });

  it("dims nothing: no `--flow-dimmed` in either style file (ST-6, M13)", () => {
    expect(appCss).not.toContain("--flow-dimmed");
    expect(tokensCss).not.toContain("--flow-dimmed");
    expect(blocks.some((block) => /\.is-dimmed/.test(block.selector))).toBe(false);
  });
});

describe("the legend at the bottom (ST-8)", () => {
  it("makes the bar the centre's last row", () => {
    const centre = ruleWith(".flow-centre", null, /grid-template-rows:/);
    expect(centre.body).toMatch(/grid-template-rows:\s*minmax\(0, 1fr\) auto;/);
    expect(centre.body).toMatch(/position:\s*relative;/);
  });

  it("lays the open legend over the canvas from its bottom edge, at most half its height, scrolling inside (M17)", () => {
    const legend = ruleWith(".flow-legend", null, /position:/);
    expect(legend.body).toMatch(/position:\s*absolute;/);
    expect(legend.body).toMatch(/bottom:\s*0;/);
    expect(legend.body).toMatch(/max-height:\s*50%;/);
    expect(legend.body).toMatch(/overflow:\s*auto;/);
    expect(ruleWith(".flow-view-area", null, /position:/).body).toMatch(/position:\s*relative;/);
  });

  it("keeps the bar to the viewport's foot under 64rem, scroll padding its height (R10)", () => {
    const bar = ruleWith(".flow-bar", STACKED, /position:/);
    expect(bar.body).toMatch(/position:\s*sticky;/);
    expect(bar.body).toMatch(/bottom:\s*0;/);
    expect(ruleWith("html:has(.flow-bar)", STACKED, /scroll-padding-bottom:/).body).toMatch(/scroll-padding-bottom:\s*3rem;/);
  });
});

describe("narrow widths (ST-11)", () => {
  it("stacks one column under 64rem: header, places, canvas, panel; the places a closed disclosure", () => {
    const studio = ruleWith(".flow-studio", null, /grid-template-areas:/);
    expect(studio.body).toMatch(/grid-template-columns:\s*minmax\(0, 1fr\);/);
    expect(studio.body).toMatch(/grid-template-areas:\s*"head" "places" "centre" "panel";/);
    expect(ruleWith(".flow-places:not(.is-open) .flow-places-body", null, /display:/).body).toMatch(/display:\s*none;/);
  });

  it("from 64 to 80rem stands the canvas and the panel side by side, the places a strip whose pane lies over the canvas", () => {
    const studio = ruleWith(".flow-studio", WIDE, /grid-template-columns:/);
    expect(studio.body).toMatch(/grid-template-columns:\s*minmax\(var\(--flow-canvas-min\), 1fr\) var\(--flow-panel-width\);/);
    expect(studio.body).toMatch(/grid-template-areas:\s*"head head" "places places" "centre panel";/);
    const open = ruleWith(".flow-places.is-open .flow-places-body", WIDE, /position:/);
    expect(open.body).toMatch(/position:\s*absolute;/);
    expect(open.body).toMatch(/width:\s*var\(--flow-places-width\);/);
  });

  it("from 80rem shows the three panes, places fixed, the toggle gone", () => {
    const studio = ruleWith(".flow-studio", WIDEST, /grid-template-columns:/);
    expect(studio.body).toMatch(/grid-template-columns:\s*var\(--flow-places-width\) minmax\(var\(--flow-canvas-min\), 1fr\) var\(--flow-panel-width\);/);
    expect(ruleWith(".flow-places-toggle", WIDEST, /display:/).body).toMatch(/display:\s*none;/);
  });

  it("gives the places no fixed width under 64rem (M23)", () => {
    for (const block of blocks.filter((candidate) => /\.flow-places/.test(candidate.selector) && candidate.media !== WIDE && candidate.media !== WIDEST)) {
      expect([block.selector, /(^|[^-])width:\s*var\(--flow-places-width\)/.test(block.body)]).toEqual([block.selector, false]);
    }
  });
});
