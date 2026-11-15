import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Proposal } from "../api/types";
import { en } from "../i18n/en";
import { ru } from "../i18n/ru";
import { aProposal, entryOf } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";

// IC2 of docs/features/inbox-closed-link.md "Acceptance criteria": a closed proposal's pane leaves out
// each row the review document holds no value for, never an empty one, and shows a status no table
// knows raw, in a neutral lozenge under lang="en". The inbox lists PR-0001 alone.

const LISTED = aProposal({ id: "PR-0001", summary: "Still open" });

/** The pane of `closed`, routed while the inbox lists only the open item. */
async function paneOf(closed: Proposal): Promise<HTMLElement> {
  const client = stubClient([LISTED, closed]);
  client.getInbox.mockResolvedValue({ proposals: [entryOf(LISTED)], notes: [] });
  renderApp(client, `#/alpha/inbox/${closed.id ?? ""}`);
  const heading = await screen.findByRole("heading", { level: 2, name: closed.summary ?? "" });
  return heading.closest("article") as HTMLElement;
}

function labels(pane: HTMLElement): string[] {
  return Array.from(within(pane).getByRole("region", { name: "Details" }).querySelectorAll("dt")).map((term) => term.textContent);
}

describe("a closed proposal's pane (IC2)", () => {
  it("a rejected one reads Rejected, with no Commit and no Record row and no empty value", async () => {
    const pane = await paneOf(
      aProposal({
        id: "PR-0010",
        kind: "update",
        status: "rejected",
        summary: "Rename the berth",
        target_id: "R-1",
        target_ids: ["R-1"],
        decided_by: "molof",
        decided_at: "2026-11-14T14:02:00Z",
        decision_note: "Kept as it is.",
      }),
    );
    const lozenge = pane.querySelector(".issue-view-meta .lozenge");
    expect([lozenge?.getAttribute("data-appearance"), lozenge?.querySelector(".lozenge-label")?.textContent]).toEqual(["danger", "Rejected"]);
    expect(labels(pane)).toEqual(["Kind", "Decided by", "Decided at", "Targets"]);
    expect(pane.querySelector(".property-empty")).toBeNull();
    expect(within(pane).getByRole("heading", { level: 3, name: "Decision note" })).toBeTruthy();
  });

  it("a withdrawn one shows its status raw, neutral, under lang=en; no note section without a note", async () => {
    const pane = await paneOf(aProposal({ id: "PR-0011", kind: "question", status: "withdrawn", summary: "Ask about the tide", decided_by: null, decided_at: null }));
    const lozenge = pane.querySelector(".issue-view-meta .lozenge");
    expect([lozenge?.getAttribute("data-appearance"), lozenge?.querySelector(".lozenge-label")?.textContent, lozenge?.querySelector(".lozenge-label")?.getAttribute("lang")]).toEqual([
      "neutral",
      "withdrawn",
      "en",
    ]);
    expect(labels(pane)).toEqual(["Kind"]);
    expect(pane.querySelector(".property-empty")).toBeNull();
    expect(within(pane).queryByRole("heading", { level: 3, name: "Decision note" })).toBeNull();
  });
});

// IC9 of docs/features/inbox-closed-link.md "Acceptance criteria": nothing is shown instead any more, so
// `inbox.absent` is gone from both catalogs; the new keys stand in both (their parity: catalog.test.ts).
describe("the closed pane's words (IC9)", () => {
  it("has no inbox.absent in either catalog, and inbox.unknown and every inbox.closed key in both", () => {
    expect([Object.hasOwn(en.inbox, "absent"), Object.hasOwn(ru.inbox, "absent")]).toEqual([false, false]);
    const keys = ["notice", "decidedBy", "decidedAt", "commit", "record", "targets", "note", "noDecision", "reading"];
    expect(Object.keys(en.inbox.closed)).toEqual(keys);
    expect(Object.keys(ru.inbox.closed)).toEqual(keys);
    expect([en.inbox.unknown, en.inbox.closed.noDecision, en.inbox.closed.reading].every((value) => value.includes("{{id}}"))).toBe(true);
    expect([ru.inbox.unknown, ru.inbox.closed.noDecision, ru.inbox.closed.reading].every((value) => value.includes("{{id}}"))).toBe(true);
  });
});

// FD7 of docs/features/auto-feed.md "Acceptance criteria": a decided proposal whose review document
// holds `delegated` shows Applied by rule, the commit's 12 hex and the rule hash's 12, the whole hash
// what a screen reader says, read-only; one without `delegated` shows nothing new, an applied one too.
describe("a rule's apply on a closed proposal (FD7 of auto-feed)", () => {
  const RULE = `b3:${"0123456789abcdef".repeat(4)}`;
  const COMMIT = "9b1e77c2d0aa4f3e8c1b2a3d4e5f60718293a4b5";

  it("shows Applied by rule with the commit's 12 and the hash's 12, the whole hash spoken; no Confirm, no stage", async () => {
    const pane = await paneOf(
      aProposal({
        id: "PR-0012",
        kind: "update",
        status: "applied",
        summary: "Moor with four",
        applied_commit: COMMIT,
        decided_by: "ann <ann@example.org>",
        decided_at: "2026-11-16T01:05:40Z",
        delegated: { by: "rule", rule: RULE, commit: COMMIT },
      }),
    );
    const line = pane.querySelector<HTMLElement>("[data-delegated]");
    expect(line?.textContent).toBe(`Applied by rule b3:0123456789ab${RULE}, commit 9b1e77c2d0aa`);
    // The twelve shown, the whole hash read out, each whole as its title.
    const [rule, commit] = Array.from(line?.querySelectorAll("code") ?? []);
    expect(rule?.querySelector('[aria-hidden="true"]')?.textContent).toBe("b3:0123456789ab");
    expect(rule?.querySelector(".sr-only")?.textContent).toBe(RULE);
    expect([rule?.getAttribute("title"), commit?.getAttribute("title"), commit?.textContent]).toEqual([RULE, COMMIT, "9b1e77c2d0aa"]);
    expect(within(pane).queryByRole("button", { name: /Confirm|Accept|Reject/ })).toBeNull();
  });

  it("an applied one without `delegated` shows no such line (M: the line shown for any applied row)", async () => {
    const pane = await paneOf(aProposal({ id: "PR-0013", kind: "update", status: "applied", summary: "Confirmed by hand", applied_commit: COMMIT, decided_by: "ann", decided_at: "2026-11-16T01:05:40Z" }));
    expect(pane.querySelector("[data-delegated]")).toBeNull();
    expect(pane.textContent).not.toContain(en.inbox.byRule.split(" {{")[0] ?? "");
  });

  it("says it in Russian from the catalog", () => {
    expect(ru.inbox.byRule).toContain("{{hash}}");
    expect(ru.inbox.byRule).toContain("{{commit}}");
  });
});
