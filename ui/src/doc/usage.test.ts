import { describe, expect, it } from "vitest";
import { aDirection, aLink, aModule, aUsage } from "../test/builders";
import { STAMINA_PATH, staminaLinks, usedRule } from "../test/usageStub";
import { insideDirections, type KindSet } from "./inside";
import { usageLayout, type UsageUser } from "./usage";

// UO10 of docs/features/usage-orphans.md "Acceptance criteria", the layout alone: the served In rows
// gathered by source, each source once with every place, grouped as Inside groups children; the
// rows of the holder's own file apart. Named mutations: a group counting rows, not names; a same-file
// row counted in its kind's group.

/** A set's label and how many users it holds. */
function labelled(set: KindSet<UsageUser>): string {
  return `${set.by === "direction" ? set.title : (set.kind ?? "none")} · ${String(set.entries.length)}`;
}

describe("who uses a document (UO10)", () => {
  it("MEC-STAMINA: its two users by raw kind, code-point order, each once, the sets summing to the served used_by (M: a group counting rows, not names)", () => {
    const links = staminaLinks();
    const { sets, within } = usageLayout(links, STAMINA_PATH, []);
    expect(sets.map(labelled)).toEqual(["decision · 1", "mechanic · 1"]);
    expect(sets.reduce((sum, set) => sum + set.entries.length, 0)).toBe(links.usage.used_by);
    expect(within).toEqual([]);
  });

  it("each user with every place it cites from, in file order, each with its raw link type and the section it lands on; one source, two types; mentions never a user (M: the type dropped)", () => {
    const { sets } = usageLayout(staminaLinks(), STAMINA_PATH, []);
    const users = sets.flatMap((set) => set.entries);
    expect(users.map((user) => [user.name, user.title, user.places.map((place) => `${place.path}:${String(place.line)} ${place.type}${place.at === null ? "" : ` at ${place.at}`}`)])).toEqual([
      ["DEC-0023", "Regeneration waits for rest", ["docs/records/DEC/DEC-0023.md:6 canon at RULE-STAM-REGEN"]],
      ["MEC-SPRINT", "Sprint", ["docs/spec/movement/sprint.md:9 depends_on", "docs/spec/movement/sprint.md:10 constrains at RULE-STAM-REGEN"]],
    ]);
    expect(users.flatMap((user) => user.places).some((place) => place.line === 16)).toBe(false);
  });

  it("directions first, in served order, each holding the kinds it is the first to list; then raw kinds; no kind last", () => {
    const directions = insideDirections([aModule({ name: "shore", root: "DOC-A" }), aDirection({ name: "systems", title: "Systems", kinds: ["mechanic"] })]);
    const links = staminaLinks();
    links.incoming.push(aLink({ type: "uses", written: "MEC-STAMINA", name: "docs/notes/loose.md", title: "Loose notes", end_kind: null, path: "docs/notes/loose.md", line: 3 }));
    const { sets } = usageLayout(links, STAMINA_PATH, directions);
    expect(sets.map(labelled)).toEqual(["Systems · 1", "decision · 1", "none · 1"]);
  });

  it("a row written in the holder's own file stands apart, outside the counted sets (M: a same-file row counted in its kind's group)", () => {
    const links = usedRule(2);
    const { sets, within } = usageLayout(links, "docs/spec/rules.md", []);
    expect(sets.map(labelled)).toEqual(["mechanic · 2"]);
    expect(sets.reduce((sum, set) => sum + set.entries.length, 0)).toBe(links.usage.used_by);
    expect(within.map((user) => [user.name, user.kind])).toEqual([["RULE-B", "rule"]]);
  });

  it("a row that does not resolve is no user, nor a weak one", () => {
    const links = { ...staminaLinks(), incoming: [aLink({ type: "uses", written: "MEC-GONE", name: null, state: "dangling", path: "docs/a.md", line: 2 }), aLink({ type: "mentions", written: "MEC-STAMINA", name: "Q-1", path: "docs/q.md", line: 4 })], usage: aUsage() };
    expect(usageLayout(links, STAMINA_PATH, [])).toEqual({ sets: [], within: [] });
  });
});
