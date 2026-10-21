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

/** The Bindings tab's sources (docs/features/ui-node-bindings.md): held to Health's rules for rows (UI-7). */
const BINDINGS_TAB: readonly string[] = ["/src/tree/BindingsPanel.tsx", "/src/tree/BindingLine.tsx", "/src/tree/commands.ts"];

/** The drift card's sources (docs/features/ui-drift-evidence.md): its rows held to Health's rules too (UI-6). */
const DRIFT_CARD: readonly string[] = ["/src/inbox/drift.ts", "/src/inbox/DriftItem.tsx"];

/** Health, the Bindings tab and the drift card: the row sources AC-U6 and the no-sort rule cover. */
const ROW_SOURCES: readonly string[] = [...BINDINGS_TAB, ...DRIFT_CARD];

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
    expect(exported("/src/ui/text.ts")).toEqual(["shownSlot", "shownText"]);
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
    const viewDirs = ["/src/tree/", "/src/inbox/", "/src/graph/", "/src/tasks/", "/src/overview/", "/src/palette/", "/src/markdown/", "/src/health/", "/src/roadmap/"];
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

  it("reads no clock in Health: the report carries no time, dates are shown as stored (AC-05 of ui-health)", () => {
    const outside = (path: string) => !path.startsWith("/src/health/") || /\.test\.tsx?$/.test(path);
    expect(Object.keys(sources).filter((path) => !outside(path)).length).toBeGreaterThan(3);
    const clock = new RegExp(["Date\\.now\\(", "new Date\\(", "Date\\.parse\\(", "performance\\.now\\(", "useNow\\b", "formatAge\\b"].join("|"));
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

  it("compares no kind, class, status, state, sync, link type, reason, ID state, binding, relation or category in Health, the Bindings tab or the drift card but in the label maps: rows 3 to 6, binding rows and stored drift rows shown raw or labelled, never branched on (AC-U6 of health-left, extended by UI-2 of health-code-ui, UI-3 of health-drift-ui, UI-7 of ui-node-bindings and UI-6 of ui-drift-evidence)", () => {
    const outside = (path: string) => !(path.startsWith("/src/health/") || ROW_SOURCES.includes(path)) || /\.test\.tsx?$/.test(path);
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
      ]),
    );
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
    ]) {
      expect([line, compared.test(line)]).toEqual([line, false]);
    }
  });

  it("sorts nothing in Health but the findings, nor in the Bindings tab or the drift card, and keeps every label table in labels.ts: rows 3 to 6, binding rows and stored drift rows drawn in the daemon's order, each word through one map (UI-3 of health-drift-ui, UI-2 of ui-node-bindings, UI-6 of ui-drift-evidence)", () => {
    const parts = (path: string) => !(path.startsWith("/src/health/") || ROW_SOURCES.includes(path)) || /\.test\.tsx?$/.test(path);
    expect(Object.keys(sources).filter((path) => !parts(path))).toEqual(expect.arrayContaining([...BINDINGS_TAB, ...DRIFT_CARD]));
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

  it("holds no raw Cyrillic letter: non-Latin test text is escaped (ADR-0024)", () => {
    const cyrillic = new RegExp(`[${String.fromCodePoint(0x400)}-${String.fromCodePoint(0x4ff)}]`);
    expect(offending(cyrillic)).toEqual([]);
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
