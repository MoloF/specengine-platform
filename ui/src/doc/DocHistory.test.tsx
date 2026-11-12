import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it } from "vitest";
import { ClientError, type NodeOptions } from "../api/client";
import type { NodeView, ShownHistory } from "../api/types";
import { aHistory, aHistoryEntry } from "../test/builders";
import { docClient, FERRY, FERRY_PATH, ferryHistory, historyView } from "../test/docStub";
import { renderApp } from "../test/render";
import { argsOf } from "../test/stubClient";

// NH7 and NH13 of docs/features/node-history.md "Acceptance criteria" on the river ferry: History is
// collapsed and read on its first expansion alone, held open per project by the Shell; each entry
// says what changed, when, who by name, and on an apply who proposed it, never an address or an OID.

// The document view's chunk loaded once for the file, so the first test never waits on its first transform.
beforeAll(async () => {
  await import("./DocBody");
});

/** The ferry's modal over the Inbox, its body landed. */
async function ferry(client = docClient()) {
  renderApp(client, `#/alpha/inbox?doc=${FERRY}`);
  const dialog = await screen.findByRole("dialog", { name: "The ferry" });
  await within(dialog).findByText("A flat boat crosses the river on a cable.");
  return { client, dialog };
}

function historyButton(dialog: HTMLElement): HTMLElement {
  return within(dialog).getByRole("button", { name: "History" });
}

/** The part's region, under its toggle. */
function historyRegion(dialog: HTMLElement): HTMLElement {
  return document.getElementById(historyButton(dialog).getAttribute("aria-controls") ?? "") as HTMLElement;
}

/** The getNode calls that asked for history, as made. */
function historyReads(client: ReturnType<typeof docClient>): unknown[][] {
  return argsOf(client.getNode).filter((args) => (args[2] as NodeOptions | undefined)?.with?.includes("history") === true);
}

/** The route changed as a link or Back changes it: the hash set, its event awaited. */
async function go(hash: string) {
  const changed = new Promise<void>((resolve) => {
    window.addEventListener(
      "hashchange",
      () => {
        resolve();
      },
      { once: true },
    );
  });
  await act(async () => {
    window.location.hash = hash;
    await changed;
  });
}

async function quiet() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
}

/** The ferry expanded: its six entries drawn. */
async function expanded(client = docClient()) {
  const { dialog } = await ferry(client);
  fireEvent.click(historyButton(dialog));
  const region = historyRegion(dialog);
  await waitFor(() => {
    expect(region.querySelectorAll(".doc-history-entry").length).toBeGreaterThan(0);
  });
  return { client, dialog, region };
}

/** A client whose history read of the ferry answers `history` and `notes`. */
function answering(history: ShownHistory | null, notes: string[] = []) {
  const client = docClient();
  const base = client.getNode.getMockImplementation();
  client.getNode.mockImplementation((project, ref, options?: NodeOptions, signal?: AbortSignal) => {
    if (options?.with?.includes("history") === true && ref === FERRY) {
      const view: NodeView = historyView(FERRY);
      return Promise.resolve({ ...view, notes, nodes: view.nodes.map((node) => ({ ...node, history })) });
    }
    return base === undefined ? Promise.reject(new Error("no stub")) : base(project, ref, options, signal);
  });
  return client;
}

