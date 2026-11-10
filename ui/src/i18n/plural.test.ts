import { describe, expect, it } from "vitest";
import { enT, ruT } from "../test/text";
import { en } from "./en";
import { ru } from "./ru";

// LG-4 of docs/features/ui-languages.md, counts: `t(key, {count})` takes the form Intl.PluralRules
// names. A form's expected text is its catalog value filled, so an owner's rewording breaks nothing;
// Russian's three whole-number forms of the key read here differ, so each is told from the others (M10).

function filled(value: string, shown: number, count: number): string {
  return value.replace("{{shown}}", String(shown)).replace("{{count}}", String(count));
}

describe("plural forms (LG-4)", () => {
  it("are three distinct texts in Russian for whole numbers, the key read here", () => {
    // `_other` (fractions) reads as `_few` in Russian; `_one`, `_few` and `_many` must differ.
    const forms = [ru.workspace.flowsShown_one, ru.workspace.flowsShown_few, ru.workspace.flowsShown_many];
    expect(new Set(forms).size).toBe(3);
    expect(ru.workspace.flowsShown_other).not.toBe(ru.workspace.flowsShown_many);
  });

  it.each([1, 21, 101])("takes _one in Russian at %i", (count) => {
    expect(ruT("workspace.flowsShown", { shown: 0, count })).toBe(filled(ru.workspace.flowsShown_one, 0, count));
  });

  it.each([2, 3, 4, 22])("takes _few in Russian at %i", (count) => {
    expect(ruT("workspace.flowsShown", { shown: 1, count })).toBe(filled(ru.workspace.flowsShown_few, 1, count));
  });

  it.each([0, 5, 11, 12, 14, 25, 111])("takes _many in Russian at %i", (count) => {
    expect(ruT("workspace.flowsShown", { shown: 1, count })).toBe(filled(ru.workspace.flowsShown_many, 1, count));
  });

  it("takes _one in English at 1 and _other at 0, 2 and 21", () => {
    expect(enT("workspace.flowsShown", { shown: 0, count: 1 })).toBe(filled(en.workspace.flowsShown_one, 0, 1));
    for (const count of [0, 2, 21]) {
      expect(enT("workspace.flowsShown", { shown: 1, count })).toBe(filled(en.workspace.flowsShown_other, 1, count));
    }
    // English stays as the module it replaced said it.
    expect(enT("workspace.flowsShown", { shown: 1, count: 2 })).toBe("Showing 1 of 2 flows.");
  });

  it("prints a number ungrouped in both languages", () => {
    expect(enT("ui.time.minutesAgo", { count: 1000 })).toBe("1000 min ago");
    expect(ruT("ui.time.minutesAgo", { count: 1000 })).toBe(ru.ui.time.minutesAgo_many.replace("{{count}}", "1000"));
  });
});

/** Every leaf of a catalog by its dotted path. */
function leavesOf(messages: object, prefix = ""): Map<string, string> {
  const out = new Map<string, string>();
  for (const [key, value] of Object.entries(messages) as [string, unknown][]) {
    const path = prefix === "" ? key : `${prefix}.${key}`;
    if (typeof value === "string") {
      out.set(path, value);
    } else if (typeof value === "object" && value !== null) {
      for (const [inner, text] of leavesOf(value, path)) {
        out.set(inner, text);
      }
    }
  }
  return out;
}

/** A value with `count` and every other placeholder filled. */
function filledAll(value: string, count: number): string {
  return value.replace(/\{\{(\w+)\}\}/g, (_match, name: string) => (name === "count" ? String(count) : `<${name}>`));
}

