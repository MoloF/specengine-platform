import * as ts from "typescript";
import { describe, expect, it } from "vitest";
import { SCALE_EXCEPTIONS, type ScaleException } from "./scale-exceptions";
import tokensCss from "./tokens.css?raw";

// DF-1, DF-2 and DF-3 of docs/features/design-foundation.md ("Guards and staging", "Size tokens",
// "Exceptions"): every style file under `ui/src` but tokens.css sets a font size, a line height, a
// space and a radius only through the scale's tokens, or through a listed exception that names why
// and the item that removes it; the tokens themselves resolve into the scale's sets; the controls are
// 32 px, or 24 px compact; no component sets a size inline.
// docs/features/design-sweep.md "Acceptance criteria", DS1 and DS2: no legacy size name anywhere in
// the sources, tokens.css included, each family exactly its steps; every font weight one token.

const SHEETS = import.meta.glob<string>("/src/**/*.css", { query: "?raw", import: "default", eager: true });
const COMPONENTS = import.meta.glob<string>(["/src/**/*.tsx", "!/src/**/*.test.tsx"], { query: "?raw", import: "default", eager: true });
const SCRIPTS = import.meta.glob<string>(["/src/**/*.{ts,tsx}", "!/src/**/*.test.{ts,tsx}"], { query: "?raw", import: "default", eager: true });

/** One declaration of a style file, with its rule's selector (whitespace collapsed) and the at-rule around it. */
interface Declaration {
  file: string;
  at: string | null;
  selector: string;
  property: string;
  value: string;
}

/** The text with its comments blanked. */
function uncommented(css: string): string {
  return css.replace(/\/\*[\s\S]*?\*\//g, (comment) => comment.replace(/[^\n]/g, " "));
}

/** Every declaration of `css`, its rules found inside `@media`, `@supports`, `@container` and `@keyframes` too. */
function declarationsOf(file: string, css: string): Declaration[] {
  const text = uncommented(css);
  const found: Declaration[] = [];
  let at = 0;
  const walk = (until: number, around: string | null): void => {
    while (at < until) {
      const open = text.indexOf("{", at);
      const close = text.indexOf("}", at);
      if (open < 0 || open >= until || (close >= 0 && close < open)) {
        at = (close < 0 || close > until ? until : close) + 1;
        return;
      }
      const head = text.slice(at, open).trim().replace(/\s+/g, " ");
      let depth = 1;
      let end = open + 1;
      for (; end < text.length && depth > 0; end += 1) {
        depth += text[end] === "{" ? 1 : text[end] === "}" ? -1 : 0;
      }
      if (/^@(media|supports|container|layer|keyframes)\b/.test(head)) {
        at = open + 1;
        walk(end - 1, head);
        at = end;
      } else {
        // Each declaration, the last one too where it has no `;`.
        for (const part of text.slice(open + 1, end - 1).split(";")) {
          const match = /^\s*(-?-?[a-z][a-z0-9-]*)\s*:\s*([\s\S]*?)\s*$/.exec(part);
          if (match !== null) {
            found.push({ file, at: around, selector: head, property: match[1] ?? "", value: match[2] ?? "" });
          }
        }
        at = end;
      }
    }
  };
  walk(text.length, null);
  return found;
}

const TOKENS_PATH = "/src/styles/tokens.css";
const DECLARATIONS = Object.entries(SHEETS)
  .filter(([path]) => path !== TOKENS_PATH)
  .flatMap(([path, css]) => declarationsOf(path, css));

const rootBlock = /(^|\n):root\s*\{([^}]*)\}/.exec(uncommented(tokensCss))?.[2] ?? "";
const TOKENS = new Map(Array.from(rootBlock.matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/g), (match): [string, string] => [match[1] ?? "", (match[2] ?? "").trim()]));

/** A token's value through its aliases. */
function resolved(name: string, seen: string[] = []): string {
  const value = TOKENS.get(name);
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
  return resolved(alias, [...seen, name]);
}

/** The tokens of a family, by prefix. */
function family(prefix: string): string[] {
  return [...TOKENS.keys()].filter((name) => name.startsWith(prefix));
}

