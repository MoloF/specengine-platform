import { describe, expect, it } from "vitest";
import packageJson from "../package.json?raw";
import eslintConfig from "../eslint.config.js?raw";
import viteConfig from "../vite.config.ts?raw";

// Source-wide rules of docs/features/ui-shell.md, read through Vite's ?raw (no Node API):
// AC-04 no browser dialogs, AC-05 the seam's imports, AC-08 colour literals only in tokens.css,
// AC-15 no hold wording, AC-21 tests run once. The patterns are assembled so this file does not
// match itself.

const sources = import.meta.glob<string>("/src/**/*.{ts,tsx,css}", {
  query: "?raw",
  import: "default",
  eager: true,
});

function offending(pattern: RegExp, skip: (path: string) => boolean = () => false): string[] {
  const hits: string[] = [];
  for (const [path, text] of Object.entries(sources)) {
    if (skip(path)) {
      continue;
    }
    text.split("\n").forEach((line, index) => {
      if (pattern.test(line)) {
        hits.push(`${path}:${String(index + 1)}: ${line.trim()}`);
      }
    });
  }
  return hits;
}

function importsOf(text: string): string[] {
  const found: string[] = [];
  for (const match of text.matchAll(/(?:\bfrom\s*|\bimport\s*\(?\s*)["']([^"']+)["']/g)) {
    if (match[1] !== undefined) {
      found.push(match[1]);
    }
  }
  return found;
}

/** The provisional types' module, with or without an extension (as in eslint.config.js). */
const PROVISIONAL = /(^|\/)provisional(\.[jt]sx?)?$/;

function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error("expected a JSON object");
  }
  return value as Record<string, unknown>;
}

