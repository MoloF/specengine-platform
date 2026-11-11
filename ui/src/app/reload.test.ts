import * as ts from "typescript";
import { describe, expect, it } from "vitest";
import { installReload, onPreloadError, RELOAD_MARK, type ReloadDeps } from "./reload";

// docs/features/canvas-clicks.md "Acceptance criteria", CK6 and CK7: a lazy chunk gone after a
// redeploy reloads the page once per build, never when the mark cannot be read or written, never
// preventing the event; installed once, at the bootstrap, before the language's chunk is asked for.

const BUILD = "/assets/index-NEW.js";

/** A sessionStorage stand-in holding `mark`, and how it may fail. */
function fakeStorage(mark: string | null, fails: { get?: boolean; set?: boolean } = {}) {
  const stored = new Map<string, string>(mark === null ? [] : [[RELOAD_MARK, mark]]);
  return {
    stored,
    getItem(key: string): string | null {
      if (fails.get === true) {
        throw new DOMException("The operation is insecure.", "SecurityError");
      }
      return stored.get(key) ?? null;
    },
    setItem(key: string, value: string): void {
      if (fails.set === true) {
        throw new DOMException("The quota has been exceeded.", "QuotaExceededError");
      }
      stored.set(key, value);
    },
  };
}

/** Deps over `storage`, counting the reloads. */
function depsOf(storage: ReturnType<typeof fakeStorage>): ReloadDeps & { reloads: () => number } {
  let count = 0;
  return {
    storage: () => storage,
    reload: () => {
      count += 1;
    },
    reloads: () => count,
  };
}

describe("onPreloadError: once per build (CK6)", () => {
  it("1. a first failure, no mark: reloads once, stores the build, returns true", () => {
    const storage = fakeStorage(null);
    const deps = depsOf(storage);
    expect(onPreloadError(BUILD, deps)).toBe(true);
    expect(deps.reloads()).toBe(1);
    expect(storage.stored.get(RELOAD_MARK)).toBe(BUILD);
    expect(RELOAD_MARK).toBe("specengine.reloaded");
  });

  it("2. a mark for this build: no reload", () => {
    const storage = fakeStorage(BUILD);
    const deps = depsOf(storage);
    // M: the guard removed.
    expect(onPreloadError(BUILD, deps)).toBe(false);
    expect(deps.reloads()).toBe(0);
    expect(storage.stored.get(RELOAD_MARK)).toBe(BUILD);
  });

  it("3. a mark for another build: a reload, the mark now this build", () => {
    const storage = fakeStorage("/assets/index-OLD.js");
    const deps = depsOf(storage);
    expect(onPreloadError(BUILD, deps)).toBe(true);
    expect(deps.reloads()).toBe(1);
    expect(storage.stored.get(RELOAD_MARK)).toBe(BUILD);
  });

  it("4. getItem throws: no reload, nothing thrown", () => {
    const deps = depsOf(fakeStorage(null, { get: true }));
    expect(onPreloadError(BUILD, deps)).toBe(false);
    expect(deps.reloads()).toBe(0);
  });

  it("5. setItem throws: no reload, nothing thrown", () => {
    const storage = fakeStorage(null, { set: true });
    const deps = depsOf(storage);
    // M: a reload after the write threw.
    expect(onPreloadError(BUILD, deps)).toBe(false);
    expect(deps.reloads()).toBe(0);
    expect(storage.stored.size).toBe(0);
  });

  it("6. storage() itself throws: no reload, nothing thrown", () => {
    let count = 0;
    const deps: ReloadDeps = {
      storage: () => {
        throw new DOMException("The operation is insecure.", "SecurityError");
      },
      reload: () => {
        count += 1;
      },
    };
    expect(onPreloadError(BUILD, deps)).toBe(false);
    expect(count).toBe(0);
  });

  it("7. through installReload, a dispatched cancelable vite:preloadError is never defaultPrevented; a second one, the mark this build's, reloads nothing", () => {
    const target = new EventTarget();
    const storage = fakeStorage(null);
    const deps = depsOf(storage);
    installReload(target, BUILD, deps);
    const first = new Event("vite:preloadError", { cancelable: true });
    // M: preventDefault called.
    expect(target.dispatchEvent(first)).toBe(true);
    expect(first.defaultPrevented).toBe(false);
    expect(deps.reloads()).toBe(1);
    const second = new Event("vite:preloadError", { cancelable: true });
    target.dispatchEvent(second);
    expect(second.defaultPrevented).toBe(false);
    expect(deps.reloads()).toBe(1);
  });
});

