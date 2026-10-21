import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import appCss from "../styles/app.css?raw";
import { ClientError } from "../api/client";
import type { DriftRow, LockDiffNode, LockDiffRow, LockDiffView, ShownBindings } from "../api/types";
import { aDriftRow } from "../test/builders";
import { nodeCommands } from "./commands";
import { renderApp } from "../test/render";
import { argsOf, type StubClient } from "../test/stubClient";
import { treeClient, viewOf } from "../test/treeStub";

// docs/features/lock-diff-ui.md on a stub client: UI-2 (a conflict's two sides side by side, a
// lone side spanning, stacked below 48em; hunks byte for byte as sent), UI-3 (each state its own
// words: equal, not shown with N1, N7 or no note, a side not sent, rows not judged, none, a REF
// naming nothing, loading, failed, failed again), UI-5 (a row's commands, its ID checked) and UI-6
// (each figure named by its title; "Added: " and "Removed: " for screen readers; the disclosure's
// state). The views mirror the mock's rows (src/mocks/lockDiff.ts), which app tests may not
// import (policy.test.ts AC-05): the mock itself is drawn in mocks/tree.smoke.test.tsx. UI-1 (one
// read, on the first expanding) is in NodePane.test.tsx, UI-4 (an apply reads it again) in
// api/live.test.tsx.

const COMMIT = "4f1c9a2e7b3d5c8e0a6f1b2d3c4e5f60718293a4";
const BLOB = "b6e2d94a1f0c38e75a2d6b9c4e1f7a0358c2d6e9";

/** `docs/canon/spec-lock-diff.md` "Output"'s example: code-a's sentence and CAP edited, uncommitted. */
const SPEC_DIFF = "@@ -1,3 +1,3 @@\n ## Bound {#LAW-BOUND rev=2}\n \n-The count never exceeds the cap.\n+The count never exceeds the cap, twelve by default.\n";
const CODE_DIFF = "@@ -1 +1 @@\n-pub const CAP: u32 = 10; // @configures LAW-BOUND@2\n+pub const CAP: u32 = 12; // @configures LAW-BOUND@2\n";

/** A non-Latin comment, escaped (ADR-0024): a berth at night. */
const NON_LATIN = String.fromCodePoint(0x41f, 0x440, 0x438, 0x447, 0x430, 0x43b, 0x20, 0x2014, 0x20, 0x43d, 0x43e, 0x447, 0x44c);

/**
 * Hunks a careless drawing would change: git's function context on the header, leading spaces, a
 * tab, trailing spaces, a CR, lines that look like file headers inside a hunk, an empty context
 * line, a non-Latin line, a "\ No newline" line, a second hunk.
 */
const HARD_DIFF = [
  "@@ -3,6 +3,7 @@ pub fn moor(vessel: &Vessel) -> Result<Moored, MoorError> {",
  "     if vessel.draft() > berth_limit(berth) {",
  "-\tcrew.start(vessel, Minutes(20));  ",
  "+\tlet minutes = if is_night() { Minutes(30) } else { Minutes(20) };\r",
  "+--- not a file header",
  "-+++ nor this one",
  " ",
  `+// ${NON_LATIN}`,
  "\\ No newline at end of file",
  "@@ -40 +41 @@",
  "-}",
  "+}  ",
  "",
].join("\n");

/** The key DOC-A's rows carry (`aDriftRow`'s binding), as the notes name it. */
const KEY = "DOC-A | implements | src/lib.rs | item alpha::lib::run function_item";

const N1 = `lock diff: ${KEY}: no spec diff: docs/spec/a.md cannot be read at ${COMMIT}`;
const N7 = `lock diff: ${KEY}: no code diff: git shows no hunk for bytes that differ`;

const SPEC_TITLE = `Node text since the lock: docs/spec/a.md at ${COMMIT}`;
const CODE_TITLE = `Code since the lock: src/lib.rs, blob ${BLOB}`;