describe("every plural group (LS-5 of ui-languages-sweep)", () => {
  const english = leavesOf(en);
  const russian = leavesOf(ru);
  const groups = [...english.keys()].filter((key) => key.endsWith("_other")).map((key) => key.slice(0, -"_other".length));
  /** `t` over a key only known at run time, every placeholder but `count` its own name in brackets. */
  const ruAny = ruT as unknown as (key: string, options: Record<string, unknown>) => string;

  it("are many: the sweep's groups among them", () => {
    expect(groups.length).toBeGreaterThan(90);
    expect(groups).toEqual(expect.arrayContaining(["roadmap.open", "trace.usesCards", "board.column.count", "health.findings.counted", "tree.search.count"]));
  });

  it.each([
    [1, "one"],
    [21, "one"],
    [3, "few"],
    [5, "many"],
  ] as const)("take the Russian form Intl names at %i (%s)", (count, form) => {
    const wrong: string[] = [];
    for (const base of groups) {
      const value = russian.get(`${base}_${form}`) ?? "";
      const options: Record<string, unknown> = { count };
      for (const match of value.matchAll(/\{\{(\w+)\}\}/g)) {
        const name = match[1] ?? "";
        if (name !== "count") {
          options[name] = `<${name}>`;
        }
      }
      const shown = ruAny(base, options);
      if (shown !== filledAll(value, count)) {
        wrong.push(`${base} at ${String(count)}: ${shown}`);
      }
    }
    expect(wrong).toEqual([]);
  });
});

// DM-17 of docs/features/doc-modal.md (RM-0091): the English `_one` forms that read wrong at 1, fixed;
// at 2 each reads as before; Russian keeps its forms.
describe("English at one (DM-17 of doc-modal, RM-0091)", () => {
  /** Each key, its options, the fixed form at 1 and the form at 2, unchanged. */
  const FIXED: readonly (readonly [string, Record<string, unknown>, string, string])[] = [
    ["board.view.omitted", {}, "1 task not shown.", "2 tasks not shown."],
    ["tree.links.omitted", {}, "1 link not shown", "2 links not shown"],
    ["tree.binding.tokens", {}, "1 token", "2 tokens"],
    ["graph.holders", {}, "1 holder", "2 holders"],
    ["tree.node.holders", { ref: "R" }, "1 holder: R is declared in one place. It is shown below.", "2 holders: R is declared in more than one place. Each is shown below."],
    ["graph.details.moreText", { column: 3 }, "The column at distance 3 holds 1 more box than the canvas draws; the List holds it.", "The column at distance 3 holds 2 more boxes than the canvas draws; the List holds every one."],
    ["tree.view.cut", {}, "Showing the first 1 node: the list was cut", "Showing the first 2 nodes: the list was cut"],
    ["tree.commands.verifyOnly", { ref: "R", drift: "D", accept: "A" }, "Only verify is offered: R names 1 node, and D and A each name one.", "Only verify is offered: R names 2 nodes, and D and A each name one."],
    ["graph.limit", { last: 4, shown: 1 }, "The canvas shows distance 0-4: 1 of 1 node; see the List.", "The canvas shows distance 0-4: 1 of 2 nodes; see the List."],
    ["flow.notDrawn", { hidden: 1 }, "1 of 1 box not drawn: choose a stage, enter a group, or read the List", "1 of 2 boxes not drawn: choose a stage, enter a group, or read the List"],
    ["roadmap.notDrawn", { hidden: 1 }, "1 of 1 item not drawn", "1 of 2 items not drawn"],
    ["workspace.flowsShown", { shown: 1 }, "Showing 1 of 1 flow.", "Showing 1 of 2 flows."],
    ["health.findings.totalShown", { shown: 1 }, "1 of 1 finding shown, by code, errors first", "1 of 2 findings shown, by code, errors first"],
  ];
  const enAny = enT as unknown as (key: string, options: Record<string, unknown>) => string;
  const ruAny = ruT as unknown as (key: string, options: Record<string, unknown>) => string;
  const russian = leavesOf(ru);

  it.each(FIXED)("%s reads right at 1, as before at 2 (M: tree.links.omitted_one restored)", (key, options, one, two) => {
    expect(enAny(key, { ...options, count: 1 })).toBe(one);
    expect(enAny(key, { ...options, count: 2 })).toBe(two);
  });

  it("leaves Russian its own forms", () => {
    for (const [key, options] of FIXED) {
      const value = russian.get(`${key}_one`) ?? "";
      expect([key, ruAny(key, { ...options, count: 1 }) === filledAll(value, 1).replace(/<(\w+)>/g, (_match, name: string) => String(options[name]))]).toEqual([key, true]);
    }
  });
});
