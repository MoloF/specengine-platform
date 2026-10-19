import { describe, expect, it } from "vitest";
import appCss from "./app.css?raw";
import tokensCss from "./tokens.css?raw";

// AC-09 and AC-10 of docs/features/ui-shell.md: the contrast pairs (WCAG 2.2 AA) of
// `ui/README.md` "Screen rules" on the resolved values, cannot-verify never ok's colour, the focus
// ring, reduced motion.

function declarations(block: string): Map<string, string> {
  const found = new Map<string, string>();
  for (const match of block.matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/g)) {
    const [, name, value] = match;
    if (name !== undefined && value !== undefined) {
      found.set(name, value.trim());
    }
  }
  return found;
}

const rootBlock = /:root\s*\{([^}]*)\}/.exec(tokensCss)?.[1] ?? "";
const motionBlock =
  /@media\s*\(prefers-reduced-motion:\s*reduce\)\s*\{\s*:root\s*\{([^}]*)\}/.exec(tokensCss)?.[1] ?? "";
const tokens = declarations(rootBlock);

function resolve(name: string, seen: string[] = []): string {
  const value = tokens.get(name);
  if (value === undefined) {
    throw new Error(`${name} is not defined in tokens.css`);
  }
  const alias = /^var\((--[a-z0-9-]+)\)$/.exec(value)?.[1];
  if (alias === undefined) {
    return value;
  }
  if (seen.includes(alias)) {
    throw new Error(`alias cycle through ${alias}`);
  }
  return resolve(alias, [...seen, name]);
}

