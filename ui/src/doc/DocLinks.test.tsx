import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ApiProvider, createQueryClient } from "../api/provider";
import { queryKeys } from "../api/queries";
import type { ShownLinks } from "../api/types";
import appCss from "../styles/app.css?raw";
import { aDirection, aLink, aUsage, aWorkspace, aWorkspaces } from "../test/builders";
import { stubClient } from "../test/stubClient";
import { STAMINA_PATH, staminaLinks, usedRule } from "../test/usageStub";
import { DocLinks } from "./DocLinks";
import { USED_BY_SHOWN } from "./usage";

// UO10 of docs/features/usage-orphans.md "Acceptance criteria", the document view's Links on the
// stub: MEC-STAMINA as spec-a serves it shows the daemon's counts and its users by kind, the counts
// summing to the served used_by, each user with every place; an unused document says so with no
// group; a row of the document's own file stands under Within this document; a group's 13th user
// only after Show all. Named mutations: a group counting rows, not names; the zero line keyed on the
// rows, not the served count; a same-file row counted in its kind's group.

/** The Links part for `links`, the Shell's workspaces read held as given (none: raw kinds). */
function links(shown: ShownLinks, path = STAMINA_PATH, held = aWorkspaces([aWorkspace({ name: "writing" })])) {
  const queryClient = createQueryClient();
  queryClient.setQueryData(queryKeys.workspaces("alpha"), held);
  render(
    <ApiProvider client={stubClient()} queryClient={queryClient}>
      <DocLinks project="alpha" links={shown} path={path} level={3} />
    </ApiProvider>,
  );
  return within(screen.getByRole("region", { name: "Links" }));
}

/** In, the group of who uses the document. */
function into(part: ReturnType<typeof links>) {
  return within(part.getByRole("group", { name: "Links in" }));
}

