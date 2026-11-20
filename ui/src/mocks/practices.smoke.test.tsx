import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "../app/App";
import { docPageHash } from "../app/routes";
import { MockClient } from "./MockClient";
import type { Scenario } from "./scenario";

// docs/features/department-practices.md "Acceptance criteria", DP21, over the mock as `pnpm dev` shows
// it (src/mocks/practices.ts): harbor-sim's copy desk holds three practices (the mooring's spec page,
// one rule since its second sits inside the first, the harbour's and the tides' canon pages), its plates desk two; C-0003's goal cites two
// IDs the corpus resolves, one of them again inside code, and one it does not; C-0015's return note
// cites the night passage; a review cites what its texts name. `practices-unread`: the index under
// them cannot be read, both null and the daemon's note. ledger-api's tasks have no discipline. The
// stub cases run beside the part (src/practices/); the mock is imported here alone (policy AC-05).

function renderMock(hash: string, scenario: Scenario = "normal") {
  window.localStorage.setItem("specengine.module.harbor-sim", "");
  window.history.replaceState(null, "", `/?scenario=${scenario}${hash}`);
  const client = new MockClient(scenario, { delayMs: 0 });
  render(<App client={client} scenario={scenario === "normal" ? null : scenario} />);
  return client;
}

/** A card's modal, by its title, its side column read. */
async function modal(title: string): Promise<HTMLElement> {
  const dialog = await screen.findByRole("dialog", { name: title });
  await within(dialog).findByRole("region", { name: "Details" });
  return dialog;
}

/** Each link of a scope: its words and its target. */
function links(scope: HTMLElement): [string, string][] {
  return within(scope)
    .queryAllByRole("link")
    .map((link) => [link.textContent, link.getAttribute("href") ?? ""]);
}

