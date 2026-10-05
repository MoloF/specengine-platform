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

  it("pins exactly the fifteen allowed packages at exact versions, no install script (AC-01)", () => {
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
      ].sort(),
    );
    for (const version of Object.values(pinned)) {
      expect(version).toMatch(/^\d+\.\d+\.\d+$/);
    }
    expect(pkg.packageManager).toMatch(/^pnpm@\d+\.\d+\.\d+$/);
    const scripts = Object.keys(record(pkg.scripts));
    expect(scripts.filter((name) => /install|prepare/.test(name))).toEqual([]);
  });
});
