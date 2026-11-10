import { describe, expect, it } from "vitest";
import viteConfig from "../../vite.config.ts?raw";
import mainSource from "../main.tsx?raw";
import fontsCss from "./fonts.css?raw";
import tokensCss from "./tokens.css?raw";

// DF-13 of docs/features/design-foundation.md ("Typography", "`fonts.css` and `ui/fonts/inter/`"):
// Inter bundled as four committed woff2 subsets of its weight axis, upright, under the OFL, declared
// in fonts.css alone, each face with its subset's unicode-range and a relative url() into
// ui/fonts/inter/; no font host, no inlined font; the stack names Inter first.

const FONT_FILES = import.meta.glob<string>("/fonts/inter/*.woff2", { query: "?raw", import: "default", eager: true });
const LICENCE = import.meta.glob<string>("/fonts/inter/OFL.txt", { query: "?raw", import: "default", eager: true });
/** The licence's copy in Vite's public folder, which the build copies to dist/assets/ beside the font files. */
const SHIPPED_LICENCE = import.meta.glob<string>("/public/assets/OFL.txt", { query: "?raw", import: "default", eager: true });
const SHEETS = import.meta.glob<string>("/src/**/*.css", { query: "?raw", import: "default", eager: true });

const SUBSETS = ["latin", "latin-ext", "cyrillic", "cyrillic-ext"];

/** Each `@font-face { … }` body of a sheet. */
function faces(css: string): string[] {
  return Array.from(css.replace(/\/\*[\s\S]*?\*\//g, "").matchAll(/@font-face\s*\{([^}]*)\}/g), (match) => match[1] ?? "");
}

function property(body: string, name: string): string | undefined {
  return new RegExp(`(?:^|;)\\s*${name}\\s*:\\s*([^;]+);`).exec(body)?.[1]?.trim();
}

/** The file a face's `url()` names, from `src/styles/`, as the root-relative path the glob keys. */
function fileOf(body: string): string | null {
  const url = /url\("([^"]+)"\)/.exec(property(body, "src") ?? "")?.[1];
  if (url === undefined || !url.startsWith("../../fonts/inter/")) {
    return null;
  }
  return `/${url.slice("../../".length)}`;
}

describe("Inter, bundled (DF-13)", () => {
  it("declares exactly four faces of Inter: the weight axis upright, swap, a unicode-range, a relative url() to a committed woff2", () => {
    const declared = faces(fontsCss);
    expect(declared).toHaveLength(4);
    const files: string[] = [];
    for (const body of declared) {
      expect(property(body, "font-family")).toBe('"Inter"');
      expect(property(body, "font-weight")).toBe("100 900");
      expect(property(body, "font-style")).toBe("normal");
      expect(property(body, "font-display")).toBe("swap");
      expect(property(body, "unicode-range")).toMatch(/^U\+[0-9A-F]/);
      const file = fileOf(body);
      expect(file).not.toBeNull();
      expect(FONT_FILES[file ?? ""]?.startsWith("wOF2")).toBe(true);
      files.push(file ?? "");
    }
    expect(files.sort()).toEqual(SUBSETS.map((subset) => `/fonts/inter/inter-${subset}-wght-normal.woff2`).sort());
    // Each subset's own range: Cyrillic's holds the Russian alphabet, Latin's ASCII.
    expect(declared.find((body) => fileOf(body)?.includes("inter-cyrillic-wght") === true)).toMatch(/U\+0400-045F/);
    expect(declared.find((body) => fileOf(body)?.includes("inter-latin-wght") === true)).toMatch(/U\+0000-00FF/);
  });

  it("ships the four files and the licence beside them", () => {
    expect(Object.keys(FONT_FILES).sort()).toEqual(SUBSETS.map((subset) => `/fonts/inter/inter-${subset}-wght-normal.woff2`).sort());
    expect(LICENCE["/fonts/inter/OFL.txt"]).toContain("SIL OPEN FONT LICENSE Version 1.1");
  });

  it("ships the licence in the build beside the font files: public/assets/OFL.txt, the same text", () => {
    expect(SHIPPED_LICENCE["/public/assets/OFL.txt"]).toBe(LICENCE["/fonts/inter/OFL.txt"]);
    expect(viteConfig).not.toMatch(/publicDir|copyPublicDir/);
  });

  it("names Inter first in the stack, the system's behind it", () => {
    expect(/--font-sans:\s*"Inter",\s*system-ui,/.test(tokensCss)).toBe(true);
  });

  it("holds no other @font-face, no url() to a host and no @import url( in any style file", () => {
    const wrong: string[] = [];
    for (const [path, css] of Object.entries(SHEETS)) {
      const text = css.replace(/\/\*[\s\S]*?\*\//g, "");
      if (path !== "/src/styles/fonts.css" && faces(text).length > 0) {
        wrong.push(`${path}: @font-face`);
      }
      if (/url\(\s*["']?\s*(https?:|\/\/)/i.test(text)) {
        wrong.push(`${path}: a url() to a host`);
      }
      if (/@import\s+url\(/i.test(text)) {
        wrong.push(`${path}: @import url(`);
      }
    }
    expect(wrong).toEqual([]);
  });

  it("imports fonts.css at the bootstrap, before the tokens", () => {
    const at = (sheet: string) => mainSource.indexOf(`import "./styles/${sheet}";`);
    expect(at("fonts.css")).toBeGreaterThan(0);
    expect(at("fonts.css")).toBeLessThan(at("tokens.css"));
  });

  it("inlines no font: every subset is far over Vite's 4 kB limit, which the config leaves as it is", () => {
    for (const [path, text] of Object.entries(FONT_FILES)) {
      expect([path, text.length > 8 * 1024]).toEqual([path, true]);
    }
    expect(viteConfig).not.toMatch(/assetsInlineLimit/);
  });
});