/** A kept row: verify's row for `node` (a conflict unless given), its sides as given. */
function kept(node: string, spec: LockDiffRow["spec"], code: LockDiffRow["code"], fields: Partial<DriftRow> = {}): LockDiffRow {
  return {
    row: aDriftRow({ node, state: "conflict", class: "fix", lock: { rev: 2, commit: COMMIT, verified_at: "2026-10-02" }, node_rev: 3, compared: "ast", ...fields }),
    spec,
    code,
  };
}

/** A spec side of docs/spec/a.md at COMMIT. */
function specSide(diff: string | null): LockDiffRow["spec"] {
  return { commit: COMMIT, path: "docs/spec/a.md", diff };
}

/** A code side of src/lib.rs's BLOB. */
function codeSide(diff: string | null): LockDiffRow["code"] {
  return { blob: BLOB, file: "src/lib.rs", diff };
}

/** As the mock's MEC-TIDES: a conflict, both sides with hunks. */
const CONFLICT = kept("DOC-A", specSide(SPEC_DIFF), codeSide(CODE_DIFF));

/** As the mock's MEC-MOORING: code ahead, the code side alone. */
const CODE_AHEAD = kept("DOC-A", null, codeSide(CODE_DIFF), { state: "code_ahead", category: "code_changed", detail: "body", class: "check" });

/** The document for `ref`'s one holder (DOC-A's place), its rows and notes as given. */
function lockDiff(rows: LockDiffRow[] | null, notes: string[] = []): LockDiffView {
  return { ref: "DOC-A", reason: null, notes, nodes: [{ id: "DOC-A", path: "docs/spec/a.md", line: 1, rows }] };
}

/** The stub's lock diff reads, as (project, REF). */
function lockDiffReads(stub: StubClient) {
  return argsOf(stub.getLockDiff).map(([project, ref]) => [project, ref]);
}

/** The stub's node reads that asked for bindings. */
function bindingsReads(stub: StubClient) {
  return argsOf(stub.getNode).filter(([, , options]) => (options as { with?: string[] } | undefined)?.with?.includes("bindings") === true);
}

/** The tree stub, its bindings read answering each holder with no row, its lock diff `answer` (its `ref` the REF asked). */
function client(answer: LockDiffView | ((ref: string) => Promise<LockDiffView>)) {
  const stub = treeClient();
  const none: ShownBindings = { rows: [], omitted: 0, tested: [] };
  stub.getNode.mockImplementation((_project, ref, options) => {
    const view = viewOf(ref, options?.with?.includes("links") === true);
    return Promise.resolve(options?.with?.includes("bindings") === true ? { ...view, nodes: view.nodes.map((node) => ({ ...node, bindings: none })) } : view);
  });
  stub.getLockDiff.mockImplementation((_project, ref) => (typeof answer === "function" ? answer(ref) : Promise.resolve({ ...structuredClone(answer), ref })));
  return stub;
}

/** Opens `ref`'s Bindings tab and waits for its read; the tab's panel. */
async function bindingsTab(stub: StubClient, ref = "DOC-A"): Promise<HTMLElement> {
  renderApp(stub, `#/alpha/tree/${encodeURIComponent(ref)}`);
  fireEvent.click(await screen.findByRole("tab", { name: "Bindings" }));
  const panel = await screen.findByRole("tabpanel", { name: "Bindings" });
  await within(panel).findByRole("button", { name: "Changes since the lock" });
  return panel;
}

/** The disclosure's section in the tab. */
function changes(panel: HTMLElement): HTMLElement {
  const section = within(panel).getByRole("button", { name: "Changes since the lock" }).closest("section");
  if (section === null) {
    throw new Error("no lock diff section");
  }
  return section;
}

