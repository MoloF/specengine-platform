import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "../app/App";
import { MockClient } from "./MockClient";

// docs/features/upstream-signal.md "Acceptance criteria", US-12 and US-13, over the mock as `pnpm dev`
// shows it (harbor-sim): a list with marks flags C-0013's and C-0019's rows, `false` on every other,
// `null` without marks; the card read serves C-0013's parent's mark, the corpus's applied decision
// PR-0056 and an edit, C-0019's sixteen changes and four more, every other card `null`. The tile and
// the work item draw them. The stub cases run in src/board/; the mock is imported here alone (policy AC-05).

function renderMock(hash: string) {
  window.localStorage.setItem("specengine.module.harbor-sim", "");
  window.history.replaceState(null, "", `/?scenario=normal${hash}`);
  const client = new MockClient("normal", { delayMs: 0 });
  render(<App client={client} scenario={null} />);
  return client;
}

/** A card's modal, by its title, its side column read. */
async function modal(title: string): Promise<HTMLElement> {
  const dialog = await screen.findByRole("dialog", { name: title });
  await within(dialog).findByRole("region", { name: "Details" });
  return dialog;
}

function lines(part: HTMLElement): string[] {
  return within(part)
    .getAllByRole("listitem")
    .map((item) => item.textContent);
}