/** "Data", "Size tokens": the scale's sets. */
const SIZES = ["0.75rem", "0.875rem", "1rem", "1.25rem", "1.5rem"];
const LINE_HEIGHTS = ["1rem", "1.25rem", "1.5rem", "1.75rem"];
const SPACES = ["0.125rem", "0.25rem", "0.375rem", "0.5rem", "0.75rem", "1rem", "1.25rem", "1.5rem", "2rem", "2.5rem", "3rem"];
const RADII = ["2px", "4px", "6px", "8px", "12px", "16px", "999px"];
/** Each family's steps, exactly: the type's seven pairs, space's eleven. */
const TYPE_STEPS = ["small", "body", "document", "heading-s", "heading-m", "heading-l", "code"];
const SPACE_STEPS = ["025", "050", "075", "100", "150", "200", "250", "300", "400", "500", "600"];
/** The four weights, each a token. */
const WEIGHTS: Readonly<Record<string, string>> = { regular: "400", medium: "500", semibold: "600", bold: "700" };
/** A legacy size name, gone with its alias (G1): `--space-1` to `-6`, `--font-size-s|m|l|xl`, `--line-height-tight`. */
const LEGACY_NAME = /--(?:space-[1-6]|font-size-(?:s|m|l|xl)|line-height-tight)(?![\w-])/g;

/** The path an exception names, as the glob keys it. */
function pathOf(exception: ScaleException): string {
  return `/${exception.file}`;
}

function listed(declaration: Declaration): boolean {
  return SCALE_EXCEPTIONS.some(
    (exception) => pathOf(exception) === declaration.file && exception.selector === declaration.selector && exception.property === declaration.property && exception.value === declaration.value,
  );
}

function shown(declaration: Declaration): string {
  return `${declaration.file} ${declaration.at === null ? "" : `${declaration.at} `}${declaration.selector} { ${declaration.property}: ${declaration.value} }`;
}

/** A value that is one `var()` of a defined token of `prefix`. */
function oneToken(value: string, prefix: string): boolean {
  const name = /^var\((--[a-z0-9-]+)\)$/.exec(value)?.[1];
  return name !== undefined && name.startsWith(prefix) && TOKENS.has(name);
}

/** A non-zero length literal: a number with a length unit. */
const LENGTH = /(^|[^\w.-])-?(\d*\.?\d+)(px|rem|em|ch|ex|vw|vh|vmin|vmax|dvh|svh|lvh|dvw|svw|lvw|pt|pc|cm|mm|in|lh|rlh|cap|ic)\b/g;

/** A value with each `var(--name)` taken out, a fallback's text left in its place to be read: `var(--a, 3px)` reads `3px`. */
function withoutTokens(value: string): string {
  let out = value;
  for (;;) {
    const next = out.replace(/var\(\s*--[a-z0-9-]+\s*(?:,([^()]*))?\)/g, (_whole, fallback: string | undefined) => ` ${fallback ?? ""} `);
    if (next === out) {
      return out;
    }
    out = next;
  }
}

/** Whether a value holds a non-zero length outside a token: `0`, `auto`, a percentage and `calc()` of tokens and plain numbers pass; a `var()`'s fallback is read. */
function holdsRawLength(value: string): boolean {
  return Array.from(withoutTokens(value).matchAll(LENGTH)).some((match) => Number(match[2]) !== 0);
}

/** The `--space-*` a value names, each to be defined. */
function spacesNamed(value: string): string[] {
  return Array.from(value.matchAll(/var\((--space-[a-z0-9-]+)\)/g), (match) => match[1] ?? "");
}

const SPACING = /^(padding|margin)(-[a-z-]+)?$|^(row-|column-)?gap$/;
const RADIUS = /^border(-[a-z]+)*-radius$/;

