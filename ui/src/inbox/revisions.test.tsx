import { act, cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Proposal, QueueEvent, RevisionChange } from "../api/types";
import { aProposal } from "../test/builders";
import { renderApp } from "../test/render";
import { STAGED_AT, stubClient } from "../test/stubClient";
import { isRefused, lockEntriesText, revisionsToKeep, revisionText } from "./revisions";

// UI-1, UI-2 and UI-3 of docs/features/rev-discipline.md on a stub client ("UI"): Accept offers
// "Editorial: keep the revision" only when the review's `revisions` is a non-empty list, by the key,
// never the kind, a refused raise among it too (Q15); checked, it stages `editorial: true`, and the
// card shows an Editorial row and copies the bare `spec approve PR-0004`, which confirms the stage
// (Q14); the card and the dialog list `revisions` as returned, `to` as sent, never `rev + 1` worked
// out here, a refused raise as refused with the daemon's reason.

const EDITORIAL = "Editorial: keep the revision";

/** R-12 at rev 3, its `to` 9 as the daemon sent it: a UI writing `rev + 1` would show 4. */
const R12: RevisionChange = { id: "R-12", rev: 3, to: 9, lock_entries: 2, refused: null };
const R13: RevisionChange = { id: "R-13", rev: 1, to: 2, lock_entries: null, refused: null };

/** Why the daemon refuses ADR-0007's raise, as apply's step 6 words it. */
const CLOSED = "`ADR-0007`: class `decision` is closed and does not admit `rev`: admit it in [classes], or approve with --editorial";

/** A raise the daemon refused: a closed class admitting no `rev`, `to` null. */
const ADR7: RevisionChange = { id: "ADR-0007", rev: 1, to: null, lock_entries: 0, refused: CLOSED };

/** Each listed node of a list as [its line, the refusal under it or null]. */
function linesOf(scope: Element | null): [string | null, string | null][] {
  return Array.from(scope?.querySelectorAll(".revision-list > li") ?? [], (item) => [
    item.querySelector(".revision-line")?.textContent ?? null,
    item.querySelector(".revision-refused")?.textContent ?? null,
  ]);
}

/** An update of R-12 whose change alters `revisions`' nodes, unless told otherwise. */
function change(fields: Partial<Omit<Proposal, "id">> = {}, id = "PR-0004"): Proposal {
  return aProposal({
    id,
    kind: "update",
    target_id: "R-12",
    target_ids: ["R-12"],
    summary: null,
    rationale: "Reword the cost rule.",
    base_hash: "b3:base",
    base_text: "## Cost {#R-12 rev=3}\n\nOld.\n",
    diff: "--- base a.md\n+++ proposed a.md\n@@ -1,3 +1,3 @@\n ## Cost {#R-12 rev=3}\n \n-Old.\n+New.",
    preview: "applies",
    revisions: [R12, R13],
    ...fields,
  });
}

/** The Inbox on `proposals`, the card of `id` read. */
async function openCard(proposals: Proposal[], id = "PR-0004") {
  const client = stubClient(proposals);
  renderApp(client, `#/alpha/inbox/${id}`);
  const card = await screen.findByRole("article");
  await within(card).findByRole("heading", { level: 3, name: "Provenance" });
  return { client, card };
}

/** The decision dialog `name` opened from the card. */
async function openDialog(card: HTMLElement, button: "Accept" | "Reject", id = "PR-0004"): Promise<HTMLElement> {
  fireEvent.click(within(card).getByRole("button", { name: button }));
  return screen.findByRole("dialog", { name: `${button} ${id}` });
}