describe("source policy", () => {
  it("reads every source file", () => {
    expect(Object.keys(sources)).toContain("/src/main.tsx");
    expect(Object.keys(sources)).toContain("/src/styles/tokens.css");
  });

  it("never calls a browser dialog (AC-04)", () => {
    const names = ["al" + "ert", "con" + "firm", "pro" + "mpt"].join("|");
    expect(offending(new RegExp(`\\b(${names})\\s*\\(`))).toEqual([]);
  });

  it("imports the mocks only from the bootstrap, the provisional types only inside src/api (AC-05)", () => {
    const wrong: string[] = [];
    for (const [path, text] of Object.entries(sources)) {
      for (const specifier of importsOf(text)) {
        const mocks = /(^|\/)mocks(\/|$)/.test(specifier);
        if (mocks && path !== "/src/main.tsx" && !path.startsWith("/src/mocks/")) {
          wrong.push(`${path} imports ${specifier}`);
        }
        if (PROVISIONAL.test(specifier) && !path.startsWith("/src/api/")) {
          wrong.push(`${path} imports ${specifier}`);
        }
      }
    }
    expect(wrong).toEqual([]);
    expect(importsOf(sources["/src/main.tsx"] ?? "").some((specifier) => specifier.includes("mocks"))).toBe(true);
  });

  it("recognises the provisional module with or without an extension (AC-05)", () => {
    for (const specifier of ["./provisional", "../api/provisional", "../api/provisional.js", "./provisional.ts", "./provisional.tsx"]) {
      expect(PROVISIONAL.test(specifier), specifier).toBe(true);
    }
    for (const specifier of ["./provisionally", "../provisional/types", "./provisional.css", "./not-provisional-yet"]) {
      expect(PROVISIONAL.test(specifier), specifier).toBe(false);
    }
  });

  it("lints the same seam pattern it checks here (AC-05)", () => {
    expect(eslintConfig).toContain(JSON.stringify(PROVISIONAL.source.replaceAll("\\/", "/")));
  });

  it("keeps colour literals in tokens.css (AC-08)", () => {
    const literal = new RegExp(["#[0-9a-f]{3,8}\\b", ...["rgb", "rgba", "hsl", "hsla", "oklch", "color-mix"].map((name) => `\\b${name}\\(`)].join("|"), "i");
    expect(offending(literal, (path) => path === "/src/styles/tokens.css")).toEqual([]);
  });

  it("never words a proposal or a discrepancy as a hold on work (AC-15)", () => {
    const stem = "bl" + "ock";
    const words = new RegExp(`${stem}ed|${stem}ing|${stem}er|un${stem}`, "i");
    expect(offending(words)).toEqual([]);
  });

  it("writes no HTML from data: no HTML sink anywhere in src (AC-07 of ui-tree-node)", () => {
    // The sinks of docs/features/ui-tree-node.md "Rules and edge cases", spelled in pieces.
    const sinks = [
      ["dangerously", "SetInnerHTML"],
      ["inner", "HTML"],
      ["outer", "HTML"],
      ["insertAdjacent", "HTML"],
      ["document", ".write"],
      ["createContextual", "Fragment"],
      ["src", "doc"],
    ].map((parts) => parts.join("").replace(".", "\\."));
    expect(offending(new RegExp(`\\b(${sinks.join("|")})\\b`), (path) => path.endsWith(".css"))).toEqual([]);
  });

  it("takes every href in the views of spec data from routes.ts (AC-07 of ui-tree-node, AC-12 of ui-graph, AC-07 of ui-tasks, AC-07 of ui-home, AC-03 of ui-markdown)", () => {
    const viewDirs = ["/src/tree/", "/src/inbox/", "/src/graph/", "/src/tasks/", "/src/overview/", "/src/palette/", "/src/markdown/"];
    const views = (path: string) => !viewDirs.some((dir) => path.startsWith(dir)) || /\.test\.tsx?$/.test(path);
    expect(Object.keys(sources).filter((path) => path.startsWith("/src/overview/") && !views(path)).length).toBeGreaterThan(0);
    const rule = /\bhref=(?!\{(sectionHash|homeHash)\()/;
    expect(offending(rule, views)).toEqual([]);
    // The detector itself: a hash spelled by hand is caught, one from routes.ts is not.
    expect(rule.test("<a href={`#/${project}/tasks`}>")).toBe(true);
    expect(rule.test('<a href={"#/" + project}>')).toBe(true);
    expect(rule.test('<a href={sectionHash(project, "tasks")}>')).toBe(false);
    expect(rule.test("<a href={homeHash(project)}>")).toBe(false);
  });

  it("renders markdown only in src/markdown, raw HTML never interpreted (AC-02 of ui-markdown)", () => {
    const markdownPackage = /^(react-markdown|remark-gfm)(\/|$)/;
    const wrong: string[] = [];
    for (const [path, text] of Object.entries(sources)) {
      for (const specifier of importsOf(text)) {
        if (markdownPackage.test(specifier) && !path.startsWith("/src/markdown/")) {
          wrong.push(`${path} imports ${specifier}`);
        }
      }
    }
    expect(wrong).toEqual([]);
    expect(importsOf(sources["/src/markdown/Markdown.tsx"] ?? "")).toEqual(expect.arrayContaining(["react-markdown", "remark-gfm"]));
    // The packages' raw-HTML switches, spelled in pieces so this file does not match itself.
    const raw = ["rehype" + "-raw", "rehype" + "Raw", "allowDangerous" + "Html"];
    expect(offending(new RegExp(raw.join("|")))).toEqual([]);
    // The lint rule says the same: the packages restricted, src/markdown/ exempt.
    expect(eslintConfig).toContain(JSON.stringify(markdownPackage.source.replaceAll("\\/", "/")));
    expect(eslintConfig).toMatch(/files:\s*\["src\/markdown\/\*\*"\]/);
  });

  it("reads no clock in the home or the palette: times are shown as stored (AC-07 of ui-home)", () => {
    const outside = (path: string) => !(path.startsWith("/src/overview/") || path.startsWith("/src/palette/")) || /\.test\.tsx?$/.test(path);
    const clock = new RegExp(["Date\\.now\\(", "new Date\\(", "performance\\.now\\(", "useNow\\b", "formatAge\\b"].join("|"));
    expect(offending(clock, outside)).toEqual([]);
  });

  it("reads the network only through src/api (AC-01 of ui-tree-node)", () => {
    const outsideApi = (path: string) => path.startsWith("/src/api/") || path.endsWith(".css") || /\.test\.tsx?$/.test(path);
    expect(offending(/\b(fetch|XMLHttpRequest|EventSource|WebSocket)\s*\(/, outsideApi)).toEqual([]);
    expect(offending(/\bnew\s+(XMLHttpRequest|EventSource|WebSocket)\b/, outsideApi)).toEqual([]);
  });

  it("quotes no link type in app code but the weak one, `mentions` (AC-14 of ui-tree-node)", () => {
    // The built-in table of docs/canon/spec-cli-graph.md "Link types".
    const types = [
      "depends_on",
      "derived_from",
      "verifies",
      "uses_term",
      "constrains",
      "supersedes",
      "revises",
      "amends",
      "answers",
      "working_answer",
      "canon",
      "adopts",
    ];
    const appCode = (path: string) =>
      path.startsWith("/src/mocks/") || path.startsWith("/src/test/") || /\.test\.tsx?$/.test(path) || path.endsWith(".css");
    expect(offending(new RegExp(`["'\`](${types.join("|")})["'\`]`), appCode)).toEqual([]);
  });

  it("compares no role, profile or kind in app code: project vocabulary is shown, never branched on (AC-13 of ui-tasks)", () => {
    const appCode = (path: string) =>
      path.startsWith("/src/mocks/") || path.startsWith("/src/test/") || /\.test\.tsx?$/.test(path) || path.endsWith(".css");
    const field = "\\.(role|profile|kind)\\b";
    // ===, !==, == or != whole: never a shorter part of a longer operator.
    const equality = "([!=]==?)(?!=)";
    const absent = "(null|undefined)\\b";
    const compared = new RegExp(
      [
        // x.kind === value (a check for presence, against null or undefined, is no comparison)
        `${field}\\s*${equality}(?!\\s*${absent})`,
        // value === x.kind
        `(?<!${absent}\\s*)${equality}\\s*[\\w.?]*${field}`,
        // a switch, a ternary or a short circuit on the value
        `switch\\s*\\([^)]*${field}`,
        `${field}\\s*\\?\\s*[^?.:\\s]`,
        `${field}\\s*(&&|\\|\\|)`,
        // a lookup by the value: table[x.kind], .includes(x.role)
        `\\[[^\\]]*${field}\\s*\\]`,
        `\\.(includes|has|indexOf|startsWith|endsWith|test)\\([^)]*${field}`,
      ].join("|"),
    );
    expect(offending(compared, appCode)).toEqual([]);
    // The detector itself.
    for (const line of [
      "if (task.profile === x) {",
      'return run.role !== "a";',
      "switch (target.kind) {",
      "const tone = a === b.kind;",
      "const look = task.profile ? a : b;",
      "const go = task.profile && run();",
      "const tone = TONES[proposal.kind];",
      "if (KNOWN.includes(claim.role)) {",
    ]) {
      expect([line, compared.test(line)]).toEqual([line, true]);
    }
    for (const line of [
      "<dd className=\"mono\">{run.role}</dd>",
      "kind: string | null;",
      "{target.kind !== null && <span className=\"kind-tag\">{target.kind}</span>}",
      "{node.kind ?? \"-\"}",
      "author.role === null ? null : `role ${author.role}`",
      "{task.profile === null ? <p>No profile</p> : <p>{task.profile}</p>}",
    ]) {
      expect([line, compared.test(line)]).toEqual([line, false]);
    }
  });

  it("holds no raw Cyrillic letter: non-Latin test text is escaped (ADR-0024)", () => {
    const cyrillic = new RegExp(`[${String.fromCodePoint(0x400)}-${String.fromCodePoint(0x4ff)}]`);
    expect(offending(cyrillic)).toEqual([]);
  });
});

describe("toolchain policy", () => {
  const pkg = record(JSON.parse(packageJson));

  it("runs tests once and caps the workers (AC-21)", () => {
    expect(record(pkg.scripts).test).toBe("vitest run");
    expect(viteConfig).toMatch(/watch:\s*false/);
    expect(viteConfig).toMatch(/maxWorkers:\s*[12]\b/);
  });

  it("pins exactly the allowed packages at exact versions, no install script (AC-01; ui-markdown AC-01)", () => {
    const pinned = { ...record(pkg.dependencies), ...record(pkg.devDependencies) };
    expect(Object.keys(pinned).sort()).toEqual(
      [
        "react",
        "react-dom",
        "@tanstack/react-query",
        "@xyflow/react",
        "vite",
        "@vitejs/plugin-react",
        "typescript",
        "@types/react",
        "@types/react-dom",
        "eslint",
        "typescript-eslint",
        "eslint-plugin-react-hooks",
        "vitest",
        "@testing-library/react",
        "jsdom",
        // ADR-0036 amends ADR-0033's allowlist: 15 + 2.
        "react-markdown",
        "remark-gfm",
      ].sort(),
    );
    expect([pinned["react-markdown"], pinned["remark-gfm"]]).toEqual(["10.1.0", "4.0.1"]);
    for (const version of Object.values(pinned)) {
      expect(version).toMatch(/^\d+\.\d+\.\d+$/);
    }
    expect(pkg.packageManager).toMatch(/^pnpm@\d+\.\d+\.\d+$/);
    const scripts = Object.keys(record(pkg.scripts));
    expect(scripts.filter((name) => /install|prepare/.test(name))).toEqual([]);
  });
});
