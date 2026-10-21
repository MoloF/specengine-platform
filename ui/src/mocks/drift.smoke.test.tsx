import { render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ClientError } from "../api/client";
import type { StageChoice } from "../api/types";
import { App } from "../app/App";
import { stamp } from "./build";
import { DRIFT_ITEM, DRIFT_ITEM_2, DRIFT_NODE, DRIFT_NODE_2 } from "./ledger-api/fixtures";
import { MockClient } from "./MockClient";

// UI-1 of docs/features/drift-queue.md over the mock ("The item"; "Review, inbox, round, stage"):
// ledger-api holds a drift item, PR-0010, as the daemon would serve it once the slice ships: an
// inbox line, a review document of 48 keys whose `drift` carries the evidence, no preview but its
// note; the mock refuses a stage on it (409, the daemon's words) and keeps it off the round sheet,
// noted; its card offers the terminal commands, never a stage. UI-1 and UI-7 of
// docs/features/ui-drift-evidence.md: a second, PR-0011, raised by a newer build (17-key rows, a
// moved anchor, levels changed), where PR-0010's rows are an older build's 15; both cards draw
// every row typed (M1: the guard requiring `levels` draws PR-0010's raw).

const SLUG = "ledger-api";

/** The time the mock's fixtures are relative to. */
const NOW = Date.UTC(2026, 9, 14, 12, 0, 0);

/** A row of an older `spec verify --json`'s `rows`, its keys in order: the 15 core's `drift::DriftRow` reads. */
const ROW_KEYS = ["node", "relation", "file", "kind", "symbol", "item_kind", "state", "category", "detail", "class", "pre_existing", "markers", "node_rev", "lock", "compared"];

/** A row of a newer `spec verify --json`'s `rows`, verify's 17 keys in order (`docs/canon/spec-verify-output.md` "JSON"). */
const ROW_KEYS_17 = [...ROW_KEYS, "levels", "anchor"];

/**
 * The refusal of a stage on a drift item ("Review, inbox, round, stage", Stage), the daemon's and
 * the CLI round's alike, byte for byte (`crates/specengine-cli/src/stage.rs`, `round.rs`).
 */
const REFUSED = `${DRIFT_ITEM} is a drift item, decided on a terminal only; nothing staged`;

function mock(scenario: "normal" | "conflict" = "normal"): MockClient {
  return new MockClient(scenario, { now: () => NOW, delayMs: 0 });
}

/** The error a promise is refused with, or null when it resolves. */
function refusalOf(promise: Promise<unknown>): Promise<unknown> {
  return promise.then(
    () => null,
    (error: unknown) => error,
  );
}

/** A stored row read back as JSON, whatever its type says: this test reads the keys as stored. */
function rowsOf(rows: readonly unknown[]): Record<string, unknown>[] {
  return rows.map((row) => {
    expect(typeof row === "object" && row !== null && !Array.isArray(row)).toBe(true);
    return Object.fromEntries(Object.entries(row as object));
  });
}