/** The card's section under `title`, or null. */
function sectionOf(card: HTMLElement, title: string): HTMLElement | null {
  return within(card).queryByRole("heading", { level: 3, name: title })?.closest("section") ?? null;
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

describe("a revisions line (UI-3 of rev-discipline)", () => {
  it("writes each node as the review sends it: its ID, rev, the daemon's to, the lock entries", () => {
    expect([R12, R13, { ...R12, lock_entries: 1 }, { ...R12, lock_entries: 0 }].map(revisionText)).toEqual([
      "R-12 rev 3 \u2192 9 (2 lock entries)",
      "R-13 rev 1 \u2192 2 (lock entries unknown)",
      "R-12 rev 3 \u2192 9 (1 lock entry)",
      "R-12 rev 3 \u2192 9 (0 lock entries)",
    ]);
    expect([lockEntriesText(0), lockEntriesText(1), lockEntriesText(7), lockEntriesText(null)]).toEqual([
      "0 lock entries",
      "1 lock entry",
      "7 lock entries",
      "lock entries unknown",
    ]);
  });

  it("writes a refused raise as refused, never a number or null (Q15)", () => {
    expect([ADR7, { ...ADR7, lock_entries: 2 }, { ...R12, to: null }].map(revisionText)).toEqual([
      "ADR-0007 rev 1 \u2192 refused (0 lock entries)",
      "ADR-0007 rev 1 \u2192 refused (2 lock entries)",
      "R-12 rev 3 \u2192 refused (2 lock entries)",
    ]);
    expect([ADR7, R12].map(isRefused)).toEqual([true, false]);
  });

  it("offers the nodes to keep only for a non-empty list, a refused raise among them too", () => {
    expect([
      revisionsToKeep({ revisions: null }),
      revisionsToKeep({ revisions: [] }),
      revisionsToKeep({ revisions: [R12] }),
      revisionsToKeep({ revisions: [ADR7] }),
      revisionsToKeep({ revisions: [R12, ADR7] }),
    ]).toEqual([null, null, [R12], [ADR7], [R12, ADR7]]);
  });
});

describe("the card lists revisions as returned (UI-3 of rev-discipline)", () => {
  it("lists each node after the section diff, `to` 9 for rev 3 as sent, never rev + 1", async () => {
    const { card } = await openCard([change()]);
    const section = sectionOf(card, "Revisions");
    if (section === null) {
      throw new Error("no Revisions section");
    }
    expect(Array.from(section.querySelectorAll("li"), (item) => item.textContent)).toEqual([
      "R-12 rev 3 \u2192 9 (2 lock entries)",
      "R-13 rev 1 \u2192 2 (lock entries unknown)",
    ]);
    expect(section.textContent).not.toContain("\u2192 4");
    expect(section.querySelector("p")?.textContent).toBe(
      "Approval raises each revision below. An editorial approval keeps them and refreshes the lock entries that vouch for the old text.",
    );
    const titles = within(card)
      .getAllByRole("heading", { level: 3 })
      .map((heading) => heading.textContent);
    expect(titles.slice(titles.indexOf("Section diff"), titles.indexOf("Section diff") + 2)).toEqual(["Section diff", "Revisions"]);
  });

  it("lists a refused raise as refused with the daemon's reason, beside a raised one; says a meaning approval is refused (Q15)", async () => {
    const { card } = await openCard([change({ revisions: [R12, ADR7] })]);
    const section = sectionOf(card, "Revisions");
    expect(linesOf(section)).toEqual([
      ["R-12 rev 3 \u2192 9 (2 lock entries)", null],
      ["ADR-0007 rev 1 \u2192 refused (0 lock entries)", CLOSED],
    ]);
    expect(section?.textContent).not.toContain("null");
    expect(section?.querySelector("p")?.textContent).toBe(
      "The terminal refuses a meaning approval while a raise below is refused, for the reason given. An editorial approval keeps every revision and refreshes the lock entries that vouch for the old text.",
    );
  });

  it("says a change of no meaning keeps every revision ([])", async () => {
    const { card } = await openCard([change({ revisions: [] })]);
    expect(sectionOf(card, "Revisions")?.textContent).toBe("RevisionsNo node's meaning changes: approval keeps every revision.");
  });

  it("shows no Revisions section for null (no existing node written, or no preview to read)", async () => {
    const { card } = await openCard([change({ revisions: null, preview: "conflicts", conflict: "<<<<<<< current" })]);
    expect(sectionOf(card, "Section diff")).not.toBeNull();
    expect(sectionOf(card, "Revisions")).toBeNull();
  });
});

describe("Accept's editorial toggle (UI-1 of rev-discipline)", () => {
  it("editorial_toggle_hidden_without_revisions: a file-form create (revisions null, no base) and a change of no meaning ([]) offer none", async () => {
    const create = change(
      { kind: "create", target_id: "docs/spec/new.md", target_path: "docs/spec/new.md", target_ids: ["docs/spec/new.md"], base_hash: null, base_text: null, revisions: null },
      "PR-0006",
    );
    for (const proposal of [create, change({ revisions: [] }, "PR-0007")]) {
      const id = proposal.id ?? "";
      const { card } = await openCard([proposal], id);
      const dialog = await openDialog(card, "Accept", id);
      expect([id, within(dialog).queryByRole("checkbox")]).toEqual([id, null]);
      expect(within(dialog).queryByText(EDITORIAL)).toBeNull();
      expect([id, sectionOf(card, "Revisions")?.querySelector("li") ?? null]).toEqual([id, null]);
      cleanup();
    }
  });

  it("is offered on Accept of a change with revisions, unchecked, naming what approval raises, to as sent; never on Reject", async () => {
    const { card } = await openCard([change()]);
    const reject = await openDialog(card, "Reject");
    expect(within(reject).queryByRole("checkbox")).toBeNull();
    fireEvent.click(within(reject).getByRole("button", { name: "Cancel" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    const dialog = await openDialog(card, "Accept");
    const toggle = within(dialog).getByRole("checkbox", { name: EDITORIAL });
    expect((toggle as HTMLInputElement).checked).toBe(false);
    const hint = document.getElementById(toggle.getAttribute("aria-describedby") ?? "");
    expect(Array.from(hint?.querySelectorAll("li") ?? [], (item) => item.textContent)).toEqual([
      "R-12 rev 3 \u2192 9 (2 lock entries)",
      "R-13 rev 1 \u2192 2 (lock entries unknown)",
    ]);
    expect(hint?.textContent).not.toContain("\u2192 4");
  });

  it("editorial_toggle_shown_for_a_refused_raise: offered, unchecked, when the one raise or one of two is refused; the reason in its hint", async () => {
    for (const [id, revisions] of [
      ["PR-0008", [ADR7]],
      ["PR-0009", [R12, ADR7]],
    ] as const) {
      const { card } = await openCard([change({ revisions: [...revisions] }, id)], id);
      const dialog = await openDialog(card, "Accept", id);
      const toggle = within(dialog).queryByRole<HTMLInputElement>("checkbox", { name: EDITORIAL });
      expect([id, toggle?.checked]).toEqual([id, false]);
      const hint = document.getElementById(toggle?.getAttribute("aria-describedby") ?? "");
      expect([id, hint?.querySelector("p")?.textContent, linesOf(hint)]).toEqual([
        id,
        "Unchecked, this is a meaning approval, which the terminal refuses while a raise below is refused:",
        revisions.map((entry) => [revisionText(entry), entry.refused]),
      ]);
      expect(hint?.textContent).not.toContain("null");
      cleanup();
    }
  });

  it("stages a meaning approve over a refused raise as any other: the terminal refuses it, not the UI", async () => {
    const { client, card } = await openCard([change({ revisions: [ADR7] })]);
    const dialog = await openDialog(card, "Accept");
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage accept" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.stageDecision.mock.calls.map((call) => call[2])).toEqual([
      { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null },
    ]);
  });

  it("decides by the key, never the kind: a kind of the project's own with revisions offers it", async () => {
    const { card } = await openCard([change({ kind: "rewording" })]);
    const dialog = await openDialog(card, "Accept");
    expect(within(dialog).getByRole("checkbox", { name: EDITORIAL })).toBeTruthy();
  });
});

describe("staging an editorial approve (UI-2 of rev-discipline)", () => {
  it("checked: stages editorial true; the card shows its Editorial row and copies the bare spec approve PR-0004 (Q14)", async () => {
    const writeText = stubClipboard();
    const { client, card } = await openCard([change()]);
    const dialog = await openDialog(card, "Accept");
    fireEvent.click(within(dialog).getByRole("checkbox", { name: EDITORIAL }));
    fireEvent.change(within(dialog).getByLabelText("Note for the record (optional)"), { target: { value: "typo" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage accept" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.stageDecision.mock.calls).toEqual([
      ["alpha", "PR-0004", { decision: "approve", option: null, answer: null, canon: null, editorial: true, note: "typo" }, "2026-10-01T10:00:00Z"],
    ]);
    expect(client.state.proposals[0]?.staged).toEqual({
      decision: "approve",
      option: null,
      answer: null,
      canon: null,
      editorial: true,
      note: "typo",
      span_hash: "b3:base",
    });
    const staged = await within(screen.getByRole("article")).findByRole("region", { name: "Staged decision" });
    expect(staged.querySelector(".staged-line")?.textContent).toBe(`Staged ${STAGED_AT}. Confirm on a terminal: spec approve PR-0004`);
    expect(Array.from(staged.querySelectorAll("dt"), (dt) => [dt.textContent, dt.nextElementSibling?.textContent])).toEqual([
      ["Decision", "Accept"],
      ["Editorial", "Keep the revision"],
      ["Note", "typo"],
    ]);
    expect(document.querySelector('[aria-live="polite"]')?.textContent).toBe(
      `Staged Accept on PR-0004 at ${STAGED_AT}; nothing is applied yet. Confirm it on a terminal: spec approve PR-0004.`,
    );
    fireEvent.click(within(staged).getByRole("button", { name: /^Copy/ }));
    await waitFor(() => {
      expect(writeText.mock.calls).toEqual([["spec approve PR-0004"]]);
    });
  });

  it("unchecked: stages editorial null, a meaning approve; the same bare command, no Editorial row", async () => {
    const { client, card } = await openCard([change()]);
    const dialog = await openDialog(card, "Accept");
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage accept" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.stageDecision.mock.calls).toEqual([
      ["alpha", "PR-0004", { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null }, "2026-10-01T10:00:00Z"],
    ]);
    const staged = await within(screen.getByRole("article")).findByRole("region", { name: "Staged decision" });
    expect(staged.querySelector(".staged-line")?.textContent).toBe(`Staged ${STAGED_AT}. Confirm on a terminal: spec approve PR-0004`);
    expect(Array.from(staged.querySelectorAll("dt"), (dt) => dt.textContent)).not.toContain("Editorial");
  });

  it("a version revised while the dialog is open unchecks it: its revisions are another change's", async () => {
    const client = stubClient([change()]);
    let emit: (event: QueueEvent) => void = () => undefined;
    client.subscribe = (_project, onEvent) => {
      emit = onEvent;
      return () => undefined;
    };
    renderApp(client, "#/alpha/inbox/PR-0004");
    const card = await screen.findByRole("article");
    await within(card).findByRole("heading", { level: 3, name: "Provenance" });
    const dialog = await openDialog(card, "Accept");
    fireEvent.click(within(dialog).getByRole("checkbox", { name: EDITORIAL }));
    expect(within(dialog).getByRole<HTMLInputElement>("checkbox", { name: EDITORIAL }).checked).toBe(true);
    client.state.proposals = client.state.proposals.map((proposal) => ({
      ...proposal,
      updated_at: "2026-10-01T11:00:00Z",
      revisions: [{ id: "R-12", rev: 4, to: 5, lock_entries: 0, refused: null }],
    }));
    act(() => {
      emit({ seq: 7, type: "proposal.apply_failed", payload: { id: "PR-0004", step: 3, reason: "elsewhere" } });
    });
    await within(dialog).findByText("This proposal changed since you opened it");
    const toggle = within(dialog).getByRole("checkbox", { name: EDITORIAL });
    expect((toggle as HTMLInputElement).checked).toBe(false);
    const hint = document.getElementById(toggle.getAttribute("aria-describedby") ?? "");
    expect(Array.from(hint?.querySelectorAll("li") ?? [], (item) => item.textContent)).toEqual(["R-12 rev 4 \u2192 5 (0 lock entries)"]);
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage accept" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.stageDecision.mock.calls.map((call) => call[2])).toEqual([
      { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null },
    ]);
  });

  it("a revision taking the revisions away stages no editorial, even if it was checked", async () => {
    const client = stubClient([change()]);
    let emit: (event: QueueEvent) => void = () => undefined;
    client.subscribe = (_project, onEvent) => {
      emit = onEvent;
      return () => undefined;
    };
    renderApp(client, "#/alpha/inbox/PR-0004");
    const card = await screen.findByRole("article");
    await within(card).findByRole("heading", { level: 3, name: "Provenance" });
    const dialog = await openDialog(card, "Accept");
    fireEvent.click(within(dialog).getByRole("checkbox", { name: EDITORIAL }));
    client.state.proposals = client.state.proposals.map((proposal) => ({ ...proposal, updated_at: "2026-10-01T11:00:00Z", preview: "conflicts", revisions: null }));
    act(() => {
      emit({ seq: 8, type: "proposal.apply_failed", payload: { id: "PR-0004", step: 3, reason: "elsewhere" } });
    });
    await within(dialog).findByText("This proposal changed since you opened it");
    expect(within(dialog).queryByRole("checkbox")).toBeNull();
    fireEvent.click(within(dialog).getByRole("button", { name: "Stage accept" }));
    await waitFor(() => {
      expect(screen.queryByRole("dialog")).toBeNull();
    });
    expect(client.stageDecision.mock.calls.map((call) => (call[2].decision === "approve" ? call[2].editorial : "reject"))).toEqual([null]);
  });
});