/** Opens `ref`'s Bindings tab, then expands "Changes since the lock"; its section. */
async function openChanges(stub: StubClient, ref = "DOC-A"): Promise<HTMLElement> {
  const panel = await bindingsTab(stub, ref);
  fireEvent.click(within(panel).getByRole("button", { name: "Changes since the lock" }));
  return changes(panel);
}

/** A figure's `<code>` text as drawn, its screen-reader words left out: the hunks exactly as shown. */
function drawn(figure: Element | undefined): string | null {
  const code = figure?.querySelector("code")?.cloneNode(true) as Element | undefined;
  if (code === undefined) {
    return null;
  }
  code.querySelectorAll(".sr-only").forEach((spoken) => {
    spoken.remove();
  });
  return code.textContent;
}

/** Each kept row's sides as drawn: which side, then its text. */
function sidesOf(section: HTMLElement): [string | null, string | null][][] {
  return Array.from(section.querySelectorAll(".health-code-row"), (row) =>
    Array.from(row.querySelectorAll(".lock-diff-side"), (side): [string | null, string | null] => [side.getAttribute("data-side"), side.textContent]),
  );
}

describe("the sides (UI-2 of lock-diff-ui)", () => {
  it("draws a conflict's two sides side by side, spec first, and a lone code side spanning both columns; stacked below 48em", async () => {
    const section = await openChanges(client(lockDiff([CONFLICT, CODE_AHEAD])));
    await within(section).findByText("2 drifted rows");
    const [conflict, ahead] = Array.from(section.querySelectorAll(".lock-diff-sides"));
    expect(Array.from(conflict?.children ?? [], (side) => side.getAttribute("data-side"))).toEqual(["spec", "code"]);
    expect(within(conflict as HTMLElement).getAllByRole("figure").map((figure) => figure.getAttribute("aria-label"))).toEqual([SPEC_TITLE, CODE_TITLE]);
    expect(Array.from(ahead?.children ?? [], (side) => side.getAttribute("data-side"))).toEqual(["code"]);
    expect(within(ahead as HTMLElement).getAllByRole("figure")).toHaveLength(1);
    // Each in its row's line, after verify's row.
    expect(conflict?.closest("li")?.querySelector(".health-code-head")?.textContent).toContain("DOC-A");
    const rule = (selector: string, css = appCss) => new RegExp(`(^|\\n)${selector.replaceAll(".", "\\.")}\\s*\\{([^}]*)\\}`).exec(css)?.[2] ?? "";
    expect(rule(".lock-diff-sides")).toContain("grid-template-columns: repeat(2, minmax(0, 1fr));");
    expect(rule(".lock-diff-side:only-child")).toContain("grid-column: 1 / -1;");
    // Only a lone side spans: the base rule places no side across both columns.
    expect(rule(".lock-diff-side")).toContain("min-width: 0;");
    expect(rule(".lock-diff-side")).not.toContain("grid-column");
    const narrow = /@media \(max-width: 48em\) \{\s*\.lock-diff-sides\s*\{([^}]*)\}\s*\}/.exec(appCss)?.[1] ?? "";
    expect(narrow).toContain("grid-template-columns: minmax(0, 1fr);");
  });

  it("draws the hunks byte for byte as sent: function context, tabs, trailing spaces, a CR, header-like lines, a non-Latin line, a second hunk", async () => {
    const section = await openChanges(client(lockDiff([kept("DOC-A", specSide(SPEC_DIFF), codeSide(HARD_DIFF))])));
    const spec = await within(section).findByRole("figure", { name: SPEC_TITLE });
    const code = within(section).getByRole("figure", { name: CODE_TITLE });
    expect(drawn(spec)).toBe(SPEC_DIFF);
    expect(drawn(code)).toBe(HARD_DIFF);
    // Each line its own, styled by its sign, never re-made: the header-like lines inside a hunk are an added and a removed line.
    expect(Array.from(code.querySelectorAll(".diff-line"), (line) => line.getAttribute("data-line"))).toEqual([
      "hunk",
      "context",
      "removed",
      "added",
      "added",
      "removed",
      "context",
      "added",
      "note",
      "hunk",
      "removed",
      "added",
    ]);
  });
});

