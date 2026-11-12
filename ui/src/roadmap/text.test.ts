import { describe, expect, it } from "vitest";
import { en } from "../i18n/en";
import { ru } from "../i18n/ru";
import { aRoadmap, aRoadmapGroup, aRoadmapItem } from "../test/builders";
import { aPlan, PLAN_LINE } from "../test/roadmapStub";
import { enT, ruT } from "../test/text";
import { countsText, headerLine, itemStatusLozenge, laneName } from "./text";

// LS-5 of docs/features/ui-languages-sweep.md: the Roadmap's lines in the CLI's form (R7) keep their
// IDs, numbers and `-`, their words the catalog's, each count its own plural group. The expected
// Russian is the catalog's own values filled, so a rewording breaks nothing; each count is checked in
// the form Intl names for it (M8: `count === 1 ?` in text.ts).

/** A value with its placeholders filled. */
function filled(value: string, values: Record<string, string | number>): string {
  return value.replace(/\{\{(\w+)\}\}/g, (_match, name: string) => String(values[name]));
}

/** A plan with `open` open items in one release, `done` done, no cut. */
function planOf(open: number, done: number) {
  const items = Array.from({ length: open }, (_, index) => aRoadmapItem({ id: `RM-${String(index + 1).padStart(4, "0")}`, estimate: { min: 1, max: 1 } }));
  return aRoadmap([aRoadmapGroup("release", "beta", items, { current: true, done })]);
}

describe("the Roadmap's lines in Russian (LS-5)", () => {
  it.each([
    [1, ru.roadmap.open_one, ru.roadmap.done_few],
    [3, ru.roadmap.open_few, ru.roadmap.done_few],
    [5, ru.roadmap.open_many, ru.roadmap.done_few],
  ])("counts %i open in the header and the lane's counts", (open, openForm, doneForm) => {
    const plan = planOf(open, 2);
    const counts = filled(ru.roadmap.counts, { open: filled(openForm, { count: open }), done: filled(doneForm, { count: 2 }) });
    const group = plan.groups[0];
    expect(group).toBeDefined();
    if (group === undefined) {
      return;
    }
    expect(countsText(group, ruT)).toBe(counts);
    expect(headerLine(plan, ruT)).toBe(filled(ru.roadmap.header, { counts, current: "beta", total: `${String(open)} days` }));
  });

  it("keeps the IDs, numbers and dashes of the cut and the sums, the words Russian", () => {
    const plan = aPlan();
    const header = headerLine(plan, ruT);
    const unestimated = (sum: string) => filled(ru.roadmap.sumUnestimated_one, { sum, count: 1 });
    expect(header).toBe(
      filled(ru.roadmap.headerCut, {
        counts: filled(ru.roadmap.counts, { open: filled(ru.roadmap.open_many, { count: 13 }), done: filled(ru.roadmap.done_many, { count: 5 }) }),
        current: "beta",
        release: "beta",
        cut: unestimated("10-12 days"),
        total: unestimated("19-24 days"),
      }),
    );
    // English stays as `spec roadmap` prints it.
    expect(headerLine(plan, enT)).toBe(PLAN_LINE);
  });

  it("names the pool and the unplaced lanes in the language shown, a release by its own name", () => {
    const [alpha, , , pool, unplaced] = aPlan().groups;
    expect([alpha, pool, unplaced].map((group) => (group === undefined ? null : laneName(group, ruT)))).toEqual(["alpha", ru.roadmap.group.pool, ru.roadmap.group.unplaced]);
    expect([alpha, pool, unplaced].map((group) => (group === undefined ? null : laneName(group, enT)))).toEqual(["alpha", en.roadmap.group.pool, en.roadmap.group.unplaced]);
    expect(laneName(aRoadmapGroup("release", null, []), ruT)).toBe(ru.roadmap.group.release);
    expect(laneName(aRoadmapGroup("someday", null, []), ruT)).toBe("someday");
  });
});

// docs/features/design-sweep-b.md "Acceptance criteria", DB4: an item's status is the plan's word,
// raw and neutral; no word is mapped, a done one neither.
describe("an item's status lozenge (DB4 of design-sweep-b)", () => {
  it.each(["done", "shipped", "open", "in-progress", "parked", "\u0413\u043e\u0442\u043e\u0432\u043e"])("draws %s raw and neutral with the info icon, no lang: the project's own word", (status) => {
    expect(itemStatusLozenge(status)).toEqual({ appearance: "neutral", icon: "info", label: status });
  });

  it("draws an absent status as `-`, a status of several lines on one", () => {
    expect([itemStatusLozenge(null).label, itemStatusLozenge("a\r\nb").label]).toEqual(["-", "a b"]);
  });
});
