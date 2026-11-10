import * as ts from "typescript";
import { describe, expect, it } from "vitest";
import type { WorkspaceDocument } from "./api/types";
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

function offending(pattern: RegExp, skip: (path: string) => boolean = () => false, seen: (path: string, line: string) => string = (_path, line) => line): string[] {
  const hits: string[] = [];
  for (const [path, text] of Object.entries(sources)) {
    if (skip(path)) {
      continue;
    }
    text.split("\n").forEach((line, index) => {
      if (pattern.test(seen(path, line))) {
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

/**
 * The source path a specifier names from the file `from` imports it in: a relative or root path
 * resolved, its extension dropped; a package name as written.
 */
function resolvedFrom(from: string, specifier: string): string {
  if (!specifier.startsWith(".") && !specifier.startsWith("/")) {
    return specifier;
  }
  const parts = specifier.startsWith("/") ? [] : from.split("/").slice(0, -1);
  for (const part of specifier.split("/")) {
    if (part === ".." && parts.length > 0) {
      parts.pop();
    } else if (part !== "" && part !== "." && part !== "..") {
      parts.push(part);
    }
  }
  return `/${parts.filter((part) => part !== "").join("/")}`.replace(/\.[jt]sx?$/, "");
}

/** The round's text module: its caps and fixed words, the round's own (UI-9 of health-ui-follow-up). */
const ROUND_TEXT = "/src/round/text";

/** The two text modules as loaded, for their exports. */
const textModules = import.meta.glob<Record<string, unknown>>(["/src/round/text.ts", "/src/ui/text.ts"], { eager: true });

/**
 * The mock's workspaces and its scenarios, loaded as data through Vite (no `import` of the mocks, which
 * only the bootstrap makes): WS-6 (c) of docs/features/workspaces.md reads their names at run time.
 */
const mockWorkspaces = import.meta.glob<{ workspacesOf: (slug: string, scenario: string) => WorkspaceDocument }>("/src/mocks/workspaces.ts", { eager: true });
const mockScenarios = import.meta.glob<{ SCENARIOS: readonly string[] }>("/src/mocks/scenario.ts", { eager: true });

/** The mock's boards, loaded as data the same way: the Board's column and flow words read at run time (design-foundation's review, the category's guard). */
const mockCards = import.meta.glob<{ mockBoard: (slug: string, scenario: string, at: number) => { flows: readonly { name: string; columns: readonly { id: string }[] }[] } }>("/src/mocks/cards.ts", { eager: true });

/** The Bindings tab's sources (docs/features/ui-node-bindings.md): held to Health's rules for rows (UI-7); its command list shared with the lock diff's rows. */
const BINDINGS_TAB: readonly string[] = ["/src/tree/BindingsPanel.tsx", "/src/tree/BindingLine.tsx", "/src/tree/commands.ts", "/src/tree/CommandList.tsx"];

/** The drift card's sources (docs/features/ui-drift-evidence.md): its rows held to Health's rules too (UI-6). */
const DRIFT_CARD: readonly string[] = ["/src/inbox/drift.ts", "/src/inbox/DriftItem.tsx"];

/** The tree's drift badges (docs/features/ui-tree-drift.md): counted and drawn under Health's rules for rows (UI-6). */
const TREE_DRIFT: readonly string[] = ["/src/tree/drift.ts", "/src/tree/RowContent.tsx", "/src/tree/DriftStatus.tsx"];

/** The Bindings tab's "Changes since the lock" (docs/features/lock-diff-ui.md): its rows and sides under Health's rules for rows (UI-7). */
const LOCK_DIFF: readonly string[] = ["/src/tree/LockDiffPanel.tsx", "/src/tree/lockDiff.ts"];

/**
 * The one `.kind` the kind scan lets through (AC-07 of docs/features/flow-canvas.md): `edge.kind`
 * under src/flow/, which is the closed `FlowEdgeKind` of the daemon's schema, no project vocabulary,
 * so a repeat is always dotted. `node.kind` stays scanned there, and `edge.kind` everywhere else.
 */
function withoutFlowEdgeKind(path: string, line: string): string {
  return path.startsWith("/src/flow/") ? line.replace(/\bedge\.kind\b/g, "edge") : line;
}

/** Health, the Bindings tab, the drift card, the tree's badges and the lock diff: the row sources AC-U6 and the no-sort rule cover. */
const ROW_SOURCES: readonly string[] = [...BINDINGS_TAB, ...DRIFT_CARD, ...TREE_DRIFT, ...LOCK_DIFF];

/** The provisional types' module, with or without an extension (as in eslint.config.js). */
const PROVISIONAL = /(^|\/)provisional(\.[jt]sx?)?$/;

/** The one file of Russian text in ui/ (ADR-0072; docs/features/ui-languages.md D2). */
const RUSSIAN_CATALOG = "/src/i18n/ru.ts";

/**
 * The directories left out of the language scan (docs/canon/ui-languages.md "Guards"): the transport,
 * the catalog itself, the mock's data, the styles and the test helpers. Every other `src/` directory
 * is scanned, a new one from the start: its chrome is all in the catalog.
 */
const NOT_SCANNED: readonly string[] = ["/src/api/", "/src/i18n/", "/src/mocks/", "/src/styles/", "/src/test/"];

/** A source the language scan reads: a non-test `.ts` or `.tsx` outside the five directories. */
function isScanned(path: string): boolean {
  return /\.tsx?$/.test(path) && !/\.d\.ts$/.test(path) && !isTestPath(path) && !NOT_SCANNED.some((directory) => path.startsWith(directory));
}

/** The attributes whose value is read or shown: never a literal word in a converted directory. */
const SHOWN_ATTRIBUTES: readonly string[] = ["aria-label", "aria-description", "aria-roledescription", "aria-valuetext", "title", "placeholder", "alt"];

/** Shared components' text props, by component: a new one joins the list. */
const TEXT_PROPS: Readonly<Record<string, readonly string[]>> = {
  Skeleton: ["label"],
  Notes: ["label"],
  Stored: ["label"],
  ErrorPanel: ["title", "message"],
  ReadFailure: ["title"],
  Badge: ["name"],
  Dialog: ["title"],
  Region: ["title", "boundary"],
  RegionBoundary: ["name"],
  CopyButton: ["label", "failed"],
  DiffView: ["label"],
  Tabs: ["label"],
  NotFound: ["reason"],
  // The primitives of docs/features/design-foundation.md "Primitives (`ui/src/ui/`)".
  Lozenge: ["label"],
  PropertyList: ["label"],
  EmptyState: ["title"],
  Disclosure: ["summary", "regionLabel"],
};

function isTestPath(path: string): boolean {
  return /\.test\.tsx?$/.test(path);
}

function parsed(path: string, text: string): ts.SourceFile {
  return ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, path.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
}

/**
 * The literals an expression shows as it is: itself, a template's text, either branch of a
 * conditional, the value after `&&`, either side of `||` and `??`. A call's arguments (a key to
 * `t`) and a condition's operands (a value compared) are not shown.
 */
function shownLiterals(node: ts.Expression): string[] {
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) {
    return [node.text];
  }
  if (ts.isTemplateExpression(node)) {
    return [node.head.text, ...node.templateSpans.flatMap((span) => [span.literal.text, ...shownLiterals(span.expression)])];
  }
  if (ts.isParenthesizedExpression(node)) {
    return shownLiterals(node.expression);
  }
  if (ts.isConditionalExpression(node)) {
    return [...shownLiterals(node.whenTrue), ...shownLiterals(node.whenFalse)];
  }
  if (ts.isBinaryExpression(node)) {
    const operator = node.operatorToken.kind;
    if (operator === ts.SyntaxKind.AmpersandAmpersandToken) {
      return shownLiterals(node.right);
    }
    if (operator === ts.SyntaxKind.BarBarToken || operator === ts.SyntaxKind.QuestionQuestionToken) {
      return [...shownLiterals(node.left), ...shownLiterals(node.right)];
    }
  }
  return [];
}

const ASCII_WORD = /[A-Za-z]/;

/** Each word a `.tsx` text spells itself: JSX text, a literal child, a literal shown or text-prop value. */
function literalWords(path: string, text: string): string[] {
  const found: string[] = [];
  const file = parsed(path, text);
  const at = (node: ts.Node) => `${path}:${String(file.getLineAndCharacterOfPosition(node.getStart(file)).line + 1)}`;
  function attributeOwner(attribute: ts.JsxAttribute): string {
    const element = attribute.parent.parent;
    return element.tagName.getText(file);
  }
  function walk(node: ts.Node): void {
    if (ts.isJsxText(node) && ASCII_WORD.test(node.text)) {
      found.push(`${at(node)}: text ${JSON.stringify(node.text.trim())}`);
    }
    if (ts.isJsxExpression(node) && node.expression !== undefined && (ts.isJsxElement(node.parent) || ts.isJsxFragment(node.parent))) {
      for (const literal of shownLiterals(node.expression).filter((value) => ASCII_WORD.test(value))) {
        found.push(`${at(node)}: child ${JSON.stringify(literal)}`);
      }
    }
    if (ts.isJsxAttribute(node) && node.initializer !== undefined) {
      const name = node.name.getText(file);
      const owner = attributeOwner(node);
      if (SHOWN_ATTRIBUTES.includes(name) || (TEXT_PROPS[owner] ?? []).includes(name)) {
        const initializer = node.initializer;
        const values = ts.isStringLiteral(initializer) ? [initializer.text] : ts.isJsxExpression(initializer) && initializer.expression !== undefined ? shownLiterals(initializer.expression) : [];
        for (const literal of values.filter((value) => ASCII_WORD.test(value))) {
          found.push(`${at(node)}: <${owner} ${name}> ${JSON.stringify(literal)}`);
        }
      }
    }
    ts.forEachChild(node, walk);
  }
  walk(file);
  return found;
}

/** A phrase of two words or more, or one capitalised word alone: what reads as English to the prose rule. */
const PHRASE = /\b[A-Za-z][a-z]+[ ,;:]+[a-z]{2,}\b/;
const CAPITALISED = /^[A-Z][a-z]+[.:]?$/;

/** The attributes and properties whose value is never shown: a class, a key, an ID, a link, a language, a role, a type. */
const NOT_SHOWN: readonly string[] = ["className", "key", "id", "href", "lang", "role", "type"];

/**
 * The DOM, storage, string and collection methods whose first argument is code: a selector, a
 * storage key, an attribute's name, a separator, a pattern, a lookup. Their other arguments (a value
 * stored, set or put in) may be shown, and are read as any literal.
 */
const CODE_METHODS: readonly string[] = [
  "querySelector",
  "querySelectorAll",
  "closest",
  "matches",
  "getItem",
  "setItem",
  "removeItem",
  "getAttribute",
  "setAttribute",
  "hasAttribute",
  "removeAttribute",
  "addEventListener",
  "removeEventListener",
  "startsWith",
  "endsWith",
  "includes",
  "indexOf",
  "split",
  "replace",
  "replaceAll",
  "get",
  "has",
  "set",
  "delete",
  "add",
];

/**
 * The literals the prose rule lets through, exact, by file, each for its reason: code that reads as a
 * word. A new entry names why it is never chrome.
 */
const PROSE_ALLOWED: Readonly<Record<string, readonly string[]>> = {
  // The palette's group IDs: a group's key and its option IDs' prefix, its shown name from the catalog.
  "/src/palette/Palette.tsx": ["Projects", "Tasks", "Inbox"],
  "/src/palette/options.ts": ["Sections", "Tasks", "Inbox", "Projects", "Spec tree"],
  // The dialog's focus selectors.
  "/src/ui/Dialog.tsx": ["button:not([disabled])", "input:not([disabled])", "select:not([disabled])", "textarea:not([disabled])"],
  // Flow's rollup words, the daemon's own, matched in labels.ts alone (AC-08 of trace-views).
  "/src/flow/labels.ts": ["no card", "in work"],
  // A CSS transform for the canvas's floating label.
  "/src/flow/FlowCanvas.tsx": ["translate(-50%, -50%) translate( px,  px)"],
  // The same transform for an edge's label; the key names a move goes on, looked up, never shown.
  "/src/graph/GraphCanvas.tsx": ["translate(-50%, -50%) translate( px,  px)", "Home", "End"],
  // The daemon's lock diff note's start, matched against its notes, never shown.
  "/src/tree/lockDiff.ts": ["lock diff:  : no   diff: "],
  // The bootstrap's own error, thrown when the page lacks its root: there is no page to show it in.
  "/src/main.tsx": ["index.html lacks the root element"],
};

/** A node's parent: none at the file's root, which the compiler's types do not say. */
function parentOf(node: ts.Node): ts.Node | undefined {
  const holder: { readonly parent?: ts.Node } = node;
  return holder.parent;
}

/** The text a literal or template holds, a template's pieces joined by a space where its values stand. */
function literalText(node: ts.Node): string | null {
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) {
    return node.text;
  }
  if (ts.isTemplateExpression(node)) {
    return [node.head.text, ...node.templateSpans.map((span) => span.literal.text)].join(" ");
  }
  return null;
}

function readsAsEnglish(text: string): boolean {
  const trimmed = text.trim();
  return PHRASE.test(trimmed) || CAPITALISED.test(trimmed);
}

/**
 * Whether a literal stands where code does, never shown: an import, a type, a property name, a `case`
 * label, an operand of `===`, `!==` or `in`, `t`'s key, a code method's argument, a value not shown
 * or a `…COMMAND` constant. A template, parentheses, a conditional's branch or `??`/`||` are looked
 * through: the place counts, not the wrapping.
 */
function isCodePosition(node: ts.Node, file: ts.SourceFile): boolean {
  let child: ts.Node = node;
  let parent = parentOf(node);
  const passThrough = (at: ts.Node, inner: ts.Node) =>
    ts.isTemplateSpan(at) ||
    ts.isTemplateExpression(at) ||
    ts.isParenthesizedExpression(at) ||
    ts.isAsExpression(at) ||
    ts.isSatisfiesExpression(at) ||
    (ts.isConditionalExpression(at) && at.condition !== inner) ||
    (ts.isBinaryExpression(at) && [ts.SyntaxKind.QuestionQuestionToken, ts.SyntaxKind.BarBarToken].includes(at.operatorToken.kind));
  while (parent !== undefined && passThrough(parent, child)) {
    child = parent;
    parent = parentOf(parent);
  }
  if (parent === undefined) {
    return false;
  }
  const nameOf = (name: ts.Node) => name.getText(file).replace(/^["']|["']$/g, "");
  if (ts.isImportDeclaration(parent) || ts.isExportDeclaration(parent) || ts.isLiteralTypeNode(parent) || ts.isCaseClause(parent)) {
    return true;
  }
  if ((ts.isPropertyAssignment(parent) || ts.isPropertySignature(parent) || ts.isPropertyDeclaration(parent) || ts.isMethodDeclaration(parent)) && parent.name === child) {
    return true;
  }
  if (ts.isElementAccessExpression(parent) && parent.argumentExpression === child) {
    return true;
  }
  if (ts.isBinaryExpression(parent) && [ts.SyntaxKind.EqualsEqualsEqualsToken, ts.SyntaxKind.ExclamationEqualsEqualsToken, ts.SyntaxKind.InKeyword].includes(parent.operatorToken.kind)) {
    return true;
  }
  if (ts.isCallExpression(parent) && parent.arguments.some((argument) => argument === child)) {
    const callee = parent.expression;
    if (callee.kind === ts.SyntaxKind.ImportKeyword) {
      return true;
    }
    if (ts.isIdentifier(callee) && callee.text === "t" && parent.arguments[0] === child) {
      return true;
    }
    if (ts.isPropertyAccessExpression(callee) && CODE_METHODS.includes(callee.name.text) && parent.arguments[0] === child) {
      return true;
    }
  }
  if (ts.isJsxAttribute(parent) && NOT_SHOWN.includes(nameOf(parent.name))) {
    return true;
  }
  if (ts.isJsxExpression(parent) && ts.isJsxAttribute(parent.parent) && NOT_SHOWN.includes(nameOf(parent.parent.name))) {
    return true;
  }
  if (ts.isPropertyAssignment(parent) && parent.initializer === child && NOT_SHOWN.includes(nameOf(parent.name))) {
    return true;
  }
  return ts.isVariableDeclaration(parent) && parent.initializer === child && /COMMAND$/.test(parent.name.getText(file));
}

/** Each literal a `.ts` or `.tsx` file holds that reads as English, outside a code position, a code token and PROSE_ALLOWED. */
function proseWords(path: string, text: string): string[] {
  const found: string[] = [];
  const file = parsed(path, text);
  const allowed = PROSE_ALLOWED[path] ?? [];
  const at = (node: ts.Node) => `${path}:${String(file.getLineAndCharacterOfPosition(node.getStart(file)).line + 1)}`;
  function walk(node: ts.Node): void {
    // A template's pieces are read once, as the whole template.
    const parent = parentOf(node);
    const words = parent !== undefined && ts.isTemplateSpan(parent) && node !== parent.expression ? null : literalText(node);
    if (words !== null && readsAsEnglish(words) && !words.startsWith("spec ") && !allowed.includes(words) && !isCodePosition(node, file)) {
      found.push(`${at(node)}: prose ${JSON.stringify(words)}`);
    }
    ts.forEachChild(node, walk);
  }
  walk(file);
  return found;
}

/** Each static import of the Russian catalog, and each `import("…/ru")`, by the file that makes it. */
function russianImports(path: string, text: string): { static: string[]; dynamic: string[] } {
  const file = parsed(path, text);
  const found = { static: [] as string[], dynamic: [] as string[] };
  const isRussian = (specifier: ts.Node) => ts.isStringLiteral(specifier) && resolvedFrom(path, specifier.text) === RUSSIAN_CATALOG.replace(/\.ts$/, "");
  function walk(node: ts.Node): void {
    if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier !== undefined && isRussian(node.moduleSpecifier)) {
      const typeOnly = ts.isImportDeclaration(node) ? node.importClause?.phaseModifier === ts.SyntaxKind.TypeKeyword : node.isTypeOnly;
      if (!typeOnly) {
        found.static.push(`${path}: ${node.getText(file)}`);
      }
    }
    if (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword && node.arguments[0] !== undefined && isRussian(node.arguments[0])) {
      found.dynamic.push(`${path}: ${node.getText(file)}`);
    }
    ts.forEachChild(node, walk);
  }
  walk(file);
  return found;
}

function isKeyRead(node: ts.Node): boolean {
  return ts.isPropertyAccessExpression(node) && node.name.text === "key";
}

function isOneLetter(node: ts.Node): boolean {
  return (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) && /^[A-Za-z]$/.test(node.text);
}

/** Each letter hotkey read from `.key` alone (docs/features/ui-languages.md LG-7): a comparison, a `switch` case, a list it is looked up in. */
function letterKeyReads(path: string, text: string): string[] {
  const found: string[] = [];
  const file = parsed(path, text);
  const at = (node: ts.Node) => `${path}:${String(file.getLineAndCharacterOfPosition(node.getStart(file)).line + 1)}: ${node.getText(file).split("\n")[0] ?? ""}`;
  const equality = [ts.SyntaxKind.EqualsEqualsEqualsToken, ts.SyntaxKind.ExclamationEqualsEqualsToken, ts.SyntaxKind.EqualsEqualsToken, ts.SyntaxKind.ExclamationEqualsToken];
  function walk(node: ts.Node): void {
    if (ts.isBinaryExpression(node) && equality.includes(node.operatorToken.kind)) {
      if ((isKeyRead(node.left) && isOneLetter(node.right)) || (isOneLetter(node.left) && isKeyRead(node.right))) {
        found.push(at(node));
      }
    }
    if (ts.isSwitchStatement(node) && isKeyRead(node.expression)) {
      for (const clause of node.caseBlock.clauses) {
        if (ts.isCaseClause(clause) && isOneLetter(clause.expression)) {
          found.push(at(clause));
        }
      }
    }
    if (ts.isCallExpression(node) && ts.isPropertyAccessExpression(node.expression) && node.expression.name.text === "includes") {
      const list = node.expression.expression;
      const [argument] = node.arguments;
      if (argument !== undefined && isKeyRead(argument) && ts.isArrayLiteralExpression(list) && list.elements.some(isOneLetter)) {
        found.push(at(node));
      }
    }
    ts.forEachChild(node, walk);
  }
  walk(file);
  return found;
}

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

  it("shows stored text through src/ui/text.ts; src/round/text.ts keeps the round's caps and words, imported from src/round/ alone (UI-9 of health-ui-follow-up)", () => {
    const wrong: string[] = [];
    for (const [path, text] of Object.entries(sources)) {
      if (path.startsWith("/src/round/")) {
        continue;
      }
      for (const specifier of importsOf(text)) {
        if (resolvedFrom(path, specifier) === ROUND_TEXT) {
          wrong.push(`${path} imports ${specifier}`);
        }
      }
    }
    expect(wrong).toEqual([]);
    const exported = (path: string) => Object.keys(textModules[path] ?? {}).sort();
    expect(exported("/src/ui/text.ts")).toEqual(["shownConfirmSlot", "shownConfirmText", "shownSlot", "shownText"]);
    // Nothing of the shared module re-exported here: its importers move, never route through the round.
    expect(exported("/src/round/text.ts")).toEqual(["ANSWER_MAX_BYTES", "REASON_MAX_BYTES", "ROUND_COMMAND", "ROUND_LIMIT_MAX", "utf8Bytes"]);
    // The detector itself.
    expect(resolvedFrom("/src/health/codeRows.tsx", "../round/text")).toBe(ROUND_TEXT);
    expect(resolvedFrom("/src/inbox/StagedChoice.tsx", "../round/text.ts")).toBe(ROUND_TEXT);
    expect(resolvedFrom("/src/main.tsx", "./round/text")).toBe(ROUND_TEXT);
    expect(resolvedFrom("/src/app/App.tsx", "/src/round/text")).toBe(ROUND_TEXT);
    expect(resolvedFrom("/src/round/RoundScreen.tsx", "./text")).toBe(ROUND_TEXT);
    expect(resolvedFrom("/src/health/codeRows.tsx", "../ui/text")).toBe("/src/ui/text");
    expect(resolvedFrom("/src/api/http.ts", "./round/text")).toBe("/src/api/round/text");
    expect(resolvedFrom("/src/api/http.ts", "react")).toBe("react");
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

  it("never words a proposal or a discrepancy as a hold on work (AC-15, narrowed by AC-11 of ui-health)", () => {
    const stem = "bl" + "ock";
    // These never, tests and mocks included.
    expect(offending(new RegExp(`${stem}ing|${stem}er|un${stem}`, "i"))).toEqual([]);
    // `spec check`'s verdict, a check outcome: only on the KnownCheckVerdict line of the provisional
    // types, under src/mocks/ and in tests.
    const mockOrTest = (path: string) => path.startsWith("/src/mocks/") || path.startsWith("/src/test/") || /\.test\.tsx?$/.test(path);
    const verdict = new RegExp(`${stem}ed`, "i");
    expect(offending(verdict, mockOrTest)).toEqual([
      expect.stringMatching(/^\/src\/api\/provisional\.ts:\d+: export type KnownCheckVerdict = \(typeof KNOWN_CHECK_VERDICTS\)\[number\]; export const KNOWN_CHECK_VERDICTS = \[/),
    ]);
    // Health's sources never spell the stem at all: they label a verdict through the tuple.
    const health = (path: string) => !path.startsWith("/src/health/") || /\.test\.tsx?$/.test(path);
    expect(Object.keys(sources).filter((path) => !health(path)).length).toBeGreaterThan(3);
    expect(offending(new RegExp(stem, "i"), health)).toEqual([]);
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

  it("takes every href in the views of spec data from routes.ts (AC-07 of ui-tree-node, AC-12 of ui-graph, AC-07 of ui-tasks, AC-07 of ui-home, AC-03 of ui-markdown, AC-07 of ui-health)", () => {
    const viewDirs = ["/src/tree/", "/src/inbox/", "/src/graph/", "/src/flow/", "/src/tasks/", "/src/overview/", "/src/palette/", "/src/markdown/", "/src/health/", "/src/roadmap/", "/src/board/", "/src/trace/", "/src/doc/"];
    const views = (path: string) => !viewDirs.some((dir) => path.startsWith(dir)) || /\.test\.tsx?$/.test(path);
    expect(Object.keys(sources).filter((path) => path.startsWith("/src/overview/") && !views(path)).length).toBeGreaterThan(0);
    // Flow's place is routes.ts's too (docs/features/writer-studio.md D4): `flowHash`; a document's page (docs/features/doc-modal.md DM-7): `docPageHash`.
    const rule = /\bhref=(?!\{(sectionHash|homeHash|flowHash|cardPageHash|docPageHash)\()/;
    expect(offending(rule, views)).toEqual([]);
    // The detector itself: a hash spelled by hand is caught, one from routes.ts is not.
    expect(rule.test("<a href={`#/${project}/tasks`}>")).toBe(true);
    expect(rule.test('<a href={"#/" + project}>')).toBe(true);
    expect(rule.test("<a href={place.href}>")).toBe(true);
    expect(rule.test('<a href={sectionHash(project, "tasks")}>')).toBe(false);
    expect(rule.test("<a href={homeHash(project)}>")).toBe(false);
    expect(rule.test("<a href={flowHash(project, place)}>")).toBe(false);
  });

  // DM-8 of docs/features/doc-modal.md: a document opens as a document (src/doc/DocLink.tsx) everywhere
  // but where the Spec tree is meant: these calls of `sectionHash(…, "tree", …)`, by file and count.
  it("links the Spec tree only where it is meant: the tree's own navigation, explicit acts, the `o` key, the ID-less records and the tree branches of Markdown and DriftPart (DM-8 of doc-modal)", () => {
    const TREE_LINKS: Readonly<Record<string, number>> = {
      // The tree's own navigation.
      "/src/tree/TextPanel.tsx": 1,
      "/src/tree/LinksPanel.tsx": 1,
      "/src/tree/TreeView.tsx": 2,
      "/src/tree/BundlePanel.tsx": 2,
      // Explicit acts: the Roadmap's Open in Spec tree, the Graph's, Flow's lost name; the `o` key in Graph and Flow.
      "/src/roadmap/RoadmapView.tsx": 1,
      "/src/graph/GraphDetails.tsx": 1,
      "/src/flow/FlowView.tsx": 2,
      "/src/graph/GraphView.tsx": 1,
      // The records without an ID live in the Spec tree (the owner's answer).
      "/src/flow/FlowRecords.tsx": 1,
      // Their branch taken inside the tree.
      "/src/markdown/Markdown.tsx": 1,
      "/src/health/DriftPart.tsx": 1,
      // The document view's own Open in Spec tree and its count of ID-less children.
      "/src/doc/DocBody.tsx": 1,
      "/src/doc/DocInside.tsx": 1,
    };
    const counted: Record<string, number> = {};
    for (const [path, text] of Object.entries(sources)) {
      // routes.ts builds the document's page on the tree's hash: the one definition, no link.
      if (!/\.tsx?$/.test(path) || isTestPath(path) || path === "/src/app/routes.ts") {
        continue;
      }
      const file = parsed(path, text);
      const walk = (node: ts.Node): void => {
        if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === "sectionHash") {
          const section = node.arguments[1];
          if (section !== undefined && ts.isStringLiteral(section) && section.text === "tree") {
            counted[path] = (counted[path] ?? 0) + 1;
          }
        }
        ts.forEachChild(node, walk);
      };
      walk(file);
    }
    expect(counted).toEqual(TREE_LINKS);
    expect(Object.values(TREE_LINKS).reduce((sum, count) => sum + count, 0)).toBe(16);
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
    const clock = new RegExp(["Date\\.now\\(", "new Date\\(", "performance\\.now\\(", "useNow", "formatAge\\b"].join("|"));
    expect(offending(clock, outside)).toEqual([]);
  });

  it("reads no clock in Health: the report carries no time, dates are shown as stored (AC-05 of ui-health)", () => {
    const outside = (path: string) => !path.startsWith("/src/health/") || /\.test\.tsx?$/.test(path);
    expect(Object.keys(sources).filter((path) => !outside(path)).length).toBeGreaterThan(3);
    const clock = new RegExp(["Date\\.now\\(", "new Date\\(", "Date\\.parse\\(", "performance\\.now\\(", "useNow", "formatAge\\b"].join("|"));
    expect(offending(clock, outside)).toEqual([]);
  });

  it("names no check code in Health but `budget`, and no budget slot (R6 of ui-health)", () => {
    const outside = (path: string) => !path.startsWith("/src/health/") || /\.test\.tsx?$/.test(path);
    // The core's CHECK_CODES (crates/specengine-core/src/check/mod.rs) but `budget`, the parser's
    // two error codes, and the five budget slots of `docs/canon/spec-check.md` "Findings, debt, verdict".
    const names = [
      "class-missing", "class-unknown", "key-missing", "key-extra", "scope-empty", "date-invalid", "status-invalid",
      "shipped-missing", "canon-missing", "tier-invalid", "id-width", "id-taken", "file-name", "canon-form", "canon-file",
      "canon-anchor", "ref-dangling", "name-skipped", "index-missing", "index-drift", "generator-unknown", "generator-path",
      "mention-dangling", "depends-cycle", "ref-superseded", "id-scope", "link-dangling", "link-anchor", "key-empty",
      "value-invalid", "part-missing", "part-empty", "text-empty", "parent-cycle", "homoglyph", "duplicate-id",
      "tier0", "tier1", "index", "decision", "canon",
    ];
    expect(offending(new RegExp(`["'\`](${names.join("|")})["'\`]`), outside)).toEqual([]);
    // `budget` itself is spelled once, as findings.ts's BUDGET_CODE; comments name it in backticks.
    expect(offending(/["']budget["']/, outside)).toEqual([expect.stringMatching(/^\/src\/health\/findings\.ts:\d+: export const BUDGET_CODE = "budget";$/)]);
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
      // ADR-0055's three (AC-07 of docs/features/flow-canvas.md): a flow's arrows, layers and arcs ask no type.
      "follows",
      "uses",
      "supplies",
    ];
    const appCode = (path: string) =>
      path.startsWith("/src/mocks/") || path.startsWith("/src/test/") || /\.test\.tsx?$/.test(path) || path.endsWith(".css");
    const quoted = new RegExp(`["'\`](${types.join("|")})["'\`]`);
    expect(offending(quoted, appCode)).toEqual([]);
    // The detector itself: the slice's mutation M5 is caught.
    expect(quoted.test('const walked = edge.type === "follows";')).toBe(true);
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
    expect(offending(compared, appCode, withoutFlowEdgeKind)).toEqual([]);
    // The exception itself (AC-07 of docs/features/flow-canvas.md): `edge.kind` under src/flow/ passes; `node.kind` there and `edge.kind` elsewhere stay red.
    const kindScan = (path: string, line: string) => compared.test(withoutFlowEdgeKind(path, line));
    expect(kindScan("/src/flow/labels.ts", 'if (edge.kind === "repeat") {')).toBe(false);
    expect(kindScan("/src/flow/labels.ts", "const dash = KIND_DASHES[edge.kind];")).toBe(false);
    expect(kindScan("/src/flow/layout.ts", 'if (node.kind === "step") {')).toBe(true);
    expect(kindScan("/src/flow/labels.ts", "const dash = DASHES[node.kind];")).toBe(true);
    expect(kindScan("/src/tree/labels.ts", 'if (edge.kind === "repeat") {')).toBe(true);
    expect(kindScan("/src/graph/layout.ts", "const dash = KIND_DASHES[edge.kind];")).toBe(true);
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
      // AC-U2 of docs/features/kind-interpretation.md: the Interpretation section follows the key, never the kind.
      '{review.kind === "interpretation" && <InterpretationSection project={project} interpretation={review.interpretation} />}',
      'const carries = review.kind === "interpretation";',
      // AC-U1 of docs/features/kind-amendment.md: the card and the stage dialog take an amendment as a new file, never by its kind.
      'const amends = review.kind === "amendment";',
      '{shown.kind !== "amendment" && <p className="dialog-text">This proposal has no options to choose from.</p>}',
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
      "{review.interpretation !== null && <InterpretationSection project={project} interpretation={review.interpretation} />}",
    ]) {
      expect([line, compared.test(line)]).toEqual([line, false]);
    }
  });

  it("compares no kind, class, status, state, sync, link type, reason, ID state, binding, relation or category in Health, the Bindings tab, the drift card, the tree's badges, the lock diff or the Flow but in the label maps: rows 3 to 6, binding rows, stored drift rows and gap rows shown raw or labelled, never branched on (AC-U6 of health-left, extended by UI-2 of health-code-ui, UI-3 of health-drift-ui, UI-7 of ui-node-bindings, UI-6 of ui-drift-evidence, UI-6 of ui-tree-drift, UI-7 of lock-diff-ui and AC-07 of flow-canvas)", () => {
    const flowSource = (path: string) => (path.startsWith("/src/flow/") || path.startsWith("/src/trace/")) && path !== "/src/flow/labels.ts";
    const outside = (path: string) => !(path.startsWith("/src/health/") || ROW_SOURCES.includes(path) || flowSource(path)) || /\.test\.tsx?$/.test(path);
    expect(Object.keys(sources).filter((path) => !outside(path))).toEqual(
      expect.arrayContaining([
        "/src/health/AmendmentsPart.tsx",
        "/src/health/AssumesPart.tsx",
        "/src/health/UnboundPart.tsx",
        "/src/health/DriftPart.tsx",
        "/src/health/codeRows.tsx",
        "/src/health/labels.ts",
        ...BINDINGS_TAB,
        ...DRIFT_CARD,
        ...TREE_DRIFT,
        ...LOCK_DIFF,
        "/src/flow/layout.ts",
        "/src/flow/FlowView.tsx",
        "/src/flow/FlowCanvas.tsx",
        "/src/flow/FlowGaps.tsx",
        "/src/trace/CardsBlock.tsx",
      ]),
    );
    expect(Object.keys(sources).filter((path) => !outside(path))).not.toContain("/src/flow/labels.ts");
    // `category` since ui-drift-evidence: a stored drift row's category is told apart in labels.ts alone.
    const field = "\\.(kind|class|status|state|sync|type|reason|id_state|binding|relation|category)\\b";
    const equality = "([!=]==?)(?!=)";
    const absent = "(null|undefined)\\b";
    const compared = new RegExp(
      [
        `${field}\\s*${equality}(?!\\s*${absent})`,
        `(?<!${absent}\\s*)${equality}\\s*[\\w.?]*${field}`,
        `switch\\s*\\([^)]*${field}`,
        `${field}\\s*\\?\\s*[^?.:\\s]`,
        `${field}\\s*(&&|\\|\\|)`,
        `\\[[^\\]]*${field}\\s*\\]`,
        `\\.(includes|has|get|indexOf|startsWith|endsWith|test|filter|find|some|every)\\([^)]*${field}`,
      ].join("|"),
    );
    expect(offending(compared, outside)).toEqual([]);
    // The detector itself: the slice's mutation and its kin are caught; showing a value or a presence check is not.
    for (const line of [
      'const amends = link.type === "amends";',
      'amendment.links.filter((link) => link.state === "resolved")',
      '{amendment.class !== "decision" && <p />}',
      "const tone = TONES[amendment.status];",
      'if (amendment.kind === "amendment") {',
      // UI-2 of health-code-ui: a row's state, a line's state, a reason, an ID state, a binding or a relation branched on.
      'if (row.state === "measured") {',
      'const record = line.state === "as_assumed" || line.state === "otherwise";',
      'marker.reason === "cannot_verify" ? a : b',
      "const word = WORDS[marker.id_state];",
      'marker.binding !== "item" && <p />',
      'rows.filter((line) => line.relation === "assumes")',
      "switch (row.state) {",
      // UI-3 of health-drift-ui: row 6's class, a row's state or an entry's state branched on.
      'const fix = row.class === "fix";',
      'row.class !== "cannot" && <p />',
      'entry.state === "stale" ? a : b',
      "const look = LOOKS[row.class];",
      "const look = LOOKS.get(row.class);",
      'rows.filter((row) => row.state === "conflict")',
      // UI-7 of ui-node-bindings: a binding row's sync branched on.
      'const fine = row.sync === "ok";',
      'row.sync !== "unlocked" && <p />',
      "const look = LOOKS[row.sync];",
      'rows.some((row) => row.sync === "conflict")',
      // UI-6 of ui-drift-evidence: a stored row's state or category branched on in the card.
      'const editorial = row.state === "spec_ahead" && row.category === "same_revision";',
      'typeof row.state === "string"',
      'const editorial = row.category === "same_revision";',
      'row.category !== "marker_behind" && <span>Editorial</span>',
      // UI-6 of ui-tree-drift: a drift row's class counted outside labels.ts.
      'if (line.class === "fix") {',
      "counts[line.class] += 1;",
      'const fixes = lines.filter((line) => line.class === "fix").length;',
      // AC-07 of flow-canvas: a link type, a gap state or an edge's kind branched on outside labels.ts.
      'const walked = edge.type === "follows";',
      'gaps.filter((gap) => gap.state === "measured")',
      'const clear = top.gaps.every((gap) => gap.state === "measured" && gap.count === 0);',
      'const dotted = edge.kind === "repeat";',
      "const dash = DASHES.get(edge.type);",
      // UI-7 of lock-diff-ui: a kept row's state branched on in the panel.
      'const both = entry.row.state === "conflict";',
      '{row.state !== "code_ahead" && <SpecSide side={spec} />}',
    ]) {
      expect([line, compared.test(line)]).toEqual([line, true]);
    }
    for (const line of [
      '<span className="kind-tag">{link.type}</span>',
      "<Badge name=\"State\" look={linkStateLook(link.state)} />",
      "{amendment.status !== null && <span>{amendment.status}</span>}",
      "<dd className=\"mono\">{amendment.class}</dd>",
      "<Badge name=\"State\" look={assumedLook(row.state).look} />",
      "if (codeRowLook(row.state).measured && row.counts !== null && row.rows !== null) {",
      "{item.reason !== null && <p>{item.reason}</p>}",
      "<span className=\"kind-tag\">{oneLine(marker.relation)}</span>",
      "<Badge name=\"Class\" look={driftClassLook(row.class)} />",
      "<Badge name=\"State\" look={syncLook(row.state)} />",
      "<Badge name=\"Sync\" look={syncLook(row.sync)} />",
      "{row.node === \"\" ? <span>{noId}</span> : <bdi className=\"mono\">{row.node}</bdi>}",
      "{holder.bindings === null ? <NotRead /> : <Rows />}",
      // ui-drift-evidence: the guard's keys by name, the label map's words passed in.
      'GUARANTEED.every(([key, fitting]) => fields.has(key) && fitting(fields.get(key)))',
      '["state", text],',
      'return state === "spec_ahead" && category === "same_revision";',
      "{markEditorial && isEditorialRow(row) && <span>Editorial</span>}",
      // A category shown or checked for presence, as row 6 and the test evidence do.
      "{entry.category !== null && <span className=\"mono health-code-word\">{oneLine(entry.category)}</span>}",
      "const judgement = [row.category, row.detail].filter((word) => word !== null).join(\" \");",
      // ui-tree-drift: the class passed to labels.ts, which tells it apart.
      "const known = driftClassOf(line.class);",
      // lock-diff-ui: a side's presence, and the row's key built from its fields as sent.
      "{spec !== null && <SpecSide side={spec} why={null} />}",
      "if (spec === null && code === null) {",
      "const binding = row.kind === null ? \"\" : ` | ${row.kind} ${row.symbol ?? \"\"}`;",
    ]) {
      expect([line, compared.test(line)]).toEqual([line, false]);
    }
  });

  it("sorts nothing in Health but the findings, nor in the Bindings tab, the drift card, the tree's badges or the lock diff, and keeps every label table in labels.ts: rows 3 to 6, binding rows, stored drift rows and kept rows drawn in the daemon's order, each word through one map (UI-3 of health-drift-ui, UI-2 of ui-node-bindings, UI-6 of ui-drift-evidence, UI-6 of ui-tree-drift, UI-7 of lock-diff-ui)", () => {
    const parts = (path: string) => !(path.startsWith("/src/health/") || ROW_SOURCES.includes(path)) || /\.test\.tsx?$/.test(path);
    expect(Object.keys(sources).filter((path) => !parts(path))).toEqual(expect.arrayContaining([...BINDINGS_TAB, ...DRIFT_CARD, ...TREE_DRIFT, ...LOCK_DIFF]));
    const sorted = /\.(sort|toSorted|reverse|toReversed)\(/;
    expect(offending(sorted, (path) => parts(path) || path === "/src/health/findings.ts")).toEqual([]);
    // The scope is right: the findings are grouped and ordered by the view (docs/features/ui-health.md).
    expect(sorted.test(sources["/src/health/findings.ts"] ?? "")).toBe(true);
    for (const line of ["const shown = [...rows].sort((a, b) => byBytes(a.state, b.state));", "rows.toSorted(byClass)", "drift.rows.slice().reverse()"]) {
      expect([line, sorted.test(line)]).toEqual([line, true]);
    }
    for (const line of ["<CodeRowList rows={rows} label={label} order={order} render={render} />", "const shown = all ? rows : rows.slice(0, CODE_ROWS_SHOWN);"]) {
      expect([line, sorted.test(line)]).toEqual([line, false]);
    }
    // A second table of words (a class's, a state's, an entry's) lives nowhere but labels.ts. A
    // heuristic: it sees a table typed `Record<…>` or checked with `satisfies`, not one built
    // another way (a Map, a switch); M8b's comparison scan above covers those.
    const table = /\bRecord<|\bsatisfies\b/;
    expect(offending(table, (path) => parts(path) || path === "/src/health/labels.ts")).toEqual([]);
    expect(table.test(sources["/src/health/labels.ts"] ?? "")).toBe(true);
    for (const line of ["const SYNC: Record<string, Look> = {", "} satisfies Record<string, Look>;"]) {
      expect([line, table.test(line)]).toEqual([line, true]);
    }
  });

  it("compares no release, priority, unit, status or lane name in the Roadmap: the plan's vocabulary is shown raw, never branched on (roadmap-screen)", () => {
    const outside = (path: string) => !path.startsWith("/src/roadmap/") || /\.test\.tsx?$/.test(path);
    expect(Object.keys(sources).filter((path) => !outside(path))).toEqual(
      expect.arrayContaining(["/src/roadmap/RoadmapView.tsx", "/src/roadmap/RoadmapLanes.tsx", "/src/roadmap/RoadmapChain.tsx", "/src/roadmap/text.ts"]),
    );
    const field = "\\.(release|priority|unit|status|name|mvpCut)\\b";
    const equality = "([!=]==?)(?!=)";
    const absent = "(null|undefined)\\b";
    const compared = new RegExp(
      [
        `${field}\\s*${equality}(?!\\s*${absent})`,
        `(?<!${absent}\\s*)${equality}\\s*[\\w.?]*${field}`,
        `switch\\s*\\([^)]*${field}`,
        `${field}\\s*\\?\\s*[^?.:\\s]`,
        `${field}\\s*(&&|\\|\\|)`,
        `\\[[^\\]]*${field}\\s*\\]`,
        `\\.(includes|has|indexOf|startsWith|endsWith|test|filter|find|some|every|sort)\\([^)]*${field}`,
      ].join("|"),
    );
    expect(offending(compared, outside)).toEqual([]);
    // The detector itself.
    for (const line of ['if (item.status === "shipped") {', "const rank = RANKS[item.priority];", 'group.name === "mvp" && <p />', "const kept = items.filter(inRelease(item.release));"]) {
      expect([line, compared.test(line)]).toEqual([line, true]);
    }
    for (const line of ["<Badge look={itemStatusLook(item.status)} />", "return release === null ? a : b;", "if (isRelease(group) && group.name !== null) {"]) {
      expect([line, compared.test(line)]).toEqual([line, false]);
    }
  });

  it("compares no column, flow, discipline or event word in the Board, labels.ts the one map; reads no clock but in due.ts, never a UTC date there (AC-06, AC-08 of board-ui)", () => {
    const boardSources = (path: string) => path.startsWith("/src/board/") && !/\.test\.tsx?$/.test(path);
    expect(Object.keys(sources).filter(boardSources)).toEqual(
      expect.arrayContaining([
        "/src/board/BoardView.tsx",
        "/src/board/BoardColumn.tsx",
        "/src/board/BoardCard.tsx",
        "/src/board/MoveMenu.tsx",
        "/src/board/CardPanel.tsx",
        "/src/board/CardDialogs.tsx",
        "/src/board/filter.ts",
        "/src/board/due.ts",
        "/src/board/labels.ts",
        // board-handoff (BH-15, R5): the modal, the page, the acts, the lanes and the rollup look the return column up in `flows[]`.
        "/src/board/CardModal.tsx",
        "/src/board/CardPage.tsx",
        "/src/board/CardActs.tsx",
        "/src/board/swimlanes.ts",
        "/src/board/rollup.ts",
      ]),
    );
    const field = "\\.(column|flow|discipline|type|from|to|name|id|title|key)\\b";
    const quote = "[\"'`]";
    const compared = new RegExp(
      [
        // A field against a spelled word; a field against another field of the read passes.
        `${field}\\s*[!=]==?\\s*${quote}`,
        `${quote}\\s*[!=]==?\\s*[\\w.?]*${field}`,
        `switch\\s*\\([^)]*${field}`,
        `${field}\\s*\\?\\s*[^?.:\\s]`,
        `\\[[^\\]]*${field}\\s*\\]`,
        `\\.(startsWith|endsWith)\\(${quote}`,
        // A spelled word; a template of the read's own values, `${row.flow}/${row.column}`, passes.
        `\\.(includes|has|get|indexOf)\\(${quote}(?!\\$\\{)`,
      ].join("|"),
    );
    const scanned = (path: string) => !boardSources(path) || path === "/src/board/labels.ts";
    // A keyboard event's key is the keyboard's, no project's word; a lane's key holds its flow and column.
    const withoutEventKey = (_path: string, line: string) => line.replace(/\bevent\.key\b/g, "event");
    expect(offending(compared, scanned, withoutEventKey)).toEqual([]);
    expect(compared.test(withoutEventKey("", 'if (event.key === "Escape") {'))).toBe(false);
    expect(compared.test(withoutEventKey("", "switch (event.key) {"))).toBe(false);
    // An event's type is spelled in labels.ts alone.
    expect(offending(/["'\`]card\.[a-z]+["'\`]/, scanned)).toEqual([]);
    // The detector itself: a branch on a project's word is caught, the look-ups the Board makes are not.
    // A return column spelled, the preset's or the mock's, is caught; the lookup the Board makes is not.
    for (const line of ['if (row.column === "sent-back") {', 'const back = card.column === "blocked";', 'if (lane.column !== "sent-back") {']) {
      expect([line, compared.test(line)]).toEqual([line, true]);
    }
    for (const line of ["if (row.column === flow.return) {", "const back = flowOf(row.flow)?.return ?? null;", "if (cell.column === back && row.column !== back) {"]) {
      expect([line, compared.test(line)]).toEqual([line, false]);
    }
    for (const line of ['if (row.column === "done") {', 'const late = "review" === card.column;', "switch (row.column) {", "const tone = TONES[row.column];", 'flow.name === "copy" && <p />', 'if (event.type === "card.moved") {', 'const hold = row.flow.includes("x");'.replace("row.flow.includes", "names.includes"), 'lane.title.startsWith("Done")', 'if (lane.column.id !== "review") {', 'if (lane.column.title !== "Done") {', 'const late = row.id === "C-0001";', 'const done = "Done" === column.title;', 'if (lane.key === "copy/column:done") {', 'const look = LOOKS[lane.key];']) {
      expect([line, compared.test(line)]).toEqual([line, true]);
    }
    for (const line of ["if (row.column === flow?.accept.from) {", "{card.column === flow?.accept.from && <button />}", 'const moves = flow.moves.filter((move) => move.from === column);', "<span>{columnTitle(flow, row.column)}</span>", "notes.filter((note) => note.includes(`${row.flow}/${row.column}`))"]) {
      expect([line, compared.test(line)]).toEqual([line, false]);
    }
    // No clock but due.ts; there the local date, never a UTC one.
    const clock = new RegExp(["Date\\.now\\(", "new Date\\(", "Date\\.parse\\(", "performance\\.now\\(", "useNow", "formatAge\\b"].join("|"));
    expect(offending(clock, (path) => !boardSources(path) || path === "/src/board/due.ts")).toEqual([]);
    expect(sources["/src/board/due.ts"]).toMatch(/useNow/);
    expect(sources["/src/board/due.ts"]).not.toMatch(/toISOString|getUTC/);
    // A second table of words: a typed record of labels or a Map literal lives in labels.ts alone.
    expect(offending(/\bnew Map\(\[/, scanned)).toEqual([]);
  });

  it("spells no column or flow the mock serves in the Board's code, and compares no bare column with a spelled word: a status category is the column's place, looked up (design-foundation)", () => {
    const mockBoard = mockCards["/src/mocks/cards.ts"]?.mockBoard;
    const scenarios = mockScenarios["/src/mocks/scenario.ts"]?.SCENARIOS ?? [];
    expect(mockBoard).toBeDefined();
    const words = new Set<string>();
    for (const scenario of scenarios) {
      for (const slug of ["harbor-sim", "ledger-api"]) {
        for (const flow of mockBoard?.(slug, scenario, 0).flows ?? []) {
          words.add(flow.name);
          for (const column of flow.columns) {
            words.add(column.id);
          }
        }
      }
    }
    expect([...words]).toEqual(expect.arrayContaining(["queued", "blocked", "checked", "struck", "draft", "proof", "filed", "copy", "plates"]));
    // The Board's own act word, the write named `edit` (CardActs.tsx, CardDialogs.tsx): code, never a column.
    const own = ["edit"];
    const banned = [...words].filter((word) => !own.includes(word));
    const boardSource = (path: string) => !path.startsWith("/src/board/") || /\.test\.tsx?$/.test(path);
    const quoted = new RegExp(`["'\`](${banned.map((word) => word.replace(/[.*+?^${}()|[\]\\-]/g, "\\$&")).join("|")})["'\`]`);
    expect(offending(quoted, boardSource)).toEqual([]);
    // A bare `column` (or `to`, `from`) against a spelled word, as the field form is caught above.
    const bare = /(?<![\w.])(column|to|from)\s*[!=]==?\s*["'`]|["'`]\s*[!=]==?\s*(column|to|from)\b/;
    expect(offending(bare, boardSource)).toEqual([]);
    // The detector itself: the category's mutation is caught, a look-up is not.
    for (const line of ['if (column === flow.return || column === "blocked") {', 'return "queued" === column ? a : b;', 'const late = to === "struck";', 'const word = "filed";']) {
      expect([line, quoted.test(line) || bare.test(line)]).toEqual([line, true]);
    }
    for (const line of ["if (column === flow.accept.to) {", 'send({ write: "edit", id: card.id });', "return column === flow.columns[0]?.id ? a : b;"]) {
      expect([line, quoted.test(line) || bare.test(line)]).toEqual([line, false]);
    }
  });

  it("branches on no rollup, stage or mark word in the Flow, the node's cards or the Board but in their labels.ts (AC-08 of trace-views)", () => {
    const labels = ["/src/flow/labels.ts", "/src/board/labels.ts"];
    // The document view's work state too (DM-12 of docs/features/doc-modal.md): a rollup looked up, never compared.
    const scoped = (path: string) => (path.startsWith("/src/flow/") || path.startsWith("/src/trace/") || path.startsWith("/src/board/") || path.startsWith("/src/doc/")) && !/\.test\.tsx?$/.test(path);
    expect(Object.keys(sources).filter(scoped)).toEqual(expect.arrayContaining(["/src/trace/CardsBlock.tsx", "/src/trace/TraceBar.tsx", "/src/flow/FlowPlaces.tsx", "/src/flow/FlowView.tsx", "/src/flow/timeline.ts", "/src/board/BoardCard.tsx", ...labels]));
    const outside = (path: string) => !scoped(path) || labels.includes(path);
    const field = "\\.(rollup|stage|mark)\\b";
    // The same two words destructured or passed in: `rollup === "x"` is the same branch as `node.rollup === "x"`.
    const bare = "(?<![\\w.])(rollup|mark)\\b";
    // A bare `stage` is the filter's prop and a lane's key, passed about freely: it is caught against a spelled word alone.
    const spelled = "(?<![\\w.])(rollup|mark|stage)\\b";
    const quote = "[\"'`]";
    const compared = new RegExp(
      [
        `${field}\\s*[!=]==?\\s*${quote}`,
        `${quote}\\s*[!=]==?\\s*[\\w.?]*${field}`,
        `switch\\s*\\([^)]*${field}`,
        `${field}\\s*\\?\\s*[^?.:\\s]`,
        `\\w\\[[^\\]]*${field}\\s*\\]`,
        `${field}\\.(includes|startsWith|endsWith)\\(`,
        `${spelled}\\s*[!=]==?\\s*${quote}`,
        `${quote}\\s*[!=]==?\\s*${spelled}`,
        `switch\\s*\\(\\s*${bare}`,
        `${bare}\\s*\\?\\s*[^?.:\\s]`,
        `\\w\\[\\s*${bare}\\s*\\]`,
      ].join("|"),
    );
    expect(offending(compared, outside)).toEqual([]);
    // No state's word is spelled there at all: not as a value, not in a table of its own. A box's
    // form is the layout's own closed word: its `"open"` says nothing of a mark.
    const withoutBoxForm = (_path: string, line: string) => line.replace(/\bform\b.*?["']open["']/g, "").replace(/\bBoxForm\b.*$/, "");
    const word = new RegExp(`${quote}(no card|in work|confirmed|suspect|departed|correcting|open|accepted)${quote}`);
    expect(offending(word, outside, withoutBoxForm)).toEqual([]);
    expect(word.test('const plain = "accepted";')).toBe(true);
    expect(word.test('const plain = word === "open";')).toBe(true);
    expect(word.test(withoutBoxForm("", 'if (box.form === "open") {'))).toBe(false);
    expect(word.test(withoutBoxForm("", 'export type BoxForm = "step" | "closed" | "open";'))).toBe(false);
    expect(word.test(withoutBoxForm("", 'form: cell.inner !== null ? "open" : "step",'))).toBe(false);
    // The detector itself: a branch on a word is caught, the lookups these screens make are not.
    for (const line of ['if (node.rollup === "confirmed") {', 'const late = "no card" !== step.rollup;', "switch (card.mark) {", "const tone = TONES[node.rollup];", 'const hollow = node.rollup === "no card";', 'lane.stage === "print" && <p />', 'const print = node.stage.startsWith("pr");', "const look = node.rollup ? a : b;", 'if (rollup === "confirmed") {', 'const plain = mark === "open";', '"accepted" !== mark && <p />', "switch (mark) {", "const look = rollup ? a : b;", "const tone = TONES[mark];", 'const print = stage === "print";', '"print" !== stage && <p />']) {
      expect([line, compared.test(line)]).toEqual([line, true]);
    }
    for (const line of ["<Badge look={rollupLook(node.rollup)} />", "{node.stage !== null && <span>{node.stage}</span>}", "!inStage(box.node.stage, stage)", "const hollow = !closed && node.rollup !== null && isNoCard(node.rollup);", "stageKey(lane.stage)", "new Map(nodes.map((node) => [node.id, node.rollup]))", "const shown = mark === null ? null : markLook(mark);", "return rollup === undefined ? null : <Badge look={rollupLook(rollup)} />;", "function TargetState({ rollup }: { rollup: string | undefined }) {", "const word = rollup ?? fallback;", "mark?: string;", "return stage === null ? NO_STAGE : `stage:${stage}`;", "!inStage(node.stage, stage)"]) {
      expect([line, compared.test(line)]).toEqual([line, false]);
    }
  });

  it("spells no workspace or module name, nor a kind a direction lists, the mock serves in app code: names and kinds are data, shown or sent, never compared (WS-6 (c) of workspaces; DP-11 of departments)", () => {
    const workspacesOf = mockWorkspaces["/src/mocks/workspaces.ts"]?.workspacesOf;
    const scenarios = mockScenarios["/src/mocks/scenario.ts"]?.SCENARIOS ?? [];
    expect(workspacesOf).toBeDefined();
    expect(scenarios.length).toBeGreaterThan(1);
    const names = new Set<string>();
    for (const scenario of scenarios) {
      for (const slug of ["harbor-sim", "ledger-api"]) {
        const served = workspacesOf?.(slug, scenario);
        // The implicit workspace's name is the core's constant, served alike to every project that configures none, never a project's.
        if (served === undefined || served.source !== "config") {
          continue;
        }
        for (const named of [...served.workspaces, ...served.modules]) {
          names.add(named.name);
        }
        for (const kind of served.modules.flatMap((module) => module.kinds ?? [])) {
          names.add(kind);
        }
      }
    }
    expect([...names].sort()).toEqual(["berths", "foundry", "galley", "mechanic", "mechanics", "waters"]);
    const appCode = (path: string) =>
      path.startsWith("/src/mocks/") || path.startsWith("/src/test/") || /\.test\.tsx?$/.test(path) || path.endsWith(".css");
    const quoted = new RegExp(`["'\`](${[...names].join("|")})["'\`]`);
    expect(offending(quoted, appCode)).toEqual([]);
    // The detector itself: the slice's M15 and its kin are caught; a name shown or sent is not.
    for (const line of ['if (workspace.name === "galley") {', "const lens = `foundry`;", "modules.find((module) => module.name === 'berths')", 'module.kinds?.includes("mechanic")', "if (chosen.name === 'mechanics') {"]) {
      expect([line, quoted.test(line)]).toEqual([line, true]);
    }
    for (const line of ["<option value={workspace.name}>{workspace.title}</option>", "moduleOf(document, workspace, stored)", 'const label = "Galley";']) {
      expect([line, quoted.test(line)]).toEqual([line, false]);
    }
  });

  it("holds no raw Cyrillic letter but in the Russian catalog: non-Latin test text is escaped (ADR-0024, ADR-0072; LG-8 of ui-languages)", () => {
    const cyrillic = new RegExp(`[${String.fromCodePoint(0x400)}-${String.fromCodePoint(0x4ff)}]`);
    expect(offending(cyrillic, (path) => path === RUSSIAN_CATALOG)).toEqual([]);
    // The exemption is the one file, whole: it holds Russian, and its neighbours are scanned.
    expect(cyrillic.test(sources[RUSSIAN_CATALOG] ?? "")).toBe(true);
    expect(Object.keys(sources)).toEqual(expect.arrayContaining(["/src/i18n/index.ts", "/src/i18n/en.ts", "/src/i18n/catalog.ts"]));
  });

  it("cites the task canon, never the task-package feature spec, whose blocks go at shipping (AC-13 of ui-live-tasks, WA-9)", () => {
    expect(offending(new RegExp(["docs", "features", "task-package[.]md"].join("/")))).toEqual([]);
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
        // ADR-0072 amends it again: 17 + 2 (docs/features/ui-languages.md LG-11).
        "i18next",
        "react-i18next",
      ].sort(),
    );
    expect([pinned["react-markdown"], pinned["remark-gfm"]]).toEqual(["10.1.0", "4.0.1"]);
    // Both in `dependencies`: they ship in the main chunk (D4).
    const runtime = record(pkg.dependencies);
    expect([runtime.i18next, runtime["react-i18next"]]).toEqual(["26.5.11", "17.1.9"]);
    for (const version of Object.values(pinned)) {
      expect(version).toMatch(/^\d+\.\d+\.\d+$/);
    }
    expect(pkg.packageManager).toMatch(/^pnpm@\d+\.\d+\.\d+$/);
    const scripts = Object.keys(record(pkg.scripts));
    expect(scripts.filter((name) => /install|prepare/.test(name))).toEqual([]);
  });
});

describe("languages policy (docs/features/ui-languages.md)", () => {
  it("imports i18next and react-i18next only in src/i18n, and src/i18n imports both (LG-11, D3)", () => {
    const i18nPackage = /^(i18next|react-i18next)(\/|$)/;
    const wrong: string[] = [];
    const inside = new Set<string>();
    for (const [path, text] of Object.entries(sources)) {
      for (const specifier of importsOf(text)) {
        if (!i18nPackage.test(specifier)) {
          continue;
        }
        if (path.startsWith("/src/i18n/")) {
          inside.add(specifier);
        } else {
          wrong.push(`${path} imports ${specifier}`);
        }
      }
    }
    expect(wrong).toEqual([]);
    expect([...inside].sort()).toEqual(expect.arrayContaining(["i18next", "react-i18next"]));
    // The lint rule says the same: the packages restricted, src/i18n/ exempt.
    expect(eslintConfig).toContain(JSON.stringify(i18nPackage.source.replaceAll("\\/", "/")));
    expect(eslintConfig).toMatch(/files:\s*\["src\/i18n\/\*\*"\]/);
    // The detector itself: a subpath and a type import are caught, a look-alike name is not.
    expect(["i18next", "react-i18next", "i18next/typescript", "react-i18next/x"].every((name) => i18nPackage.test(name))).toBe(true);
    expect(["i18next-http-backend", "my-i18next", "./i18n"].some((name) => i18nPackage.test(name))).toBe(false);
  });

  it("scans every screen: every src/ directory but the five, every non-test .tsx of the ten RM-0090 converted (LS-1 of ui-languages-sweep)", () => {
    expect(NOT_SCANNED).toEqual(["/src/api/", "/src/i18n/", "/src/mocks/", "/src/styles/", "/src/test/"]);
    const directories = new Set(Object.keys(sources).filter((path) => path.split("/").length > 3).map((path) => `${path.split("/").slice(0, 3).join("/")}/`));
    const scannedDirectories = [...directories].filter((directory) => Object.keys(sources).some((path) => path.startsWith(directory) && isScanned(path)));
    expect(scannedDirectories.sort()).toEqual([...directories].filter((directory) => !NOT_SCANNED.includes(directory)).sort());
    // The document view's directory from its start (DM-18 of docs/features/doc-modal.md).
    expect(scannedDirectories).toEqual(expect.arrayContaining(["/src/app/", "/src/palette/", "/src/overview/", "/src/ui/", "/src/workspace/", "/src/flow/", "/src/doc/"]));
    expect(Object.keys(sources).filter((path) => path.startsWith("/src/doc/") && path.endsWith(".tsx") && !isTestPath(path)).every(isScanned)).toBe(true);
    const swept = ["inbox", "tasks", "board", "roadmap", "tree", "graph", "health", "round", "trace", "markdown"].map((name) => `/src/${name}/`);
    const views = Object.keys(sources).filter((path) => swept.some((directory) => path.startsWith(directory)) && path.endsWith(".tsx") && !isTestPath(path));
    expect(views.length).toBeGreaterThan(70);
    expect(views.filter((path) => !isScanned(path))).toEqual([]);
    expect(views).toEqual(expect.arrayContaining(["/src/tree/LinksPanel.tsx", "/src/trace/CardsBlock.tsx", "/src/health/HealthView.tsx", "/src/round/RoundScreen.tsx"]));
    // The bootstrap at src/ itself is scanned; the tests are not.
    expect([isScanned("/src/main.tsx"), isScanned("/src/main.test.tsx"), isScanned("/src/i18n/en.ts"), isScanned("/src/api/http.ts")]).toEqual([true, false, false, false]);
  });

  it("holds no word of their own in the scanned .tsx files' JSX (LG-10)", () => {
    const found: string[] = [];
    for (const [path, text] of Object.entries(sources)) {
      if (path.endsWith(".tsx") && isScanned(path)) {
        found.push(...literalWords(path, text));
      }
    }
    expect(found).toEqual([]);
  });

  it("holds no prose of their own in the scanned .ts and .tsx files (LS-2 of ui-languages-sweep)", () => {
    const found: string[] = [];
    for (const [path, text] of Object.entries(sources)) {
      if (isScanned(path)) {
        found.push(...proseWords(path, text));
      }
    }
    expect(found).toEqual([]);
    // Every exemption names a file that holds it: a stale one goes.
    for (const [path, literals] of Object.entries(PROSE_ALLOWED)) {
      const file = parsed(path, sources[path] ?? "");
      const held: string[] = [];
      const walk = (node: ts.Node): void => {
        const words = literalText(node);
        if (words !== null) {
          held.push(words);
        }
        ts.forEachChild(node, walk);
      };
      walk(file);
      expect([path, literals.filter((literal) => !held.includes(literal))]).toEqual([path, []]);
    }
  });

  it("sees the prose LS-2 forbids and passes keys, code tokens, comparisons, labels and classes (the prose detector)", () => {
    const prose = (text: string) => proseWords("/src/board/Probe.tsx", text).map((hit) => hit.replace(/^[^ ]+ /, ""));
    // M2: `label="Title"` restored on a field; M3: a lane name restored in a table.
    expect(prose('export const A = () => <Field label="Title" />;')).toEqual(['prose "Title"']);
    expect(prose('const NAMES = { pool: "Pool" };')).toEqual(['prose "Pool"']);
    expect(prose("const line = `${n} open, ${m} done`;")).toEqual(['prose "  open,   done"']);
    expect(prose('const error = new Error("the card could not be moved");')).toEqual(['prose "the card could not be moved"']);
    // M4: the code-token exemption is the text's start, never the word anywhere.
    expect(prose('const hint = "Open the spec in your editor";')).toEqual(['prose "Open the spec in your editor"']);
    expect(prose("const hint = `Ask your AI: spec ${id}`;")).toEqual(['prose "Ask your AI: spec  "']);
    // A code method's first argument alone is code: the value it sets, stores, adds or puts in is read.
    expect(
      prose(
        [
          'el.setAttribute("aria-label", "Close panel");',
          'map.set(key, "Shown text");',
          's.replace(x, "Some words");',
          's.replaceAll(/x/g, "Other words");',
          'window.localStorage.setItem(KEY, "Saved words");',
          'names.add("Added words");',
        ].join("\n"),
      ),
    ).toEqual(['prose "Close panel"', 'prose "Shown text"', 'prose "Some words"', 'prose "Other words"', 'prose "Saved words"']);
    // Keys, commands, comparisons, case labels, classes and the code positions pass.
    expect(
      prose(
        [
          'const a = t("roadmap.pool");',
          "const b = `spec approve ${id}`;",
          'const c = x === "Release";',
          'switch (key) { case "Escape": break; }',
          'export const D = () => <p className="a b-c" id="Main panel" role="status">{t("x.y")}</p>;',
          'const e = row.querySelector("button:not([disabled])");',
          'const f = text.split(", ").join("; ");',
          'type G = "Release" | "Pool";',
          'const h = { "Release": 1 };',
          'const REVIEW_COMMAND = "approve or reject";',
          'import { x } from "./Some module";',
          'const i = "Release" in groups;',
          "const j = { className: `card ${state} card-wide` };",
        ].join("\n"),
      ),
    ).toEqual([]);
  });

  it("reaches the Russian catalog through one import(\"./ru\") in src/i18n/index.ts, and no non-test module imports it statically (LS-7 of ui-languages-sweep)", () => {
    const statics: string[] = [];
    const dynamics: string[] = [];
    for (const [path, text] of Object.entries(sources)) {
      if (!/\.tsx?$/.test(path) || isTestPath(path)) {
        continue;
      }
      const found = russianImports(path, text);
      statics.push(...found.static);
      dynamics.push(...found.dynamic);
    }
    expect(statics).toEqual([]);
    expect(dynamics).toEqual(['/src/i18n/index.ts: import("./ru")']);
    // The detector itself: M11's static import is caught, a type-only import is not.
    expect(russianImports("/src/i18n/catalog.ts", 'import { ru } from "./ru";').static).toHaveLength(1);
    expect(russianImports("/src/app/Shell.tsx", 'export { ru } from "../i18n/ru";').static).toHaveLength(1);
    expect(russianImports("/src/i18n/catalog.ts", 'import type { ru } from "./ru";').static).toEqual([]);
    expect(russianImports("/src/i18n/index.ts", 'const m = await import("./ru.ts");').dynamic).toHaveLength(1);
  });

  it("covers the Flow studio's views: every src/flow/*.tsx is scanned, and a literal word in one is caught (ST-14 of writer-studio)", () => {
    const flowViews = Object.keys(sources).filter((path) => path.startsWith("/src/flow/") && path.endsWith(".tsx") && !isTestPath(path));
    expect(flowViews).toEqual(expect.arrayContaining(["/src/flow/FlowView.tsx", "/src/flow/FlowCanvas.tsx", "/src/flow/FlowPanel.tsx", "/src/flow/FlowPlaces.tsx", "/src/flow/FlowList.tsx", "/src/flow/FlowGaps.tsx", "/src/flow/FlowLegend.tsx"]));
    expect(flowViews.every(isScanned)).toBe(true);
    // M29: a literal `Open` restored in a view.
    const words = (text: string) => literalWords("/src/flow/Probe.tsx", text).map((hit) => hit.replace(/^[^ ]+ /, ""));
    expect(words("export const A = () => <a href={flowHash(project, place)}>Open</a>;")).toEqual(['text "Open"']);
    expect(words('export const A = () => <span>{t("flow.openGroup", { name })}</span>;')).toEqual([]);
  });

  it("sees each literal form LG-10 forbids and passes keys, data and code (the detector)", () => {
    const words = (text: string) => literalWords("/src/app/Probe.tsx", text).map((hit) => hit.replace(/^[^ ]+ /, ""));
    // M25: the literal restored in DataSource.tsx.
    expect(words('export const A = () => <span>Mock data</span>;')).toEqual(['text "Mock data"']);
    expect(words('export const A = ({ on }: { on: boolean }) => <p>{on ? "Retrying" : t("ui.retry")}</p>;')).toEqual(['child "Retrying"']);
    expect(words('export const A = () => <p>{n > 0 && " or "}</p>;')).toEqual(['child " or "']);
    expect(words('export const A = () => <input placeholder="Section, task" aria-label={`Jump ${x}`} />;')).toEqual(['<input placeholder> "Section, task"', '<input aria-label> "Jump "']);
    expect(words('export const A = () => <Skeleton label="Opening the first project" lines={3} />;')).toEqual(['<Skeleton label> "Opening the first project"']);
    expect(words('export const A = () => <Badge name="State" look={look} />;')).toEqual(['<Badge name> "State"']);
    expect(words('export const A = () => <ErrorPanel title={kept ? "Kept" : "Not kept"} message={m} onRetry={r} />;')).toEqual(['<ErrorPanel title> "Kept"', '<ErrorPanel title> "Not kept"']);
    // Keys to `t`, data, a comparison's operand, a class or an ID are not words shown.
    expect(
      words(
        'export const A = () => <div className="mono" id={`${id}-hint`} title={t("workspace.workspaceHint")}>{t("app.brand")} {project.name ?? project.slug} {x.type === "section" ? <b>{t("app.overview")}</b> : null} : </div>;',
      ),
    ).toEqual([]);
  });

  it("reads no letter hotkey from .key alone: a comparison, a switch case, a lookup list (LG-7)", () => {
    const found: string[] = [];
    for (const [path, text] of Object.entries(sources)) {
      if (/\.tsx?$/.test(path) && !isTestPath(path)) {
        found.push(...letterKeyReads(path, text));
      }
    }
    expect(found).toEqual([]);
  });

  it("sees each letter read LG-7 forbids and passes the keys that stay on .key (the detector)", () => {
    const reads = (text: string) => letterKeyReads("/src/x.tsx", text).length;
    // M18: `event.key === "c"` kept in FlowCanvas.tsx.
    expect(reads('if (event.key === "c") { centre(); }')).toBe(1);
    expect(reads('if ("n" !== event.key) { return; }')).toBe(1);
    // M20: a new `case "x":` under `switch (event.key)`.
    expect(reads('switch (event.key) { case "ArrowDown": case "x": move(); }')).toBe(1);
    expect(reads('if (["ArrowDown", "j"].includes(event.key)) { move(); }')).toBe(1);
    expect(reads('if (event.key === "?" || event.key === "Escape" || event.key === "Enter") { open(); }')).toBe(0);
    expect(reads('switch (hotkeyOf(event)) { case "j": move(); } if (letterOf(event) === "n") { open(); }')).toBe(0);
  });
});