describe("ledger-api's drift item (UI-1 of drift-queue)", () => {
  it("lists PR-0010 as `spec inbox` does: kind drift, its node, open, its rows' states; high, no rationale, nothing staged", async () => {
    const { proposals } = await mock().getInbox(SLUG);
    expect(proposals.find((entry) => entry.id === DRIFT_ITEM)).toEqual({
      id: DRIFT_ITEM,
      kind: "drift",
      status: "open",
      target_id: DRIFT_NODE,
      target_ids: [DRIFT_NODE],
      branch: "main",
      created_at: stamp(NOW, 10),
      rationale: null,
      severity: "high",
      summary: "broken_binding symbol_gone, code_ahead code_changed",
      record_id: null,
      staged_at: null,
    });
  });

  it("serves its review: 48 keys, drift after revisions as {hash, rows, more}; the lock its path; no texts, diff, preview or revisions; the note", async () => {
    const review = await mock().getProposal(SLUG, DRIFT_ITEM);
    const keys = Object.keys(review);
    expect(keys).toHaveLength(48);
    expect(keys.slice(keys.indexOf("revisions"), keys.indexOf("revisions") + 3)).toEqual(["revisions", "drift", "decided_by"]);
    expect(review).toMatchObject({
      id: DRIFT_ITEM,
      kind: "drift",
      status: "open",
      target_id: DRIFT_NODE,
      target_path: "spec.lock",
      target_ids: [DRIFT_NODE],
      base_hash: null,
      base_text: null,
      new_text: null,
      patch_hash: null,
      rationale: null,
      diagnostics: [],
      diff: null,
      preview: null,
      conflict: null,
      revisions: null,
      evidence: [],
      options: [],
      staged: null,
      staged_at: null,
      interpretation: null,
      notes: [`spec approve ${DRIFT_ITEM} shows the rows before asking`],
    });
    expect(review.base_commit).toMatch(/^[0-9a-f]{40}$/);
    const drift = review.drift;
    expect(drift === null ? null : Object.keys(drift)).toEqual(["hash", "rows", "more"]);
    expect(drift?.hash).toMatch(/^b3:[0-9a-f]{64}$/);
    expect(drift?.more).toBe(0);
  });

  it("stores rows as `spec verify --json` writes them, Pre-existing, by key; the summary their distinct states, high for a Fix row", async () => {
    const review = await mock().getProposal(SLUG, DRIFT_ITEM);
    const rows = rowsOf(review.drift?.rows ?? []);
    expect(rows).toHaveLength(2);
    expect(rows.map((row) => Object.keys(row))).toEqual([ROW_KEYS, ROW_KEYS]);
    expect(rows.map((row) => [row.node, row.pre_existing, row.state])).toEqual([
      [DRIFT_NODE, true, "broken_binding"],
      [DRIFT_NODE, true, "code_ahead"],
    ]);
    const files = rows.map((row) => (typeof row.file === "string" ? row.file : ""));
    expect(files).toEqual([...files].sort());
    // `state[ category]`, a null category left out.
    const states = [...new Set(rows.map((row) => [row.state, row.category].filter((part) => typeof part === "string").join(" ")))];
    expect(review.summary).toBe(states.join(", "));
    expect([rows.some((row) => row.class === "fix"), review.severity]).toEqual([true, "high"]);
  });

  it("holds the mock's two drift items, PR-0010 and PR-0011, both ledger-api's: every other review's drift is null", async () => {
    const client = mock();
    const found: string[] = [];
    const drifts = new Set([DRIFT_ITEM, DRIFT_ITEM_2]);
    for (const slug of ["harbor-sim", SLUG]) {
      for (const entry of (await client.getInbox(slug)).proposals) {
        const review = await client.getProposal(slug, entry.id);
        const drift = slug === SLUG && drifts.has(entry.id);
        expect([slug, entry.id, review.drift !== null, review.kind === "drift"]).toEqual([slug, entry.id, drift, drift]);
        if (review.drift !== null) {
          found.push(entry.id);
        }
      }
    }
    expect(found.sort()).toEqual([DRIFT_ITEM, DRIFT_ITEM_2]);
  });

  it("stores PR-0011's rows as a newer verify writes them: 17 keys in DriftRow's order, by key; the summary their distinct states, high for a Fix row; its own note", async () => {
    const review = await mock().getProposal(SLUG, DRIFT_ITEM_2);
    expect(review).toMatchObject({
      id: DRIFT_ITEM_2,
      kind: "drift",
      status: "open",
      target_id: DRIFT_NODE_2,
      target_path: "spec.lock",
      target_ids: [DRIFT_NODE_2],
      severity: "high",
      summary: "spec_ahead same_revision, conflict",
      notes: [`spec approve ${DRIFT_ITEM_2} shows the rows before asking`],
    });
    expect(review.drift?.hash).toMatch(/^b3:[0-9a-f]{64}$/);
    expect(review.drift?.hash).not.toBe((await mock().getProposal(SLUG, DRIFT_ITEM)).drift?.hash);
    expect(review.drift?.more).toBe(0);
    const rows = rowsOf(review.drift?.rows ?? []);
    expect(rows.map((row) => Object.keys(row))).toEqual([ROW_KEYS_17, ROW_KEYS_17]);
    const files = rows.map((row) => (typeof row.file === "string" ? row.file : ""));
    expect(files).toEqual([...files].sort());
    const states = [...new Set(rows.map((row) => [row.state, row.category].filter((part) => typeof part === "string").join(" ")))];
    expect(review.summary).toBe(states.join(", "));
    expect([rows.some((row) => row.class === "fix"), rows.every((row) => row.pre_existing === true)]).toEqual([true, true]);
  });

  it("refuses a stage on it, approve or reject, 409 in the daemon's words; nothing stored", async () => {
    const client = mock();
    const before = await client.getProposal(SLUG, DRIFT_ITEM);
    const stages: StageChoice[] = [
      { decision: "approve", option: null, answer: null, canon: null, editorial: null, note: null },
      { decision: "approve", option: null, answer: null, canon: null, editorial: true, note: null },
      { decision: "reject", reason: "Not now" },
    ];
    for (const stage of stages) {
      const refused = await client.stageDecision(SLUG, DRIFT_ITEM, stage, before.updated_at ?? "").then(
        () => null,
        (error: unknown) => error,
      );
      expect(refused).toBeInstanceOf(ClientError);
      expect([stage.decision, (refused as ClientError).status, (refused as ClientError).message]).toEqual([stage.decision, 409, REFUSED]);
    }
    expect(await client.getProposal(SLUG, DRIFT_ITEM)).toEqual(before);
  });

  it("checks updated_at first, as the daemon does: a stale read is refused as changed, not as a drift item", async () => {
    const client = mock();
    const before = await client.getProposal(SLUG, DRIFT_ITEM);
    const refused = await refusalOf(client.stageDecision(SLUG, DRIFT_ITEM, { decision: "reject", reason: "Not now" }, "2026-01-01T00:00:00Z"));
    expect(refused).toBeInstanceOf(ClientError);
    const { status, message } = refused as ClientError;
    expect(status).toBe(409);
    expect(message).toBe(
      `\`${DRIFT_ITEM}\` changed since it was read: updated at ${before.updated_at ?? ""}, the choice was made on the one of 2026-01-01T00:00:00Z; read it again; nothing staged`,
    );
    expect(message).not.toContain("drift item");
    expect(await client.getProposal(SLUG, DRIFT_ITEM)).toEqual(before);
  });

  it("finds nothing staged elsewhere on it in the conflict scenario: a drift item holds no stage; the first stage meets the drift refusal", async () => {
    const client = mock("conflict");
    const before = await client.getProposal(SLUG, DRIFT_ITEM);
    const refused = await refusalOf(client.stageDecision(SLUG, DRIFT_ITEM, { decision: "reject", reason: "Not now" }, before.updated_at ?? ""));
    expect([(refused as ClientError).status, (refused as ClientError).message]).toEqual([409, REFUSED]);
    const outcome = await client.stageRoundAnswers(SLUG, [{ id: DRIFT_ITEM, stage: { decision: "reject", reason: "Not now", updated_at: before.updated_at ?? "" } }]);
    expect(outcome.answers.map((answer) => [answer.outcome, answer.reason])).toEqual([["refused", REFUSED]]);
    expect(await client.getProposal(SLUG, DRIFT_ITEM)).toEqual(before);
  });

  it("keeps it off the round sheet, noted with its commands; an answer on it refused alone", async () => {
    const client = mock();
    const sheet = await client.getRound(SLUG);
    expect(sheet.items.map((item) => item.id)).not.toContain(DRIFT_ITEM);
    expect(sheet.notes).toContain(`2 open drift item(s) not on this sheet: ${DRIFT_ITEM} and ${DRIFT_ITEM_2}; spec review <ID>, then spec approve or spec reject`);
    const read = await client.getProposal(SLUG, DRIFT_ITEM);
    const outcome = await client.stageRoundAnswers(SLUG, [{ id: DRIFT_ITEM, stage: { decision: "reject", reason: "Not now", updated_at: read.updated_at ?? "" } }]);
    expect(outcome.answers).toEqual([
      { id: DRIFT_ITEM, outcome: "refused", stage: null, staged_at: null, replaced: null, reason: REFUSED },
    ]);
    expect((await client.getProposal(SLUG, DRIFT_ITEM)).staged).toBeNull();
  });

  it("drift_item_never_offers_a_stage over the mock: the evidence, the note and the terminal commands; no decision button", async () => {
    window.history.replaceState(null, "", `/?scenario=normal#/${SLUG}/inbox/${DRIFT_ITEM}`);
    render(<App client={mock()} scenario={null} />);
    const card = await screen.findByRole("article");
    await within(card).findByRole("heading", { level: 3, name: "Drift evidence" });
    expect(screen.getByText("Mock data")).toBeTruthy();
    for (const name of ["Accept", "Reject", "Needs clarification", "Defer"]) {
      expect([name, within(card).queryByRole("button", { name })]).toEqual([name, null]);
    }
    const terminal = within(card).getByRole("region", { name: "Decided on a terminal" });
    expect(Array.from(terminal.querySelectorAll(".command-text"), (command) => command.textContent)).toEqual([
      `spec approve ${DRIFT_ITEM}`,
      `spec approve ${DRIFT_ITEM} --editorial`,
      `spec reject ${DRIFT_ITEM} --reason "..."`,
    ]);
    expect(terminal.textContent).toContain("(/work/ledger-api)");
    const evidence = within(card).getByRole("heading", { level: 3, name: "Drift evidence" }).closest("section");
    expect(evidence?.querySelector("dd")?.textContent).toBe("2 rows");
    // An older build's 15-key rows, typed all the same: two drift lines, none shown as sent (UI-1 of ui-drift-evidence, M1).
    const list = within(evidence ?? card).getByRole("list", { name: "Stored rows, in spec verify's order" });
    expect(Array.from(list.children, (row) => row.getAttribute("data-drift"))).toEqual([DRIFT_NODE, DRIFT_NODE]);
    expect(list.querySelectorAll("[data-raw]")).toHaveLength(0);
    expect(Array.from(list.querySelectorAll(".health-drift-facts"), (facts) => Array.from(facts.children, (fact) => fact.textContent))).toEqual([
      ["No marker", "Node rev 5", "Locked rev 5, a0cea48d2f61b9e07c3a5d8f14b26e9c7d0a3f51, 2026-10-09", "Not compared"],
      ["Marker line 41 @5", "Node rev 5", "Locked rev 5, a0cea48d2f61b9e07c3a5d8f14b26e9c7d0a3f51, 2026-10-09", "Compared ast"],
    ]);
    expect(list.querySelectorAll(".health-drift-levels, .health-code-tail, .drift-editorial")).toHaveLength(0);
    // Neither options nor a section diff: the review sends none for a drift item.
    for (const title of ["Options", "Section diff"]) {
      expect([title, within(card).queryByRole("heading", { level: 3, name: title })]).toEqual([title, null]);
    }
    expect(within(card).queryByText("No options attached.")).toBeNull();
    await waitFor(() => {
      expect(within(card).getByText(`spec approve ${DRIFT_ITEM} shows the rows before asking`)).toBeTruthy();
    });
  });

  it("PR-0011's card over the mock: two typed rows, the first marked Editorial alone, the second moved with its levels changed; --editorial listed", async () => {
    window.history.replaceState(null, "", `/?scenario=normal#/${SLUG}/inbox/${DRIFT_ITEM_2}`);
    render(<App client={mock()} scenario={null} />);
    const card = await screen.findByRole("article");
    const evidence = (await within(card).findByRole("heading", { level: 3, name: "Drift evidence" })).closest("section") ?? card;
    expect(evidence.querySelector("dd")?.textContent).toBe("2 rows");
    const list = within(evidence).getByRole("list", { name: "Stored rows, in spec verify's order" });
    const rows = Array.from(list.children);
    expect(rows.map((row) => [row.getAttribute("data-drift"), row.querySelector(".health-code-head .badge-label")?.textContent, row.querySelector(".health-code-meta .badge-label")?.textContent])).toEqual([
      [DRIFT_NODE_2, "Check", "Spec ahead"],
      [DRIFT_NODE_2, "Fix", "Conflict"],
    ]);
    expect(list.querySelectorAll("[data-raw]")).toHaveLength(0);
    expect(rows.map((row) => Array.from(row.querySelectorAll(".health-code-head .plain-tag"), (tag) => tag.textContent))).toEqual([
      ["Pre-existing", "Editorial"],
      ["Pre-existing"],
    ]);
    expect(Array.from(rows[1]?.querySelectorAll(".health-code-tail") ?? [], (tail) => tail.textContent)).toEqual([
      "Moved from src/refunds/policy.rs | item refunds::policy::is_refundable function_item",
    ]);
    expect(Array.from(rows[1]?.querySelectorAll(".health-drift-levels .health-code-fact") ?? [], (fact) => fact.textContent)).toEqual([
      "lock sig, body, deps",
      "markers sig, body, deps",
      "changed body, deps",
    ]);
    expect(Array.from(rows[0]?.querySelectorAll(".health-drift-levels .health-code-fact") ?? [], (fact) => fact.textContent)).toEqual(["lock sig, body", "markers sig, body"]);
    const terminal = within(card).getByRole("region", { name: "Decided on a terminal" });
    expect(Array.from(terminal.querySelectorAll(".command-text"), (command) => command.textContent)).toEqual([
      `spec approve ${DRIFT_ITEM_2}`,
      `spec approve ${DRIFT_ITEM_2} --editorial`,
      `spec reject ${DRIFT_ITEM_2} --reason "..."`,
    ]);
    expect(card.textContent).not.toMatch(/block/i);
  });
});
