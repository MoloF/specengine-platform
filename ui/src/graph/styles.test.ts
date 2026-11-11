import { describe, expect, it } from "vitest";
import baseCss from "@xyflow/react/dist/base.css?raw";
import appCss from "../styles/app.css?raw";
import tokensCss from "../styles/tokens.css?raw";
import graphCanvas from "./GraphCanvas.tsx?raw";

// AC-12 of docs/features/ui-graph.md: React Flow's base.css only, never style.css; each
// `--xy-<name>-default` base.css declares has `--xy-<name>` set to a tokens.css role in app.css;
// the edge labels drawn above the boxes.

const sources = import.meta.glob<string>("/src/**/*.{ts,tsx,css}", { query: "?raw", import: "default", eager: true });

const defaults = [...new Set(Array.from(baseCss.matchAll(/--xy-([a-z0-9-]+)-default\s*:/g), (match) => match[1] ?? ""))];

describe("React Flow's styles (AC-12)", () => {
  it("reads the variables base.css declares", () => {
    expect(defaults.length).toBeGreaterThanOrEqual(24);
    expect(defaults).toContain("edge-stroke");
    expect(defaults).toContain("node-border");
  });

  it.each(defaults)("sets --xy-%s from a tokens.css role", (name) => {
    const value = new RegExp(`--xy-${name}\\s*:\\s*var\\((--[a-z0-9-]+)\\)\\s*;`).exec(appCss)?.[1];
    expect(value).toBeDefined();
    expect(tokensCss).toMatch(new RegExp(`${value ?? "--none"}\\s*:`));
  });

  it("lifts the edge labels above the boxes (React Flow puts its label layer before the nodes, at z-index 0)", () => {
    const rule = /\.graph-canvas \.react-flow__edgelabel-renderer\s*\{([^}]*)\}/.exec(appCss)?.[1] ?? "";
    expect(Number(/z-index:\s*(\d+)/.exec(rule)?.[1] ?? "0")).toBeGreaterThanOrEqual(1);
  });

  // design-foundation ("Themes"): the canvases follow the page's theme through these roles, so none
  // pins React Flow's own colour mode, and every role an `--xy-*` takes resolves in both themes.
  it("sets no colorMode on a canvas, and maps every --xy-* to a role that resolves in both themes (design-foundation)", () => {
    const canvases = ["/src/graph/GraphCanvas.tsx", "/src/flow/FlowCanvas.tsx", "/src/roadmap/RoadmapChain.tsx"];
    for (const path of canvases) {
      expect([path, (sources[path] ?? "").length > 0, /colorMode/.test(sources[path] ?? "")]).toEqual([path, true, false]);
    }
    const root = /(^|\n):root\s*\{([^}]*)\}/.exec(tokensCss.replace(/\/\*[\s\S]*?\*\//g, ""))?.[2] ?? "";
    const roles = new Map(Array.from(root.matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/g), (match): [string, string] => [match[1] ?? "", (match[2] ?? "").trim()]));
    /** Every role a value names, through its aliases: each must be defined, the colours as light-dark(). */
    const reached = (value: string, seen: string[] = []): string[] =>
      Array.from(value.matchAll(/var\((--[a-z0-9-]+)\)/g), (match) => match[1] ?? "").flatMap((name) => (seen.includes(name) ? [name] : [name, ...reached(roles.get(name) ?? "", [...seen, name])]));
    for (const name of defaults) {
      const role = new RegExp(`--xy-${name}\\s*:\\s*(var\\([^;]+\\))\\s*;`).exec(appCss)?.[1] ?? "";
      const chain = reached(role);
      expect([name, chain.length > 0, chain.every((step) => roles.has(step))]).toEqual([name, true, true]);
      const colours = chain.map((step) => roles.get(step) ?? "").filter((value) => /#[0-9a-f]{3,8}\b|\brgba?\(/i.test(value));
      expect([name, colours.every((value) => value.startsWith("light-dark("))]).toEqual([name, true]);
    }
  });

  it("imports base.css once, at the bootstrap, and style.css nowhere", () => {
    const imports = (path: string) => Array.from((sources[path] ?? "").matchAll(/import\s+["']([^"']+\.css)["']/g), (match) => match[1]);
    expect(imports("/src/main.tsx")).toContain("@xyflow/react/dist/base.css");
    for (const [path, text] of Object.entries(sources)) {
      expect([path, /@xyflow\/react\/dist\/style\.css/.test(text) && !path.endsWith(".test.ts")]).toEqual([path, false]);
    }
  });
});

/** The file's top-level rules, comments dropped: an at-rule's block (`@media` and the like) and the rules inside it left out. */
function topLevelRules(css: string): { selector: string; body: string }[] {
  const text = css.replace(/\/\*[\s\S]*?\*\//g, "");
  const found: { selector: string; body: string }[] = [];
  let at = 0;
  while (at < text.length) {
    const open = text.indexOf("{", at);
    if (open < 0) {
      break;
    }
    const head = text.slice(at, open).trim();
    let depth = 1;
    let close = open + 1;
    for (; close < text.length && depth > 0; close += 1) {
      depth += text[close] === "{" ? 1 : text[close] === "}" ? -1 : 0;
    }
    if (!head.startsWith("@")) {
      found.push({ selector: head, body: text.slice(open + 1, close - 1) });
    }
    at = close;
  }
  return found;
}

// docs/features/canvas-clicks.md "Acceptance criteria", CK4: no canvas's minimap pans or zooms, so the
// rule every canvas wrapper (`.graph-canvas`) shares lets the pointer through to the box under it.
describe("the minimap takes no pointer (CK4)", () => {
  it("holds `pointer-events: none` in the one top-level `.graph-canvas .react-flow__minimap` rule; Graph's minimap neither pans nor zooms", () => {
    const rules = topLevelRules(appCss).filter((rule) => rule.selector.split(",").some((part) => part.trim() === ".graph-canvas .react-flow__minimap"));
    expect(rules).toHaveLength(1);
    // M: the declaration dropped.
    expect(rules[0]?.body).toMatch(/(^|;)\s*pointer-events:\s*none;/);
    expect(graphCanvas).toContain("<MiniMap pannable={false} zoomable={false} />");
  });

  it("puts every canvas inside a `.graph-canvas` wrapper, so the rule reaches each", () => {
    for (const path of ["/src/graph/GraphCanvas.tsx", "/src/flow/FlowCanvas.tsx", "/src/roadmap/RoadmapChain.tsx"]) {
      expect([path, /className="graph-canvas( [^"]*)?"/.test(sources[path] ?? "")]).toEqual([path, true]);
    }
  });
});