describe("the states (UI-3 of lock-diff-ui)", () => {
  it("equal: No change since the lock, no figure", async () => {
    const section = await openChanges(client(lockDiff([kept("DOC-A", null, codeSide(""), { state: "code_ahead", category: "levels_changed", class: "check" })])));
    const equal = await within(section).findByText("No change since the lock");
    expect(equal.getAttribute("data-diff-state")).toBe("equal");
    expect(sidesOf(section)).toEqual([[["code", `${CODE_TITLE}No change since the lock`]]]);
    expect(within(section).queryAllByRole("figure")).toHaveLength(0);
  });

  it("not shown: No diff and the why of its note (N1, N7), else the notes above; never No change, never a figure", async () => {
    const rows = [
      kept("DOC-A", specSide(null), null, { state: "spec_ahead", category: "marker_behind", class: "check" }),
      kept("DOC-A", null, codeSide(null), { state: "code_ahead", category: "code_changed", class: "check" }),
      // The same key (rows holders share): each side takes the first note naming its row and side.
      kept("DOC-A", specSide(null), codeSide(null)),
    ];
    const section = await openChanges(client(lockDiff(rows, [N1, N7])));
    await within(section).findByText("3 drifted rows");
    expect(within(section).getByRole("list", { name: "Notes from the daemon on the changes since the lock" }).textContent).toBe(`${N1}${N7}`);
    expect(sidesOf(section)).toEqual([
      [["spec", `${SPEC_TITLE}No diff: docs/spec/a.md cannot be read at ${COMMIT}`]],
      [["code", `${CODE_TITLE}No diff: git shows no hunk for bytes that differ`]],
      [
        ["spec", `${SPEC_TITLE}No diff: docs/spec/a.md cannot be read at ${COMMIT}`],
        ["code", `${CODE_TITLE}No diff: git shows no hunk for bytes that differ`],
      ],
    ]);
    expect(section.textContent).not.toContain("No change");
    expect(within(section).queryAllByRole("figure")).toHaveLength(0);
    for (const state of section.querySelectorAll(".lock-diff-side .bindings-state")) {
      expect(state.getAttribute("data-diff-state")).toBe("not-shown");
    }
  });

  it("not shown with no note naming its row and side: No diff, the notes above say why; a why on one line", async () => {
    const other = `lock diff: ${KEY.replace("DOC-A", "SEC-A1")}: no spec diff: elsewhere`;
    const section = await openChanges(
      client(lockDiff([kept("DOC-A", specSide(null), null, { state: "spec_ahead", category: "spec_changed", class: "check" }), kept("DOC-A", null, codeSide(null))], [other, `lock diff: ${KEY}: no code diff: git\ndiff --no-index failed`])),
    );
    await within(section).findByText("2 drifted rows");
    expect(sidesOf(section)).toEqual([
      [["spec", `${SPEC_TITLE}No diff: the daemon\u2019s notes above say why.`]],
      [["code", `${CODE_TITLE}No diff: git diff --no-index failed`]],
    ]);
  });

  it("a side not sent draws nothing; neither sent: the row line and its commands alone", async () => {
    const section = await openChanges(client(lockDiff([CODE_AHEAD, kept("DOC-A", null, null, { state: "spec_ahead", category: "marker_behind", class: "check" })])));
    await within(section).findByText("2 drifted rows");
    expect(sidesOf(section).map((sides) => sides.map(([which]) => which))).toEqual([["code"], []]);
    const [, alone] = Array.from(section.querySelectorAll<HTMLElement>(".health-code-row"));
    expect(alone?.querySelector(".lock-diff-sides")).toBeNull();
    expect(alone?.querySelectorAll(".command-text")).toHaveLength(3);
    expect(section.textContent).not.toMatch(/Not moved|No change/);
  });

  it("rows null: Not judged, the note above saying why; no list, None or digit", async () => {
    const cause = "bindings: specengine.toml has no [code] table: the code layer is off (add [code] with roots)";
    const section = await openChanges(client(lockDiff(null, [cause])));
    const state = await within(section).findByText("Not judged: the daemon\u2019s note above says why.");
    expect(state.closest("p")?.getAttribute("data-diff-state")).toBe("not-judged");
    const holder = section.querySelector(".lock-diff-holder");
    expect(holder?.textContent).toBe("Not judged: the daemon\u2019s note above says why.");
    expect(holder?.querySelector("ol, ul")).toBeNull();
    expect(holder?.textContent).not.toMatch(/\d|None/);
  });

  it("rows []: None, said as such", async () => {
    const section = await openChanges(client(lockDiff([])));
    const none = await within(section).findByText("None: no drifted binding of this node has a lock entry.");
    expect(none.getAttribute("data-diff-state")).toBe("none");
  });

  it("a REF naming nothing (exit 1, a 404 as data): the reason alone, verbatim", async () => {
    const reason = "`DOC-A` resolves to no ID and no alias";
    const section = await openChanges(client({ ref: "DOC-A", reason, notes: ["note: an @rev is ignored"], nodes: [] }));
    expect((await within(section).findByText(reason)).className).toBe("verbatim");
    expect(within(section).queryByRole("list", { name: "Notes from the daemon on the changes since the lock" })).toBeNull();
    expect(section.querySelectorAll(".lock-diff-holder")).toHaveLength(0);
  });

  it("puts each note on one line: a line feed a space, a control as \\u{..}, in the daemon's order", async () => {
    const section = await openChanges(client(lockDiff([], ["note: first\nline", "second\u0007"])));
    const notes = await within(section).findByRole("list", { name: "Notes from the daemon on the changes since the lock" });
    expect(Array.from(notes.querySelectorAll("li"), (item) => item.textContent)).toEqual(["note: first line", "second\\u{7}"]);
  });

  it("loading: a busy skeleton until the read answers", async () => {
    const section = await openChanges(client(() => new Promise(() => undefined)));
    expect((await within(section).findByLabelText("Loading the changes since the lock of DOC-A")).getAttribute("aria-busy")).toBe("true");
  });

  it("failed: the daemon's words and Retry, which reads again; the bindings stay", async () => {
    const message = "spec: the index is locked by another process";
    const stub = client(() => Promise.reject(new ClientError({ status: 503, message })));
    const section = await openChanges(stub);
    expect((await within(section).findByRole("alert")).textContent).toBe(`The changes since the lock of DOC-A could not be read${message}`);
    fireEvent.click(within(section).getByRole("button", { name: "Retry" }));
    await waitFor(() => {
      expect(lockDiffReads(stub)).toHaveLength(2);
    });
    expect(screen.getByText("Code bindings")).toBeTruthy();
  });

  it("failed again: its own Read again; the last answer stays up, the failure a note under it", async () => {
    const message = "spec: the index is locked by another process";
    let calls = 0;
    const stub = client(() => {
      calls += 1;
      return calls === 1 ? Promise.resolve(lockDiff([CONFLICT])) : Promise.reject(new ClientError({ status: 503, message }));
    });
    const section = await openChanges(stub);
    await within(section).findAllByRole("figure");
    fireEvent.click(within(section).getByRole("button", { name: "Read again" }));
    expect((await within(section).findByRole("alert")).textContent).toBe(`The changes since the lock could not be read again: ${message}`);
    expect(within(section).getAllByRole("figure")).toHaveLength(2);
  });

  it("several holders: each its rows under its place, an h3 under the disclosure's h2", async () => {
    const holder = (line: number, rows: LockDiffRow[] | null): LockDiffNode => ({ id: "SEC-DUP", path: line === 30 ? "docs/spec/a.md" : "docs/spec/b.md", line, rows });
    const section = await openChanges(client({ ref: "SEC-DUP", reason: null, notes: [], nodes: [holder(30, [kept("SEC-DUP", null, codeSide(CODE_DIFF))]), holder(9, [])] }), "SEC-DUP");
    const parts = await within(section).findAllByRole("region", { name: /^Changes at / });
    expect(parts.map((part) => [part.getAttribute("aria-label"), part.querySelector("h3")?.textContent])).toEqual([
      ["Changes at docs/spec/a.md:30", "docs/spec/a.md:30"],
      ["Changes at docs/spec/b.md:9", "docs/spec/b.md:9"],
    ]);
    expect(within(parts[0] as HTMLElement).getByRole("list", { name: "Changes since the lock of SEC-DUP, in spec verify's order" })).toBeTruthy();
    expect(section.querySelector("h2")?.textContent).toBe("Changes since the lock");
  });
});