describe("the type scale (DF-1)", () => {
  it("reads every style file under src but tokens.css", () => {
    expect(Object.keys(SHEETS)).toEqual(expect.arrayContaining(["/src/styles/app.css", "/src/styles/fonts.css", TOKENS_PATH]));
    expect(DECLARATIONS.length).toBeGreaterThan(3000);
  });

  it("sets every font size and line height through one var() of a token, or a listed exception", () => {
    const raw = DECLARATIONS.filter((declaration) => {
      if (declaration.property === "font-size") {
        return !oneToken(declaration.value, "--font-size-");
      }
      if (declaration.property === "line-height") {
        return !oneToken(declaration.value, "--line-height-");
      }
      return false;
    });
    expect(raw.filter((declaration) => !listed(declaration)).map(shown)).toEqual([]);
  });

  it("writes the font shorthand only as inherit", () => {
    expect(DECLARATIONS.filter((declaration) => declaration.property === "font" && declaration.value !== "inherit" && !listed(declaration)).map(shown)).toEqual([]);
  });

  it("holds exactly the seven steps in --font-size-* and --line-height-*, each resolving into the scale", () => {
    expect(family("--font-size-")).toEqual(TYPE_STEPS.map((step) => `--font-size-${step}`));
    expect(family("--line-height-")).toEqual(TYPE_STEPS.map((step) => `--line-height-${step}`));
    for (const name of family("--font-size-")) {
      expect([name, SIZES.includes(resolved(name))]).toEqual([name, true]);
    }
    for (const name of family("--line-height-")) {
      expect([name, LINE_HEIGHTS.includes(resolved(name))]).toEqual([name, true]);
    }
    // The steps' pairs (docs/features/design-foundation.md "Typography").
    const pairs = ["small", "body", "document", "heading-s", "heading-m", "heading-l", "code"].map((step) => [step, resolved(`--font-size-${step}`), resolved(`--line-height-${step}`)]);
    expect(pairs).toEqual([
      ["small", "0.75rem", "1rem"],
      ["body", "0.875rem", "1.25rem"],
      ["document", "1rem", "1.5rem"],
      ["heading-s", "1rem", "1.25rem"],
      ["heading-m", "1.25rem", "1.5rem"],
      ["heading-l", "1.5rem", "1.75rem"],
      ["code", "0.75rem", "1.25rem"],
    ]);
  });

  it("lists no exception that matches nothing, and names each one's reason and its item", () => {
    const unmatched = SCALE_EXCEPTIONS.filter(
      (exception) =>
        !DECLARATIONS.some(
          (declaration) => declaration.file === pathOf(exception) && declaration.selector === exception.selector && declaration.property === exception.property && declaration.value === exception.value,
        ),
    );
    expect(unmatched).toEqual([]);
    for (const exception of SCALE_EXCEPTIONS) {
      expect([exception.selector, exception.reason.length > 10]).toEqual([exception.selector, true]);
    }
    // Kept for good: the visually hidden pattern and a border laid over a border, nothing else.
    expect(SCALE_EXCEPTIONS.filter((exception) => exception.until === null).map((exception) => exception.selector)).toEqual([".sr-only", ".tab"]);
  });
});

