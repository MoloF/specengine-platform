import { describe, expect, it } from "vitest";
import indexHtml from "../../index.html?raw";
import appCss from "./app.css?raw";
import tokensCss from "./tokens.css?raw";

// DF-9 and DF-12 of docs/features/design-foundation.md ("Data", "Contrast matrix", "Theme choice"),
// with AC-09 and AC-10 of docs/features/ui-shell.md: one block of roles, a colour role written once
// as `light-dark(<light>, <dark>)`, each side resolved per theme through its aliases; the matrix's
// pairs (WCAG 2.2 AA) in the light and the dark theme; today's distinctness checks per theme
// (cannot-verify never ok's colour, row 4's nine states, the four flow edges, the trace states);
// the theme's `color-scheme`; the focus ring and reduced motion.

type Theme = "light" | "dark";
const THEMES: readonly Theme[] = ["light", "dark"];

/** The text with its comments blanked, so a comment's words never read as a declaration. */
function uncommented(css: string): string {
  return css.replace(/\/\*[\s\S]*?\*\//g, "");
}

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

const plainTokens = uncommented(tokensCss);
const rootBlock = /(^|\n):root\s*\{([^}]*)\}/.exec(plainTokens)?.[2] ?? "";
const motionBlock = /@media\s*\(prefers-reduced-motion:\s*reduce\)\s*\{\s*:root\s*\{([^}]*)\}/.exec(plainTokens)?.[1] ?? "";
const tokens = declarations(rootBlock);

/** A value's arguments split at its top-level commas: a `light-dark()` whose light side holds commas of its own still gives two. */
function topLevelArguments(inner: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let start = 0;
  for (let at = 0; at < inner.length; at += 1) {
    const char = inner[at];
    if (char === "(") {
      depth += 1;
    } else if (char === ")") {
      depth -= 1;
    } else if (char === "," && depth === 0) {
      parts.push(inner.slice(start, at).trim());
      start = at + 1;
    }
  }
  parts.push(inner.slice(start).trim());
  return parts;
}

/** A `light-dark(L, D)` value's two sides, or null for any other value. */
function sidesOf(value: string): [string, string] | null {
  const inner = /^light-dark\(([\s\S]*)\)$/.exec(value)?.[1];
  if (inner === undefined) {
    return null;
  }
  const parts = topLevelArguments(inner);
  return parts.length === 2 ? [parts[0] ?? "", parts[1] ?? ""] : null;
}

