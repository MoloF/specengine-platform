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
    expect(enT("workspace.flowsShown", { shown: 1, count: 2 })).toBe("Showing 1 of 2 card flows.");
  });

  it("prints a number ungrouped in both languages", () => {
    expect(enT("ui.time.minutesAgo", { count: 1000 })).toBe("1000 min ago");
    expect(ruT("ui.time.minutesAgo", { count: 1000 })).toBe(ru.ui.time.minutesAgo_many.replace("{{count}}", "1000"));
  });
});