describe("spacing (DF-2)", () => {
  it("holds no non-zero length literal outside var() of a --space-* in padding, margin and gap, but listed exceptions", () => {
    const raw = DECLARATIONS.filter((declaration) => SPACING.test(declaration.property) && holdsRawLength(declaration.value));
    expect(raw.filter((declaration) => !listed(declaration)).map(shown)).toEqual([]);
    // Every space named is a token.
    const unknown = DECLARATIONS.filter((declaration) => SPACING.test(declaration.property)).flatMap((declaration) => spacesNamed(declaration.value).filter((name) => !TOKENS.has(name)).map((name) => `${shown(declaration)}: ${name}`));
    expect(unknown).toEqual([]);
    // The detector itself.
    expect(["0", "auto", "0 auto", "var(--space-100) var(--space-200)", "calc(var(--space-1) * 2)", "calc(-1 * var(--space-050))", "50%"].map(holdsRawLength)).toEqual([false, false, false, false, false, false, false]);
    expect(["3px", "0 1px", "calc(var(--space-1) + 1px)", "0.2em", "min(10vh, 5rem)"].map(holdsRawLength)).toEqual([true, true, true, true, true]);
  });

  it("holds no raw length in a custom property declared outside tokens.css, but listed exceptions", () => {
    const custom = DECLARATIONS.filter((declaration) => declaration.property.startsWith("--"));
    expect(custom.length).toBeGreaterThan(20);
    expect(custom.filter((declaration) => holdsRawLength(declaration.value) && !listed(declaration)).map(shown)).toEqual([]);
  });

  it("reads a last declaration with no semicolon, a custom property, and a fallback's length (the detector)", () => {
    const probe = declarationsOf("/probe.css", ".a { display: block; gap: 3px }\n@media (min-width: 1rem) { .b { --gap: 3px } }");
    expect(probe.map((declaration) => [declaration.at, declaration.selector, declaration.property, declaration.value])).toEqual([
      [null, ".a", "display", "block"],
      [null, ".a", "gap", "3px"],
      ["@media (min-width: 1rem)", ".b", "--gap", "3px"],
    ]);
    expect(probe.filter((declaration) => (SPACING.test(declaration.property) || declaration.property.startsWith("--")) && holdsRawLength(declaration.value)).map((declaration) => declaration.property)).toEqual(["gap", "--gap"]);
    expect(["var(--space-100, 3px)", "var(--a, var(--b, 1px))", "calc(var(--space-050) + var(--x, 2px))"].map(holdsRawLength)).toEqual([true, true, true]);
    expect(["var(--space-100, var(--space-050))", "var(--depth, 0)", "var(--board-columns, 1)"].map(holdsRawLength)).toEqual([false, false, false]);
  });

  it("holds exactly the eleven --space-* steps, each resolving into the 8 px scale", () => {
    expect(family("--space-")).toEqual(SPACE_STEPS.map((step) => `--space-${step}`));
    for (const name of family("--space-")) {
      expect([name, SPACES.includes(resolved(name))]).toEqual([name, true]);
    }
  });

  it("sets no font size, weight, line height, padding, margin, gap or radius in a component's inline style", () => {
    const sizing = /^(fontSize|fontWeight|lineHeight|padding\w*|margin\w*|gap|rowGap|columnGap|borderRadius|border\w*Radius)$/;
    const found: string[] = [];
    for (const [path, text] of Object.entries(COMPONENTS)) {
      const file = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
      const walk = (node: ts.Node): void => {
        if (ts.isJsxAttribute(node) && node.name.getText(file) === "style" && node.initializer !== undefined && ts.isJsxExpression(node.initializer)) {
          const expression = node.initializer.expression;
          if (expression !== undefined && ts.isObjectLiteralExpression(expression)) {
            for (const property of expression.properties) {
              const name = property.name?.getText(file).replace(/^["']|["']$/g, "") ?? "";
              if (sizing.test(name)) {
                found.push(`${path}: style ${name}`);
              }
            }
          }
        }
        ts.forEachChild(node, walk);
      };
      walk(file);
    }
    expect(Object.keys(COMPONENTS)).toEqual(expect.arrayContaining(["/src/board/BoardCard.tsx", "/src/ui/Lozenge.tsx"]));
    expect(found).toEqual([]);
  });
});

describe("no legacy size name (G1, DS1 of design-sweep)", () => {
  it("names no legacy size in any style file, tokens.css included, nor in a non-test script", () => {
    expect(Object.keys(SCRIPTS)).toEqual(expect.arrayContaining(["/src/styles/scale-exceptions.ts", "/src/ui/Button.tsx"]));
    expect(Object.keys(SCRIPTS).filter((path) => /\.test\.tsx?$/.test(path))).toEqual([]);
    const found = Object.entries({ ...SHEETS, ...SCRIPTS }).flatMap(([path, text]) => Array.from(text.matchAll(LEGACY_NAME), (match) => `${path}: ${match[0]}`));
    expect(found).toEqual([]);
  });

  it("matches each legacy name and no step that starts like one (the detector)", () => {
    const probe = "var(--space-1) var(--space-6) --font-size-s --font-size-xl --line-height-tight --space-100 --space-050 --font-size-small --font-size-heading-s --line-height-body --space-10";
    expect(Array.from(probe.matchAll(LEGACY_NAME), (match) => match[0])).toEqual(["--space-1", "--space-6", "--font-size-s", "--font-size-xl", "--line-height-tight"]);
  });
});

describe("font weights (G2, DS2 of design-sweep)", () => {
  const ALL = Object.entries(SHEETS).flatMap(([path, css]) => declarationsOf(path, css));
  const weights = ALL.filter((declaration) => declaration.property === "font-weight");

  it("sets every font-weight outside @font-face as one var() of the four weight tokens, or inherit", () => {
    expect(weights.length).toBeGreaterThan(100);
    const allowed = new Set(["inherit", ...Object.keys(WEIGHTS).map((step) => `var(--font-weight-${step})`)]);
    expect(weights.filter((declaration) => declaration.selector !== "@font-face" && !allowed.has(declaration.value)).map(shown)).toEqual([]);
  });

  it("leaves fonts.css's @font-face descriptors as they are", () => {
    const faces = weights.filter((declaration) => declaration.selector === "@font-face");
    expect(faces.map((declaration) => [declaration.file, declaration.value])).toEqual(Array.from({ length: 4 }, () => ["/src/styles/fonts.css", "100 900"]));
  });

  it("holds exactly the four --font-weight-* tokens at 400, 500, 600 and 700", () => {
    expect(Object.fromEntries(family("--font-weight-").map((name) => [name.slice("--font-weight-".length), TOKENS.get(name)]))).toEqual(WEIGHTS);
  });
});

describe("markdown headings carry their step (DS3 of design-sweep)", () => {
  const at = (selector: string, property: string) =>
    DECLARATIONS.filter((declaration) => declaration.at === null && declaration.selector === selector && declaration.property === property).map((declaration) => declaration.value);

  it("gives h2 the heading-m pair, h3 and h4 heading-s, h5 and h6 body, and the shared rule no line height", () => {
    expect([at(".markdown h2.md-heading", "font-size"), at(".markdown h2.md-heading", "line-height")]).toEqual([["var(--font-size-heading-m)"], ["var(--line-height-heading-m)"]]);
    expect([at(".markdown h3.md-heading, .markdown h4.md-heading", "font-size"), at(".markdown h3.md-heading, .markdown h4.md-heading", "line-height")]).toEqual([
      ["var(--font-size-heading-s)"],
      ["var(--line-height-heading-s)"],
    ]);
    expect([at(".markdown h5.md-heading, .markdown h6.md-heading", "font-size"), at(".markdown h5.md-heading, .markdown h6.md-heading", "line-height")]).toEqual([
      ["var(--font-size-body)"],
      ["var(--line-height-body)"],
    ]);
    expect(at(".markdown .md-heading", "line-height")).toEqual([]);
  });

  it("holds no unitless line height in a .markdown rule", () => {
    const markdown = DECLARATIONS.filter((declaration) => declaration.selector.includes(".markdown") && declaration.property === "line-height");
    expect(markdown.filter((declaration) => /^\d*\.?\d+$/.test(declaration.value)).map(shown)).toEqual([]);
  });
});

describe("the Spec tree's sizes are tokens (DS8 of design-sweep)", () => {
  const at = (selector: string, property: string) =>
    DECLARATIONS.filter((declaration) => declaration.at === null && declaration.selector === selector && declaration.property === property).map((declaration) => declaration.value);

  it("lists no exception for .text-view, .tree or .tree-row, whose rules take tokens", () => {
    expect(SCALE_EXCEPTIONS.filter((exception) => [".text-view", ".tree", ".tree-row"].includes(exception.selector))).toEqual([]);
    expect(at(".text-view", "line-height")).toEqual(["var(--line-height-body)"]);
    expect(at(".tree", "gap")).toEqual(["var(--space-025)"]);
    // The indent: 16 px a level.
    expect(at(".tree-row", "padding")).toEqual(["var(--space-050) var(--space-100) var(--space-050) calc(var(--space-050) + var(--depth, 0) * var(--space-200))"]);
  });

  it("drops the chevron's column from a direction's flat list", () => {
    expect(at(".tree-flat .tree-row", "grid-template-columns")).toEqual(["minmax(0, 1fr)"]);
  });
});

describe("radii and control heights (DF-3)", () => {
  it("rounds only with 0 or var(--radius-*), but listed exceptions", () => {
    const raw = DECLARATIONS.filter(
      (declaration) =>
        RADIUS.test(declaration.property) &&
        !declaration.value
          .split(/[\s/]+/)
          .filter((part) => part !== "")
          .every((part) => part === "0" || oneToken(part, "--radius-")),
    );
    expect(raw.filter((declaration) => !listed(declaration)).map(shown)).toEqual([]);
  });

  // Flow's terminators' 16 px among them (docs/features/room-and-scroll.md "Acceptance criteria", RS-3).
  it("resolves every --radius-* into the scale, --radius-xl at 16 px", () => {
    // M: `--radius-xl` at 20px, outside the scale.
    expect(TOKENS.get("--radius-xl")).toBe("16px");
    expect(family("--radius-").length).toBe(7);
    for (const name of family("--radius-")) {
      expect([name, RADII.includes(resolved(name))]).toEqual([name, true]);
    }
  });

  it("makes .button and .field 32 px tall and .button-compact 24 px, from the tokens", () => {
    const minHeight = (selector: string) =>
      DECLARATIONS.filter((declaration) => declaration.at === null && declaration.selector === selector && declaration.property === "min-height").map((declaration) => declaration.value);
    expect(minHeight(".button")).toEqual(["var(--control-height)"]);
    expect(minHeight(".field")).toEqual(["var(--control-height)"]);
    expect(minHeight(".button-compact")).toEqual(["var(--control-height-compact)"]);
    expect([resolved("--control-height"), resolved("--control-height-compact"), resolved("--target-min"), resolved("--nav-height")]).toEqual(["2rem", "1.5rem", "24px", "3rem"]);
  });
});

// DM-19 of docs/features/doc-modal.md: the document view's styles take tokens alone, and its entry
// leaves the exceptions, none for RM-0095; DS8 of docs/features/design-sweep.md: the Spec tree's three
// leave too, so 10 stand.
describe("the document view's tokens (DM-19 of doc-modal)", () => {
  const exceptionsSource = import.meta.glob<string>("/src/styles/scale-exceptions.ts", { query: "?raw", import: "default", eager: true })["/src/styles/scale-exceptions.ts"] ?? "";

  it("lists 10 exceptions, none until RM-0095, and the `until` type names it no more", () => {
    expect(SCALE_EXCEPTIONS).toHaveLength(10);
    expect(new Set(SCALE_EXCEPTIONS.map((exception) => exception.until))).toEqual(new Set([null, "RM-0099"]));
    expect(exceptionsSource).toMatch(/until: "RM-0099" \| null;/);
    expect(exceptionsSource).not.toContain("RM-0095");
  });

  it("sets a raw URL's and an image path's size by a type token", () => {
    const sizes = DECLARATIONS.filter((declaration) => declaration.selector === ".md-url, .md-image-path" && declaration.property === "font-size");
    expect(sizes.map((declaration) => declaration.value)).toEqual(["var(--font-size-code)"]);
  });

  it("styles the document view by tokens alone: every size, space and radius of a `.doc-` rule one var(), no exception (M: a raw font size in .doc-body)", () => {
    const doc = DECLARATIONS.filter((declaration) => /\.doc-/.test(declaration.selector));
    expect(doc.length).toBeGreaterThan(20);
    const sized = doc.filter((declaration) => /^(font-size|line-height|gap|row-gap|column-gap|border-radius|padding|margin)(-[a-z-]+)?$/.test(declaration.property));
    expect(sized.filter((declaration) => !/^(0|var\(--[a-z0-9-]+\)( var\(--[a-z0-9-]+\))*)$/.test(declaration.value)).map(shown)).toEqual([]);
    expect(SCALE_EXCEPTIONS.filter((exception) => exception.selector.includes(".doc-"))).toEqual([]);
    expect(doc.filter(listed).map(shown)).toEqual([]);
  });
});