describe("practices and cited links over the mock (DP21)", () => {
  it("serves a card's practices by path with their rule counts and only the IDs the corpus resolves, first holder first", async () => {
    const client = new MockClient("normal", { delayMs: 0 });
    const card = await client.getCard("harbor-sim", "C-0003");
    if (!("title" in card)) {
      throw new Error("no C-0003");
    }
    // The mooring's two ID sections nest: one direct child, one rule.
    expect(card.practices?.map((practice) => [practice.name, practice.path, practice.rules])).toEqual([
      ["MEC-MOORING", "docs/spec/berths/mooring.md", 1],
      ["DOM-HARBOR", "docs/spec/harbor.md", 1],
      ["MEC-TIDES", "docs/spec/tides/tide-cycle.md", 1],
    ]);
    expect(card.cited?.map((entry) => [entry.written, entry.name, entry.path])).toEqual([
      ["RULE-HARBOR-CLOCK", "RULE-HARBOR-CLOCK", "docs/spec/harbor.md"],
      ["RULE-FAIRWAY-SPEED", "RULE-FAIRWAY-SPEED", "docs/spec/fairway/README.md"],
    ]);
    // The plates desk's two; a task with no discipline none; the Tier 3 page naming copy never.
    const plate = await client.getCard("harbor-sim", "C-0007");
    expect("title" in plate ? plate.practices?.map((practice) => practice.name) : null).toEqual(["MEC-CARGO-CRANES", "DOM-HARBOR"]);
    const ledger = await client.getCard("ledger-api", "C-0101");
    expect("title" in ledger ? [ledger.discipline, ledger.practices, ledger.cited] : null).toEqual([null, [], []]);
    expect(card.practices?.some((practice) => practice.name === "MEC-OLD-QUAYS")).toBe(false);
  });

  it("draws C-0003's Practices after Source and links its goal's served IDs, the one in code and the unresolved one left as text", async () => {
    renderMock("#/harbor-sim/board/C-0003");
    const dialog = await modal("Night passage rules, plain words");
    const part = await within(dialog).findByRole("region", { name: "Practices" });
    expect(links(part)).toEqual([
      ["MEC-MOORING", docPageHash("harbor-sim", "MEC-MOORING")],
      ["DOM-HARBOR", docPageHash("harbor-sim", "DOM-HARBOR")],
      ["MEC-TIDES", docPageHash("harbor-sim", "MEC-TIDES")],
    ]);
    expect(within(part).getAllByRole("listitem").map((row) => row.lastElementChild?.textContent)).toEqual(["1 rule", "1 rule", "1 rule"]);
    const goal = within(dialog).getByRole("region", { name: "Goal" });
    expect(links(goal)).toEqual([
      ["RULE-HARBOR-CLOCK", docPageHash("harbor-sim", "RULE-HARBOR-CLOCK")],
      ["RULE-FAIRWAY-SPEED", docPageHash("harbor-sim", "RULE-FAIRWAY-SPEED")],
    ]);
    expect(goal.textContent).toContain("`RULE-FAIRWAY-SPEED`. RULE-PILOT-HOURS is not written yet.");
  });

  it("links C-0015's return note to the night passage", async () => {
    renderMock("#/harbor-sim/board/C-0015");
    const dialog = await modal("Night passage chart");
    const activity = within(dialog).getByRole("region", { name: "Activity" });
    const note = within(activity)
      .getAllByRole("listitem")
      .find((line) => line.textContent.includes("buoy numbers"));
    expect(links(note as HTMLElement)).toEqual([["MEC-NIGHT-PASSAGE", docPageHash("harbor-sim", "MEC-NIGHT-PASSAGE")]]);
  });

  it("says a ledger-api task has no discipline, so no practices", async () => {
    renderMock("#/ledger-api/board/C-0101");
    const dialog = await modal("Write the refund runbook");
    expect(within(dialog).getByRole("region", { name: "Practices" }).textContent).toContain("This task has no discipline, so no practices apply");
  });

  it("shows `practices-unread`'s failure in the daemon's words with Retry, nothing linked", async () => {
    renderMock("#/harbor-sim/board/C-0003", "practices-unread");
    const dialog = await modal("Night passage rules, plain words");
    const part = within(dialog).getByRole("region", { name: "Practices" });
    expect(within(part).getByRole("alert").textContent).toContain("practices: not read: the index at");
    expect(within(part).getByRole("button", { name: "Retry" })).toBeTruthy();
    expect(links(within(dialog).getByRole("region", { name: "Goal" }))).toEqual([]);
  });

  it("links a review's served citation in the Inbox: PR-0049's rationale names DEC-0012", async () => {
    const client = renderMock("#/harbor-sim/inbox/PR-0049");
    const review = await client.getProposal("harbor-sim", "PR-0049");
    expect(review.cited?.map((entry) => entry.written)).toEqual(["DEC-0012"]);
    const card = await screen.findByRole("article");
    const heading = await within(card).findByRole("heading", { level: 3, name: "Rationale" });
    const rationale = heading.closest("section") as HTMLElement;
    await waitFor(() => {
      expect(links(rationale)).toEqual([["DEC-0012", docPageHash("harbor-sim", "DEC-0012")]]);
    });
  });

  it("filters Documents by Discipline: each desk's corner", async () => {
    renderMock("#/harbor-sim/documents");
    await screen.findByRole("table");
    // All modules stored (renderMock), Class at canon.
    fireEvent.click(screen.getByRole("button", { name: /^Discipline: / }));
    const panel = screen.getByRole("group", { name: "Discipline" });
    expect(within(panel).getAllByRole("button").map((button) => button.textContent)).toEqual(["copy 2", "plates 2", "No discipline 3"]);
    fireEvent.click(within(panel).getByRole("button", { name: "plates 2" }));
    expect(Array.from(document.querySelectorAll(".documents-table tbody tr[data-row]"), (row) => row.getAttribute("data-row"))).toEqual(["MEC-CARGO-CRANES", "DOM-HARBOR"]);
  });
});
