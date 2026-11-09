import { describe, expect, it } from "vitest";
import { enT } from "../test/text";
import { isLanguage, LANGUAGES, type Messages } from "./catalog";
import { en } from "./en";
import { ru } from "./ru";

// LG-1 and LG-9 of docs/features/ui-languages.md: the two catalogs hold the same keys, plural
// groups in each language's forms, the same placeholders, `{{count}}` in plural forms alone, plain
// text; `t` is typed by the catalog. The checks run on the catalogs and, as their own proof, on
// copies with one fault each (M1 to M4).

const PLURAL = /_(one|few|many|other)$/;
const PLACEHOLDER = /\{\{([a-z][A-Za-z0-9]*)\}\}/g;
const SEGMENT = /^[a-z][A-Za-z0-9_]*$/;
const FIRST_SEGMENTS = ["app", "palette", "home", "ui", "workspace", "board", "flow", "health", "inbox", "tasks", "tree", "roadmap", "round", "graph", "trace", "markdown", "language"];

/** Every leaf by its dotted path. */
function flat(messages: Messages, prefix = ""): Map<string, string> {
  const out = new Map<string, string>();
  for (const [key, value] of Object.entries(messages)) {
    const path = prefix === "" ? key : `${prefix}.${key}`;
    if (typeof value === "string") {
      out.set(path, value);
    } else {
      for (const [inner, text] of flat(value, path)) {
        out.set(inner, text);
      }
    }
  }
  return out;
}

function placeholders(value: string): string[] {
  return [...value.matchAll(PLACEHOLDER)].map((match) => match[1] ?? "").sort();
}

/** The plural groups by their base key, each with the forms the catalog holds. */
function pluralGroups(leaves: ReadonlyMap<string, string>): Map<string, string[]> {
  const groups = new Map<string, string[]>();
  for (const key of leaves.keys()) {
    const match = PLURAL.exec(key);
    if (match !== null) {
      const base = key.slice(0, match.index);
      groups.set(base, [...(groups.get(base) ?? []), match[1] ?? ""].sort());
    }
  }
  return groups;
}

/** What is wrong with a pair of catalogs, one line per fault; empty when they agree. */
function problemsOf(english: Messages, russian: Messages): string[] {
  const enLeaves = flat(english);
  const ruLeaves = flat(russian);
  const problems: string[] = [];
  const plain = (leaves: ReadonlyMap<string, string>) => [...leaves.keys()].filter((key) => !PLURAL.test(key));
  for (const key of plain(enLeaves)) {
    if (!ruLeaves.has(key)) {
      problems.push(`ru lacks ${key}`);
    }
  }
  for (const key of plain(ruLeaves)) {
    if (!enLeaves.has(key)) {
      problems.push(`ru adds ${key}`);
    }
  }
  const enGroups = pluralGroups(enLeaves);
  const ruGroups = pluralGroups(ruLeaves);
  for (const [base, forms] of enGroups) {
    if (forms.join() !== "one,other") {
      problems.push(`en ${base} has ${forms.join()}`);
    }
    if (ruGroups.get(base)?.join() !== "few,many,one,other") {
      problems.push(`ru ${base} has ${ruGroups.get(base)?.join() ?? "nothing"}`);
    }
  }
  for (const base of ruGroups.keys()) {
    if (!enGroups.has(base)) {
      problems.push(`ru adds the plural ${base}`);
    }
  }
  // Placeholders: as English's, a plural form's as English's `_other`.
  const englishOf = (key: string) => enLeaves.get(key) ?? (PLURAL.test(key) ? enLeaves.get(key.replace(PLURAL, "_other")) : undefined);
  for (const [key, value] of ruLeaves) {
    const source = englishOf(key);
    if (source !== undefined && placeholders(value).join() !== placeholders(source).join()) {
      problems.push(`ru ${key} holds {${placeholders(value).join()}}, en {${placeholders(source).join()}}`);
    }
  }
  for (const [base] of enGroups) {
    const one = enLeaves.get(`${base}_one`) ?? "";
    const other = enLeaves.get(`${base}_other`) ?? "";
    if (placeholders(one).join() !== placeholders(other).join()) {
      problems.push(`en ${base}_one and _other differ in placeholders`);
    }
  }
  for (const [language, leaves] of [["en", enLeaves] as const, ["ru", ruLeaves] as const]) {
    for (const [key, value] of leaves) {
      const counted = placeholders(value).includes("count");
      if (PLURAL.test(key) !== counted) {
        problems.push(`${language} ${key}: {{count}} ${counted ? "outside a plural form" : "missing from a plural form"}`);
      }
      if (value === "") {
        problems.push(`${language} ${key} is empty`);
      }
      if (/<[A-Za-z/]/.test(value)) {
        problems.push(`${language} ${key} holds markup`);
      }
      if (value.includes("$t(")) {
        problems.push(`${language} ${key} nests a key`);
      }
      if (value.replace(PLACEHOLDER, "").includes("{{")) {
        problems.push(`${language} ${key} holds a {{ that opens no placeholder`);
      }
    }
  }
  return problems;
}

/** A deep copy with `edit` applied to the object holding the last segment of `path`. */
function changed(messages: Messages, path: string, edit: (holder: Record<string, string | Messages>, key: string) => void): Messages {
  const copy = structuredClone(messages) as Record<string, string | Messages>;
  const segments = path.split(".");
  const last = segments.pop() ?? "";
  let holder = copy;
  for (const segment of segments) {
    holder = holder[segment] as Record<string, string | Messages>;
  }
  edit(holder, last);
  return copy;
}

