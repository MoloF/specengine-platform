import { act, cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DriftEvidence, Proposal } from "../api/types";
import { aProposal } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";
import { driftCommands, driftRowsText, driftRowText } from "./drift";

// UI-1 of docs/features/drift-queue.md on a stub client ("Review, inbox, round, stage"; Q-A): a
// drift item is listed in the inbox and read in its review, and never offers a stage. Its card
// shows the evidence (the rows counted, the commit judged, the hash, each stored row as sent) and,
// in place of the four decisions, the terminal commands that decide it: `spec approve PR`, its
// `--editorial` form, `spec reject PR --reason`. The review document's `drift` key decides it,
// never the kind (ADR-0031); the decision keys open no dialog on it and nothing is sent.

/** Two rows as `spec verify --json` writes them, untyped here: the UI shows them as sent. */
const ROWS: unknown[] = [
  { node: "R-12", relation: "implements", file: "src/a.rs", kind: "item", symbol: "a::f", item_kind: "function_item", state: "conflict", category: null },
  { node: "R-12", relation: "implements", file: "src/b.rs", kind: "item", symbol: "b::g", item_kind: "function_item", state: "code_ahead", category: "code_changed" },
];

const HASH = `b3:${"0123456789abcdef".repeat(4)}`;

const EVIDENCE: DriftEvidence = { hash: HASH, rows: ROWS, more: 0 };

const COMMIT = "4d2b9e7a1c3f5d8b0e2a4c6f8b1d3e5a7c9f0b2d";

/** A drift item of R-12 as the review document carries one: no texts, diff, preview or revisions. */
function driftItem(fields: Partial<Omit<Proposal, "id">> = {}, id = "PR-0012"): Proposal {
  return aProposal({
    id,
    kind: "drift",
    severity: "high",
    target_id: "R-12",
    target_path: "spec.lock",
    target_ids: ["R-12"],
    worktree: "/work/alpha",
    base_commit: COMMIT,
    summary: "conflict, code_ahead code_changed",
    drift: EVIDENCE,
    notes: [`spec approve ${id} shows the rows before asking`],
    ...fields,
  });
}

/** An update beside it, offering the four decisions as any proposal does. */
const UPDATE = aProposal({ id: "PR-0004", target_id: "R-4", target_ids: ["R-4"], created_at: "2026-10-01T11:00:00Z" });

/** The Inbox on `proposals`, the card of `id` read. */
async function openCard(proposals: Proposal[], id = "PR-0012") {
  const client = stubClient(proposals);
  renderApp(client, `#/alpha/inbox/${id}`);
  const list = await screen.findByRole("listbox");
  const card = await screen.findByRole("article");
  await within(card).findByRole("heading", { level: 3, name: "Provenance" });
  return { client, list, card };
}

/** The card's section under `title`, or null. */
function sectionOf(card: HTMLElement, title: string): HTMLElement | null {
  return within(card).queryByRole("heading", { level: 3, name: title })?.closest("section") ?? null;
}

/** The four decisions' buttons the card shows. */
function decisionButtons(card: HTMLElement): HTMLElement[] {
  return ["Accept", "Reject", "Needs clarification", "Defer"].flatMap((name) => within(card).queryAllByRole("button", { name }));
}

function selectedOption(list: HTMLElement): HTMLElement {
  const selected = within(list)
    .getAllByRole("option")
    .find((option) => option.getAttribute("aria-selected") === "true");
  if (selected === undefined) {
    throw new Error("no option is selected");
  }
  return selected;
}

function politeRegion(): HTMLElement {
  const region = document.querySelector<HTMLElement>('[aria-live="polite"]');
  if (region === null) {
    throw new Error("no polite live region");
  }
  return region;
}

const clipboard = Object.getOwnPropertyDescriptor(navigator, "clipboard");

function stubClipboard() {
  const writeText = vi.fn(() => Promise.resolve());
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
  return writeText;
}

afterEach(() => {
  if (clipboard === undefined) {
    Reflect.deleteProperty(navigator, "clipboard");
  } else {
    Object.defineProperty(navigator, "clipboard", clipboard);
  }
});

describe("a drift item's commands and rows (UI-1 of drift-queue)", () => {
  it("decides it on a terminal: approve, approve --editorial, reject with a reason; fixed words and a checked ID only", () => {
    expect(driftCommands("PR-0012")?.map((command) => [command.action, command.text])).toEqual([
      ["approve", "spec approve PR-0012"],
      ["editorial", "spec approve PR-0012 --editorial"],
      ["reject", 'spec reject PR-0012 --reason "..."'],
    ]);
    for (const id of ["PR-12", "PR-0012; rm -rf ~", "pr-0012", ""]) {
      expect([id, driftCommands(id)]).toEqual([id, null]);
    }
  });

  it("counts the rows stored and those past them; writes a row as its compact JSON", () => {
    expect([
      driftRowsText(EVIDENCE),
      driftRowsText({ hash: HASH, rows: [ROWS[0]], more: 0 }),
      driftRowsText({ hash: HASH, rows: ROWS, more: 70 }),
    ]).toEqual(["2 rows", "1 row", "72 rows, the first 2 stored with the item"]);
    expect(driftRowText(ROWS[0])).toBe(JSON.stringify(ROWS[0]));
    expect(driftRowText({ b: 1, a: [null] })).toBe('{"b":1,"a":[null]}');
  });
});