function luminance(hex: string): number {
  const digits = /^#([0-9a-f]{6})$/i.exec(hex)?.[1];
  if (digits === undefined) {
    throw new Error(`not a six-digit sRGB hex colour: ${hex}`);
  }
  const [r, g, b] = [0, 2, 4].map((at) => {
    const channel = parseInt(digits.slice(at, at + 2), 16) / 255;
    return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * (r ?? 0) + 0.7152 * (g ?? 0) + 0.0722 * (b ?? 0);
}

function contrast(a: string, b: string): number {
  const [light, dark] = [luminance(resolve(a)), luminance(resolve(b))].sort((x, y) => y - x);
  return ((light ?? 0) + 0.05) / ((dark ?? 0) + 0.05);
}

const SURFACES = ["--surface-canvas", "--surface-panel", "--surface-raised", "--surface-overlay"];
const PROPOSAL = ["open", "changes-requested", "approved", "applied", "rejected", "deferred", "superseded", "unknown"];
const TASK = [
  "draft",
  "analysis",
  "review",
  "changes-requested",
  "ready",
  "in-progress",
  "in-review",
  "done",
  "accepted",
  "cancelled",
  "unknown",
];
const SYNC = [
  "ok",
  "spec-ahead",
  "code-ahead",
  "conflict",
  "broken-binding",
  "unbound",
  "predated",
  "cannot-verify",
];
const LINK = ["resolved", "dangling", "skipped", "unchecked", "unknown"];
// docs/features/ui-tasks.md: run outcomes and the staleness of a task's frozen spec.
const RUN = ["completed", "partial", "failed", "abandoned", "unknown"];
const STALE = ["changed", "unchanged", "unknown"];
const MARK = ["dangling-parent", "parent-cycle", "unknown"];
// docs/features/ui-health.md: the check's verdicts (the third spelled as its tone) and a finding's severities.
const CHECK = ["clean", "observed", "fails", "cannot-check", "unknown"];
const FINDING = ["error", "warning", "unknown"];
// docs/features/health-code-ui.md: row 4's states (`unknown` an item no queue holds, as `missing`) and row 5's reasons.
const ASSUMED = ["pending", "as-assumed", "otherwise", "applied", "rejected", "missing", "archived", "in-question", "unknown"];
const UNBOUND = ["homoglyph", "unanchored", "cannot-verify", "relation", "unknown"];
const STATUS = [
  ...["high", "normal", "low", "unknown"].map((value) => `--severity-${value}`),
  ...LINK.map((value) => `--link-${value}`),
  ...MARK.map((value) => `--mark-${value}`),
  "--highlight-line",
  ...PROPOSAL.map((value) => `--proposal-${value}`),
  ...TASK.map((value) => `--task-${value}`),
  ...RUN.map((value) => `--run-${value}`),
  ...STALE.map((value) => `--stale-${value}`),
  ...SYNC.map((value) => `--sync-${value}`),
  ...CHECK.map((value) => `--check-${value}`),
  ...FINDING.map((value) => `--finding-${value}`),
  ...ASSUMED.map((value) => `--assumed-${value}`),
  ...UNBOUND.map((value) => `--unbound-${value}`),
  "--attention",
  // The graph canvas (docs/features/ui-graph.md): edges and minimap nodes are read against the surfaces.
  "--graph-edge",
  "--graph-edge-emphasis",
  "--graph-minimap-node",
];

const pairs: [string, string, number][] = [
  ...["--text-primary", "--text-secondary", "--accent"].flatMap((text) =>
    SURFACES.map((surface): [string, string, number] => [text, surface, 4.5]),
  ),
  ["--text-primary", "--diff-added-surface", 4.5],
  ["--text-primary", "--diff-removed-surface", 4.5],
  ["--text-primary", "--highlight-surface", 4.5],
  ["--text-on-accent", "--accent", 4.5],
  ...["--focus-ring", "--border-control", ...STATUS].flatMap((token) =>
    SURFACES.map((surface): [string, string, number] => [token, surface, 3]),
  ),
];

describe("tokens.css (AC-09)", () => {
  it("defines every role of the slice", () => {
    const required = [
      ...SURFACES,
      "--border-subtle",
      "--border-control",
      "--focus-ring",
      "--text-primary",
      "--text-secondary",
      "--text-on-accent",
      "--accent",
      "--scrim",
      ...STATUS,
      "--diff-added-surface",
      "--diff-removed-surface",
      "--highlight-surface",
      ...[1, 2, 3, 4, 5, 6].map((step) => `--space-${String(step)}`),
      "--radius-s",
      "--radius-m",
      "--font-sans",
      "--font-mono",
      ...["s", "m", "l", "xl"].map((size) => `--font-size-${size}`),
      "--line-height-body",
      "--line-height-tight",
      "--duration-fast",
      "--duration-normal",
      "--z-header",
      "--z-dialog",
      "--target-min",
    ];
    expect(required.filter((name) => !tokens.has(name))).toEqual([]);
    expect(tokens.get("--target-min")).toBe("24px");
  });

  it.each(pairs)("%s on %s reaches %f:1", (foreground, background, minimum) => {
    expect(contrast(foreground, background)).toBeGreaterThanOrEqual(minimum);
  });

  it("never gives an unchecked or skipped link a resolved link's colour", () => {
    const resolved = resolve("--link-resolved").toLowerCase();
    for (const state of ["unchecked", "skipped", "dangling", "unknown"]) {
      expect(resolve(`--link-${state}`).toLowerCase()).not.toBe(resolved);
    }
  });

  it("never gives a task's unknown staleness the unchanged colour, nor a run's unknown outcome a completed one's", () => {
    expect(resolve("--stale-unknown").toLowerCase()).not.toBe(resolve("--stale-unchanged").toLowerCase());
    expect(resolve("--stale-unknown").toLowerCase()).toBe(resolve("--sync-cannot-verify").toLowerCase());
    expect(resolve("--stale-changed").toLowerCase()).not.toBe(resolve("--stale-unchanged").toLowerCase());
    expect(resolve("--run-unknown").toLowerCase()).not.toBe(resolve("--run-completed").toLowerCase());
  });

  it("draws a check that could not vouch for the corpus in cannot-verify's colour, never clean's or another verdict's (AC-03 of ui-health)", () => {
    const own = resolve("--check-cannot-check").toLowerCase();
    expect(own).toBe(resolve("--sync-cannot-verify").toLowerCase());
    for (const verdict of CHECK.filter((value) => value !== "cannot-check")) {
      expect([verdict, resolve(`--check-${verdict}`).toLowerCase() === own]).toEqual([verdict, false]);
    }
    expect(resolve("--check-clean").toLowerCase()).not.toBe(resolve("--check-fails").toLowerCase());
  });

  it("draws a marker that cannot be verified in cannot-verify's colour, and no reason a marker binds not in ok's (health-code-ui)", () => {
    expect(resolve("--unbound-cannot-verify").toLowerCase()).toBe(resolve("--sync-cannot-verify").toLowerCase());
    for (const reason of UNBOUND) {
      expect([reason, resolve(`--unbound-${reason}`).toLowerCase() === resolve("--sync-ok").toLowerCase()]).toEqual([reason, false]);
    }
    const unknown = resolve("--assumed-unknown").toLowerCase();
    for (const state of ASSUMED.filter((value) => value !== "unknown")) {
      expect([state, resolve(`--assumed-${state}`).toLowerCase() === unknown]).toEqual([state, false]);
    }
  });

  it("gives row 4's nine states nine colours, each at least 3:1 on every surface: otherwise never rejected's (UI-8 of health-ui-follow-up)", () => {
    expect(ASSUMED).toHaveLength(9);
    const colours = ASSUMED.map((state) => resolve(`--assumed-${state}`).toLowerCase());
    expect(new Set(colours).size).toBe(ASSUMED.length);
    expect(resolve("--assumed-otherwise").toLowerCase()).not.toBe(resolve("--assumed-rejected").toLowerCase());
    for (const state of ASSUMED) {
      for (const surface of SURFACES) {
        expect([state, surface, contrast(`--assumed-${state}`, surface) >= 3]).toEqual([state, surface, true]);
      }
    }
  });

  it("gives cannot-verify its own colour, never ok's or another sync state's", () => {
    const own = resolve("--sync-cannot-verify").toLowerCase();
    for (const state of SYNC.filter((value) => value !== "cannot-verify")) {
      expect(resolve(`--sync-${state}`).toLowerCase()).not.toBe(own);
    }
  });
});

describe("focus and motion (AC-10)", () => {
  it("draws a global :focus-visible ring from --focus-ring", () => {
    const rule = /(^|\n|,)\s*:focus-visible\s*\{([^}]*)\}/.exec(appCss)?.[2] ?? "";
    expect(rule).toMatch(/outline:\s*\d+px\s+solid\s+var\(--focus-ring\)/);
  });

  it("never removes the outline", () => {
    for (const css of [appCss, tokensCss]) {
      expect(css).not.toMatch(/outline(-style)?\s*:\s*(none|0)\b/);
    }
  });

  it("zeroes every duration under prefers-reduced-motion", () => {
    const durations = [...tokens.keys()].filter((name) => name.startsWith("--duration-"));
    expect(durations.length).toBeGreaterThan(0);
    const reduced = declarations(motionBlock);
    for (const name of durations) {
      expect(reduced.get(name)).toMatch(/^0(ms|s)?$/);
    }
  });

  it("animates and transitions only through the duration tokens", () => {
    for (const match of appCss.matchAll(/(transition|animation)\s*:([^;]+);/g)) {
      expect(match[2]).toMatch(/var\(--duration-/);
    }
  });
});