describe("the catalogs (LG-1)", () => {
  it("hold the same keys, forms and placeholders, as plain text", () => {
    expect(problemsOf(en, ru)).toEqual([]);
  });

  it("are the two languages the select offers", () => {
    expect(LANGUAGES).toEqual(["en", "ru"]);
    expect([isLanguage("en"), isLanguage("ru"), isLanguage("ru-RU"), isLanguage("uk"), isLanguage("")]).toEqual([true, true, false, false, false]);
  });

  it("name their keys by English segments, the first a src directory, no plural form but the four", () => {
    for (const leaves of [flat(en), flat(ru)]) {
      for (const key of leaves.keys()) {
        const segments = key.split(".");
        expect([key, segments.every((segment) => SEGMENT.test(segment))]).toEqual([key, true]);
        expect([key, FIRST_SEGMENTS.includes(segments[0] ?? "")]).toEqual([key, true]);
        expect([key, /_(zero|two|ordinal)/.test(key)]).toEqual([key, false]);
      }
    }
  });

  it("see each fault in a copy (the checks' own proof: M1 to M4)", () => {
    // M1: a Russian key deleted.
    expect(problemsOf(en, changed(ru, "app.mockData", (holder, key) => {
      Reflect.deleteProperty(holder, key);
    }))).toEqual(["ru lacks app.mockData"]);
    // M2: a Russian placeholder renamed.
    expect(problemsOf(en, changed(ru, "workspace.notesButton", (holder, key) => (holder[key] = "({{count2}})")))).toEqual([
      "ru workspace.notesButton holds {count2}, en {n}",
    ]);
    // M3: a Russian `_many` dropped.
    expect(problemsOf(en, changed(ru, "workspace.flowsShown_many", (holder, key) => {
      Reflect.deleteProperty(holder, key);
    }))).toEqual(["ru workspace.flowsShown has few,one,other"]);
    // M4: `{{count}}` in a key that is not plural.
    const counted = (catalog: Messages) => changed(catalog, "workspace.notesButton", (holder, key) => (holder[key] = "Config notes ({{count}})"));
    expect(problemsOf(counted(en), counted(ru))).toEqual(["en workspace.notesButton: {{count}} outside a plural form", "ru workspace.notesButton: {{count}} outside a plural form"]);
    // Markup, nesting and a stray `{{`.
    expect(problemsOf(en, changed(ru, "app.mockData", (holder, key) => (holder[key] = "<b>x</b>")))).toEqual(["ru app.mockData holds markup"]);
    expect(problemsOf(en, changed(ru, "app.mockData", (holder, key) => (holder[key] = "$t(app.brand)")))).toEqual(["ru app.mockData nests a key"]);
    expect(problemsOf(en, changed(ru, "app.mockData", (holder, key) => (holder[key] = "{{ x")))).toEqual(["ru app.mockData holds a {{ that opens no placeholder"]);
  });
});

describe("typed keys (LG-9)", () => {
  it("accepts a key of the catalog and refuses one it lacks at compile time", () => {
    expect(enT("app.mockData")).toBe("Mock data");
    // @ts-expect-error: no such key; `tsc --noEmit` fails once keys are untyped (M24).
    expect(enT("app.nope")).toBe("app.nope");
  });
});

/**
 * The keys `fill.ts` `around()` reads: a sentence holding a count that is drawn, the words around it
 * read out. `around` cuts at the first place the count's digits stand, so the count's placeholder
 * must come first in every language, no digit before it. Each by the placeholder it searches for.
 */
const AROUND: Readonly<Record<string, string>> = {
  "tasks.groupCount": "count",
  "board.column.count": "count",
  "health.left.queueTotal": "count",
  "health.amendments.total": "count",
  "health.code.rowsOf": "count",
  "health.unboundRow.total": "unbound",
  "health.drift.total": "shown",
  "health.findings.counted": "count",
};

const viewSources = import.meta.glob<string>(["/src/**/*.tsx", "!/src/**/*.test.tsx"], { query: "?raw", import: "default", eager: true });

describe("the sentences around() cuts (fill.ts)", () => {
  it("are every key a view passes to around(), a Counted's phrase included", () => {
    const read = new Set<string>();
    for (const text of Object.values(viewSources)) {
      for (const match of text.matchAll(/around\(\s*t\("([\w.]+)"|said=\{t\("([\w.]+)"/g)) {
        read.add(match[1] ?? match[2] ?? "");
      }
    }
    expect([...read].sort()).toEqual(Object.keys(AROUND).sort());
  });

  it("put the searched count first in every form of every language, no digit before it", () => {
    const wrong: string[] = [];
    for (const [language, leaves] of [["en", flat(en)] as const, ["ru", flat(ru)] as const]) {
      for (const [key, searched] of Object.entries(AROUND)) {
        const forms = [...leaves.entries()].filter(([path]) => path === key || path.replace(PLURAL, "") === key);
        if (forms.length === 0) {
          wrong.push(`${language} ${key}: no value`);
        }
        for (const [path, value] of forms) {
          const first = /\{\{([a-z][A-Za-z0-9]*)\}\}/.exec(value);
          if (first?.[1] !== searched || /\d/.test(value.slice(0, first.index))) {
            wrong.push(`${language} ${path}: ${value}`);
          }
        }
      }
    }
    expect(wrong).toEqual([]);
  });
});