describe("who uses a document (UO10)", () => {
  it("MEC-STAMINA: Used by 2 · Mentioned by 6, then its users by kind, the groups summing to the served count (M: a group counting rows, not names)", () => {
    const shown = into(links(staminaLinks()));
    expect(shown.getByText("Used by 2 · Mentioned by 6").className).toBe("doc-usage-counts");
    const groups = shown.getAllByRole("group").map((group) => group.getAttribute("aria-labelledby") ?? "");
    const labels = groups.map((id) => document.getElementById(id)?.textContent);
    expect(labels).toEqual(["decision · 1", "mechanic · 1"]);
    expect(labels.reduce((sum, label) => sum + Number(label?.split(" · ")[1]), 0)).toBe(2);
    expect(document.getElementById(groups[0] ?? "")?.querySelector("bdi")?.getAttribute("lang")).toBe("en");
    expect(shown.queryByText("Declared, nothing uses it yet.")).toBeNull();
  });

  it("each user a document link by title with every place it cites from: path:line in mono, then the link's raw type, neutral, lang=en, then the section it lands on; one source, two types (M: the type dropped)", () => {
    const shown = into(links(staminaLinks()));
    const sprint = shown.getByRole("link", { name: "Sprint" });
    expect(sprint.getAttribute("href")).toBe("#/alpha/tree/MEC-SPRINT/page");
    const places = Array.from(sprint.closest("li")?.querySelectorAll(".doc-usage-place") ?? []);
    expect(places.map((place) => place.textContent)).toEqual(["docs/spec/movement/sprint.md:9 depends_on", "docs/spec/movement/sprint.md:10 constrains at RULE-STAM-REGEN"]);
    expect(places.map((place) => place.querySelector(".mono")?.textContent)).toEqual(["docs/spec/movement/sprint.md:9", "docs/spec/movement/sprint.md:10"]);
    const types = places.map((place) => place.querySelector(".lozenge"));
    expect(types.map((type) => [type?.textContent, type?.getAttribute("data-appearance"), type?.querySelector(".lozenge-label")?.getAttribute("lang")])).toEqual([
      ["depends_on", "neutral", "en"],
      ["constrains", "neutral", "en"],
    ]);
    expect(places[0]?.firstElementChild?.className).toBe("mono");
    expect(shown.getByRole("link", { name: "Regeneration waits for rest" }).closest("li")?.querySelector(".doc-usage-place")?.textContent).toBe("docs/records/DEC/DEC-0023.md:6 canon at RULE-STAM-REGEN");
  });

  it("Mentions and Out unchanged: the six mentions' rows folded under Mentions, never a user", () => {
    const part = links(staminaLinks());
    expect(part.getByRole("button", { name: "Mentions (7)" }).getAttribute("aria-expanded")).toBe("false");
    expect(into(part).queryByRole("link", { name: "Movement" })).toBeNull();
  });

  it("a direction groups its kinds first, under its title in the project's words", () => {
    const held = aWorkspaces([aWorkspace({ name: "writing" })], [aDirection({ name: "systems", title: "Systems", kinds: ["mechanic"] })]);
    const shown = into(links(staminaLinks(), STAMINA_PATH, held));
    const labels = shown.getAllByRole("group").map((group) => document.getElementById(group.getAttribute("aria-labelledby") ?? "")?.textContent);
    expect(labels).toEqual(["Systems · 1", "decision · 1"]);
  });

  it("an unused document: the served counts, then Declared, nothing uses it yet, no counted group, whatever In rows stand: an archived user's row the daemon does not count, its own file's row (M: the zero line keyed on the rows)", () => {
    // `--archive` widens In and never the counts (UO2): a row stands that the served 0 does not count.
    const shown = into(links({ ...usedRule(1), usage: aUsage({ used_by: 0, mentioned_by: 3 }) }, "docs/spec/rules.md"));
    expect(shown.getByText("Used by 0 · Mentioned by 3")).toBeTruthy();
    expect(shown.getByText("Declared, nothing uses it yet.").className).toBe("doc-usage-unused");
    const groups = shown.getAllByRole("group");
    expect(groups.map((group) => document.getElementById(group.getAttribute("aria-labelledby") ?? "")?.textContent)).toEqual(["Within this document"]);
    expect(shown.queryByRole("link", { name: "User 1" })).toBeNull();
  });

  it("a used document whatever its rows: the served count drawn, never one counted here", () => {
    const shown = into(links({ ...usedRule(2), usage: aUsage({ used_by: 3, mentioned_by: 0 }) }, "docs/spec/rules.md"));
    expect(shown.getByText("Used by 3 · Mentioned by 0")).toBeTruthy();
    expect(shown.queryByText("Declared, nothing uses it yet.")).toBeNull();
  });

  it("no link at all: No links., and Declared, nothing uses it yet, by the served count", () => {
    const part = links({ outgoing: [], incoming: [], usage: aUsage(), left_out: { generated: 0, tier3: 0 }, omitted: 0 });
    expect(part.getByText("No links.")).toBeTruthy();
    expect(part.getByText("Declared, nothing uses it yet.")).toBeTruthy();
    expect(part.queryByRole("group")).toBeNull();
  });

  it("a row written in the document's own file stands last, under Within this document, outside the count (M: a same-file row counted in its kind's group)", () => {
    const shown = into(links(usedRule(2), "docs/spec/rules.md"));
    const groups = shown.getAllByRole("group");
    expect(groups.map((group) => document.getElementById(group.getAttribute("aria-labelledby") ?? "")?.textContent)).toEqual(["mechanic · 2", "Within this document"]);
    expect(within(groups[1] as HTMLElement).getByRole("link", { name: "The neighbour rule" })).toBeTruthy();
    expect(within(groups[0] as HTMLElement).queryByRole("link", { name: "The neighbour rule" })).toBeNull();
  });

  it(`a group shows ${String(USED_BY_SHOWN)} users, then Show all; the 13th only after it, focus on it`, async () => {
    const shown = into(links(usedRule(14), "docs/spec/rules.md"));
    const group = shown.getAllByRole("group")[0] as HTMLElement;
    expect(within(group).getAllByRole("link")).toHaveLength(USED_BY_SHOWN);
    expect(within(group).queryByRole("link", { name: "User 13" })).toBeNull();
    fireEvent.click(within(group).getByRole("button", { name: "Show all 14" }));
    await waitFor(() => {
      expect(document.activeElement).toBe(within(group).getByRole("link", { name: "User 13" }));
    });
    expect(within(group).getAllByRole("link")).toHaveLength(14);
    expect(within(group).queryByRole("button", { name: /^Show all/ })).toBeNull();
  });

  it("no group, place or label widens the view: titles, labels and paths wrap anywhere", () => {
    for (const selector of [".doc-usage-label", ".doc-usage-link", ".doc-usage-place"]) {
      const rule = new RegExp(`(^|\\n)${selector.replace(".", "\\.")}[^{]*\\{[^}]*overflow-wrap:\\s*anywhere;`);
      expect([selector, rule.test(appCss)]).toEqual([selector, true]);
    }
    expect(aLink({ type: "uses", written: "X" }).state).toBe("resolved");
  });
});