describe("Read again (lock-diff-ui \"Description and interactions\")", () => {
  it("the panel's sends one GET per click, the lock diff only; the tab's reads the bindings only", async () => {
    const stub = client(lockDiff([CONFLICT]));
    const panel = await bindingsTab(stub);
    const section = changes(panel);
    fireEvent.click(within(section).getByRole("button", { name: "Changes since the lock" }));
    await within(section).findAllByRole("figure");
    expect([lockDiffReads(stub).length, bindingsReads(stub).length]).toEqual([1, 1]);
    for (const times of [2, 3]) {
      fireEvent.click(within(section).getByRole("button", { name: "Read again" }));
      await waitFor(() => {
        expect(lockDiffReads(stub)).toHaveLength(times);
      });
      await within(section).findByRole("button", { name: "Read again" });
    }
    expect(bindingsReads(stub)).toHaveLength(1);
    const tabs = within(panel).getAllByRole("button", { name: "Read again" }).filter((button) => !section.contains(button));
    expect(tabs).toHaveLength(1);
    fireEvent.click(tabs[0] as HTMLElement);
    await waitFor(() => {
      expect(bindingsReads(stub)).toHaveLength(2);
    });
    expect(lockDiffReads(stub)).toHaveLength(3);
  });
});

describe("the commands beside a row (UI-5 of lock-diff-ui)", () => {
  /** A row's command texts, as copied. */
  function commandsOf(row: Element | undefined): (string | null)[] {
    return Array.from(row?.querySelectorAll(".command-text") ?? [], (command) => command.textContent);
  }

  it("offers a LAW-BOUND row its three commands in order, each with Copy, fixed words and the ID", async () => {
    const section = await openChanges(client(lockDiff([kept("LAW-BOUND", null, codeSide(CODE_DIFF))])));
    const list = await within(section).findByRole("list", { name: "Commands for the row of LAW-BOUND" });
    expect(commandsOf(list)).toEqual(["spec verify --node LAW-BOUND", "spec propose drift LAW-BOUND", "spec lock accept LAW-BOUND"]);
    expect(within(list).getAllByRole("button", { name: /^Copy/ })).toHaveLength(3);
    expect(Array.from(list.querySelectorAll(".command-meaning"), (meaning) => meaning.textContent)).toEqual(nodeCommands("LAW-BOUND")?.map((command) => command.meaning));
  });

  it.each([
    ["empty", ""],
    ["a space", "A B"],
    ["a semicolon", "X;Y"],
    ["a command substitution", "$(x)"],
    ["a backtick", "LAW-`id`"],
    ["a quote", 'LAW-"BOUND"'],
    // LAW-BOUND with a Cyrillic capital A (U+0410), a look-alike.
    ["a Cyrillic look-alike", `L${String.fromCodePoint(0x410)}W-BOUND`],
  ])("refuses a row node with %s: the refusal, no command", async (_what, node) => {
    const section = await openChanges(client(lockDiff([kept(node, null, codeSide(CODE_DIFF))])));
    await within(section).findByText("1 drifted row");
    const row = section.querySelector(".health-code-row") ?? undefined;
    expect(commandsOf(row)).toEqual([]);
    expect(row?.querySelector(".lock-diff-commands")?.textContent).toBe(`No command for ${node}: only a REF of Latin letters, digits and . _ / - is put in a command.`);
    expect(row?.querySelector(".lock-diff-commands bdi.mono")?.textContent).toBe(node);
    cleanup();
  });
});