describe("a drift item in the inbox (UI-1 of drift-queue)", () => {
  it("is listed with its kind and states; its card reads the review", async () => {
    const { list, card } = await openCard([UPDATE, driftItem()]);
    const option = within(list).getAllByRole("option").find((item) => item.dataset.proposal === "PR-0012");
    expect(option?.textContent).toContain("conflict, code_ahead code_changed");
    expect(option?.textContent).toContain("drift");
    expect(within(card).getByRole("heading", { level: 2 }).textContent).toBe("conflict, code_ahead code_changed");
  });

  it("drift_item_never_offers_a_stage: no decision button, the terminal commands in their place, each to copy", async () => {
    const writeText = stubClipboard();
    const { client, card } = await openCard([UPDATE, driftItem()]);
    expect(decisionButtons(card)).toEqual([]);
    expect(within(card).queryByRole("group", { name: "Decide PR-0012" })).toBeNull();
    const terminal = within(card).getByRole("region", { name: "Decided on a terminal" });
    expect(terminal.textContent).toContain("in the worktree where it was raised (/work/alpha), which asks [y/N]. Nothing is staged here");
    const commands = Array.from(terminal.querySelectorAll(".command"), (row) => [
      row.getAttribute("data-action"),
      row.querySelector(".command-text")?.textContent,
    ]);
    expect(commands).toEqual([
      ["approve", "spec approve PR-0012"],
      ["editorial", "spec approve PR-0012 --editorial"],
      ["reject", 'spec reject PR-0012 --reason "..."'],
    ]);
    fireEvent.click(within(terminal).getByRole("button", { name: "Copy the command: spec approve PR-0012" }));
    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith("spec approve PR-0012");
    });
    expect(client.stageDecision).not.toHaveBeenCalled();
    expect(client.unstageDecision).not.toHaveBeenCalled();
  });

  it("drift_item_never_offers_a_stage: the decision keys open no dialog on it, send nothing, and say where it is decided", async () => {
    const { client, list } = await openCard([UPDATE, driftItem()]);
    for (const key of ["a", "r", "c", "d"]) {
      fireEvent.keyDown(selectedOption(list), { key });
      await waitFor(() => {
        expect(politeRegion().textContent).toBe(
          "PR-0012 is a drift item, decided on a terminal only; nothing is staged here. Its card lists the commands.",
        );
      });
      await act(async () => {
        await import("./DecisionDialog");
      });
      expect([key, screen.queryByRole("dialog")]).toEqual([key, null]);
    }
    expect(client.stageDecision).not.toHaveBeenCalled();
  });

  it("shows the evidence first: the rows counted, the commit judged, the hash, each stored row as sent; the review's note", async () => {
    const { card } = await openCard([driftItem()]);
    const evidence = sectionOf(card, "Drift evidence");
    expect(evidence).not.toBeNull();
    const titles = Array.from(card.querySelectorAll(".card-section-title"), (heading) => heading.textContent);
    expect(titles.indexOf("Drift evidence")).toBe(titles.indexOf("Targets") + 1);
    const pairs = Array.from(evidence?.querySelectorAll("dt") ?? [], (term) => [term.textContent, term.nextElementSibling?.textContent]);
    expect(pairs).toEqual([
      ["Rows", "2 rows"],
      ["Commit judged", COMMIT],
      ["Evidence hash", HASH],
    ]);
    const rows = within(evidence ?? card).getByRole("list", { name: "Rows as spec verify --json writes them" });
    expect(Array.from(rows.querySelectorAll("li"), (row) => row.textContent)).toEqual(ROWS.map((row) => JSON.stringify(row)));
    expect(within(sectionOf(card, "Notes") ?? card).getByText("spec approve PR-0012 shows the rows before asking")).toBeTruthy();
  });

  it("shows no options and no section diff: a drift item has neither, so neither says it is missing", async () => {
    const { card } = await openCard([UPDATE, driftItem()]);
    expect(sectionOf(card, "Drift evidence")).not.toBeNull();
    expect(sectionOf(card, "Options")).toBeNull();
    expect(sectionOf(card, "Section diff")).toBeNull();
    expect(within(card).queryByText("No options attached.")).toBeNull();
    expect(within(card).queryByText("No section diff attached.")).toBeNull();
    expect(card.querySelector(".diff")).toBeNull();
  });

  it("decides by the key, never the kind: a drift key on another kind offers no stage; the kind without the key offers the four", async () => {
    const { card } = await openCard([driftItem({ kind: "lock-drift" }, "PR-0013")], "PR-0013");
    expect(decisionButtons(card)).toEqual([]);
    expect(within(card).getByRole("region", { name: "Decided on a terminal" })).toBeTruthy();
    expect(sectionOf(card, "Drift evidence")).not.toBeNull();
    expect([sectionOf(card, "Options"), sectionOf(card, "Section diff")]).toEqual([null, null]);
  });

  it("offers the four decisions on the kind without the key, and on any other proposal; no drift section, the options and the section diff", async () => {
    for (const proposal of [driftItem({ drift: null }, "PR-0014"), UPDATE]) {
      const id = proposal.id ?? "";
      const { card } = await openCard([proposal], id);
      expect([id, decisionButtons(card).map((button) => button.textContent)]).toEqual([id, ["Accepta", "Rejectr", "Needs clarificationc", "Deferd"]]);
      expect([id, within(card).queryByRole("region", { name: "Decided on a terminal" })]).toEqual([id, null]);
      expect([id, sectionOf(card, "Drift evidence")]).toEqual([id, null]);
      expect([id, sectionOf(card, "Options") !== null, sectionOf(card, "Section diff") !== null]).toEqual([id, true, true]);
      cleanup();
    }
  });

  it("offers no command to copy for an ID that is not a proposal ID", async () => {
    const { card } = await openCard([driftItem({}, "PR-12")], "PR-12");
    const terminal = within(card).getByRole("region", { name: "Decided on a terminal" });
    expect(terminal.querySelector(".command-list")).toBeNull();
    expect(within(terminal).queryByRole("button")).toBeNull();
    expect(within(terminal).getByText("No command for this ID")).toBeTruthy();
  });
});