const sources = import.meta.glob<string>("/src/**/*.{ts,tsx}", { query: "?raw", import: "default", eager: true });

/** The app's own sources: no test, no declaration file. */
const appSources = Object.entries(sources).filter(([path]) => !/\.test\.tsx?$/.test(path) && !path.endsWith(".d.ts"));

function parsed(path: string, text: string): ts.SourceFile {
  return ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, path.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
}

/** Every node of `file` that `pick` takes, in source order. */
function nodesOf<T extends ts.Node>(file: ts.SourceFile, pick: (node: ts.Node) => node is T): T[] {
  const found: T[] = [];
  const walk = (node: ts.Node) => {
    if (pick(node)) {
      found.push(node);
    }
    ts.forEachChild(node, walk);
  };
  walk(file);
  return found;
}

const EVENT = ["vite", "preloadError"].join(":");

function isEventLiteral(node: ts.Node): node is ts.StringLiteral | ts.NoSubstitutionTemplateLiteral {
  return (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) && node.text === EVENT;
}

function callsOf(name: string) {
  return (node: ts.Node): node is ts.CallExpression => ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === name;
}

describe("installed once, at the bootstrap (CK7)", () => {
  it("names the event as a literal once in the app's sources, in reload.ts's addEventListener call", () => {
    const found = appSources.flatMap(([path, text]) => {
      const file = parsed(path, text);
      return nodesOf(file, isEventLiteral).map((literal) => {
        const call = literal.parent;
        const method = ts.isCallExpression(call) && ts.isPropertyAccessExpression(call.expression) ? call.expression.name.text : "";
        return `${path} ${method}`;
      });
    });
    expect(found).toEqual(["/src/app/reload.ts addEventListener"]);
  });

  it("lets no failure handler stand on a bare import(): Vite would move it inside its preload wrapper, before the event; Russian's import stands alone, its handlers on its result", () => {
    const isImport = (node: ts.Node): node is ts.CallExpression => ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword;
    const handled = appSources.flatMap(([path, text]) => {
      const file = parsed(path, text);
      return nodesOf(file, isImport).flatMap((call) => {
        const access = call.parent;
        const chained = ts.isPropertyAccessExpression(access) && access.expression === call ? access : undefined;
        const outer = chained?.parent;
        const catches = chained !== undefined && outer !== undefined && ts.isCallExpression(outer) && (chained.name.text === "catch" || (chained.name.text === "then" && outer.arguments.length > 1));
        return catches ? [`${path}: ${call.getText(file)}.${chained.name.text}`] : [];
      });
    });
    // M: `import("./ru").then(ok, fail)` again in i18n/index.ts.
    expect(handled).toEqual([]);
    const index = parsed("/src/i18n/index.ts", sources["/src/i18n/index.ts"] ?? "");
    const russian = nodesOf(index, isImport).filter((call) => call.arguments[0]?.getText(index) === '"./ru"');
    expect(russian).toHaveLength(1);
    const declaration = russian[0]?.parent;
    expect(declaration !== undefined && ts.isVariableDeclaration(declaration) && declaration.initializer === russian[0]).toBe(true);
  });

  it("calls installReload once in the app, in main.tsx, a statement of its own before readyLanguage()", () => {
    const calls = appSources.flatMap(([path, text]) => nodesOf(parsed(path, text), callsOf("installReload")).map(() => path));
    // M: the call removed from main.tsx.
    expect(calls).toEqual(["/src/main.tsx"]);
    const main = parsed("/src/main.tsx", sources["/src/main.tsx"] ?? "");
    const [install] = nodesOf(main, callsOf("installReload"));
    const [ready] = nodesOf(main, callsOf("readyLanguage"));
    expect(install?.parent.parent).toBe(main);
    expect(install?.arguments.map((argument) => argument.getText(main)).slice(0, 2)).toEqual(["window", "import.meta.url"]);
    expect((install?.getStart(main) ?? Infinity) < (ready?.getStart(main) ?? -Infinity)).toBe(true);
  });
});