describe("History read on expansion (NH7 of node-history)", () => {
  it("is collapsed after Inside and reads nothing on open; the first expansion reads `with: [history]` once; a collapse and a second expansion read nothing (M: read on open)", async () => {
    const { client, dialog } = await ferry();
    const button = historyButton(dialog);
    expect(button.getAttribute("aria-expanded")).toBe("false");
    expect(historyRegion(dialog).hidden).toBe(true);
    const inside = within(dialog).getByRole("button", { name: "Inside" });
    expect(inside.compareDocumentPosition(button) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    // In a heading at the part's level, as Inside's toggle.
    expect(button.closest("h3")).not.toBeNull();
    await quiet();
    expect(historyReads(client)).toEqual([]);
    fireEvent.click(button);
    await within(dialog).findByText("Wording only.");
    expect(historyReads(client)).toEqual([["alpha", FERRY, { with: ["history"] }]]);
    fireEvent.click(button);
    expect(historyRegion(dialog).hidden).toBe(true);
    fireEvent.click(button);
    await quiet();
    expect(historyReads(client)).toHaveLength(1);
    expect(historyRegion(dialog).hidden).toBe(false);
  });

  it("is held open for the project's next document: opened there, it reads that one on open", async () => {
    const { client, dialog } = await ferry();
    fireEvent.click(historyButton(dialog));
    await within(dialog).findByText("Wording only.");
    fireEvent.click(within(within(dialog).getByRole("region", { name: "Links" })).getByRole("link", { name: "Boarding" }));
    const other = await screen.findByRole("dialog", { name: "Boarding" });
    expect(historyButton(other).getAttribute("aria-expanded")).toBe("true");
    expect(await within(other).findByText("No change to this document is recorded yet.")).toBeTruthy();
    expect(historyReads(client)).toEqual([
      ["alpha", FERRY, { with: ["history"] }],
      ["alpha", "MEC-BOARDING", { with: ["history"] }],
    ]);
  });

  it("keeps the choice per project: another project's document opens collapsed and unread; back, the first is still expanded (M: one DocState for all projects)", async () => {
    const { client, dialog } = await ferry();
    fireEvent.click(historyButton(dialog));
    await within(dialog).findByText("Wording only.");
    await go(`#/beta/inbox?doc=${FERRY}`);
    const beta = await screen.findByRole("dialog", { name: "The ferry" });
    await waitFor(() => {
      expect(window.location.hash).toBe(`#/beta/inbox?doc=${FERRY}`);
    });
    const betaButton = await within(beta).findByRole("button", { name: "History" });
    expect(betaButton.getAttribute("aria-expanded")).toBe("false");
    await quiet();
    expect(historyReads(client).filter(([project]) => project === "beta")).toEqual([]);
    await go(`#/alpha/inbox?doc=${FERRY}`);
    const alpha = await screen.findByRole("dialog", { name: "The ferry" });
    await waitFor(() => {
      expect(historyButton(alpha).getAttribute("aria-expanded")).toBe("true");
    });
    expect(historyReads(client).filter(([project]) => project === "beta")).toEqual([]);
  });

  it("keeps Inside's choice and History's apart: Inside expanded leaves History collapsed and unread; each kept as the other changes (M: one choice resets the other)", async () => {
    const { client, dialog } = await ferry();
    const inside = within(dialog).getByRole("button", { name: "Inside" });
    fireEvent.click(inside);
    await within(dialog).findByRole("heading", { name: "Directly under this document" });
    expect(historyButton(dialog).getAttribute("aria-expanded")).toBe("false");
    await quiet();
    expect(historyReads(client)).toEqual([]);
    fireEvent.click(historyButton(dialog));
    expect([inside.getAttribute("aria-expanded"), historyButton(dialog).getAttribute("aria-expanded")]).toEqual(["true", "true"]);
    fireEvent.click(inside);
    expect([inside.getAttribute("aria-expanded"), historyButton(dialog).getAttribute("aria-expanded")]).toEqual(["false", "true"]);
    fireEvent.click(inside);
    expect([inside.getAttribute("aria-expanded"), historyButton(dialog).getAttribute("aria-expanded")]).toEqual(["true", "true"]);
  });

  it("on the page too: collapsed, read on its first expansion", async () => {
    const client = docClient();
    renderApp(client, `#/alpha/tree/${FERRY}/page`);
    const page = (await screen.findByRole("heading", { level: 1, name: "The ferry" })).closest("section") as HTMLElement;
    const button = await within(page).findByRole("button", { name: "History" });
    expect(button.closest("h2")).not.toBeNull();
    await quiet();
    expect(historyReads(client)).toEqual([]);
    fireEvent.click(button);
    expect(await within(page).findByText("Tighten the window.")).toBeTruthy();
    expect(historyReads(client)).toHaveLength(1);
  });
});

describe("the History part (NH13 of node-history)", () => {
  it("says each change in order: an edit outside a proposal, a task accepted, a wording fix, a decision, a version, and the creation, marked", async () => {
    const { region } = await expanded();
    const entries = Array.from(region.querySelectorAll(".doc-history-entry"));
    expect(region.querySelector("ol.doc-history-list")).not.toBeNull();
    expect(entries.map((entry) => entry.querySelector(".doc-history-change")?.textContent)).toEqual([
      "Edited outside a proposal",
      "Task C-0001 accepted",
      "Wording fixed, version 2 kept",
      "Decision recorded in DEC-0031",
      "Version 2",
      "Edited outside a proposal",
    ]);
    expect(entries.map((entry) => entry.querySelector(".lozenge")?.textContent ?? null)).toEqual([null, null, null, null, null, "Created"]);
    expect(entries.map((entry) => entry.querySelector(".doc-history-text")?.textContent ?? null)).toEqual([
      "Fix a typo in the window rule",
      "Paint the ferry",
      "Wording only.",
      null,
      "Tighten the window.",
      "Add the rules",
    ]);
  });

  it("links the task to its Board place and the decision's record as a document: a plain click opens it over the view", async () => {
    const { region } = await expanded();
    const part = within(region);
    expect(part.getByRole("link", { name: "C-0001" }).getAttribute("href")).toBe("#/alpha/board/C-0001");
    const record = part.getByRole("link", { name: "DEC-0031" });
    expect(record.getAttribute("href")).toBe("#/alpha/tree/DEC-0031/page");
    fireEvent.click(record);
    expect(window.location.hash).toMatch(/[?&]doc=DEC-0031$/);
  });

  it("dates each entry as stored, in UTC, and names who by name alone; an apply its proposer under lang=en and its proposal an Inbox link", async () => {
    const { region } = await expanded();
    const [hand, , editorial] = Array.from(region.querySelectorAll<HTMLElement>(".doc-history-entry"));
    const time = hand?.querySelector("time");
    expect([time?.getAttribute("dateTime"), time?.textContent]).toEqual(["2026-01-04T10:00:00Z", "2026-01-04 10:00 UTC"]);
    expect(hand?.querySelector(".doc-history-meta")?.textContent).toBe("2026-01-04 10:00 UTCby Bob Hand");
    expect(hand?.querySelector(".doc-history-meta bdi")?.textContent).toBe("Bob Hand");
    const meta = editorial?.querySelector(".doc-history-meta") as HTMLElement;
    expect(meta.textContent).toBe("2026-01-03 10:00 UTCby Ann Ownerproposed by agent spec-writer in PR-0002");
    expect(within(meta).getByText("agent spec-writer").getAttribute("lang")).toBe("en");
    expect(within(meta).getByRole("link", { name: "PR-0002" }).getAttribute("href")).toBe("#/alpha/inbox/PR-0002");
  });

  it("draws `by_name`, never `by`: no address and no OID anywhere in the part (M: `by` rendered)", async () => {
    const { region } = await expanded();
    const text = region.textContent;
    expect(within(region).getAllByText("Ann Owner").length).toBeGreaterThan(0);
    expect(text).not.toContain("@");
    expect(text).not.toContain("example.org");
    for (const digit of ["a", "b", "c", "d"]) {
      expect(text).not.toContain(digit.repeat(12));
    }
  });

  it("draws `by_name` exactly as served, core's name of the identity, never parsed again (M: a cut at `@`)", async () => {
    const { region } = await expanded(answering(aHistory([aHistoryEntry({ id: "f".repeat(40), by: "ann@home", by_name: "ann@home" }), aHistoryEntry({ id: "e".repeat(40), by: "@bot", by_name: "@bot" })])));
    expect(Array.from(region.querySelectorAll(".doc-history-meta bdi"), (name) => name.textContent)).toEqual(["ann@home", "@bot"]);
  });

  it("says an item with no record without one: no record link and no ID in its place (M: the ID in the slot)", async () => {
    const { region } = await expanded(answering(aHistory([aHistoryEntry({ source: "item", id: "PR-0040", change: "decision", rev: null, rev_before: null, record: null })])));
    expect(region.querySelector(".doc-history-change")?.textContent).toBe("Decision recorded");
    expect(within(region).queryByRole("link")).toBeNull();
    expect(region.textContent).not.toContain("PR-0040");
  });

  it("with no proposer made of the trailer, draws the proposal link alone, never the trailer (M: `proposed_by` drawn)", async () => {
    const trailer = "human role=unknown model=unknown run=unknown login=ann";
    const { region } = await expanded(answering(aHistory([aHistoryEntry({ id: "f".repeat(40), change: "raised", rev: 2, proposal: "PR-0001", proposed_by: trailer, proposer: null })])));
    const meta = region.querySelector(".doc-history-meta") as HTMLElement;
    expect(within(meta).getByRole("link", { name: "PR-0001" }).getAttribute("href")).toBe("#/alpha/inbox/PR-0001");
    expect(meta.textContent).toBe("2026-01-01 10:00 UTCby Ann OwnerPR-0001");
    expect(region.textContent).not.toContain("login=");
  });

  it("shows a change word this build does not know raw, under lang=en", async () => {
    const { region } = await expanded(answering(aHistory([aHistoryEntry({ id: "e".repeat(40), change: "squashed", rev: 3, rev_before: 2 })])));
    const change = region.querySelector(".doc-history-change");
    expect([change?.textContent, change?.getAttribute("lang")]).toEqual(["squashed", "en"]);
  });

  it("says when older changes are not listed", async () => {
    const { region } = await expanded(answering({ ...ferryHistory(), truncated: true }));
    expect(within(region).getByText("Older changes are not listed.")).toBeTruthy();
    expect(region.querySelectorAll(".doc-history-entry")).toHaveLength(6);
  });

  it("says nothing is recorded for an empty list", async () => {
    const { dialog } = await ferry(answering(aHistory([])));
    fireEvent.click(historyButton(dialog));
    const region = historyRegion(dialog);
    expect(await within(region).findByText("No change to this document is recorded yet.")).toBeTruthy();
    expect(within(region).queryByText("Older changes are not listed.")).toBeNull();
    expect(region.querySelector("ol")).toBeNull();
  });

  it("an empty list with older changes not listed, the scan stopped short: that line alone, never that none is recorded (M: both lines)", async () => {
    const { dialog } = await ferry(answering(aHistory([], { truncated: true })));
    fireEvent.click(historyButton(dialog));
    const region = historyRegion(dialog);
    expect(await within(region).findByText("Older changes are not listed.")).toBeTruthy();
    expect(within(region).queryByText("No change to this document is recorded yet.")).toBeNull();
    expect(region.querySelector("ol")).toBeNull();
  });

  it("with no history read, shows the read's history notes under lang=en, no list and no empty line", async () => {
    const note = `history: ${FERRY_PATH}: not read: a shallow clone`;
    const { dialog } = await ferry(answering(null, ["bindings: off", note]));
    fireEvent.click(historyButton(dialog));
    const region = historyRegion(dialog);
    const shown = await within(region).findByText(note);
    expect(shown.getAttribute("lang")).toBe("en");
    expect(region.textContent).not.toContain("bindings: off");
    expect(region.querySelector("ol")).toBeNull();
    expect(within(region).queryByText("No change to this document is recorded yet.")).toBeNull();
  });

  it("shows the queue's note beside the commits it could read", async () => {
    const note = "history: the queue not read: locked; commits only";
    const { region } = await expanded(answering(ferryHistory(), [note]));
    expect(within(region).getByText(note).getAttribute("lang")).toBe("en");
    expect(region.querySelectorAll(".doc-history-entry")).toHaveLength(6);
  });

  it("shows a skeleton while read, then the entries", async () => {
    const client = docClient();
    const base = client.getNode.getMockImplementation();
    let land: () => void = () => undefined;
    client.getNode.mockImplementation((project, ref, options?: NodeOptions, signal?: AbortSignal) =>
      options?.with?.includes("history") === true
        ? new Promise((resolve) => {
            land = () => {
              resolve(historyView(ref));
            };
          })
        : base === undefined
          ? Promise.reject(new Error("no stub"))
          : base(project, ref, options, signal),
    );
    const { dialog } = await ferry(client);
    fireEvent.click(historyButton(dialog));
    expect(await within(dialog).findByRole("status", { name: "Reading the history…" })).toBeTruthy();
    act(() => {
      land();
    });
    expect(await within(dialog).findByText("Tighten the window.")).toBeTruthy();
  });

  it("says a failed read in the daemon's words under lang=en, and Retry reads again", async () => {
    const client = docClient();
    const base = client.getNode.getMockImplementation();
    let failed = false;
    client.getNode.mockImplementation((project, ref, options?: NodeOptions, signal?: AbortSignal) => {
      if (options?.with?.includes("history") === true && !failed) {
        failed = true;
        return Promise.reject(new ClientError({ status: 400, message: "`with=history`: the values are `links` and `bindings`" }));
      }
      return base === undefined ? Promise.reject(new Error("no stub")) : base(project, ref, options, signal);
    });
    const { dialog } = await ferry(client);
    fireEvent.click(historyButton(dialog));
    expect(await within(dialog).findByText("The history could not be read.")).toBeTruthy();
    expect(within(dialog).getByText("`with=history`: the values are `links` and `bindings`").getAttribute("lang")).toBe("en");
    fireEvent.click(within(historyRegion(dialog)).getByRole("button", { name: "Retry" }));
    expect(await within(dialog).findByText("Tighten the window.")).toBeTruthy();
    expect(historyReads(client)).toHaveLength(2);
  });
});
