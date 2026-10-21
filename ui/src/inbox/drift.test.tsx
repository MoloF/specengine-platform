import { act, cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DriftEvidence, DriftRow, Proposal, StoredDriftRow } from "../api/types";
import { sectionHash } from "../app/routes";
import { driftClassLook, isEditorialRow } from "../health/labels";
import { aDriftRow, aProposal } from "../test/builders";
import { renderApp } from "../test/render";
import { stubClient } from "../test/stubClient";
import { syncLook } from "../tree/labels";
import { driftCommands, driftRowsText, driftRowText, storedRowOf } from "./drift";

// UI-1 of docs/features/drift-queue.md on a stub client ("Review, inbox, round, stage"; Q-A): a
// drift item is listed in the inbox and read in its review, and never offers a stage. Its card
// shows the evidence (the rows counted, the commit judged, the hash, each stored row) and, in place
// of the four decisions, the terminal commands that decide it: `spec approve PR`, its
// `--editorial` form, `spec reject PR --reason`. The review document's `drift` key decides it,
// never the kind (ADR-0031); the decision keys open no dialog on it and nothing is sent.
//
// UI-1 to UI-4 of docs/features/ui-drift-evidence.md: each stored row typed (`storedRowOf`) and
// drawn by Health's row component with every field (M2: the lock's commit dropped); a row not in
// the shape shown as sent, still counted (M3: dropped); `--editorial` always listed, a `spec_ahead
// same_revision` row marked Editorial (M4: the command only with such a row).

/** A stored row as an older build wrote it: the 15 keys core's `drift::DriftRow` reads, no `levels` or `anchor`. */
function olderRow(row: DriftRow): StoredDriftRow {
  const kept: StoredDriftRow = { ...row };
  delete kept.levels;
  delete kept.anchor;
  return kept;
}

const LOCKED = "a0cea48d2f61b9e07c3a5d8f14b26e9c7d0a3f51";

/** Two rows as `spec verify --json` writes them, by key: one of a newer build (17 keys), one of an older (15). */
const ROWS: StoredDriftRow[] = [
  aDriftRow({
    node: "R-12",
    file: "src/a.rs",
    symbol: "a::f",
    state: "conflict",
    class: "fix",
    pre_existing: true,
    lock: { rev: 2, commit: LOCKED, verified_at: "2026-10-10" },
    compared: "ast",
    levels: { lock: ["sig", "body"], markers: ["sig", "body"], changed: ["body"] },
    anchor: { state: "exact", from: null, candidates: null },
  }),
  olderRow(aDriftRow({ node: "R-12", file: "src/b.rs", symbol: "b::g", state: "code_ahead", category: "code_changed", detail: "body", pre_existing: true })),
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
      driftRowsText({ hash: HASH, rows: ROWS.slice(0, 1), more: 0 }),
      driftRowsText({ hash: HASH, rows: ROWS, more: 70 }),
    ]).toEqual(["2 rows", "1 row", "72 rows, the first 2 stored with the item"]);
    expect(driftRowText(ROWS[0])).toBe(JSON.stringify(ROWS[0]));
    expect(driftRowText({ b: 1, a: [null] })).toBe('{"b":1,"a":[null]}');
  });
});

/** A row as the store would hand it on, the guard's input: any JSON value. */
const asSent = (value: unknown) => value as StoredDriftRow;