/** A role's value in a theme, through `light-dark()` and its aliases. */
function resolve(name: string, theme: Theme, seen: string[] = []): string {
  const value = tokens.get(name);
  if (value === undefined) {
    throw new Error(`${name} is not defined in tokens.css`);
  }
  const sides = sidesOf(value);
  const own = sides === null ? value : theme === "light" ? sides[0] : sides[1];
  const alias = /^var\((--[a-z0-9-]+)\)$/.exec(own)?.[1];
  if (alias === undefined) {
    return own.toLowerCase();
  }
  if (seen.includes(alias)) {
    throw new Error(`alias cycle through ${alias}`);
  }
  return resolve(alias, theme, [...seen, name]);
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

function contrast(a: string, b: string, theme: Theme): number {
  const [light, dark] = [luminance(resolve(a, theme)), luminance(resolve(b, theme))].sort((x, y) => y - x);
  return ((light ?? 0) + 0.05) / ((dark ?? 0) + 0.05);
}

/** The five surfaces every status colour reads on. */
const SURFACES = ["--surface-canvas", "--surface-sunken", "--surface-panel", "--surface-raised", "--surface-overlay"];
/** The surfaces text reads on: the five, a hovered row and a chosen one ("Surfaces*" of the matrix). */
const TEXT_SURFACES = [...SURFACES, "--surface-hovered", "--surface-selected"];
const APPEARANCES = ["neutral", "information", "success", "warning", "danger", "discovery"];
const LOZENGE_SURFACES = APPEARANCES.map((appearance) => `--lozenge-${appearance}-surface`);
/** The roles that take an alpha channel, excepted by name from the six-digit rule. */
const ALPHA_ROLES = ["--scrim", "--shadow-key", "--shadow-ambient", "--graph-minimap-mask", "--graph-selection-surface"];

const PROPOSAL = ["open", "changes-requested", "approved", "applied", "rejected", "deferred", "superseded", "unknown"];
const TASK = ["draft", "analysis", "review", "changes-requested", "ready", "in-progress", "in-review", "done", "accepted", "cancelled", "unknown"];
const SYNC = [
  "ok",
  "spec-ahead",
  "code-ahead",
  "conflict",
  "broken-binding",
  "unbound",
  "predated",
  "cannot-verify",
  // docs/features/bundle-bindings.md: a binding row's two other states, a row not judged, an unknown word.
  "bad-marker",
  "unlocked",
  "not-judged",
  "unknown",
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
// docs/features/roadmap-screen.md: the Roadmap's marks, the range bar and its track; the item status is a
// neutral lozenge (docs/features/design-sweep-b.md "Acceptance criteria", DB4), no role of its own.
const ROADMAP = ["current", "cut", "cycle", "waits", "unestimated", "bar", "bar-track"];
// docs/features/flow-canvas.md: an edge's colour by its type's place, the gap rows' states, the walk's marks.
const FLOW_EDGE = ["0", "1", "2", "3"];
const FLOW_GAP = ["none", "found", "not-configured", "unknown"];
// docs/features/upstream-signal.md "Acceptance criteria", US-13: what changed above a subtask, the information lozenge's colour.
const CARD = ["accepted", "cancelled", "overdue", "drop", "suspect", "departed", "correcting", "mark-unknown", "open", "returned", "handed-off", "taken", "upstream"];
// docs/features/trace-views.md: a node's states against its cards.
const TRACE = ["confirmed", "in-work", "suspect", "departed", "no-card", "unknown"];
// docs/features/delivery-timeline.md "Acceptance criteria", DT11: the timeline's marks, read against the surfaces in both themes.
const DELIVERY = ["waiting", "work", "accepted", "due", "as-of", "first-taken", "last-accepted"];
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
  ...ROADMAP.map((value) => `--roadmap-${value}`),
  ...FLOW_EDGE.map((value) => `--flow-edge-${value}`),
  ...FLOW_GAP.map((value) => `--flow-gap-${value}`),
  ...CARD.map((value) => `--card-${value}`),
  ...TRACE.map((value) => `--trace-${value}`),
  ...DELIVERY.map((value) => `--delivery-${value}`),
  "--flow-start",
  "--flow-end",
  "--flow-marked",
  "--attention",
  // The graph canvas (docs/features/ui-graph.md): edges and minimap nodes are read against the surfaces.
  "--graph-edge",
  "--graph-edge-emphasis",
  "--graph-minimap-node",
];

/** "Data", "Contrast matrix": each foreground on each background, at its minimum. */
const MATRIX: [string, string, number][] = [
  ...["--text-primary", "--text-secondary", "--text-subtlest", "--accent"].flatMap((text) => TEXT_SURFACES.map((surface): [string, string, number] => [text, surface, 4.5])),
  ["--text-on-accent", "--accent", 4.5],
  ["--text-on-accent", "--accent-hovered", 4.5],
  ["--text-on-warning", "--warning-bold", 4.5],
  ["--text-on-danger", "--danger-bold", 4.5],
  ...APPEARANCES.map((appearance): [string, string, number] => [`--lozenge-${appearance}-text`, `--lozenge-${appearance}-surface`, 4.5]),
  ...["--text-primary", "--text-secondary"].flatMap((text) => LOZENGE_SURFACES.map((surface): [string, string, number] => [text, surface, 4.5])),
  ...["--diff-added-surface", "--diff-removed-surface", "--highlight-surface"].map((surface): [string, string, number] => ["--text-primary", surface, 4.5]),
  ...["--focus-ring", "--border-control"].flatMap((token) => TEXT_SURFACES.map((surface): [string, string, number] => [token, surface, 3])),
  ...STATUS.flatMap((token) => SURFACES.map((surface): [string, string, number] => [token, surface, 3])),
];

const PAIRS: [Theme, string, string, number][] = THEMES.flatMap((theme) => MATRIX.map(([foreground, background, minimum]): [Theme, string, string, number] => [theme, foreground, background, minimum]));

const HEX6 = /^#[0-9a-f]{6}$/i;
const ALIAS = /^var\(--[a-z0-9-]+\)$/;
const COLOUR_LITERAL = /#[0-9a-f]{3,8}\b|\b(rgb|rgba|hsl|hsla|oklch|color-mix)\(/i;

describe("tokens.css: one block of roles, two themes (DF-9)", () => {
  it("writes every role holding a colour literal as light-dark(L, D), each side a six-digit hex or a var(); the alpha roles by name", () => {
    const literal = [...tokens].filter(([, value]) => COLOUR_LITERAL.test(value));
    expect(literal.length).toBeGreaterThan(70);
    const wrong: string[] = [];
    for (const [name, value] of literal) {
      const sides = sidesOf(value);
      const sideOk = (side: string) => HEX6.test(side) || ALIAS.test(side) || (ALPHA_ROLES.includes(name) && (/^#[0-9a-f]{8}$/i.test(side) || /^rgba\([^()]*\)$/.test(side)));
      if (sides === null || !sides.every(sideOk)) {
        wrong.push(`${name}: ${value}`);
      }
    }
    expect(wrong).toEqual([]);
    // Each alpha role is one: excepted only where it holds alpha.
    for (const name of ALPHA_ROLES) {
      expect([name, sidesOf(tokens.get(name) ?? "") !== null]).toEqual([name, true]);
    }
  });

  it("declares light-dark( in tokens.css alone", () => {
    const sheets = import.meta.glob<string>("/src/**/*.css", { query: "?raw", import: "default", eager: true });
    expect(Object.keys(sheets)).toEqual(expect.arrayContaining(["/src/styles/app.css", "/src/styles/fonts.css", "/src/styles/tokens.css"]));
    const others = Object.entries(sheets).filter(([path, text]) => path !== "/src/styles/tokens.css" && text.includes("light-dark("));
    expect(others.map(([path]) => path)).toEqual([]);
  });

  it("defines every role, the slice's new ones too", () => {
    const required = [
      ...TEXT_SURFACES,
      "--border-subtle",
      "--border-control",
      "--focus-ring",
      "--text-primary",
      "--text-secondary",
      "--text-subtlest",
      "--text-on-accent",
      "--accent",
      "--accent-hovered",
      "--warning-bold",
      "--text-on-warning",
      "--danger-bold",
      "--text-on-danger",
      ...APPEARANCES.flatMap((appearance) => [`--lozenge-${appearance}-surface`, `--lozenge-${appearance}-text`]),
      "--scrim",
      "--shadow-key",
      "--shadow-ambient",
      "--shadow-raised",
      "--shadow-overlay",
      ...STATUS,
      "--diff-added-surface",
      "--diff-removed-surface",
      "--highlight-surface",
      ...["050", "100", "150", "200", "300", "400"].map((step) => `--space-${step}`),
      "--radius-s",
      "--radius-m",
      "--font-sans",
      "--font-mono",
      ...["body", "heading-s", "heading-m", "heading-l"].map((step) => `--font-size-${step}`),
      "--line-height-body",
      "--duration-fast",
      "--duration-normal",
      "--z-header",
      "--z-dialog",
      "--target-min",
    ];
    expect(required.filter((name) => !tokens.has(name))).toEqual([]);
    expect(tokens.get("--target-min")).toBe("24px");
    expect(tokens.get("--shadow-raised")).toBe("0 1px 1px var(--shadow-key), 0 0 1px var(--shadow-ambient)");
    expect(tokens.get("--shadow-overlay")).toBe("0 8px 12px var(--shadow-key), 0 0 1px var(--shadow-ambient)");
  });

  it.each(PAIRS)("%s: %s on %s reaches %f:1", (theme, foreground, background, minimum) => {
    expect(contrast(foreground, background, theme)).toBeGreaterThanOrEqual(minimum);
  });

  it("tells the accent from an in-progress lozenge's information blue, in both themes (W3)", () => {
    for (const theme of THEMES) {
      expect([theme, resolve("--accent", theme) === resolve("--lozenge-information-text", theme)]).toEqual([theme, false]);
    }
  });
});

describe("the theme's colour scheme (DF-12)", () => {
  it("lets the page follow the system in :root, and pins light and dark under data-theme", () => {
    expect(/color-scheme:\s*([^;]+);/.exec(rootBlock)?.[1]?.trim()).toBe("light dark");
    expect(/:root\[data-theme="light"\]\s*\{\s*color-scheme:\s*light;\s*\}/.test(plainTokens)).toBe(true);
    expect(/:root\[data-theme="dark"\]\s*\{\s*color-scheme:\s*dark;\s*\}/.test(plainTokens)).toBe(true);
  });

  it("names both schemes in index.html's meta, so the page before the entry module follows the system", () => {
    expect(/<meta name="color-scheme" content="light dark" \/>/.test(indexHtml)).toBe(true);
  });
});

describe.each(THEMES)("today's distinctness in the %s theme (AC-09)", (theme) => {
  const shade = (name: string) => resolve(name, theme);

  it("never gives an unchecked or skipped link a resolved link's colour", () => {
    const resolved = shade("--link-resolved");
    for (const state of ["unchecked", "skipped", "dangling", "unknown"]) {
      expect(shade(`--link-${state}`)).not.toBe(resolved);
    }
  });

  it("never gives a task's unknown staleness the unchanged colour, nor a run's unknown outcome a completed one's", () => {
    expect(shade("--stale-unknown")).not.toBe(shade("--stale-unchanged"));
    expect(shade("--stale-unknown")).toBe(shade("--sync-cannot-verify"));
    expect(shade("--stale-changed")).not.toBe(shade("--stale-unchanged"));
    expect(shade("--run-unknown")).not.toBe(shade("--run-completed"));
  });

  it("draws a check that could not vouch for the corpus in cannot-verify's colour, never clean's or another verdict's (AC-03 of ui-health)", () => {
    const own = shade("--check-cannot-check");
    expect(own).toBe(shade("--sync-cannot-verify"));
    for (const verdict of CHECK.filter((value) => value !== "cannot-check")) {
      expect([verdict, shade(`--check-${verdict}`) === own]).toEqual([verdict, false]);
    }
    expect(shade("--check-clean")).not.toBe(shade("--check-fails"));
  });

  it("draws a marker that cannot be verified in cannot-verify's colour, and no reason a marker binds not in ok's (health-code-ui)", () => {
    expect(shade("--unbound-cannot-verify")).toBe(shade("--sync-cannot-verify"));
    for (const reason of UNBOUND) {
      expect([reason, shade(`--unbound-${reason}`) === shade("--sync-ok")]).toEqual([reason, false]);
    }
    const unknown = shade("--assumed-unknown");
    for (const state of ASSUMED.filter((value) => value !== "unknown")) {
      expect([state, shade(`--assumed-${state}`) === unknown]).toEqual([state, false]);
    }
  });

  it("gives row 4's nine states nine colours, each at least 3:1 on every surface: otherwise never rejected's (UI-8 of health-ui-follow-up)", () => {
    expect(ASSUMED).toHaveLength(9);
    const colours = ASSUMED.map((state) => shade(`--assumed-${state}`));
    expect(new Set(colours).size).toBe(ASSUMED.length);
    expect(shade("--assumed-otherwise")).not.toBe(shade("--assumed-rejected"));
    for (const state of ASSUMED) {
      for (const surface of SURFACES) {
        expect([state, surface, contrast(`--assumed-${state}`, surface, theme) >= 3]).toEqual([state, surface, true]);
      }
    }
  });

  it("gives the Roadmap's five marks five colours, each at least 3:1 on every surface, the item status no role (roadmap-screen; DB4 of design-sweep-b)", () => {
    const marks = ["current", "cut", "cycle", "waits", "unestimated"].map((mark) => shade(`--roadmap-${mark}`));
    expect(new Set(marks).size).toBe(5);
    expect(tokensCss).not.toContain("--roadmap-status");
    for (const role of ROADMAP) {
      for (const surface of SURFACES) {
        expect([role, surface, contrast(`--roadmap-${role}`, surface, theme) >= 3]).toEqual([role, surface, true]);
      }
    }
  });

  // docs/features/handoff-progress.md "Acceptance criteria", HP-3: a parent's progress bar, beside its words line.
  it("fills a parent's progress bar with the lozenges' text roles, each at least 3:1 on every surface, no new role (HP-3)", () => {
    for (const appearance of ["success", "neutral", "warning", "information"]) {
      for (const surface of SURFACES) {
        expect([appearance, surface, contrast(`--lozenge-${appearance}-text`, surface, theme) >= 3]).toEqual([appearance, surface, true]);
      }
    }
  });

  // docs/features/delivery-timeline.md "Acceptance criteria", DT11: a row's waiting, work and accept apart, the day asked none of them, no mark in an alarm's colour.
  it("draws the timeline's waiting, work, accept and day asked in four colours, each at least 3:1 on every surface, none the danger's (DT11 of delivery-timeline)", () => {
    const marks = ["waiting", "work", "accepted", "as-of"].map((mark) => shade(`--delivery-${mark}`));
    expect(new Set(marks).size).toBe(4);
    for (const role of DELIVERY) {
      expect([role, shade(`--delivery-${role}`) === shade("--danger-bold")]).toEqual([role, false]);
      for (const surface of SURFACES) {
        expect([role, surface, contrast(`--delivery-${role}`, surface, theme) >= 3]).toEqual([role, surface, true]);
      }
    }
  });

  it("gives the flow's edge types four colours, each at least 3:1 on every surface, none a state's (AC-06 of flow-canvas)", () => {
    const colours = FLOW_EDGE.map((value) => shade(`--flow-edge-${value}`));
    expect(new Set(colours).size).toBe(4);
    for (const value of FLOW_EDGE) {
      for (const surface of [...SURFACES, "--flow-box-surface"]) {
        expect([value, surface, contrast(`--flow-edge-${value}`, surface, theme) >= 3]).toEqual([value, surface, true]);
      }
    }
    const states = ["--sync-ok", "--sync-conflict", "--sync-cannot-verify", "--attention"].map(shade);
    expect(colours.filter((colour) => states.includes(colour))).toEqual([]);
  });

  it("gives each state of a node its own colour, at least 3:1 on the Flow's and the panels' surfaces, none a sync colour of a check that passed or cannot verify (trace-views)", () => {
    const states = TRACE.filter((state) => state !== "unknown").map((state) => shade(`--trace-${state}`));
    expect(new Set(states).size).toBe(states.length);
    expect(states).not.toContain(shade("--sync-ok"));
    expect(states).not.toContain(shade("--sync-cannot-verify"));
    for (const state of TRACE) {
      for (const surface of [...SURFACES, "--flow-box-surface"]) {
        expect([state, surface, contrast(`--trace-${state}`, surface, theme) >= 3]).toEqual([state, surface, true]);
      }
    }
  });

  it("keeps a gap row that cannot be measured off None's colour, and None's off cannot-verify's (AC-08 of flow-canvas)", () => {
    expect(shade("--flow-gap-none")).not.toBe(shade("--sync-cannot-verify"));
    for (const state of FLOW_GAP.filter((value) => value !== "none")) {
      expect([state, shade(`--flow-gap-${state}`) === shade("--flow-gap-none")]).toEqual([state, false]);
    }
    expect(contrast("--text-primary", "--flow-box-surface", theme)).toBeGreaterThanOrEqual(4.5);
    expect(contrast("--text-secondary", "--flow-box-surface", theme)).toBeGreaterThanOrEqual(4.5);
  });

  it("never gives a binding row ok's colour unless it is in sync, nor one not judged cannot-verify's (bundle-bindings)", () => {
    const ok = shade("--sync-ok");
    for (const state of SYNC.filter((value) => value !== "ok")) {
      expect([state, shade(`--sync-${state}`) === ok]).toEqual([state, false]);
    }
    expect(shade("--sync-not-judged")).not.toBe(shade("--sync-cannot-verify"));
    expect(shade("--sync-not-judged")).toBe(shade("--severity-unknown"));
  });

  it("gives cannot-verify its own colour, never ok's or another sync state's", () => {
    const own = shade("--sync-cannot-verify");
    for (const state of SYNC.filter((value) => value !== "cannot-verify")) {
      expect(shade(`--sync-${state}`)).not.toBe(own);
    }
  });
});

describe("the Board's and the trace's tones (trace-views)", () => {
  it("gives a tone class to every state of a node, an unknown word dashed", () => {
    for (const state of TRACE) {
      expect(appCss).toContain(`.tone-trace-${state} { --badge-color: var(--trace-${state}); }`);
    }
    expect(/\.tone-trace-unknown,[^{]*\{\s*border-style: dashed;/.test(appCss)).toBe(true);
  });

  it("sizes the Flow studio's panes: places 15rem, panel 22rem, the canvas 20rem at least; nothing dims (writer-studio D6, D7)", () => {
    expect([tokens.get("--flow-places-width"), tokens.get("--flow-panel-width"), tokens.get("--flow-canvas-min")]).toEqual(["15rem", "22rem", "20rem"]);
    expect(tokens.has("--flow-dimmed")).toBe(false);
    // No frame is drawn: a group is entered, never opened in place (D1).
    expect([...tokens.keys()].filter((name) => name.startsWith("--flow-frame"))).toEqual([]);
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

  it("never animates a frame or a box of the Flow (AC-13 of flow-canvas)", () => {
    const rules = [...appCss.matchAll(/([^{}]*\.flow-[^{}]*)\{([^}]*)\}/g)];
    expect(rules.length).toBeGreaterThan(10);
    for (const [, selector, body] of rules) {
      expect([selector?.trim(), /transition|animation/.test(body ?? "")]).toEqual([selector?.trim(), false]);
    }
  });

  it("animates and transitions only through the duration tokens", () => {
    for (const match of appCss.matchAll(/(transition|animation)\s*:([^;]+);/g)) {
      expect(match[2]).toMatch(/var\(--duration-/);
    }
  });
});