describe("the figures and the disclosure for screen readers (UI-6 of lock-diff-ui)", () => {
  it("names each figure by its title, none Section diff; an added line says Added:, a removed one Removed:", async () => {
    const section = await openChanges(client(lockDiff([CONFLICT])));
    const spec = await within(section).findByRole("figure", { name: SPEC_TITLE });
    expect(within(section).getByRole("figure", { name: CODE_TITLE })).toBeTruthy();
    expect(within(section).queryByRole("figure", { name: "Section diff" })).toBeNull();
    expect(Array.from(section.querySelectorAll(".lock-diff-side-title"), (title) => title.textContent)).toEqual([SPEC_TITLE, CODE_TITLE]);
    const spoken = (type: string) => Array.from(spec.querySelectorAll(`.diff-line[data-line="${type}"]`), (line) => line.querySelector(".sr-only")?.textContent ?? null);
    expect([spoken("added"), spoken("removed"), spoken("context")]).toEqual([["Added: "], ["Removed: "], [null, null]]);
  });

  it("says the disclosure's state: collapsed, nothing controlled; expanded, the body it controls; the about line either way", async () => {
    const panel = await bindingsTab(client(lockDiff([CONFLICT])));
    const button = within(panel).getByRole("button", { name: "Changes since the lock" });
    button.focus();
    const about =
      "For each drifted binding with a lock entry, what moved in the node\u2019s text and in its code since the lock, as git\u2019s hunks against the working tree. Read when first opened; a code edit shows after Read again.";
    expect([button.getAttribute("aria-expanded"), button.getAttribute("aria-controls"), changes(panel).querySelector(".lock-diff-about")?.textContent]).toEqual(["false", null, about]);
    expect(button.closest("h2")).not.toBeNull();
    fireEvent.click(button);
    const controls = button.getAttribute("aria-controls") ?? "";
    expect([button.getAttribute("aria-expanded"), document.getElementById(controls)?.closest("section")]).toEqual(["true", changes(panel)]);
    expect(changes(panel).querySelector(".lock-diff-about")?.textContent).toBe(about);
    // Focus stays on the button, expanded and collapsed.
    expect(document.activeElement).toBe(button);
    await within(changes(panel)).findAllByRole("figure");
    expect(document.activeElement).toBe(button);
    fireEvent.click(button);
    expect([button.getAttribute("aria-expanded"), button.getAttribute("aria-controls")]).toEqual(["false", null]);
    expect(document.activeElement).toBe(button);
  });

  it("describes the panel's Read again by the panel's heading, its text unchanged: a screen reader tells it from the tab's", async () => {
    const section = await openChanges(client(lockDiff([CONFLICT])));
    const readAgain = await within(section).findByRole("button", { name: "Read again" });
    const described = (readAgain.getAttribute("aria-describedby") ?? "").split(" ").map((id) => document.getElementById(id));
    expect(described.map((element) => [element?.textContent, element?.closest("h2") !== null && element?.closest("section") === section])).toEqual([["Changes since the lock", true]]);
    expect(readAgain.textContent).toBe("Read again");
    const tabs = within(screen.getByRole("tabpanel", { name: "Bindings" })).getAllByRole("button", { name: "Read again" }).filter((button) => !section.contains(button));
    expect(tabs.map((button) => button.hasAttribute("aria-describedby"))).toEqual([false]);
  });
});
