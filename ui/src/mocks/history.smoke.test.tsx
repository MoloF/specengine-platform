import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { DRAFT_FILE } from "./harbor-sim/fixtures";
import { MockClient } from "./MockClient";

// NH13 of docs/features/node-history.md "Acceptance criteria" on the mock's sample, as `pnpm dev`
// shows it: MEC-MOORING's whole story (a creation, a raise, a wording fix, C-0005 accepted, a
// decision recorded in DEC-0014, a hand edit); the pilots' log's 64 of more; DOM-HARBOR's word no
// build knows; the uncommitted draft file's none; ledger-api, a shallow clone, its note. The stub
// cases run in src/doc/; the mock is imported here alone (policy AC-05).

// The document view's chunk loaded once for the file, so its parts never wait on their first transform.
beforeAll(async () => {
  await import("../doc/DocBody");
});

function renderMock(hash: string) {
  window.history.replaceState(null, "", `/?scenario=normal${hash}`);
  const client = new MockClient("normal", { delayMs: 0 });
  const getNode = vi.spyOn(client, "getNode");
  render(<App client={client} scenario={null} />);
  return { client, getNode };
}

/** A document's modal over the Board, by title; History expanded and read. */
async function history(project: string, ref: string, title: string): Promise<HTMLElement> {
  renderMock(`#/${project}/board?doc=${encodeURIComponent(ref)}`);
  const dialog = await screen.findByRole("dialog", { name: title });
  const button = await within(dialog).findByRole("button", { name: "History" });
  expect(button.getAttribute("aria-expanded")).toBe("false");
  fireEvent.click(button);
  const region = document.getElementById(button.getAttribute("aria-controls") ?? "") as HTMLElement;
  await waitFor(() => {
    expect(region.querySelector('[aria-busy="true"]')).toBeNull();
    expect(region.textContent).not.toBe("");
  });
  return region;
}

describe("History over the mock (NH13 of node-history)", () => {
  it("MEC-MOORING: each change said, newest first; the creation marked; C-0005 a Board link, DEC-0014 a document; names alone", async () => {
    const region = await history("harbor-sim", "MEC-MOORING", "Mooring");
    const entries = Array.from(region.querySelectorAll(".doc-history-entry"));
    expect(entries.map((entry) => entry.querySelector(".doc-history-change")?.textContent)).toEqual([
      "Edited outside a proposal",
      "Decision recorded in DEC-0014",
      "Task C-0005 accepted",
      "Wording fixed, version 2 kept",
      "Version 2",
      "Edited outside a proposal",
    ]);
    expect(entries.map((entry) => entry.querySelector(".lozenge")?.textContent ?? null)).toEqual([null, null, null, null, null, "Created"]);
    const part = within(region);
    expect(part.getByRole("link", { name: "C-0005" }).getAttribute("href")).toBe("#/harbor-sim/board/C-0005");
    expect(part.getByRole("link", { name: "DEC-0014" }).getAttribute("href")).toBe("#/harbor-sim/tree/DEC-0014/page");
    expect(part.getByRole("link", { name: "PR-0031" }).getAttribute("href")).toBe("#/harbor-sim/inbox/PR-0031");
    expect(part.getAllByText("Mira Holt").length).toBeGreaterThan(0);
    expect(region.textContent).not.toContain("@");
    expect(region.textContent).not.toMatch(/[0-9a-f]{12}/);
  });

  it("opens DEC-0014 from its record link, over the view", async () => {
    const region = await history("harbor-sim", "MEC-MOORING", "Mooring");
    fireEvent.click(within(region).getByRole("link", { name: "DEC-0014" }));
    expect(await screen.findByRole("dialog", { name: "Quay lights stay amber" })).toBeTruthy();
  });

  it("the pilots' log: 64 entries, and older ones not listed", async () => {
    renderMock("#/harbor-sim/board?doc=MEC-HARBOUR-LOG");
    const dialog = await screen.findByRole("dialog", { name: /^Harbour log/ });
    fireEvent.click(await within(dialog).findByRole("button", { name: "History" }));
    expect(await within(dialog).findByText("Older changes are not listed.")).toBeTruthy();
    expect(dialog.querySelectorAll(".doc-history-entry")).toHaveLength(64);
  });

  it("DOM-HARBOR: a change word no build knows, raw under lang=en, before its creation", async () => {
    const region = await history("harbor-sim", "DOM-HARBOR", "Harbor simulation");
    const changes = Array.from(region.querySelectorAll(".doc-history-change"));
    expect(changes.map((change) => [change.textContent, change.getAttribute("lang")])).toEqual([
      ["reverted", "en"],
      ["Edited outside a proposal", null],
    ]);
  });

  it("the uncommitted draft file: nothing recorded", async () => {
    const region = await history("harbor-sim", DRAFT_FILE, "Berth limits");
    expect(within(region).getByText("No change to this document is recorded yet.")).toBeTruthy();
    expect(region.querySelector("ol")).toBeNull();
  });

  it("ledger-api, a shallow clone: no list, the read's note under lang=en; nothing read before the expansion", async () => {
    const { getNode } = renderMock("#/ledger-api/board?doc=SVC-LEDGER");
    const dialog = await screen.findByRole("dialog", { name: "Ledger API" });
    const button = await within(dialog).findByRole("button", { name: "History" });
    expect(getNode.mock.calls.filter(([, , options]) => options?.with?.includes("history") === true)).toEqual([]);
    fireEvent.click(button);
    const note = await within(dialog).findByText("history: docs/spec/README.md: not read: a shallow clone");
    expect(note.getAttribute("lang")).toBe("en");
    expect(dialog.querySelector(".doc-history-list")).toBeNull();
    expect(getNode.mock.calls.filter(([, , options]) => options?.with?.includes("history") === true)).toHaveLength(1);
  });
});
