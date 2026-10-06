import { describe, expect, it } from "vitest";
import baseCss from "@xyflow/react/dist/base.css?raw";
import appCss from "../styles/app.css?raw";
import tokensCss from "../styles/tokens.css?raw";

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

  it("imports base.css once, at the bootstrap, and style.css nowhere", () => {
    const imports = (path: string) => Array.from((sources[path] ?? "").matchAll(/import\s+["']([^"']+\.css)["']/g), (match) => match[1]);
    expect(imports("/src/main.tsx")).toContain("@xyflow/react/dist/base.css");
    for (const [path, text] of Object.entries(sources)) {
      expect([path, /@xyflow\/react\/dist\/style\.css/.test(text) && !path.endsWith(".test.ts")]).toEqual([path, false]);
    }
  });
});
