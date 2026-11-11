import { describe, expect, it } from "vitest";
import { SCALE_EXCEPTIONS } from "../styles/scale-exceptions";
import appCss from "../styles/app.css?raw";
import tokensCss from "../styles/tokens.css?raw";
import flowCanvas from "./FlowCanvas.tsx?raw";
import graphCanvas from "../graph/GraphCanvas.tsx?raw";
import { FLOW_FIT_MIN_ZOOM, FLOW_PINCH_MIN_ZOOM } from "./geometry";
import { LABEL_WIDTH } from "./layout";

// docs/features/writer-studio.md ST-4, ST-8, ST-11 (and ST-6's no dimming); flow-scenario FS-14 and
// FS-22 (RM-0094): jsdom lays nothing out, so these pin the studio's rules, as the Board's
// styles.test.ts does; the looks hold them to a browser.

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
    expect(studio.body).toMatch(/height:\s*calc\(100dvh - var\(--flow-studio-top[^)]*\) - var\(--space-400\)\);/);
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

/** A token's px, by its rem value in tokens.css. */
function tokenPx(name: string): number {
  const value = new RegExp(`${name}:\\s*([0-9.]+)rem;`).exec(tokensCss)?.[1];
  return Number(value) * 16;
}

describe("readable boxes (FS-14)", () => {
  it("sets a box's title at the body size, whose px at Flow's fit floor is 12 or more; every font size of the box a token", () => {
    expect(ruleWith(".flow-box-title", null, /font-size:/).body).toMatch(/font-size:\s*var\(--font-size-body\);/);
    expect(FLOW_FIT_MIN_ZOOM).toBe(0.92);
    expect(tokenPx("--font-size-body") * FLOW_FIT_MIN_ZOOM).toBeGreaterThanOrEqual(12);
    const sizes = blocks.filter((block) => /\.flow-(box|tab|edge-label|band|divider)/.test(block.selector)).flatMap((block) => Array.from(block.body.matchAll(/font-size:\s*([^;]+);/g), (match) => `${block.selector} ${match[1] ?? ""}`));
    expect(sizes.length).toBeGreaterThan(5);
    expect(sizes.filter((size) => !/ var\(--font-size-(small|body)\)$/.test(size))).toEqual([]);
  });

  it("gives Flow its own pinch floor, 0.5, and leaves Graph its 0.1; Flow takes no floor of Graph's", () => {
    expect(FLOW_PINCH_MIN_ZOOM).toBe(0.5);
    expect(flowCanvas).toContain("minZoom={FLOW_PINCH_MIN_ZOOM}");
    expect(graphCanvas).toContain("minZoom={0.1}");
    const flowSources = import.meta.glob<string>(["/src/flow/*.ts", "/src/flow/*.tsx", "!/src/flow/*.test.*"], { query: "?raw", import: "default", eager: true });
    expect(Object.keys(flowSources).length).toBeGreaterThan(8);
    for (const [path, text] of Object.entries(flowSources)) {
      expect([path, /\bFIT_MIN_ZOOM\b/.test(text.replace(/FLOW_FIT_MIN_ZOOM/g, ""))]).toEqual([path, false]);
    }
  });

  it("names no scale exception for RM-0094: Flow's sizes are the scale's", () => {
    expect(SCALE_EXCEPTIONS.filter((exception) => (exception.until as string | null) === "RM-0094" || /\.flow-/.test(exception.selector))).toEqual([]);
  });
});

describe("the path pane (FS-22)", () => {
  it("keeps a group row's chevron on its title's line: no wrap, the words taking the room, the icon none", () => {
    const row = ruleWith(".flow-places-body a.flow-places-group", null, /flex-wrap:/);
    expect(row.body).toMatch(/flex-wrap:\s*nowrap;/);
    expect(row.body).toMatch(/align-items:\s*flex-start;/);
    const words = ruleWith(".flow-places-group .flow-places-words", null, /flex:/);
    expect(words.body).toMatch(/flex:\s*1 1 0;/);
    expect(words.body).toMatch(/min-width:\s*0;/);
    expect(ruleWith(".flow-places-group .icon", null, /flex:/).body).toMatch(/flex:\s*none;/);
  });

  it("indents the path one step a level, the same token step each, six at most, the groups under it one more", () => {
    for (const depth of [1, 2, 3, 4, 5, 6]) {
      const rule = ruleWith(`.flow-path li[data-depth="${String(depth)}"]`, null, /margin-left:/);
      expect(rule.selector).toContain(`.flow-places-groups[data-depth="${String(depth)}"]`);
      const step = depth === 1 ? "var(--space-150)" : `calc(var(--space-150) * ${String(depth)})`;
      expect([depth, /margin-left:\s*([^;]+);/.exec(rule.body)?.[1]]).toEqual([depth, step]);
    }
    expect(blocks.some((block) => block.selector.includes('data-depth="7"'))).toBe(false);
  });

  it("lays the Gaps control over the canvas as the legend lies, from its bottom edge, at most half its height", () => {
    const gaps = ruleWith(".flow-gaps", null, /position:/);
    expect(gaps.body).toMatch(/position:\s*absolute;/);
    expect(gaps.body).toMatch(/bottom:\s*0;/);
    expect(gaps.body).toMatch(/max-height:\s*50%;/);
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

// docs/features/handoff-progress.md "Acceptance criteria", C-2: the minimap takes no pointer, so a
// box under it takes its click; it neither pans nor zooms, so it loses nothing.
describe("the minimap (C-2)", () => {
  it("lets every pointer through: `.flow-canvas .react-flow__minimap` holds `pointer-events: none`", () => {
    // M: the declaration dropped.
    expect(ruleWith(".flow-canvas .react-flow__minimap", null, /pointer-events:\s*none;/).body).toMatch(/pointer-events:\s*none;/);
    expect(flowCanvas).toContain("pannable={false} zoomable={false}");
  });

  it("draws a divider's rule down its left side, its words two lines at most, then an ellipsis (F-3)", () => {
    expect(ruleWith(".flow-divider", null, /border-left:/).body).toMatch(/border-left:\s*1px dashed var\(--border-control\);/);
    expect(ruleWith(".flow-divider-words", null, /-webkit-line-clamp:\s*2;/).body).toMatch(/overflow:\s*hidden;/);
  });
});

// The orchestrator's look L1 of handoff-progress: an edge's words wrap within its gap, two lines at most.
describe("an edge's words in its gap (L1 of the looks)", () => {
  it("caps a label at the gap less its insets, two lines, then an ellipsis; each word a line", () => {
    const label = ruleWith(".flow-edge-label", null, /max-width:/).body;
    expect(label).toMatch(/max-width:\s*var\(--flow-edge-label-width\);/);
    expect(label).toMatch(/-webkit-line-clamp:\s*2;/);
    expect(label).toMatch(/overflow:\s*hidden;/);
    expect(tokensCss).toMatch(new RegExp(`--flow-edge-label-width:\\s*${String(LABEL_WIDTH / 16)}rem;`));
    expect(ruleWith(".flow-edge-word", null, /display:/).body).toMatch(/display:\s*block;/);
  });
});