describe("what changed above a subtask, over the mock (upstream-signal)", () => {
  it("flags C-0019's and C-0013's rows under marks, in the list's order, a cancelled row `false`, every row `null` without marks (US-12)", async () => {
    const client = new MockClient("normal", { delayMs: 0 });
    const marked = await client.getCards("harbor-sim", { marks: true, with_cancelled: true });
    expect(marked.cards.filter((row) => row.upstream === true).map((row) => row.id)).toEqual(["C-0019", "C-0013"]);
    expect(marked.cards.filter((row) => row.upstream !== true).every((row) => row.upstream === false)).toBe(true);
    expect(marked.cards.find((row) => row.id === "C-0017")?.upstream).toBe(false);
    const plain = await client.getCards("harbor-sim");
    expect(plain.cards.every((row) => row.upstream === null)).toBe(true);
  });

  it("serves C-0013's mark, PR-0056 at its applied event and an edit, C-0019's sixteen changes and four more, and `null` on every other card (US-12)", async () => {
    const client = new MockClient("normal", { delayMs: 0 });
    const read = async (id: string) => {
      const card = await client.getCard("harbor-sim", id);
      if (!("upstream" in card)) {
        throw new Error(`no card ${id}`);
      }
      return card;
    };
    const plate = await read("C-0013");
    expect([plate.mark, plate.upstream?.parent, plate.upstream?.mark, plate.upstream?.on, plate.upstream?.more]).toEqual([null, "C-0001", "suspect", "MEC-TIDE-TABLES", 0]);
    expect(plate.upstream?.changes.map((change) => [change.source, change.fields, change.kind, change.item, change.record, change.on, change.by])).toEqual([
      ["item", [], "decision", "PR-0056", "DEC-0013", "MEC-CARGO-CRANES", "Mira Holt"],
      ["edit", ["goal"], null, null, null, null, "Mira Holt"],
    ]);
    // The decision is the corpus's own: its `seq` and `at` its `proposal.applied` event's, as C-0008, which it names, reads it.
    const applied = (await read("C-0008")).history?.find((event) => event.type === "proposal.applied" && (event.payload as { id?: string }).id === "PR-0056");
    expect([applied?.seq, applied?.at]).toEqual([plate.upstream?.changes[0]?.seq, plate.upstream?.changes[0]?.at]);
    const glossary = await read("C-0019");
    expect([glossary.upstream?.changes.length, glossary.upstream?.more, glossary.upstream?.mark]).toEqual([16, 4, null]);
    const seqs = glossary.upstream?.changes.map((change) => change.seq) ?? [];
    expect(seqs).toEqual([...seqs].sort((a, b) => a - b));
    for (const id of ["C-0001", "C-0002", "C-0016", "C-0017", "C-0018"]) {
      expect([id, (await read(id)).upstream]).toEqual([id, null]);
    }
  });

  it("draws Changed above on C-0019's tile alone on the copy desk's tab, and on C-0013's alone on the plates' (US-13)", async () => {
    renderMock("#/harbor-sim/board");
    fireEvent.click(await screen.findByRole("button", { name: "Show all flows" }));
    await waitFor(() => {
      expect(document.querySelector('[data-card="C-0019"]')).not.toBeNull();
    });
    const tilesFlagged = () => Array.from(document.querySelectorAll<HTMLElement>('.board-card-mark [data-tone="card-upstream"]'), (found) => found.closest("[data-card]")?.getAttribute("data-card"));
    expect(tilesFlagged()).toEqual(["C-0019"]);
    fireEvent.click(screen.getByRole("tab", { name: "plates" }));
    const plate = await waitFor(() => {
      const found = document.querySelector<HTMLElement>('[data-card="C-0013"]');
      if (found === null) {
        throw new Error("no tile C-0013");
      }
      return found;
    });
    const badge = plate.querySelector<HTMLElement>('.board-card-mark [data-tone="card-upstream"]');
    expect([badge?.textContent, badge?.querySelectorAll("svg.icon").length]).toEqual(["Changed above", 1]);
    expect(tilesFlagged()).toEqual(["C-0013"]);
  });

  it("C-0013's work item: the part first in its main column, the mark line, the decision with DEC-0013 a document link, the edit, each with its time and who (US-13)", async () => {
    renderMock("#/harbor-sim/board/C-0013");
    const dialog = await modal("Tide table plate");
    const part = await within(dialog).findByRole("region", { name: "Changed above" });
    const main = dialog.querySelector<HTMLElement>(".issue-view-main");
    expect(main === null ? null : within(main).getAllByRole("region")[0]).toBe(part);
    const shown = lines(part);
    expect(shown[0]).toBe("The parent's document changed at MEC-TIDE-TABLES.");
    expect(shown[1]).toMatch(/^Decision DEC-0013 applied on MEC-CARGO-CRANES\d{4}-\d{2}-\d{2} \d{2}:\d{2} UTC by Mira Holt$/);
    expect(shown[2]).toMatch(/^The parent's goal changed\d{4}-\d{2}-\d{2} \d{2}:\d{2} UTC by Mira Holt$/);
    expect(within(part).getByRole("link", { name: "DEC-0013" }).getAttribute("href")).toBe("#/harbor-sim/tree/DEC-0013/page");
    expect(part.querySelectorAll("time")).toHaveLength(2);
    expect(Array.from(part.querySelectorAll("li > span > bdi:not(.mono)"), (name) => name.textContent)).toEqual(["Mira Holt", "Mira Holt"]);
    // The record is a document the mock serves: its link opens it over the task.
    fireEvent.click(within(part).getByRole("link", { name: "DEC-0013" }));
    expect(await screen.findByRole("heading", { name: /Crane reach at full boom/ })).toBeTruthy();
  });

  it("C-0019's work item lists sixteen changes and ends with the four not listed (US-13)", async () => {
    renderMock("#/harbor-sim/board/C-0019");
    const dialog = await modal("Tide table glossary");
    const part = await within(dialog).findByRole("region", { name: "Changed above" });
    const shown = lines(part);
    expect([shown.length, shown.at(-1)]).toEqual([17, "4 earlier changes not listed"]);
    expect(part.querySelectorAll("time")).toHaveLength(16);
    // Its question with no record links the item in the Inbox; its records are documents.
    expect(within(part).getByRole("link", { name: "PR-0032" }).getAttribute("href")).toBe("#/harbor-sim/inbox/PR-0032");
    expect(within(part).getByRole("link", { name: "DEC-0012" }).getAttribute("href")).toBe("#/harbor-sim/tree/DEC-0012/page");
  });
});