describe("a stored row's shape (UI-1, UI-3 of ui-drift-evidence)", () => {
  it("types a row of 15 keys (an older build's) and of 17, and one with a later key, ignoring that key", () => {
    const newer = ROWS[0];
    const older = ROWS[1];
    expect([Object.keys(newer ?? {}).length, Object.keys(older ?? {}).length]).toEqual([17, 15]);
    expect(storedRowOf(newer)).toBe(newer);
    expect(storedRowOf(older)).toBe(older);
    const later = { ...newer, since: { build: 9 } };
    expect(storedRowOf(later)).toBe(later);
    // levels and anchor null are as good as absent.
    const nulls = { ...older, levels: null, anchor: null };
    expect(storedRowOf(nulls)).toBe(nulls);
  });

  it("refuses a row missing any of the 15 keys core reads, but never one missing levels or anchor", () => {
    const row = ROWS[0] ?? aDriftRow({ node: "R-12" });
    const keys = Object.keys(row);
    for (const key of keys) {
      const missing = Object.fromEntries(Object.entries(row).filter(([name]) => name !== key));
      expect([key, storedRowOf(missing) === null]).toEqual([key, key !== "levels" && key !== "anchor"]);
    }
  });

  it("refuses a value of the wrong type, at any depth; and anything but an object", () => {
    const row = ROWS[0] ?? aDriftRow({ node: "R-12" });
    const wrong: [string, unknown][] = [
      ["node", null],
      ["relation", 3],
      ["file", ["src/a.rs"]],
      ["kind", 1],
      ["state", null],
      ["class", false],
      ["pre_existing", "true"],
      ["markers", null],
      ["markers", [{ line: "41", rev: 2 }]],
      ["markers", [{ line: 41 }]],
      ["markers", [{ line: -1, rev: 2 }]],
      ["node_rev", 1.5],
      ["lock", { rev: 2, commit: LOCKED }],
      ["lock", { rev: 2, commit: 7, verified_at: "2026-10-10" }],
      ["compared", 0],
      ["levels", { lock: "sig", markers: [], changed: null }],
      ["levels", { lock: [], markers: [] }],
      ["anchor", { state: "moved", from: { file: "src/old.rs", kind: "item" }, candidates: null }],
      ["anchor", "exact"],
    ];
    for (const [key, value] of wrong) {
      expect([key, value, storedRowOf({ ...row, [key]: value })]).toEqual([key, value, null]);
    }
    for (const value of [null, "row", 42, true, [], [row]]) {
      expect([value, storedRowOf(value)]).toEqual([value, null]);
    }
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

  it("shows the evidence first: the rows counted, the commit judged, the hash, each stored row typed in the stored order; the review's note", async () => {
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
    const rows = within(evidence ?? card).getByRole("list", { name: "Stored rows, in spec verify's order" });
    expect(Array.from(rows.children, (row) => [row.getAttribute("data-drift"), row.querySelector("code")])).toEqual([
      ["R-12", null],
      ["R-12", null],
    ]);
    expect(Array.from(rows.querySelectorAll(".health-code-place"), (place) => place.textContent).filter((text) => text.startsWith("src/"))).toEqual(["src/a.rs", "src/b.rs"]);
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

describe("a drift item's rows on its card (UI-2, UI-3, UI-4 of ui-drift-evidence)", () => {
  const ESC = String.fromCharCode(0x1b);
  const LF = String.fromCharCode(10);
  const RLO = String.fromCodePoint(0x202e);

  /** The editorial command's meaning, word for word. */
  const EDITORIAL_MEANING =
    "Approve as editorial: only when the node's text changed at its locked revision and keeps its meaning; rows marked Editorial were so when the item was raised, and the terminal judges HEAD again; a plain approve refused for that names this command";

  /**
   * Rows of every field's forms, in no order the card could sort them into (by state, class, file
   * or node): a newer build's (17 keys), an older build's (15), one with a later key (18), then a
   * `spec_ahead marker_behind` and a `conflict` row, neither an editorial approve's.
   */
  const FIELDS: StoredDriftRow[] = [
    aDriftRow({
      node: "R-12",
      file: "src/zz.rs",
      symbol: "alpha::zz::regen",
      state: "code_ahead",
      category: "code_changed",
      detail: "body",
      pre_existing: true,
      markers: [
        { line: 41, rev: 5 },
        { line: 88, rev: null },
      ],
      node_rev: 3,
      lock: { rev: 2, commit: LOCKED, verified_at: "2026-10-09" },
      compared: "ast",
      levels: { lock: ["sig", "body"], markers: ["sig", "body"], changed: ["body"] },
      anchor: { state: "moved", from: { file: "src/old.rs", kind: "item", symbol: "alpha::old::regen", item_kind: "function_item" }, candidates: null },
    }),
    olderRow(
      aDriftRow({
        node: "",
        relation: "verifies",
        file: `src/b${LF}c${ESC}.rs`,
        kind: null,
        symbol: null,
        item_kind: null,
        state: "bad_marker",
        category: "unanchored",
        class: "fix",
        pre_existing: true,
        markers: [],
        node_rev: null,
      }),
    ),
    {
      ...aDriftRow({
        node: "A-1",
        file: "src/aa.rs",
        symbol: "alpha::aa::tick",
        state: "spec_ahead",
        category: "same_revision",
        pre_existing: true,
        markers: [{ line: 9, rev: null }],
        node_rev: 2,
        lock: { rev: 2, commit: "c41f7a2e9b0d6c3f8a5e1b7d4c9f2a6e0b3d8c17", verified_at: "2026-10-11" },
        compared: "blob",
        levels: { lock: ["sig"], markers: [], changed: null },
        anchor: {
          state: "ambiguous",
          from: null,
          candidates: [
            { file: "src/aa.rs", kind: "item", symbol: "alpha::aa::tick", item_kind: "function_item" },
            { file: "src/ab.rs", kind: "module", symbol: "alpha::ab", item_kind: null },
          ],
        },
      }),
      // A key a later build adds: kept by the daemon, ignored here.
      ...{ evidence_of: "a later build" },
    },
    aDriftRow({ node: "A-1", file: "src/ab.rs", symbol: "alpha::ab::tock", state: "spec_ahead", category: "marker_behind" }),
    aDriftRow({ node: "A-1", file: "src/ac.rs", symbol: "alpha::ac::tack", state: "conflict", class: "fix" }),
  ];

  /** The card of an item holding `rows`, `more` past them; its stored rows' list, none when no row is stored. */
  async function rowsOn(rows: StoredDriftRow[], more = 0) {
    const { card } = await openCard([driftItem({ drift: { hash: HASH, rows, more } })]);
    const evidence = sectionOf(card, "Drift evidence") ?? card;
    const list = within(evidence).queryByRole("list", { name: "Stored rows, in spec verify's order" });
    return { card, evidence, list: list ?? document.createElement("ol") };
  }

  const textOf = (element: Element | null | undefined) => element?.textContent ?? null;

  /** A badge's words and colour role. */
  function badge(element: Element | null | undefined): [string | null, string | null] {
    return [textOf(element?.querySelector(".badge-label")), element?.querySelector(".badge")?.getAttribute("data-tone") ?? null];
  }

  it("types rows of 17, 15 and 18 keys alike", () => {
    expect(FIELDS.slice(0, 3).map((row) => Object.keys(row).length)).toEqual([17, 15, 18]);
    expect(FIELDS.map((row) => storedRowOf(row) === row)).toEqual([true, true, true, true, true]);
  });

  it("draws each row as Health does: class, node linked to the tree (a bare marker's as text), relation, Pre-existing; state labelled, category and detail raw; file and binding on one line; moved from and candidates", async () => {
    const { list } = await rowsOn(FIELDS);
    const lines = Array.from(list.children);
    expect(lines.map((line) => line.getAttribute("data-drift"))).toEqual(["R-12", "", "A-1", "A-1", "A-1"]);
    expect(lines.every((line) => !line.hasAttribute("data-raw"))).toBe(true);
    expect(
      lines.map((line) => {
        const head = line.querySelector(".health-code-head");
        const id = head?.querySelector(".health-code-id");
        return [...badge(head), textOf(id), id?.getAttribute("href") ?? null, textOf(head?.querySelector(".kind-tag")), Array.from(head?.querySelectorAll(".plain-tag") ?? [], (tag) => tag.textContent)];
      }),
    ).toEqual([
      ["Check", "finding-warning", "R-12", sectionHash("alpha", "tree", "R-12"), "implements", ["Pre-existing"]],
      ["Fix", "finding-error", "No ID", null, "verifies", ["Pre-existing"]],
      ["Check", "finding-warning", "A-1", sectionHash("alpha", "tree", "A-1"), "implements", ["Pre-existing", "Editorial"]],
      ["Check", "finding-warning", "A-1", sectionHash("alpha", "tree", "A-1"), "implements", []],
      ["Fix", "finding-error", "A-1", sectionHash("alpha", "tree", "A-1"), "implements", []],
    ]);
    const metas = lines.map((line) => Array.from(line.querySelectorAll(".health-code-meta")));
    expect(metas.map(([state]) => [...badge(state), textOf(state?.querySelector(".health-code-word"))])).toEqual([
      ["Code ahead", "sync-code-ahead", "code_changed body"],
      ["Bad marker", "sync-bad-marker", "unanchored"],
      ["Spec ahead", "sync-spec-ahead", "same_revision"],
      ["Spec ahead", "sync-spec-ahead", "marker_behind"],
      ["Conflict", "sync-conflict", null],
    ]);
    // A line break in a file shows as a space; another control character as \u{..}.
    expect(metas.map(([, place]) => Array.from(place?.children ?? [], (part) => part.textContent))).toEqual([
      ["src/zz.rs", "item alpha::zz::regen function_item"],
      ["src/b c\\u{1b}.rs", "No symbol"],
      ["src/aa.rs", "item alpha::aa::tick function_item"],
      ["src/ab.rs", "item alpha::ab::tock function_item"],
      ["src/ac.rs", "item alpha::ac::tack function_item"],
    ]);
    expect(lines.map((line) => Array.from(line.querySelectorAll(".health-code-tail"), (tail) => tail.textContent))).toEqual([
      ["Moved from src/old.rs | item alpha::old::regen function_item"],
      [],
      ["Candidate src/aa.rs | item alpha::aa::tick function_item", "Candidate src/ab.rs | module alpha::ab"],
      [],
      [],
    ]);
    expect(list.textContent).not.toMatch(/null|undefined|block|a later build|now/i);
  });

  it("draws the six fields Health left out: markers (@N, unversioned, none), the node's revision, the lock's revision, whole commit and date, compared raw, the levels (absent when null or not stored)", async () => {
    const { list } = await rowsOn(FIELDS);
    const lines = Array.from(list.children);
    expect(lines.map((line) => Array.from(line.querySelectorAll(".health-drift-facts .health-code-fact"), (fact) => fact.textContent))).toEqual([
      ["Markers line 41 @5, line 88 unversioned", "Node rev 3", `Locked rev 2, ${LOCKED}, 2026-10-09`, "Compared ast"],
      ["No marker", "No node revision", "Not locked", "Not compared"],
      ["Marker line 9 unversioned", "Node rev 2", "Locked rev 2, c41f7a2e9b0d6c3f8a5e1b7d4c9f2a6e0b3d8c17, 2026-10-11", "Compared blob"],
      ["Marker line 3 @1", "Node rev 1", "Not locked", "Not compared"],
      ["Marker line 3 @1", "Node rev 1", "Not locked", "Not compared"],
    ]);
    expect(lines.map((line) => line.querySelector(".health-drift-facts time")?.getAttribute("dateTime") ?? null)).toEqual(["2026-10-09", null, "2026-10-11", null, null]);
    expect(
      lines.map((line) => {
        const levels = line.querySelector(".health-drift-levels");
        return levels === null ? null : [textOf(levels.firstElementChild), ...Array.from(levels.querySelectorAll(".health-code-fact"), (fact) => fact.textContent)];
      }),
    ).toEqual([["Levels", "lock sig, body", "markers sig, body", "changed body"], null, ["Levels", "lock sig", "markers none"], null, null]);
    // The second row is an older build's: no levels or anchor key at all, drawn all the same.
    expect(Object.keys(FIELDS[1] ?? {})).not.toContain("levels");
  });

  it("shows a row not in the shape as its compact JSON in its stored place, between typed rows, and counts it; a bidirectional mark JSON leaves as is written \\u{..}", async () => {
    const typed = FIELDS[3] ?? aDriftRow({ node: "A-1" });
    const odd: unknown[] = [
      null,
      "row",
      [],
      Object.fromEntries(Object.entries(typed).filter(([name]) => name !== "state")),
      { ...typed, node: 7, file: `src/a${RLO}b.rs` },
      { ...typed, levels: "sig" },
      { ...typed, markers: [{ line: -1, rev: null }] },
    ];
    const rows = [FIELDS[0], ...odd.slice(0, 3), FIELDS[1], ...odd.slice(3), FIELDS[4]].map(asSent);
    const { evidence, list } = await rowsOn(rows, 3);
    expect(evidence.querySelector("dd")?.textContent).toBe("13 rows, the first 10 stored with the item");
    expect(list.children).toHaveLength(10);
    // JSON escapes a line feed itself; the override it leaves as is, the card writes \u{202e}.
    const asShown = (row: unknown) => JSON.stringify(row).replaceAll(RLO, "\\u{202e}");
    expect(Array.from(list.children, (line) => (line.hasAttribute("data-raw") ? line.querySelector("code")?.textContent : line.getAttribute("data-drift")))).toEqual([
      "R-12",
      ...odd.slice(0, 3).map(asShown),
      "",
      ...odd.slice(3).map(asShown),
      "A-1",
    ]);
    expect(list.textContent).not.toContain(RLO);
    expect(list.querySelector("[data-raw]:nth-child(7) code")?.textContent).toContain('"file":"src/a\\u{202e}b.rs"');
    expect(Array.from(list.querySelectorAll("[data-raw]"), (line) => line.querySelector(".drift-raw-note")?.textContent)).toEqual(
      Array.from(odd, () => "Shown as sent: not in the shape this screen reads."),
    );
  });

  it("writes an unlisted state or class word on one line in its neutral badge: a line feed a space, a bidirectional mark \\u{..}", async () => {
    const row = aDriftRow({ node: "A-1", state: `drift${LF}ing${RLO}`, class: `advisory${RLO}${LF}soon` });
    const { list } = await rowsOn([row]);
    const line = list.querySelector("[data-drift]");
    expect([badge(line?.querySelector(".health-code-head")), badge(line?.querySelector(".health-code-meta"))]).toEqual([
      ["advisory\\u{202e} soon", "sync-unknown"],
      ["drift ing\\u{202e}", "sync-unknown"],
    ]);
    expect(list.textContent).not.toContain(RLO);
    expect(list.textContent).not.toContain(LF);
    // The two look functions themselves.
    expect([syncLook(`a${LF}b${RLO}`).label, driftClassLook(`c${ESC}d`).label, syncLook("conflict").label, driftClassLook(null).label]).toEqual([
      "a b\\u{202e}",
      "c\\u{1b}d",
      "Conflict",
      "No class",
    ]);
  });

  it("marks only a spec_ahead same_revision row as an editorial approve's (labels.ts)", () => {
    const pairs: [string, string | null, boolean][] = [
      ["spec_ahead", "same_revision", true],
      ["spec_ahead", "marker_behind", false],
      ["spec_ahead", "spec_changed", false],
      ["spec_ahead", null, false],
      ["conflict", null, false],
      ["conflict", "same_revision", false],
      ["code_ahead", "same_revision", false],
      ["Spec_ahead", "same_revision", false],
    ];
    for (const [state, category, editorial] of pairs) {
      expect([state, category, isEditorialRow({ state, category })]).toEqual([state, category, editorial]);
    }
  });

  it("always lists spec approve --editorial with its meaning: with no row an editorial approve may fit, with one, and with none stored; only the spec_ahead same_revision row is marked Editorial", async () => {
    const cases: [unknown[], string[]][] = [
      [[FIELDS[0], FIELDS[1], FIELDS[3], FIELDS[4]], []],
      [FIELDS, ["src/aa.rs"]],
      [[], []],
    ];
    for (const [rows, marked] of cases) {
      const { card } = await rowsOn(rows.map(asSent));
      const terminal = within(card).getByRole("region", { name: "Decided on a terminal" });
      expect(Array.from(terminal.querySelectorAll(".command"), (command) => [command.getAttribute("data-action"), textOf(command.querySelector(".command-text"))])).toEqual([
        ["approve", "spec approve PR-0012"],
        ["editorial", "spec approve PR-0012 --editorial"],
        ["reject", 'spec reject PR-0012 --reason "..."'],
      ]);
      expect(textOf(terminal.querySelector('[data-action="editorial"] .command-meaning'))).toBe(EDITORIAL_MEANING);
      const editorial = Array.from(card.querySelectorAll(".drift-editorial"), (tag) => textOf(tag.closest("li")?.querySelector(".health-code-place")));
      expect(editorial).toEqual(marked);
      cleanup();
    }
  });
});
